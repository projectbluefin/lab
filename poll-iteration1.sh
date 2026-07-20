#!/usr/bin/env bash
set -euo pipefail
export KUBECONFIG=~/.kube/bluespeed.yaml
REPORT=/var/home/jorge/src/lab/dakota-iteration1-report.md
WORKFLOW=dakota-build-pipeline-z7m77

poll_status() {
  argo get -n argo "$WORKFLOW" 2>/dev/null || true
}

get_phase() {
  kubectl get workflow "$WORKFLOW" -n argo -o jsonpath='{.status.phase}' 2>/dev/null || echo "Unknown"
}

get_startedAt() {
  kubectl get workflow "$WORKFLOW" -n argo -o jsonpath='{.status.startedAt}' 2>/dev/null || echo ""
}

get_finishedAt() {
  kubectl get workflow "$WORKFLOW" -n argo -o jsonpath='{.status.finishedAt}' 2>/dev/null || echo ""
}

append_table_row() {
  local ts phase duration progress notes
  ts="$1"
  phase="$2"
  duration="$3"
  progress="$4"
  notes="$5"
  printf '| %s | %s | %s | %s | %s |\n' "$ts" "$phase" "$duration" "$progress" "$notes" >> "$REPORT"
}

while true; do
  sleep 300
  now=$(date -Iseconds)
  phase=$(get_phase)
  duration=$(kubectl get workflow "$WORKFLOW" -n argo -o jsonpath='{.status.duration}' 2>/dev/null || echo "")
  progress=$(kubectl get workflow "$WORKFLOW" -n argo -o jsonpath='{.status.progress}' 2>/dev/null || echo "")
  current_step=$(argo get -n argo "$WORKFLOW" 2>/dev/null | grep -E '^[[:space:]]*●' | head -1 | sed 's/[[:space:]]\+/ /g' || echo '—')
  append_table_row "$now" "$phase" "$duration" "$progress" "$current_step"

  if [[ "$phase" == "Succeeded" || "$phase" == "Failed" || "$phase" == "Error" ]]; then
    echo "Terminal phase: $phase"
    break
  fi
done

echo '' >> "$REPORT"
echo '## Final status' >> "$REPORT"
echo '' >> "$REPORT"
printf 'Terminal phase: %s\n' "$(get_phase)" >> "$REPORT"
printf 'Started at: %s\n' "$(get_startedAt)" >> "$REPORT"
printf 'Finished at: %s\n' "$(get_finishedAt)" >> "$REPORT"
echo '```' >> "$REPORT"
poll_status >> "$REPORT"
echo '```' >> "$REPORT"
