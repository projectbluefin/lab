from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FORBIDDEN = (
    "assert-cd",
    "containerdisk-tag",
    "provision-containerdisk-vm",
    "run-gnome-tests",
    "teardown-vm",
    "qa-vm-fleet",
    "kubectl delete vm",
)


def test_bluefin_image_poll_qa_is_container_only():
    content = (ROOT / "argo/workflow-templates/bluefin-qa-pipeline.yaml").read_text(
        encoding="utf-8"
    )
    assert "name: run-container-tests" in content
    assert all(token not in content for token in FORBIDDEN)
