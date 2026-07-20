import sys
from pathlib import Path

# Add scripts directory to path to import publish_test_results
scripts_path = Path(__file__).parent.parent.parent / "scripts"
sys.path.insert(0, str(scripts_path))

from publish_test_results import parse_results_and_build_update  # noqa: E402

def test_parse_results_and_build_update():
    # Sample behave JSON
    data = [
        {
            "keyword": "Feature",
            "name": "Feature 1",
            "elements": [
                {
                    "type": "scenario",
                    "status": "passed",
                    "name": "Scenario 1 (passed)",
                    "steps": [
                        {
                            "name": "Given a passed step",
                            "result": {
                                "status": "passed",
                                "duration": 1.25
                            }
                        },
                        {
                            "name": "Then another passed step",
                            "result": {
                                "status": "passed",
                                "duration": 2.5
                            }
                        }
                    ]
                },
                {
                    "type": "scenario",
                    "status": "failed",
                    "name": "Scenario 2 (failed)",
                    "steps": [
                        {
                            "name": "Given a passed step in fail",
                            "result": {
                                "status": "passed",
                                "duration": 0.5
                            }
                        },
                        {
                            "name": "When a step fails",
                            "result": {
                                "status": "failed",
                                "duration": 5.12,
                                "error_message": "  AssertionError: something went wrong\n"
                            }
                        }
                    ]
                }
            ]
        }
    ]

    existing_data = {
        "history": [
            {
                "run_date": "2026-07-09T12:00:00Z",
                "workflow_name": "previous-workflow",
                "status": "passed",
                "scenarios": 2,
                "failed": 0
                # duration_seconds is missing here! Should be defaulted to 0.0
            }
        ]
    }

    current_utc = "2026-07-10T01:00:00Z"
    workflow_name = "test-workflow"
    img_slug = "bluefin-testing"
    suite = "smoke"

    updated = parse_results_and_build_update(
        data=data,
        existing_data=existing_data,
        current_utc=current_utc,
        workflow_name=workflow_name,
        img_slug=img_slug,
        suite=suite
    )

    # Assertions on main structure
    assert updated["variant"] == "bluefin-testing"
    assert updated["suite"] == "smoke"
    assert updated["last_run"] == current_utc
    assert updated["workflow_name"] == workflow_name
    assert updated["status"] == "failed"
    assert updated["scenarios"] == 2
    assert updated["failed"] == 1
    assert updated["failed_scenarios"] == ["Scenario 2 (failed)"]
    
    # Assertions on duration_seconds (total duration = 1.25 + 2.5 + 0.5 + 5.12 = 9.37)
    assert updated["duration_seconds"] == 9.37

    # Assertions on failed_scenarios_detailed
    assert len(updated["failed_scenarios_detailed"]) == 1
    failed_details = updated["failed_scenarios_detailed"][0]
    assert failed_details["scenario_name"] == "Scenario 2 (failed)"
    assert failed_details["duration_seconds"] == 5.62 # 0.5 + 5.12
    assert failed_details["failing_step"] == "When a step fails"
    assert failed_details["error_message"] == "AssertionError: something went wrong"

    # Assertions on history
    assert len(updated["history"]) == 2
    # The new entry is at index 0
    new_entry = updated["history"][0]
    assert new_entry["run_date"] == current_utc
    assert new_entry["workflow_name"] == workflow_name
    assert new_entry["status"] == "failed"
    assert new_entry["scenarios"] == 2
    assert new_entry["failed"] == 1
    assert new_entry["duration_seconds"] == 9.37

    # The existing entry is at index 1 and should have defaulted duration_seconds to 0.0
    old_entry = updated["history"][1]
    assert old_entry["workflow_name"] == "previous-workflow"
    assert old_entry["duration_seconds"] == 0.0

def test_parse_real_sample_results():
    import json
    sample_path = Path(__file__).parent.parent.parent / "docs" / "screenshots" / "results" / "results.json"
    assert sample_path.exists()
    
    with open(sample_path, "r") as f:
        data = json.load(f)
        
    updated = parse_results_and_build_update(
        data=data,
        existing_data=None,
        current_utc="2026-07-10T01:00:00Z",
        workflow_name="test-workflow",
        img_slug="bluefin-testing",
        suite="smoke"
    )
    
    assert updated["variant"] == "bluefin-testing"
    assert updated["scenarios"] > 0
    assert updated["failed"] > 0
    assert updated["duration_seconds"] > 0.0
    assert len(updated["failed_scenarios"]) == updated["failed"]
    assert len(updated["failed_scenarios_detailed"]) == updated["failed"]
    
    # Assert that detailed elements have the expected schema
    for item in updated["failed_scenarios_detailed"]:
        assert isinstance(item["scenario_name"], str)
        assert isinstance(item["duration_seconds"], float)
        assert isinstance(item["failing_step"], str)
        assert isinstance(item["error_message"], str)
        assert len(item["failing_step"]) > 0
        assert len(item["error_message"]) > 0

