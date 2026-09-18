"""Behaviour tests for the pr-poller RETIRED_REPOS guard.

A repo whose lab checks are retired must never be dispatched, by any path.
Dropping it from AUTO_REPOS alone is not enough: Pass 2's `test-on-lab`
catch-all reaches every open PR in the org, so a retired repo would still be
dispatched — and would still get a ghost-lab status — the moment someone
applied that label.

These tests run the real embedded poller script with stubbed `kubectl` and
`curl`, so they fail if the guard is removed, moved after the dispatch, or
narrowed to only one of the two passes.
"""

import json
import os
import subprocess
from pathlib import Path

import pytest
import yaml

from tests.unit.test_pr_poller_reaper import KUBECTL_STUB, make_pr, poller_script

ROOT = Path(__file__).resolve().parents[2]
POLLER = ROOT / "argo/workflow-templates/pr-poller.yaml"

pytestmark = pytest.mark.skipif(
    __import__("shutil").which("jq") is None,
    reason="jq is required to run the poller script",
)

RETIRED = "projectbluefin/testsuite"

# Unlike the reaper suite's stub, this one can return PRs for the
# `label:test-on-lab` search, which is what exercises the Pass 2 path.
CURL_STUB = '''#!/usr/bin/env python3
import json, os, sys, urllib.parse

state = json.load(open(os.environ["FAKE_STATE"]))
log = open(os.environ["FAKE_LOG"], "a")
url = [a for a in sys.argv[1:] if a.startswith("http")][-1]

if "/dispatches" in url:
    log.write("dispatch %s\\n" % url)
    sys.exit(0)

if "/search/issues" in url:
    query = urllib.parse.parse_qs(urllib.parse.urlparse(url).query)
    q = query["q"][0]
    page = int(query.get("page", ["1"])[0])
    if "label:test-on-lab" in q:
        labelled = state.get("labelled_prs", [])
        items = [] if page > 1 else [
            {
                "number": p["number"],
                "repository_url": "https://api.github.com/repos/%s"
                % p["base"]["repo"]["full_name"],
            }
            for p in labelled
        ]
        print(json.dumps({"total_count": len(labelled), "items": items}))
        sys.exit(0)
    repo = q.split("repo:")[1].split()[0]
    prs = [p for p in state["open_prs"] if p["base"]["repo"]["full_name"] == repo]
    items = [] if page > 1 else [
        {"number": p["number"], "pull_request": {"url": "https://api.github.com/pr/%s/%s" % (repo, p["number"])}}
        for p in prs
    ]
    print(json.dumps({"total_count": len(prs), "items": items}))
    sys.exit(0)

if "/repos/" in url and "/pulls" in url:
    query = urllib.parse.parse_qs(urllib.parse.urlparse(url).query)
    path = urllib.parse.urlparse(url).path
    repo = path.split("/repos/", 1)[1].split("/pulls", 1)[0]
    page = int(query.get("page", ["1"])[0])
    pool = state["open_prs"] + state.get("labelled_prs", [])
    prs = [p for p in pool if p["base"]["repo"]["full_name"] == repo]
    print(json.dumps([] if page > 1 else prs))
    sys.exit(0)

sys.exit(22)
'''


