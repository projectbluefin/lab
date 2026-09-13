import json
import sys
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))

import dakota_validation as dv


def mock_pr_data(number=101, sha="a" * 40, state="open"):
    return {
        "number": number,
        "state": state,
        "head": {
            "sha": sha,
            "ref": "feature-branch",
        },
    }


def make_workflow(name, pr, sha, phase="Running", shutdown=None, message=""):
    wf = {
        "metadata": {
            "name": name,
            "labels": {
                "bluefin.io/trigger": "on-demand",
                "bluefin.io/validation-class": "dakota-bst-qa",
                "bluefin.io/repository": "dakota",
                "bluefin.io/pr-number": str(pr),
                "bluefin.io/pr-sha": sha[:12],
            },
        },
        "spec": {
            "arguments": {
                "parameters": [
                    {"name": "repository", "value": "projectbluefin/dakota"},
                    {"name": "pr-number", "value": str(pr)},
                    {"name": "commit-sha", "value": sha},
                    {"name": "validation-class", "value": "dakota-bst-qa"},
                ]
            }
        },
        "status": {
            "phase": phase,
            "message": message,
        },
    }
    if shutdown:
        wf["spec"]["shutdown"] = shutdown
    return wf


# --- Acceptance Criteria 1: Rejection tests ---

def test_rejects_unsupported_repository():
    valid, err = dv.validate_request_parameters(
        repository="projectbluefin/bluefin",
        validation_class="dakota-bst-qa",
        pr_number=100,
        sha="a" * 40,
    )
    assert not valid
    assert "unsupported repository" in err

    result = dv.submit_validation_request(
        repository="projectbluefin/bluefin",
        validation_class="dakota-bst-qa",
        pr_number=100,
        sha="a" * 40,
        publish=False,
    )
    assert result["status"] == "rejected"
    assert "unsupported repository" in result["reason"]


def test_rejects_unsupported_validation_class():
    valid, err = dv.validate_request_parameters(
        repository="projectbluefin/dakota",
        validation_class="dakota-qa",
        pr_number=100,
        sha="a" * 40,
    )
    assert not valid
    assert "unsupported validation class" in err

    result = dv.submit_validation_request(
        repository="projectbluefin/dakota",
        validation_class="arbitrary-class",
        pr_number=100,
        sha="a" * 40,
        publish=False,
    )
    assert result["status"] == "rejected"
    assert "unsupported validation class" in result["reason"]


@pytest.mark.parametrize(
    "bad_sha",
    [
        "a" * 39,
        "a" * 41,
        "xyz" + "0" * 37,
        "main",
        "HEAD",
        "",
        "   ",
    ],
)
def test_rejects_malformed_sha(bad_sha):
    valid, err = dv.validate_request_parameters(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=100,
        sha=bad_sha,
    )
    assert not valid
    assert "malformed commit SHA" in err

    result = dv.submit_validation_request(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=100,
        sha=bad_sha,
        publish=False,
    )
    assert result["status"] == "rejected"
    assert "malformed commit SHA" in result["reason"]


@pytest.mark.parametrize("bad_pr", [0, -1, "abc", ""])
def test_rejects_invalid_pr_number(bad_pr):
    valid, err = dv.validate_request_parameters(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=bad_pr,
        sha="a" * 40,
    )
    assert not valid
    assert "invalid PR number" in err


def test_rejects_closed_pr():
    sha = "b" * 40
    pr_num = 120

    def fake_fetcher(url, headers):
        return 200, mock_pr_data(number=pr_num, sha=sha, state="closed")

    valid_head, head_err, _ = dv.check_live_pr_head(
        repository="projectbluefin/dakota",
        pr_number=pr_num,
        sha=sha,
        fetcher=fake_fetcher,
    )
    assert not valid_head
    assert "closed PR" in head_err

    result = dv.submit_validation_request(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha,
        fetcher=fake_fetcher,
        publish=False,
    )
    assert result["status"] == "rejected"
    assert "closed PR" in result["reason"]


