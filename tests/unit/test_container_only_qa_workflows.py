from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PIPELINE = ROOT / "argo/workflow-templates/bluefin-qa-pipeline.yaml"
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
    content = PIPELINE.read_text(encoding="utf-8")
    assert "name: run-container-tests" in content
    assert all(token not in content for token in FORBIDDEN)


def test_image_poller_has_no_containerdisk_parameter_or_reference():
    content = (ROOT / "argo/workflow-templates/image-poller.yaml").read_text(
        encoding="utf-8"
    )
    assert "containerdisk-tag" not in content
    assert "build-containerdisk" not in content


def test_bluefin_container_only_pipeline_preserves_all_suite_lanes():
    content = PIPELINE.read_text(encoding="utf-8")
    assert "withItems: [smoke, common, developer, software, system]" in content
    assert 'value: "{{item}}"' in content


def test_bluefin_pipeline_validates_raw_suites_against_exact_allow_list():
    content = PIPELINE.read_text(encoding="utf-8")
    assert "- name: validate-suites" in content
    assert '- name: suites\n            value: "{{workflow.parameters.suites}}"' in content
    assert '- name: SUITES\n        value: "{{inputs.parameters.suites}}"' in content
    assert 'IFS=\',\' read -r -a raw_suites <<< "$SUITES"' in content
    assert "{{inputs.parameters.suites}}" not in content.split("source: |", 1)[1]
    assert "case \"${suite}\" in" in content
    assert "smoke|common|developer|software|system) ;;" in content


def test_bluefin_test_lane_depends_on_suite_validation():
    content = PIPELINE.read_text(encoding="utf-8")
    assert 'depends: "validate-suites.Succeeded"' in content
    assert content.index("- name: validate-suites") < content.index("- name: test-lane")


def test_run_container_tests_explicitly_allows_system_suite():
    content = (ROOT / "argo/workflow-templates/run-container-tests.yaml").read_text(
        encoding="utf-8"
    )
    assert "smoke|common|developer|software|system" in content
    assert "Unsupported container suite: ${SUITE}" in content