def run_poller(tmp_path, open_prs, labelled_prs=()):
    state = {
        "open_prs": list(open_prs),
        "labelled_prs": list(labelled_prs),
        "workflows": [],
        "search_fails": [],
    }
    state_file = tmp_path / "state.json"
    log_file = tmp_path / "calls.log"
    state_file.write_text(json.dumps(state), encoding="utf-8")
    log_file.write_text("", encoding="utf-8")

    bindir = tmp_path / "bin"
    bindir.mkdir()
    for name, body in (("kubectl", KUBECTL_STUB), ("curl", CURL_STUB)):
        path = bindir / name
        path.write_text(body, encoding="utf-8")
        path.chmod(0o755)

    script = tmp_path / "poller.sh"
    script.write_text(poller_script(), encoding="utf-8")

    env = dict(os.environ)
    env["PATH"] = f"{bindir}:{env['PATH']}"
    env["GITHUB_TOKEN"] = "stub-token"
    env["REFRESH_EXISTING"] = "false"
    env["FAKE_STATE"] = str(state_file)
    env["FAKE_LOG"] = str(log_file)

    proc = subprocess.run(
        ["bash", str(script)],
        env=env,
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    lines = log_file.read_text().splitlines()
    created = [line.split()[1:] for line in lines if line.startswith("create ")]
    dispatched = [line.split()[1] for line in lines if line.startswith("dispatch ")]
    return proc, created, dispatched


@pytest.fixture
def tmp_path(tmp_path_factory):
    return tmp_path_factory.mktemp("poller-retired")


def labelled_pr(repo, number, sha):
    """A PR as Pass 2 sees it — the `test-on-lab` label must be present.

    Pass 2 re-filters the pull listing on the label before dispatching, so a
    fixture without it never reaches `dispatch_pr` and the guard assertion
    would pass vacuously.
    """
    pr = make_pr(repo, number, sha)
    pr["labels"] = [{"name": "test-on-lab"}]
    return pr


def test_retired_repo_is_absent_from_auto_repos():
    """Pass 1 must not enumerate a retired repo."""
    source = poller_script()
    auto = source.split("AUTO_REPOS=(", 1)[1].split(")", 1)[0]
    assert RETIRED not in auto
    retired = source.split("RETIRED_REPOS=(", 1)[1].split(")", 1)[0]
    assert RETIRED in retired


def test_retired_repo_is_never_dispatched_by_the_label_catch_all(tmp_path):
    """The `test-on-lab` label must not resurrect a retired repo's checks.

    This is the case a bare AUTO_REPOS removal misses: Pass 2 searches the whole
    org, so without the dispatch_pr guard this PR would be dispatched and would
    receive a ghost-lab status.
    """
    labelled = [labelled_pr(RETIRED, 860, "a" * 40)]
    proc, created, dispatched = run_poller(tmp_path, open_prs=[], labelled_prs=labelled)

    assert proc.returncode == 0, proc.stderr
    assert created == []
    assert dispatched == []
    assert "lab checks are retired" in proc.stderr


def test_active_repos_still_dispatch(tmp_path):
    """The guard must be scoped to retired repos only."""
    open_prs = [make_pr("projectbluefin/knuckle", 42, "b" * 40)]
    proc, created, _ = run_poller(tmp_path, open_prs=open_prs)

    assert proc.returncode == 0, proc.stderr
    assert ["knuckle", "42", "b" * 12] in created


def test_retired_and_active_repos_in_one_poll(tmp_path):
    """A retired repo must not suppress dispatch for everyone else."""
    open_prs = [make_pr("projectbluefin/knuckle", 43, "c" * 40)]
    labelled = [labelled_pr(RETIRED, 861, "d" * 40)]
    proc, created, _ = run_poller(tmp_path, open_prs=open_prs, labelled_prs=labelled)

    assert proc.returncode == 0, proc.stderr
    products = {row[0] for row in created}
    assert "knuckle" in products
    assert "testsuite" not in products


def test_retirement_is_documented_where_it_is_declared():
    """A bare list entry is not self-explanatory; keep the rationale adjacent."""
    source = poller_script()
    preamble = source.split("RETIRED_REPOS=(", 1)[0].rsplit("done", 1)[-1]
    assert "RETIRED" in preamble
    assert "Pass 2" in preamble


def test_poller_template_still_parses_as_a_workflow_template():
    """Guard against a YAML-level regression in the embedded script block."""
    doc = yaml.safe_load(POLLER.read_text(encoding="utf-8"))
    assert doc["kind"] == "WorkflowTemplate"
    names = [t["name"] for t in doc["spec"]["templates"]]
    assert "poll-labeled-prs" in names
