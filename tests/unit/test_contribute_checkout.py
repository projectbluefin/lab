import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]


def checkout_script(tmp_path):
    documents = yaml.safe_load_all((ROOT / "manifests/contribute.yaml").read_text())
    data = next(d["data"] for d in documents if d and d.get("kind") == "ConfigMap")
    assert "checkout.py" in data, "deterministic checkout asset is not implemented"
    script = tmp_path / "checkout.py"
    script.write_text(data["checkout.py"])
    return script


def run_checkout(tmp_path, request, *, env=None):
    script = checkout_script(tmp_path)
    checkout_env = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}
    if env:
        checkout_env.update(env)
    return subprocess.run(
        [sys.executable, str(script), json.dumps(request)],
        cwd=tmp_path, env=checkout_env, capture_output=True, text=True,
    )


def git(cwd, *args):
    env = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}
    return subprocess.run(["git", "-C", str(cwd), *args], env=env, check=True, capture_output=True, text=True).stdout.strip()


def source_repo(tmp_path):
    source = tmp_path / "source"
    source.mkdir()
    git(source, "init", "-q")
    git(source, "config", "user.name", "Checkout Test")
    git(source, "config", "user.email", "test@example.invalid")
    commits = []
    for version in range(3):
        (source / "value.txt").write_text(str(version))
        git(source, "add", "value.txt")
        git(source, "commit", "-qm", f"version {version}")
        commits.append(git(source, "rev-parse", "HEAD"))
    return source, commits


def test_checkout_fetches_current_tip_shallow(tmp_path):
    source, commits = source_repo(tmp_path)
    destination = tmp_path / "work"
    result = run_checkout(tmp_path, {"url": source.as_uri(), "destination": str(destination)})
    assert result.returncode == 0, result.stderr + result.stdout
    response = json.loads(result.stdout)
    assert response["cwd"] == str(destination.resolve())
    assert response["commit"] == commits[-1]
    assert (destination / "value.txt").read_text() == "2"
    assert git(destination, "rev-list", "--count", "HEAD") == "1"
    assert response["ok"] is True
    assert response["ref"] == "HEAD"
    assert response["depth"] == 1
    assert git(destination, "remote", "get-url", "upstream") == source.as_uri()
    assert git(destination, "branch", "--show-current") == ""


def test_checkout_never_overwrites_existing_destination(tmp_path):
    source, _ = source_repo(tmp_path)
    destination = tmp_path / "existing"
    destination.mkdir()
    marker = destination / "keep.txt"
    marker.write_text("user work")
    result = run_checkout(tmp_path, {"url": source.as_uri(), "destination": str(destination)})
    assert result.returncode != 0
    assert marker.read_text() == "user work"
    assert not (destination / ".git").exists()


def test_checkout_preserves_explicit_commit_ref(tmp_path):
    source, commits = source_repo(tmp_path)
    destination = tmp_path / "older"
    result = run_checkout(tmp_path, {"url": source.as_uri(), "destination": str(destination), "ref": commits[0]})
    assert result.returncode == 0, result.stderr + result.stdout
    assert json.loads(result.stdout)["commit"] == commits[0]
    assert (destination / "value.txt").read_text() == "0"


@pytest.mark.parametrize("kind", ["branch", "tag"])
def test_checkout_preserves_explicit_named_ref(tmp_path, kind):
    source, commits = source_repo(tmp_path)
    ref = "older-release"
    if kind == "branch":
        git(source, "branch", ref, commits[0])
    else:
        git(source, "tag", "-a", ref, commits[0], "-m", "older release")
    destination = tmp_path / "named-ref"
    result = run_checkout(tmp_path, {
        "url": source.as_uri(), "destination": str(destination), "ref": ref,
    })
    assert result.returncode == 0, result.stderr + result.stdout
    response = json.loads(result.stdout)
    assert response["commit"] == commits[0]
    assert response["ref"] == ref
    assert (destination / "value.txt").read_text() == "0"
    assert git(destination, "rev-list", "--count", "HEAD") == "1"
    assert git(source, "rev-parse", "HEAD") == commits[-1]


def test_checkout_honors_explicit_positive_depth(tmp_path):
    source, commits = source_repo(tmp_path)
    destination = tmp_path / "two-commits"
    result = run_checkout(tmp_path, {
        "url": source.as_uri(), "destination": str(destination), "depth": 2,
    })
    assert result.returncode == 0, result.stderr + result.stdout
    response = json.loads(result.stdout)
    assert response["depth"] == 2
    assert response["commit"] == commits[-1]
    assert git(destination, "rev-list", "--count", "HEAD") == "2"


@pytest.mark.parametrize("depth", [0, -1, True, False, 1.5, "1", None])
def test_checkout_rejects_invalid_depth_before_creating_destination(tmp_path, depth):
    source, _ = source_repo(tmp_path)
    destination = tmp_path / "invalid-depth"
    result = run_checkout(tmp_path, {
        "url": source.as_uri(), "destination": str(destination), "depth": depth,
    })
    assert result.returncode != 0
    response = json.loads(result.stdout)
    assert response["ok"] is False
    assert "depth" in response["error"].lower()
    assert not destination.exists()