def test_rejects_head_mismatch():
    requested_sha = "c" * 40
    live_sha = "d" * 40
    pr_num = 121

    def fake_fetcher(url, headers):
        return 200, mock_pr_data(number=pr_num, sha=live_sha, state="open")

    valid_head, head_err, _ = dv.check_live_pr_head(
        repository="projectbluefin/dakota",
        pr_number=pr_num,
        sha=requested_sha,
        fetcher=fake_fetcher,
    )
    assert not valid_head
    assert "head mismatch" in head_err
    assert requested_sha in head_err
    assert live_sha in head_err

    result = dv.submit_validation_request(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=requested_sha,
        fetcher=fake_fetcher,
        publish=False,
    )
    assert result["status"] == "rejected"
    assert "head mismatch" in result["reason"]


# --- Acceptance Criteria 2: Canonical reporting statuses ---

def test_reports_all_canonical_statuses():
    # 8 required statuses:
    # unavailable, rejected, queued, running, success, failure, cancelled, timeout
    assert dv.VALID_STATUSES == {
        "unavailable",
        "rejected",
        "queued",
        "running",
        "success",
        "failure",
        "cancelled",
        "timeout",
    }

    sha = "e" * 40
    pr_num = 130

    # 1. unavailable: query without any workflows or commit statuses
    res = dv.query_validation_status(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha,
        runner=lambda cmd: json.dumps({"items": []}),
        fetcher=lambda url, h: (200, []),
    )
    assert res["status"] == "unavailable"

    # 2. rejected: invalid SHA
    res = dv.query_validation_status(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha="bad-sha",
    )
    assert res["status"] == "rejected"

    # 3. queued
    wf_queued = make_workflow("wf-queued", pr_num, sha, phase="Pending")
    res = dv.query_validation_status(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha,
        runner=lambda cmd: json.dumps({"items": [wf_queued]}),
    )
    assert res["status"] == "queued"

    # 4. running
    wf_running = make_workflow("wf-running", pr_num, sha, phase="Running")
    res = dv.query_validation_status(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha,
        runner=lambda cmd: json.dumps({"items": [wf_running]}),
    )
    assert res["status"] == "running"

    # 5. success
    wf_succeeded = make_workflow("wf-succeeded", pr_num, sha, phase="Succeeded")
    res = dv.query_validation_status(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha,
        runner=lambda cmd: json.dumps({"items": [wf_succeeded]}),
    )
    assert res["status"] == "success"

    # 6. failure
    wf_failed = make_workflow("wf-failed", pr_num, sha, phase="Failed", message="build step error")
    res = dv.query_validation_status(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha,
        runner=lambda cmd: json.dumps({"items": [wf_failed]}),
    )
    assert res["status"] == "failure"

    # 7. cancelled
    wf_cancelled = make_workflow("wf-cancelled", pr_num, sha, phase="Running", shutdown="Stop")
    res = dv.query_validation_status(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha,
        runner=lambda cmd: json.dumps({"items": [wf_cancelled]}),
    )
    assert res["status"] == "cancelled"

    # 8. timeout
    wf_timeout = make_workflow(
        "wf-timeout", pr_num, sha, phase="Failed", message="activeDeadlineSeconds exceeded (timeout)"
    )
    res = dv.query_validation_status(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha,
        runner=lambda cmd: json.dumps({"items": [wf_timeout]}),
    )
    assert res["status"] == "timeout"


# --- Acceptance Criteria 3: Deduplication of identical active requests ---

