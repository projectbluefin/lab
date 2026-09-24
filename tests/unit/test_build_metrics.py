import json
from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[2]


def load_documents(path):
    return list(yaml.safe_load_all((ROOT / path).read_text(encoding="utf-8")))


def test_zot_metrics_are_enabled_on_both_registries():
    for path, workload_kind in (
        ("manifests/zot-cache.yaml", "DaemonSet"),
        ("manifests/zot-writable.yaml", "Deployment"),
    ):
        documents = load_documents(path)
        config_map = next(doc for doc in documents if doc["kind"] == "ConfigMap")
        workload = next(doc for doc in documents if doc["kind"] == workload_kind)
        config = json.loads(config_map["data"]["config.json"])

        assert config["extensions"]["metrics"] == {
            "enable": True,
            "prometheus": {"path": "/metrics"},
        }
        config_version = workload["spec"]["template"]["metadata"]["annotations"].get(
            "lab.projectbluefin.io/config-version"
        )
        assert isinstance(config_version, str) and config_version.strip()


def test_safe_build_workflows_emit_only_low_cardinality_metrics():
    pipelines = {
        "bluefin-server-build-pipeline.yaml": "bluefin-server",
        "bst-qa-pipeline.yaml": "bst-qa",
        "dakota-build-pipeline.yaml": "dakota",
    }

    for filename, pipeline in pipelines.items():
        workflow = yaml.safe_load(
            (ROOT / "argo/workflow-templates" / filename).read_text(encoding="utf-8")
        )
        metrics = workflow["spec"]["metrics"]["prometheus"]

        assert {metric["name"] for metric in metrics} == {
            "lab_build_workflow_completed_total",
            "lab_build_workflow_duration_seconds",
        }
        for metric in metrics:
            assert metric["labels"] == [
                {"key": "pipeline", "value": pipeline},
                {"key": "status", "value": "{{workflow.status}}"},
            ]