@pytest.mark.parametrize("url", [
    "https://username:secret@example.invalid/repo.git",
    "https://username@example.invalid/repo.git",
    "http://:secret@example.invalid/repo.git",
    "file://username:secret@localhost/repo.git",
])
def test_checkout_rejects_embedded_credentials_before_mutations(tmp_path, url):
    destination = tmp_path / "credentials"
    result = run_checkout(tmp_path, {"url": url, "destination": str(destination)})
    assert result.returncode != 0
    response = json.loads(result.stdout)
    assert response["ok"] is False
    assert "credential" in response["error"].lower()
    assert "secret" not in result.stdout + result.stderr
    assert not destination.exists()


@pytest.mark.parametrize("url", [
    "", "not-a-url", "ssh://example.invalid/repo.git", "https:///repo.git",
    "file:relative/repo.git", "file:/absolute/repo.git", "file://other-host/repo.git",
    "https://example.invalid:bad/repo.git", "https://example.invalid/rep\no.git",
])
def test_checkout_rejects_invalid_url_before_mutations(tmp_path, url):
    destination = tmp_path / "invalid-url"
    result = run_checkout(tmp_path, {"url": url, "destination": str(destination)})
    assert result.returncode != 0
    assert json.loads(result.stdout)["ok"] is False
    assert not destination.exists()


@pytest.mark.parametrize("destination", [None, "", "relative/path", 1])
def test_checkout_rejects_invalid_destination(tmp_path, destination):
    source, _ = source_repo(tmp_path)
    before = set(tmp_path.iterdir())
    result = run_checkout(tmp_path, {"url": source.as_uri(), "destination": destination})
    assert result.returncode != 0
    response = json.loads(result.stdout)
    assert response["ok"] is False
    assert "destination" in response["error"].lower()
    assert set(tmp_path.iterdir()) - before == {tmp_path / "checkout.py"}


@pytest.mark.parametrize("payload", [None, [], "a request", {}, {"url": "https://example.invalid/repo.git"}])
def test_checkout_rejects_incomplete_or_nonobject_requests(tmp_path, payload):
    result = run_checkout(tmp_path, payload)
    assert result.returncode != 0
    response = json.loads(result.stdout)
    assert response["ok"] is False
    assert response["error"]
    assert set(tmp_path.iterdir()) == {tmp_path / "checkout.py"}


@pytest.mark.parametrize("ref", [None, "", "--upload-pack=touch", "HEAD:refs/heads/other", "HEAD\n", 1])
def test_checkout_rejects_invalid_ref_before_mutations(tmp_path, ref):
    source, _ = source_repo(tmp_path)
    destination = tmp_path / "invalid-ref"
    result = run_checkout(tmp_path, {
        "url": source.as_uri(), "destination": str(destination), "ref": ref,
    })
    assert result.returncode != 0
    response = json.loads(result.stdout)
    assert response["ok"] is False
    assert "ref" in response["error"].lower()
    assert not destination.exists()


@pytest.mark.parametrize("kind", ["empty-directory", "file", "symlink", "dangling-symlink"])
def test_checkout_leaves_all_existing_destination_types_untouched(tmp_path, kind):
    source, _ = source_repo(tmp_path)
    destination = tmp_path / "existing"
    if kind == "empty-directory":
        destination.mkdir()
    elif kind == "file":
        destination.write_text("user file")
    else:
        target = tmp_path / "target"
        if kind == "symlink":
            target.mkdir()
            (target / "keep.txt").write_text("user work")
        destination.symlink_to(target, target_is_directory=True)
    original_stat = destination.lstat()
    result = run_checkout(tmp_path, {"url": source.as_uri(), "destination": str(destination)})
    assert result.returncode != 0
    response = json.loads(result.stdout)
    assert response["ok"] is False
    assert str(destination) in response["error"]
    assert destination.lstat().st_ino == original_stat.st_ino
    if kind == "empty-directory":
        assert list(destination.iterdir()) == []
    elif kind == "file":
        assert destination.read_text() == "user file"
    else:
        assert destination.is_symlink()
        assert destination.readlink() == target
        if kind == "symlink":
            assert (target / "keep.txt").read_text() == "user work"
            assert not (target / ".git").exists()
        else:
            assert not target.exists()


@pytest.mark.parametrize("parent_kind", ["missing", "file"])
def test_checkout_reports_filesystem_creation_failure(tmp_path, parent_kind):
    source, _ = source_repo(tmp_path)
    parent = tmp_path / "parent"
    if parent_kind == "file":
        parent.write_text("not a directory")
    destination = parent / "checkout"
    result = run_checkout(tmp_path, {"url": source.as_uri(), "destination": str(destination)})
    assert result.returncode != 0
    response = json.loads(result.stdout)
    assert response["ok"] is False
    assert str(destination) in response["error"]
    assert not destination.exists()
    if parent_kind == "file":
        assert parent.read_text() == "not a directory"
    else:
        assert not parent.exists()