def test_dedupes_identical_active_requests():
    sha = "f" * 40
    pr_num = 140

    def fake_fetcher(url, headers):
        return 200, mock_pr_data(number=pr_num, sha=sha, state="open")

    active_wf = make_workflow("wf-active-140", pr_num, sha, phase="Running")

    def fake_runner(cmd):
        return json.dumps({"items": [active_wf]})

    created = []
    def fake_creator(manifest):
        created.append(manifest)
        return "should-not-be-created"

    result = dv.submit_validation_request(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha,
        fetcher=fake_fetcher,
        runner=fake_runner,
        creator=fake_creator,
        publish=False,
    )

    assert result["status"] == "running"
    assert result["deduped"] is True
    assert result["workflow_name"] == "wf-active-140"
    assert len(created) == 0, "must not submit a duplicate active workflow"


# --- Acceptance Criteria 4: Allow explicit terminal rerun ---

def test_allows_explicit_terminal_rerun():
    sha = "1" * 40
    pr_num = 150

    def fake_fetcher(url, headers):
        return 200, mock_pr_data(number=pr_num, sha=sha, state="open")

    terminal_wf = make_workflow("wf-terminal-150", pr_num, sha, phase="Failed", message="error")

    def fake_runner(cmd):
        return json.dumps({"items": [terminal_wf]})

    created = []
    def fake_creator(manifest):
        created.append(manifest)
        return "wf-rerun-150"

    # Without rerun=True: dedupes terminal run, does not create new workflow
    result1 = dv.submit_validation_request(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha,
        rerun=False,
        fetcher=fake_fetcher,
        runner=fake_runner,
        creator=fake_creator,
        publish=False,
    )
    assert result1["status"] == "failure"
    assert result1["deduped"] is True
    assert len(created) == 0

    # With rerun=True: submits a new workflow
    result2 = dv.submit_validation_request(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha,
        rerun=True,
        fetcher=fake_fetcher,
        runner=fake_runner,
        creator=fake_creator,
        publish=False,
    )
    assert result2["status"] == "queued"
    assert result2["deduped"] is False
    assert result2["rerun"] is True
    assert result2["workflow_name"] == "wf-rerun-150"
    assert len(created) == 1


# --- Acceptance Criteria 5: Independent requests for multiple PRs ---

def test_independent_requests_for_multiple_prs():
    sha1 = "2" * 40
    sha2 = "3" * 40
    pr1 = 201
    pr2 = 202

    def fake_fetcher(url, headers):
        if str(pr1) in url:
            return 200, mock_pr_data(number=pr1, sha=sha1)
        if str(pr2) in url:
            return 200, mock_pr_data(number=pr2, sha=sha2)
        return 404, {}

    active_wf_pr1 = make_workflow("wf-pr1", pr1, sha1, phase="Running")

    def fake_runner(cmd):
        # Runner filters by pr-number
        cmd_str = " ".join(cmd)
        if f"bluefin.io/pr-number={pr1}" in cmd_str:
            return json.dumps({"items": [active_wf_pr1]})
        return json.dumps({"items": []})

    created = []
    def fake_creator(manifest):
        created.append(manifest)
        return "wf-pr2"

    # PR 1 is active -> deduped
    res1 = dv.submit_validation_request(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr1,
        sha=sha1,
        fetcher=fake_fetcher,
        runner=fake_runner,
        creator=fake_creator,
        publish=False,
    )
    assert res1["status"] == "running"
    assert res1["deduped"] is True
    assert len(created) == 0

    # PR 2 is submitted independently
    res2 = dv.submit_validation_request(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr2,
        sha=sha2,
        fetcher=fake_fetcher,
        runner=fake_runner,
        creator=fake_creator,
        publish=False,
    )
    assert res2["status"] == "queued"
    assert res2["deduped"] is False
    assert len(created) == 1
    assert res2["workflow_name"] == "wf-pr2"


# --- Acceptance Criteria 6: Evidence from SHA A never applies to SHA B ---

