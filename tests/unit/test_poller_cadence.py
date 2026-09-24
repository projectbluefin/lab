from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[2]


def load_manifest(name: str) -> dict:
    return yaml.safe_load((ROOT / "manifests" / name).read_text(encoding="utf-8"))


def workflow_parameters(spec: dict) -> dict:
    return {
        parameter["name"]: parameter.get("value")
        for parameter in spec["workflowSpec"]["arguments"]["parameters"]
    }


def test_dakota_image_poller_keeps_freshness_checks_without_qa_fanout():
    spec = load_manifest("image-poll-dakota.yaml")["spec"]

    assert spec["suspend"] is False
    assert spec["schedules"] == ["8/10 * * * *"]
    assert workflow_parameters(spec)["run-qa"] == "false"


def test_nightly_dakota_remains_the_daily_qa_path():
    spec = load_manifest("nightly-dakota.yaml")["spec"]

    assert spec["suspend"] is False
    assert spec["schedules"] == ["0 3 * * *"]
    assert workflow_parameters(spec)["variant"] == "dakota"
