"""Exercise the pinned cargo2 downloader against HTTP and its real file cache.

CARGO2_SOURCE_ARCHIVE must name the checksum-verified community 2.3.1 archive;
CARGO2_SDK_SOURCE_ARCHIVE must name the SDK's pinned community 2.3.3 archive.
CARGO2_TEST_UNPATCHED=1 runs the same regressions against both pristine sources.
Only unrelated git-source imports are omitted; downloader and registry code,
BuildStream checksum helpers, and SourceError are the actual implementations.
"""

import ast
import contextlib
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from types import SimpleNamespace

from buildstream import SourceError, SourceFetcher, utils
import pytest
import yaml


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml"
SOURCE_PATH = Path("src/buildstream_plugins_community/sources/cargo2.py")
BODY = b"the crate bytes verified by the lockfile checksum\n"
SHA256 = hashlib.sha256(BODY).hexdigest()
ETAG = '"crate-content-v1"'


@pytest.fixture(scope="module", params=[
    pytest.param(
        ("2.3.1", "CARGO2_SOURCE_ARCHIVE",
         "85c2bae9d3a1eb3121c9674f68ef6e87211fddb9c930f3e5057b9a1e66eb41d9"),
        id="dakota-community-2.3.1",
    ),
    pytest.param(
        ("2.3.3", "CARGO2_SDK_SOURCE_ARCHIVE",
         "28ccf91c48044ea2889b2dc2a5b5644e5e625479d87a8feb7214a4d1bfd6179a"),
        id="sdk-community-2.3.3",
    ),
])
def cargo2(request, tmp_path_factory):
    version, archive_env, archive_sha = request.param
    archive_name = os.environ.get(archive_env)
    if not archive_name:
        pytest.fail(f"{archive_env} must name the pinned community {version} tar.gz")
    archive = Path(archive_name)
    with archive.open("rb") as stream:
        actual_sha = hashlib.file_digest(stream, "sha256").hexdigest()
    assert actual_sha == archive_sha, f"cargo2 {version} source archive checksum mismatch"

    directory = tmp_path_factory.mktemp(f"cargo2-{version}-source")
    source_path = directory / SOURCE_PATH
    source_path.parent.mkdir(parents=True)
    archive_member = f"buildstream_plugins_community-{version}/" + SOURCE_PATH.as_posix()
    with tarfile.open(archive) as source_archive:
        with source_archive.extractfile(archive_member) as source_stream:
            source_path.write_bytes(source_stream.read())

    if os.environ.get("CARGO2_TEST_UNPATCHED") != "1":
        # Apply exactly the patch shipped by source preparation, not a test copy.
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        script = next(
            template["script"]["source"]
            for template in workflow["spec"]["templates"]
            if "CARGO2_SOURCE_PATCH" in template.get("script", {}).get("source", "")
        )
        preparation = script.split("<<'CARGO2_SOURCE_PATCH'\n", 1)[1].split(
            "\nCARGO2_SOURCE_PATCH", 1
        )[0]
        assignment = next(
            node for node in ast.parse(preparation).body
            if isinstance(node, ast.Assign)
            and any(isinstance(target, ast.Name) and target.id == "cargo2_patch"
                    for target in node.targets)
        )
        patch = directory / "cargo2.patch"
        patch.write_text(ast.literal_eval(assignment.value), encoding="utf-8")
        for arguments in (("--check",), ()):
            result = subprocess.run(
                ["git", "apply", *arguments, str(patch)], cwd=directory,
                capture_output=True, text=True, check=False, timeout=10,
            )
            assert result.returncode == 0, result.stdout + result.stderr

    definitions = {
        "CrateRegistry", "_NetrcPasswordManager", "_parse_netrc",
        "_UrlOpenerCreator", "download_file",
    }
    stdlib_imports = {
        "contextlib", "glob", "json", "netrc", "os", "shutil", "tarfile",
        "threading", "urllib",
    }
    tree = ast.parse(source_path.read_text(encoding="utf-8"))
    selected = [
        node for node in tree.body
        if (isinstance(node, (ast.ClassDef, ast.FunctionDef))
            and node.name in definitions)
        or (isinstance(node, ast.Import)
            and all(alias.name.split(".")[0] in stdlib_imports
                    for alias in node.names))
    ]
    namespace = {
        "__name__": "cargo2_under_test", "SourceFetcher": SourceFetcher,
        "SourceError": SourceError, "utils": utils,
    }
    exec(compile(ast.Module(body=selected, type_ignores=[]),
                 str(source_path), "exec"), namespace)
    return SimpleNamespace(CrateRegistry=namespace["CrateRegistry"])


class _CrateHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        conditional = self.headers.get("If-None-Match")
        if self.server.mode == "failure":
            status = 503
        elif self.server.mode == "unsolicited304" or (
            self.server.mode == "304" and conditional == ETAG
        ):
            status = 304
        else:
            status = 200
        self.server.requests.append((conditional, status))
        if self.server.before_response is not None:
            self.server.before_response()
        self.send_response(status)
        self.send_header("ETag", ETAG)
        self.send_header("Content-Length", str(len(BODY) if status == 200 else 0))
        self.end_headers()
        if status == 200:
            # The downloader deliberately closes a matching-ETag 200 early.
            with contextlib.suppress(BrokenPipeError, ConnectionResetError):
                self.wfile.write(BODY)

    def log_message(self, format, *args):
        pass


