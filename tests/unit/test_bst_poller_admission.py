from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[2]


def load(path):
    return yaml.safe_load((ROOT / path).read_text(encoding="utf-8"))


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

    source = templates["check-sha"]["script"]["source"]
    assert "bluefin.io/bst-workload=true" in source
    assert "ACTIVE >= 2" in source
    assert '"${FORCE}" == "false" && "${REMOTE}" == "${STORED}"' in source
    assert "Forced rebuild requested; BST queue has capacity" in source
    assert "kubectl patch configmap" not in source

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
