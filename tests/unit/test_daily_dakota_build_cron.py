from pathlib import Path
import yaml

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "manifests/daily-dakota-build.yaml"
POLLER = ROOT / "argo/workflow-templates/bst-commit-poller.yaml"
PIPELINE = ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml"


def test_daily_dakota_build_manifest():
    assert MANIFEST.is_file(), f"Expected manifest file at {MANIFEST}"
    doc = yaml.safe_load(MANIFEST.read_text(encoding="utf-8"))

    assert doc["apiVersion"] == "argoproj.io/v1alpha1"
    assert doc["kind"] == "CronWorkflow"
    assert doc["metadata"]["name"] == "daily-dakota-build"
    assert doc["metadata"]["namespace"] == "argo"

    spec = doc["spec"]
    assert spec["suspend"] is True
    assert spec["schedules"] == ["30 0 * * *"]
    assert spec["timezone"] == "UTC"
    assert spec["concurrencyPolicy"] == "Forbid"
    assert spec["startingDeadlineSeconds"] == 300

    labels = spec["workflowMetadata"]["labels"]
    assert labels["bluefin.io/bst-workload"] == "true"
    assert labels["bluefin.io/bst-source"] == "dakota"
    assert labels["bluefin.io/trigger"] == "daily-build"

    wf_spec = spec["workflowSpec"]
    assert wf_spec["serviceAccountName"] == "argo"
    assert wf_spec["activeDeadlineSeconds"] == 86400
    assert wf_spec["entrypoint"] == "poll-dakota"
    assert wf_spec["workflowTemplateRef"]["name"] == "bst-commit-poller"

    params = {item["name"]: item["value"] for item in wf_spec["arguments"]["parameters"]}
    assert params["repo"] == "projectbluefin/dakota"
    assert params["branch"] == "testing"
    assert params["state-key"] == "sha-dakota-testing"
    assert params["force"] == "true"
    assert params["registry"] == "192.168.1.102:30500"


def test_bst_commit_poller_nested_template_parameter_contract():
    assert POLLER.is_file(), f"Expected template file at {POLLER}"
    doc = yaml.safe_load(POLLER.read_text(encoding="utf-8"))

    templates = {t["name"]: t for t in doc["spec"]["templates"]}
    poll_dakota = templates["poll-dakota"]
    tasks = {task["name"]: task for task in poll_dakota["dag"]["tasks"]}

    # Verify run-build task exists, targets dakota-build-pipeline build template
    run_build = tasks["run-build"]
    assert run_build["templateRef"]["name"] == "dakota-build-pipeline"
    assert run_build["templateRef"]["template"] == "build"

    # Verify explicitly bound parameters (preventing unpopulated caller workflow.parameters defaults)
    build_params = {p["name"]: p["value"] for p in run_build["arguments"]["parameters"]}
    assert build_params["ref"] == "{{workflow.parameters.branch}}"
    assert build_params["commit-sha"] == "{{tasks.check-sha.outputs.parameters.sha}}"
    assert build_params["repo"] == "https://github.com/{{workflow.parameters.repo}}.git"
    assert build_params["registry"] == "{{workflow.parameters.registry}}"
    assert build_params["build-mode"] == "re"
    assert build_params["image-tag"] == "testing"
    assert build_params["lock-key"] == "bst-build"

    # Verify update-sha task persists
    update_sha = tasks["update-sha"]
    assert update_sha["depends"] == "run-build.Succeeded"
