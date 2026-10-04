import json
import os
import subprocess
from pathlib import Path

import pytest
import yaml


ROOT = Path(__file__).resolve().parents[2]
REMOTE_SHA = "a" * 40
STORED_SHA = "b" * 40


def load(path):
    return yaml.safe_load((ROOT / path).read_text(encoding="utf-8"))


def run_check_sha(tmp_path, *, lanes, running, force="false", stored=STORED_SHA):
    """Run the real check-sha script against stub kubectl/curl; return (changed, result)."""
    template = load("argo/workflow-templates/bst-commit-poller.yaml")
    check = next(t for t in template["spec"]["templates"] if t["name"] == "check-sha")
    source = check["script"]["source"].replace("/tmp/", f"{tmp_path}/")
    for name, value in {
        "repo": "projectbluefin/dakota",
        "branch": "testing",
        "state-key": "dakota-testing",
        "force": force,
    }.items():
        source = source.replace(f"{{{{workflow.parameters.{name}}}}}", value)

    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    (tmp_path / "workflows.json").write_text(
        json.dumps({"items": [{"status": {"phase": "Running"}}] * running
                   + [{"status": {"phase": "Succeeded"}}] * 3})
    )
    stubs = {
        "curl": f"echo '[{{\"sha\": \"{REMOTE_SHA}\"}}]'",
        "kubectl": f"""echo "$*" >> {tmp_path}/kubectl.log
case "$*" in
  *"configmap workflow-semaphores"*) printf '%s' '{lanes}' ;;
  *"configmap image-polling-digests"*) printf '%s' '{stored}' ;;
  *"get workflows"*"bluefin.io/bst-workload=true"*) cat {tmp_path}/workflows.json ;;
  *) exit 1 ;;
esac""",
    }
    for name, body in stubs.items():
        stub = bin_dir / name
        stub.write_text(f"#!/bin/bash\n{body}\n")
        stub.chmod(0o755)

    env = {**os.environ, "PATH": f"{bin_dir}:{os.environ['PATH']}", "GITHUB_TOKEN": "x"}
    result = subprocess.run(["bash", "-c", source], env=env, capture_output=True, text=True)
    log = (tmp_path / "kubectl.log").read_text() if (tmp_path / "kubectl.log").exists() else ""
    assert "patch" not in log, "check-sha must not persist state"
    changed = (tmp_path / "changed").read_text().strip() if result.returncode == 0 else None
    return changed, result


@pytest.mark.parametrize(
    ("lanes", "running", "admitted"),
    [
        # Running bst-workload workflows include the poller itself; every
        # bst-build lane may execute and one more workflow may wait.
        (1, 1, "true"),
        (1, 2, "false"),
        (2, 2, "true"),
        (2, 3, "false"),
        (3, 3, "true"),
        (3, 4, "false"),
    ],
)
def test_poller_admits_one_waiter_beyond_bst_build_lanes(tmp_path, lanes, running, admitted):
    changed, result = run_check_sha(tmp_path, lanes=lanes, running=running)
    assert result.returncode == 0, result.stderr
    assert changed == admitted, result.stderr


@pytest.mark.parametrize(("running", "admitted"), [(2, "true"), (3, "false")])
def test_forced_poll_still_respects_lane_ceiling(tmp_path, running, admitted):
    changed, result = run_check_sha(tmp_path, lanes=2, running=running, force="true", stored=REMOTE_SHA)
    assert changed == admitted, result.stderr


def test_unchanged_commit_is_not_rebuilt(tmp_path):
    changed, result = run_check_sha(tmp_path, lanes=2, running=1, stored=REMOTE_SHA)
    assert changed == "false", result.stderr


def test_poller_fails_on_invalid_lane_count(tmp_path):
    changed, result = run_check_sha(tmp_path, lanes="", running=1)
    assert result.returncode != 0
    assert "bst-build must be a positive integer" in result.stderr


def test_shared_bst_pollers_are_suspended_but_staggered_for_on_demand():
    # The dakota commit poller stays suspended since #609 (failing poller).
    # The file and schedule are kept so `argo submit --from
    # cronworkflow/dakota-commit-poller` and `just force-dakota-poll` remain
    # the on-demand escape hatch.
    template = load("argo/workflow-templates/bst-commit-poller.yaml")
    templates = {item["name"]: item for item in template["spec"]["templates"]}

    assert template["metadata"]["name"] == "bst-commit-poller"
    assert {"poll-dakota", "check-sha", "update-sha"} <= templates.keys()
    parameters = {
        item["name"]: item.get("value")
        for item in template["spec"]["arguments"]["parameters"]
    }
    assert parameters["force"] == "false"

    for name, schedule, entrypoint in (
        ("dakota", "2-59/5 * * * *", "poll-dakota"),
    ):
        cron = load(f"manifests/{name}-commit-poller.yaml")
        assert cron["spec"]["suspend"] is True
        assert cron["spec"]["schedules"] == [schedule]
        assert cron["spec"]["concurrencyPolicy"] == "Forbid"
        assert cron["spec"]["workflowSpec"]["entrypoint"] == entrypoint
        assert cron["spec"]["workflowSpec"]["workflowTemplateRef"]["name"] == "bst-commit-poller"
        assert cron["spec"]["workflowMetadata"]["labels"]["bluefin.io/bst-workload"] == "true"
        arguments = {
            item["name"]: item["value"]
            for item in cron["spec"]["workflowSpec"]["arguments"]["parameters"]
        }
        assert arguments["force"] == "false"


def test_bst_poller_persists_only_successful_non_stale_builds():
    template = load("argo/workflow-templates/bst-commit-poller.yaml")
    templates = {item["name"]: item for item in template["spec"]["templates"]}

    for entrypoint in ("poll-dakota",):
        tasks = {
            item["name"]: item
            for item in templates[entrypoint]["dag"]["tasks"]
        }
        assert tasks["update-sha"]["depends"] == "run-build.Succeeded"
        update_arguments = {
            item["name"]: item["value"]
            for item in tasks["update-sha"]["arguments"]["parameters"]
        }
        assert update_arguments["expected-sha"] == (
            "{{tasks.check-sha.outputs.parameters.stored-sha}}"
        )

    update_source = templates["update-sha"]["script"]["source"]
    assert "State changed since admission" in update_source
    assert update_source.index('if [[ "${STORED}" != "${EXPECTED}" ]]') < (
        update_source.index("kubectl patch configmap")
    )

    justfile = (ROOT / "Justfile").read_text(encoding="utf-8")
    assert "force-dakota-poll:" in justfile
    assert "--from cronworkflow/dakota-commit-poller" in justfile
    assert "-p force=true" in justfile
