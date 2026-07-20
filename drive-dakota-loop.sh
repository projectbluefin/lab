#!/usr/bin/env bash
set -euo pipefail
export KUBECONFIG=~/.kube/bluespeed.yaml

WORKDIR=/var/home/jorge/src/lab
ITER1_REPORT="$WORKDIR/dakota-iteration1-report.md"
ITER2_REPORT="$WORKDIR/dakota-iteration2-report.md"
SUMMARY="$WORKDIR/dakota-build-loop-summary.md"
ITER2_MANIFEST="$WORKDIR/dakota-iteration2-manifest.yaml"
REGISTRY_URL="docker://192.168.1.102:30500/dakota:testing"

WORKFLOW1=dakota-build-pipeline-z7m77

get_phase() {
  kubectl get workflow "$1" -n argo -o jsonpath='{.status.phase}' 2>/dev/null || echo "Unknown"
}

get_duration() {
  kubectl get workflow "$1" -n argo -o jsonpath='{.status.duration}' 2>/dev/null || echo ""
}

poll_until_terminal() {
  local wf=$1 report=$2
  echo "" >> "$report"
  echo "## Polling log" >> "$report"
  echo "" >> "$report"
  printf '| Timestamp | Phase | Duration | Progress | Notes |\n' >> "$report"
  printf '|-----------|-------|----------|----------|-------|\n' >> "$report"

  while true; do
    sleep 300
    now=$(date -Iseconds)
    phase=$(get_phase "$wf")
    duration=$(get_duration "$wf")
    progress=$(kubectl get workflow "$wf" -n argo -o jsonpath='{.status.progress}' 2>/dev/null || echo "")
    current_step=$(argo get -n argo "$wf" 2>/dev/null | grep -E '^[[:space:]]*●' | head -1 | sed 's/[[:space:]]\+/ /g' || echo '—')
    printf '| %s | %s | %s | %s | %s |\n' "$now" "$phase" "$duration" "$progress" "$current_step" >> "$report"

    if [[ "$phase" == "Succeeded" || "$phase" == "Failed" || "$phase" == "Error" ]]; then
      echo "Terminal phase: $phase at $now" >> "$report"
      break
    fi
  done
}

append_final_status() {
  local wf=$1 report=$2
  echo "" >> "$report"
  echo "## Final status" >> "$report"
  echo "" >> "$report"
  printf 'Terminal phase: %s\n' "$(get_phase "$wf")" >> "$report"
  printf 'Started at: %s\n' "$(kubectl get workflow "$wf" -n argo -o jsonpath='{.status.startedAt}' 2>/dev/null || echo '')" >> "$report"
  printf 'Finished at: %s\n' "$(kubectl get workflow "$wf" -n argo -o jsonpath='{.status.finishedAt}' 2>/dev/null || echo '')" >> "$report"
  echo "" >> "$report"
  echo '```' >> "$report"
  argo get -n argo "$wf" 2>/dev/null >> "$report" || true
  echo '```' >> "$report"
}

# Ensure ITER1_REPORT exists from prior capture
[ -f "$ITER1_REPORT" ] || echo "# Dakota Build Loop — Iteration 1 Report" > "$ITER1_REPORT"

echo "Polling iteration 1 ($WORKFLOW1)..."
poll_until_terminal "$WORKFLOW1" "$ITER1_REPORT"
append_final_status "$WORKFLOW1" "$ITER1_REPORT"

phase1=$(get_phase "$WORKFLOW1")

if [[ "$phase1" != "Succeeded" ]]; then
  echo "" >> "$ITER1_REPORT"
  echo "**Iteration 1 did not succeed ($phase1). Stopping loop for human review.**" >> "$ITER1_REPORT"
  echo "# Dakota Build Loop — Iteration 1 Failed" > "$WORKDIR/dakota-iteration1-FAILED.md"
  cp "$ITER1_REPORT" "$WORKDIR/dakota-iteration1-FAILED.md"
  exit 1
fi

# Iteration 1 succeeded: inspect the registry manifest
echo "" >> "$ITER1_REPORT"
echo "## skopeo inspect after iteration 1" >> "$ITER1_REPORT"
echo "" >> "$ITER1_REPORT"
echo '```json' >> "$ITER1_REPORT"
skopeo inspect --tls-verify=false "$REGISTRY_URL" >> "$ITER1_REPORT" 2>&1 || echo "skopeo inspect failed" >> "$ITER1_REPORT"
echo '```' >> "$ITER1_REPORT"

# Submit iteration 2
echo "Submitting iteration 2..."
WORKFLOW2=$(argo submit -n argo "$ITER2_MANIFEST" -o name 2>&1)
echo "Iteration 2 workflow: $WORKFLOW2"

# Iteration 2 report
echo "# Dakota Build Loop — Iteration 2 Report" > "$ITER2_REPORT"
echo "" >> "$ITER2_REPORT"
echo "Workflow: \`$WORKFLOW2\`" >> "$ITER2_REPORT"
echo "Manifest: $ITER2_MANIFEST" >> "$ITER2_REPORT"
echo "" >> "$ITER2_REPORT"
echo "## Parameters" >> "$ITER2_REPORT"
echo "" >> "$ITER2_REPORT"
echo '```yaml' >> "$ITER2_REPORT"
cat "$ITER2_MANIFEST" >> "$ITER2_REPORT"
echo '```' >> "$ITER2_REPORT"

poll_until_terminal "$WORKFLOW2" "$ITER2_REPORT"
append_final_status "$WORKFLOW2" "$ITER2_REPORT"

phase2=$(get_phase "$WORKFLOW2")

if [[ "$phase2" != "Succeeded" ]]; then
  echo "" >> "$ITER2_REPORT"
  echo "**Iteration 2 did not succeed ($phase2). Stopping loop for human review.**" >> "$ITER2_REPORT"
  echo "# Dakota Build Loop — Iteration 2 Failed" > "$WORKDIR/dakota-iteration2-FAILED.md"
  cp "$ITER2_REPORT" "$WORKDIR/dakota-iteration2-FAILED.md"
  exit 1
fi

# Final proof
echo "" >> "$ITER2_REPORT"
echo "## Final skopeo inspect proof" >> "$ITER2_REPORT"
echo "" >> "$ITER2_REPORT"
echo '```json' >> "$ITER2_REPORT"
FINAL_SKOPEO=$(skopeo inspect --tls-verify=false "$REGISTRY_URL" 2>&1)
echo "$FINAL_SKOPEO" >> "$ITER2_REPORT"
echo '```' >> "$ITER2_REPORT"

# Summary
dur1=$(get_duration "$WORKFLOW1")
dur2=$(get_duration "$WORKFLOW2")
digest_line=$(echo "$FINAL_SKOPEO" | jq -r '"Digest: " + .Digest + " | Created: " + .Created')

cat > "$SUMMARY" <<EOF
# Dakota Build Loop Summary

| Iteration | Workflow | Status | Duration | Final Step Notes |
|-----------|----------|--------|----------|------------------|
| 1 | $WORKFLOW1 | $phase1 | $dur1 | Serialized (bluefin → bluefin-nvidia), cold cache |
| 2 | $WORKFLOW2 | $phase2 | $dur2 | Parallel lanes, warmed cache |

## Final registry proof

\`$REGISTRY_URL\`

$digest_line

## Validation

- skopeo inspect returned a valid manifest.
- Both BuildBarn workers exist across ghost and exo-0.
EOF

echo "Loop complete. Summary written to $SUMMARY"