def test_evidence_from_sha_a_never_applies_to_sha_b():
    sha_a = "4" * 40
    sha_b = "5" * 40
    pr_num = 300

    wf_sha_a = make_workflow("wf-sha-a", pr_num, sha_a, phase="Succeeded")

    def fake_runner(cmd):
        cmd_str = " ".join(cmd)
        if sha_a[:12] in cmd_str:
            return json.dumps({"items": [wf_sha_a]})
        return json.dumps({"items": []})

    # Query for SHA A reports success
    res_a = dv.query_validation_status(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha_a,
        runner=fake_runner,
        fetcher=lambda u, h: (404, {}),
    )
    assert res_a["status"] == "success"
    assert res_a["sha"] == sha_a

    # Query for SHA B reports unavailable (never leaks SHA A's evidence)
    res_b = dv.query_validation_status(
        repository="projectbluefin/dakota",
        validation_class="dakota-bst-qa",
        pr_number=pr_num,
        sha=sha_b,
        runner=fake_runner,
        fetcher=lambda u, h: (404, {}),
    )
    assert res_b["status"] == "unavailable"
    assert res_b["sha"] == sha_b


# --- Integration & Manifest verification ---

def test_dakota_bst_qa_workflow_template_contract():
    tmpl_path = ROOT / "argo/workflow-templates/dakota-bst-qa.yaml"
    assert tmpl_path.exists()
    doc = yaml.safe_load(tmpl_path.read_text(encoding="utf-8"))

    assert doc["metadata"]["name"] == "dakota-bst-qa"
    assert doc["spec"]["entrypoint"] == "pipeline"
    assert doc["spec"]["onExit"] == "report-final"

    templates = {t["name"]: t for t in doc["spec"]["templates"]}
    assert {"pipeline", "report-final"} <= templates.keys()

    tasks = {t["name"]: t for t in templates["pipeline"]["dag"]["tasks"]}
    assert {"report-start", "build", "qa-dakota"} <= tasks.keys()

    # Build must invoke dakota-build-pipeline with exact SHA parameters
    build_task = tasks["build"]
    assert build_task["templateRef"]["name"] == "dakota-build-pipeline"
    assert build_task["templateRef"]["template"] == "build"
    build_params = {p["name"]: p["value"] for p in build_task["arguments"]["parameters"]}
    assert build_params["commit-sha"] == "{{workflow.parameters.commit-sha}}"
    assert build_params["image-tag"] == "{{workflow.parameters.commit-sha}}"
    assert build_params["build-mode"] == "{{workflow.parameters.build-mode}}"

    # QA must invoke dakota-qa-pipeline after build succeeds
    qa_task = tasks["qa-dakota"]
    assert qa_task["depends"] == "build.Succeeded"
    assert qa_task["templateRef"]["name"] == "dakota-qa-pipeline"
    assert qa_task["templateRef"]["template"] == "pipeline"
    qa_params = {p["name"]: p["value"] for p in qa_task["arguments"]["parameters"]}
    assert qa_params["image-tag"] == "{{workflow.parameters.commit-sha}}"


def test_github_status_reporter_allowlist_includes_dakota():
    reporter_path = ROOT / "argo/workflow-templates/github-status-reporter.yaml"
    content = reporter_path.read_text(encoding="utf-8")
    assert "projectbluefin/dakota" in content


def test_dakota_validation_dispatch_workflow():
    wf_path = ROOT / ".github/workflows/dakota-validation-dispatch.yml"
    assert wf_path.exists()
    doc = yaml.safe_load(wf_path.read_text(encoding="utf-8"))

    triggers = doc.get("on") or doc.get(True)
    assert "repository_dispatch" in triggers
    assert "workflow_dispatch" in triggers
    assert "dakota-validation" in triggers["repository_dispatch"]["types"]
    assert "dakota-bst-qa" in triggers["repository_dispatch"]["types"]

    job = doc["jobs"]["submit-dakota-validation"]
    assert job["runs-on"] == "ghost-runners"
    assert job["container"]["image"] == "ghcr.io/projectbluefin/arc-runner:latest"
