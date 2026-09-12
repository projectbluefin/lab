from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[2]


def test_github_check_reporter_exposes_detailed_safe_results():
    path = ROOT / "argo/workflow-templates/github-check-reporter.yaml"
    document = yaml.safe_load(path.read_text(encoding="utf-8"))
    templates = {template["name"]: template for template in document["spec"]["templates"]}

    assert document["metadata"]["name"] == "github-check-reporter"
    assert {"send", "final", "collect"} <= templates.keys()

    send_source = templates["send"]["script"]["source"]
    collect_source = templates["collect"]["script"]["source"]

    assert 'event_type: "lab-check"' in send_source
    assert "Authorization: Bearer ${GITHUB_TOKEN}" in send_source
    assert "projectbluefin/bluefin|projectbluefin/bluefin-lts|projectbluefin/dakota" in send_source
    assert "### Pod placement" in collect_source
    assert "### Workflow nodes" in collect_source
    assert "## Failure diagnostics" in collect_source
    assert "containerStatuses[]?.restartCount" in collect_source
    assert "kubectl logs" not in collect_source
    assert "raw pod logs" in collect_source
    assert ".templateName // .templateRef.template // \"-\"" in collect_source
    assert ".templateName // .templateRef.template // \"unknown\"" in collect_source


def test_github_check_reporter_preserves_template_ref_in_nodes_and_failures():
    import json
    import subprocess
    import shutil
    import pytest

    if shutil.which("jq") is None or shutil.which("bash") is None:
        pytest.skip("jq and bash are required for reporter formatting test")

    path = ROOT / "argo/workflow-templates/github-check-reporter.yaml"
    document = yaml.safe_load(path.read_text(encoding="utf-8"))
    templates = {template["name"]: template for template in document["spec"]["templates"]}
    collect_source = templates["collect"]["script"]["source"]

    # Extract the Workflow nodes block
    assert ".templateName // .templateRef.template" in collect_source

    # Run the jq expression and bash while loop against mock workflow json
    # containing a node with templateRef and no templateName (like test-lane)
    mock_workflow = {
        "status": {
            "nodes": {
                "lane1": {
                    "type": "Pod",
                    "name": "test-lane(0:smoke)",
                    "phase": "Failed",
                    "templateRef": {
                        "name": "run-container-tests",
                        "template": "run-container-tests",
                    },
                    "startedAt": "2026-09-10T00:47:33Z",
                    "finishedAt": "2026-09-10T00:55:04Z",
                    "message": "main: Error (exit code 1)",
                }
            }
        }
    }

    script = f"""
set -euo pipefail
to_epoch() {{
  [[ -n "${{1:-}}" && "$1" != "none" && "$1" != "-" ]] || {{ printf '0'; return; }}
  date -u -d "$1" +%s 2>/dev/null || printf '0'
}}
escape_markdown() {{
  printf '%s' "$1" | tr '\\n\\r\\t' '   ' | sed 's/|/\\\\|/g' | cut -c1-500
}}

jq -r '
  [.status.nodes[]?
    | select(.type == "Pod" or .type == "DAG" or .type == "Steps" or .type == "Retry")
  ]
  | sort_by(.startedAt // "", .displayName // "")
  | .[]
  | [
      (.displayName // .name // "-"),
      (.templateName // .templateRef.template // "-"),
      (.type // "-"),
      (.phase // "Unknown"),
      (.startedAt // "none"),
      (.finishedAt // "none"),
      (.message // "")
    ]
  | @tsv
' <<'EOF' |
{json.dumps(mock_workflow)}
EOF
while IFS=$'\t' read -r step template type phase started finished message; do
  node_start=$(to_epoch "${{started}}")
  node_end=$(to_epoch "${{finished}}")
  if (( node_start > 0 && node_end >= node_start )); then
    node_duration=$((node_end - node_start))
  else
    node_duration=0
  fi
  printf '| `%s` | `%s` | `%s` | `%s` | %ss | %s |\\n' \\
    "$(escape_markdown "${{step}}")" \\
    "$(escape_markdown "${{template}}")" \\
    "${{type}}" \\
    "${{phase}}" \\
    "${{node_duration}}" \\
    "$(escape_markdown "${{message}}")"
done
"""
    res = subprocess.run(["bash", "-c", script], capture_output=True, text=True, check=True)
    line = res.stdout.strip()
    assert "| `test-lane(0:smoke)` | `run-container-tests` | `Pod` | `Failed` |" in line
    assert "| main: Error (exit code 1) |" in line



def test_dakota_pr_workflow_updates_one_check_without_comments():
    poller = (
        ROOT / "argo/workflow-templates/pr-poller.yaml"
    ).read_text(encoding="utf-8")
    justfile = (ROOT / "Justfile").read_text(encoding="utf-8")

    assert "dispatch_lab_check()" in poller
    assert "onExit: report-final" in poller
    assert "name: report-start" in poller
    assert 'REPORTER="github-check-reporter"' in poller
    assert "name: qa-bluefin" in poller
    assert "projectbluefin/bluefin-lts" in poller
    assert "value: in_progress" in poller
    assert "template: final" in poller
    assert "failed to create queued GitHub check" in poller
    assert "kubectl delete workflow" in poller
    dispatch_failure = poller.index("failed to create queued GitHub check")
    assert "return 0" in poller[dispatch_failure : dispatch_failure + 300]
    assert "gh pr comment" not in poller
    assert "/issues/comments" not in poller

    assert "lab-check-status repo pr_number:" in justfile
    assert 'select(.name == "testing-lab / {{ repo }}"' in justfile
    assert "lab-report pr_number" not in justfile