@pytest.fixture
def registry(cargo2, tmp_path, monkeypatch):
    home = tmp_path / "home"
    home.mkdir()
    monkeypatch.setenv("HOME", str(home))
    monkeypatch.setenv("NO_PROXY", "127.0.0.1")
    monkeypatch.setenv("no_proxy", "127.0.0.1")
    server = HTTPServer(("127.0.0.1", 0), _CrateHandler)
    server.mode = "304"
    server.requests = []
    server.before_response = None
    thread = threading.Thread(
        target=server.serve_forever, kwargs={"poll_interval": 0.01}, daemon=True,
    )
    thread.start()
    cargo = _Cargo(tmp_path, f"http://127.0.0.1:{server.server_port}/")
    crate = cargo2.CrateRegistry(cargo, "example", "1.0.0", sha=SHA256)
    try:
        yield crate, server
    finally:
        server.shutdown()
        thread.join(timeout=5)
        server.server_close()


class _Cargo:
    """Only the hosting Source API; all cache/download methods stay in cargo2."""

    def __init__(self, root, url):
        self.root = root
        self.url = url
        self.before_activity = None

    def get_mirror_directory(self):
        return str(self.root / "mirror")

    def translate_url(self, url, suffix="", alias_override=None, extra_data=None):
        return url + suffix

    def tempdir(self):
        return tempfile.TemporaryDirectory(prefix="crate-", dir=self.root)

    @contextlib.contextmanager
    def timed_activity(self, message, silent_nested=False):
        if self.before_activity is not None:
            self.before_activity()
        yield


def _seed_cache(crate, contents=BODY):
    Path(crate._get_mirror_dir()).mkdir(parents=True, exist_ok=True)
    if contents is not None:
        Path(crate._get_mirror_file()).write_bytes(contents)
    crate._store_etag(crate.sha, ETAG)


def test_fresh_fetch_verifies_and_caches_the_http_body(registry):
    crate, server = registry
    crate.fetch()
    cached = Path(crate._get_mirror_file())
    assert cached.read_bytes() == BODY
    assert utils.sha256sum(str(cached)) == crate.sha
    assert crate._get_etag(crate.sha) == ETAG
    assert server.requests == [(None, 200)]


@pytest.mark.parametrize("response", ["304", "200"])
def test_fetch_reuses_verified_cache_written_after_initial_cache_check(registry, response):
    crate, server = registry
    server.mode = response
    # Deterministically reproduce the concurrent oo7 fetcher's cache publication.
    # fetch() has already observed no cache when its timed activity starts.
    crate.cargo.before_activity = lambda: _seed_cache(crate)
    crate.fetch()
    assert Path(crate._get_mirror_file()).read_bytes() == BODY
    assert server.requests == [(ETAG, int(response))]


@pytest.mark.parametrize("contents", [None, b"corrupted cached crate"])
def test_stale_etag_without_verified_cache_downloads_a_fresh_body(registry, contents):
    crate, server = registry
    _seed_cache(crate, contents)
    assert crate._download(crate._get_url()[0], None) == SHA256
    assert Path(crate._get_mirror_file()).read_bytes() == BODY
    assert server.requests == [(None, 200)]


def test_http_failure_remains_a_temporary_source_error(registry):
    crate, server = registry
    server.mode = "failure"
    with pytest.raises(SourceError, match="503") as caught:
        crate._download(crate._get_url()[0], None)
    assert caught.value.temporary is True
    assert not Path(crate._get_mirror_file()).exists()


def test_fetch_still_rejects_a_body_that_disagrees_with_the_lockfile(registry):
    crate, server = registry
    crate.sha = hashlib.sha256(b"different lockfile content").hexdigest()
    with pytest.raises(SourceError, match="sha256sum") as caught:
        crate.fetch()
    assert caught.value.temporary is False
    assert not Path(crate._get_mirror_file()).exists()
    assert Path(crate._get_mirror_file(SHA256)).read_bytes() == BODY
    assert server.requests == [(None, 200)]


@pytest.mark.parametrize("cache_state", ["missing", "corrupted", "unresolved"])
def test_unsolicited_not_modified_never_claims_invalid_cache_success(registry, cache_state):
    crate, server = registry
    server.mode = "unsolicited304"
    if cache_state == "corrupted":
        _seed_cache(crate, b"corrupted cached crate")
    elif cache_state == "unresolved":
        crate.sha = None
    with pytest.raises(SourceError, match="valid cached copy") as caught:
        crate._download(crate._get_url()[0], None)
    assert caught.value.temporary is True
    assert server.requests == [(None, 304)]


@pytest.mark.parametrize("response", ["304", "200"])
@pytest.mark.parametrize("change", ["remove", "corrupt"])
def test_not_modified_rechecks_cache_after_the_http_response(registry, response, change):
    crate, server = registry
    _seed_cache(crate)
    server.mode = response
    cached = Path(crate._get_mirror_file())
    if change == "remove":
        server.before_response = cached.unlink
    else:
        server.before_response = lambda: cached.write_bytes(b"changed during request")
    with pytest.raises(SourceError, match="valid cached copy") as caught:
        crate._download(crate._get_url()[0], None)
    assert caught.value.temporary is True
    assert server.requests == [(ETAG, int(response))]