def test_checkout_failed_requested_ref_keeps_partial_path_without_fallback(tmp_path):
    source, commits = source_repo(tmp_path)
    destination = tmp_path / "partial"
    result = run_checkout(tmp_path, {
        "url": source.as_uri(), "destination": str(destination), "ref": "missing-ref",
    })
    assert result.returncode != 0
    response = json.loads(result.stdout)
    assert response["ok"] is False
    assert str(destination.resolve()) in response["error"]
    assert (destination / ".git").is_dir()
    assert not (destination / "value.txt").exists()
    assert git(source, "rev-parse", "HEAD") == commits[-1]


def test_checkout_accepts_space_and_shell_metacharacters_in_paths(tmp_path):
    source, commits = source_repo(tmp_path)
    mirror = tmp_path / "local mirror ; $(touch forbidden)"
    git(tmp_path, "clone", "--mirror", source.as_uri(), str(mirror))
    destination = tmp_path / "working tree ; $(touch forbidden)"
    result = run_checkout(tmp_path, {"url": mirror.as_uri(), "destination": str(destination)})
    assert result.returncode == 0, result.stderr + result.stdout
    assert json.loads(result.stdout)["commit"] == commits[-1]
    assert (destination / "value.txt").read_text() == "2"
    assert not (tmp_path / "forbidden").exists()


def test_checkout_does_not_mutate_global_git_configuration(tmp_path):
    source, _ = source_repo(tmp_path)
    config = tmp_path / "global.gitconfig"
    original = "[user]\n\tname = Existing User\n\temail = existing@example.invalid\n"
    config.write_text(original)
    destination = tmp_path / "isolated"
    result = run_checkout(
        tmp_path, {"url": source.as_uri(), "destination": str(destination)},
        env={"GIT_CONFIG_GLOBAL": str(config)},
    )
    assert result.returncode == 0, result.stderr + result.stdout
    assert config.read_text() == original


def test_checkout_ignores_inherited_repository_write_overrides(tmp_path):
    source, commits = source_repo(tmp_path)
    index = tmp_path / "unrelated.index"
    index.write_text("do not replace this index")
    destination = tmp_path / "confined"
    result = run_checkout(
        tmp_path, {
            "url": source.as_uri(), "destination": str(destination), "ref": commits[0],
        },
        env={
            "GIT_DIR": str(source / ".git"),
            "GIT_WORK_TREE": str(source),
            "GIT_INDEX_FILE": str(index),
            "GIT_OBJECT_DIRECTORY": str(source / ".git" / "objects"),
        },
    )
    assert result.returncode == 0, result.stderr + result.stdout
    assert json.loads(result.stdout)["commit"] == commits[0]
    assert (destination / "value.txt").read_text() == "0"
    assert git(source, "rev-parse", "HEAD") == commits[-1]
    assert (source / "value.txt").read_text() == "2"
    assert index.read_text() == "do not replace this index"
    assert git(source, "remote") == ""


def test_checkout_reports_real_destination_through_symlink_parent(tmp_path):
    source, commits = source_repo(tmp_path)
    parent = tmp_path / "real-parent"
    parent.mkdir()
    link = tmp_path / "parent-link"
    link.symlink_to(parent, target_is_directory=True)
    destination = link / "checkout"
    result = run_checkout(tmp_path, {"url": source.as_uri(), "destination": str(destination)})
    assert result.returncode == 0, result.stderr + result.stdout
    response = json.loads(result.stdout)
    assert response["cwd"] == str(parent / "checkout")
    assert response["commit"] == commits[-1]
    assert (parent / "checkout" / "value.txt").read_text() == "2"
    assert link.is_symlink()


def test_checkout_missing_repository_keeps_partial_checkout(tmp_path):
    source = tmp_path / "nonexistent.git"
    destination = tmp_path / "partial"
    result = run_checkout(tmp_path, {"url": source.as_uri(), "destination": str(destination)})
    assert result.returncode != 0
    response = json.loads(result.stdout)
    assert response["ok"] is False
    assert str(destination) in response["error"]
    assert (destination / ".git").is_dir()
    assert not source.exists()


def test_checkout_invalid_json_returns_structured_error(tmp_path):
    script = checkout_script(tmp_path)
    result = subprocess.run(
        [sys.executable, str(script), "{"],
        cwd=tmp_path, capture_output=True, text=True,
    )
    assert result.returncode != 0
    response = json.loads(result.stdout)
    assert response["ok"] is False
    assert response["error"]
    assert set(tmp_path.iterdir()) == {tmp_path / "checkout.py"}


def test_local_mirror_works_without_external_upload_pack(tmp_path):
    source, commits = source_repo(tmp_path)
    binaries = tmp_path / "only_git"
    binaries.mkdir()
    (binaries / "git").symlink_to(shutil.which("git"))
    result = run_checkout(tmp_path, {
        "url": source.as_uri(), "destination": str(tmp_path / "slim_checkout"),
    }, env={"PATH": str(binaries), "GIT_EXEC_PATH": str(binaries)})
    assert result.returncode == 0, result.stderr + result.stdout
    assert json.loads(result.stdout)["commit"] == commits[-1]
    assert (tmp_path / "slim_checkout/value.txt").read_text() == "2"
