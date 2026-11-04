# Hive contributor on the lab (projectbluefin/lab docs/skills/hive-contribute/SKILL.md).
# Works from any directory; repo justfiles with `set fallback` also reach it.

export KUBECONFIG := env("KUBECONFIG", home_directory() / ".kube/bluespeed.yaml")

alias contribute := contribute-on

# Start models and Hive contributor with local Ornith advisor (default), or advisor="luna"
contribute-on advisor="local": contribute-learning-save
    #!/usr/bin/env bash
    set -euo pipefail
    reg="${HIVE_CONTRIBUTE_REGISTRATION:-$HOME/.config/hive/contributor.bluefin.env}"
    profile="$HOME/.omp/profiles/ghost/agent"
    omp="${HIVE_CONTRIBUTE_OMP_CONFIG:-$profile/config.yml}"
    tok="$(mktemp)"; trap 'rm -f "$tok"' EXIT
    printf %s "$(gh auth token)" > "$tok"
    # Refresh the Hive registration token with the gh identity (upstream's contribute-move call),
    # so a stale or reissued-elsewhere token never needs a manual repair. The lab is the one contributor.
    hub="$(sed -nE 's/^HIVE_HUB=//p' "$reg" | head -1)"; http="https://${hub#wss://}"; http="${http%/contribute}"
    new="$(curl -fsS --max-time 20 -X POST "${http}/api/contribute/reissue-token" -H "Authorization: Bearer $(cat "$tok")" | jq -er .registration_token)"
    sed -i "s|^HIVE_REGISTRATION_TOKEN=.*|HIVE_REGISTRATION_TOKEN=${new}|" "$reg"
    # Ship standalone contributor config with Ornith worker and Ornith advisor
    cfg="$(mktemp)"; bundle="$(mktemp)"; context="$(mktemp -d)"
    trap 'rm -f "$tok" "$cfg" "$bundle"; rm -rf "$context"' EXIT
    mkdir -p "$profile/managed-skills"
    kubectl -n contribute get configmap contribute-omp -o json > "$context/config.json"
    jq -er '.data["mcp.json"]' "$context/config.json" > "$context/mcp.json"
    { cat "$profile/APPEND_SYSTEM.md"; printf '\n'; jq -er '.data["tooling.md"]' "$context/config.json"; } > "$context/APPEND_SYSTEM.md"
    tar -czhf "$bundle" -C "$profile" skills managed-skills -C "$context" APPEND_SYSTEM.md mcp.json
    advisor_mode="{{advisor}}"
    if [ "$advisor_mode" = "luna" ]; then
      cat > "$cfg" <<'EOF'
    modelRoles:
      default: lab-worker/ornith
      smol: lab-worker/ornith
      advisor: github-copilot/gpt-6-luna:max
    advisor:
      enabled: true
      reviewMode: turn
      syncBacklog: "off"
    async:
      enabled: false
    task:
      batch: false
    autolearn:
      enabled: false
    EOF
    else
      cat > "$cfg" <<'EOF'
    modelRoles:
      default: lab-worker/ornith
      smol: lab-worker/ornith
      advisor: lab-advisor/ornith
    advisor:
      enabled: true
      reviewMode: turn
      syncBacklog: "off"
    async:
      enabled: false
    task:
      batch: false
    autolearn:
      enabled: false
    EOF
    fi
    kubectl -n contribute create secret generic contribute \
      --from-file=GH_TOKEN="$tok" --from-file=contributor.env="$reg" --from-file=config.yml="$cfg" \
      --from-file=ghost-context.tgz="$bundle" \
      --dry-run=client -o yaml | kubectl apply -f - >/dev/null
    kubectl -n llm-d scale deploy/llm-d-exo-0 deploy/llm-d-ghost --replicas=1 >/dev/null
    echo "Warming Ornith on both nodes before starting contributor..."
    kubectl -n llm-d rollout status deploy/llm-d-exo-0 --timeout=60m
    kubectl -n llm-d rollout status deploy/llm-d-ghost --timeout=60m
    # Warm-up gate: load Ornith on each node (exo-0 worker, ghost advisor).
    warm() {
      kubectl -n llm-d exec "deploy/llm-d-$1" -- curl -fsS --max-time 3600 localhost:8000/v1/chat/completions \
        -H 'Content-Type: application/json' \
        -d '{"model":"ornith","messages":[{"role":"user","content":"ok"}],"max_tokens":1}' >/dev/null
    }
    warm exo-0 & a=$!; warm ghost & b=$!
    wait "$a"; wait "$b"
    kubectl -n contribute scale sts/contribute --replicas=1 >/dev/null
    kubectl -n contribute rollout restart sts/contribute >/dev/null
    kubectl -n contribute rollout status sts/contribute --timeout=10m
    until kubectl -n contribute exec contribute-0 -- tmux has-session -t contributor 2>/dev/null; do sleep 2; done
    just contribute-attach

# Attach to the contributor (contribute-0 on exo-0; ghost runs advisor).
# Detach: C-b d; contributor keeps running.
contribute-attach:
    kubectl -n contribute exec -it contribute-0 -- env TERM=xterm-256color COLORTERM=truecolor tmux attach -t contributor

# Stop both contributors and free both GPUs.
contribute-off: contribute-learning-save
    kubectl -n contribute scale sts/contribute --replicas=0
    kubectl -n llm-d scale deploy/llm-d-exo-0 deploy/llm-d-ghost --replicas=0

# Read-only node resources, pod state and Ornith inference snapshots (no model requests).
contribute-status:
    #!/usr/bin/env bash
    set -uo pipefail
    k() { timeout --kill-after=2s 15s kubectl --request-timeout=10s "$@"; }
    echo 'Node CPU/memory (not GPU utilization):'
    k top nodes || echo 'Node CPU/memory: unavailable (metrics/API query failed).'
    echo
    echo 'Model servers:'
    k -n llm-d get deploy,pods -o wide || echo 'Model server pod listing: unavailable.'
    echo
    echo 'Contributors:'
    k -n contribute get sts,pods -o wide || echo 'Contributor pod listing: unavailable.'
    echo
    echo 'Inference snapshot (cache = cached/total prompt tokens, not aggregate hit rate):'
    for node in exo-0 ghost; do
      if ! pods="$(k -n llm-d get pods -l "app.kubernetes.io/name=llm-d,app.kubernetes.io/instance=$node" -o json)"; then
        printf '%s: unavailable (pod query failed).\n' "$node"
        continue
      fi
      pod="$(jq -r '[.items[] | select(.metadata.deletionTimestamp == null and .status.phase == "Running")
        | select(any(.status.conditions[]?; .type == "Ready" and .status == "True"))
        | select(any(.status.containerStatuses[]?; .name == "modelserver" and .ready == true and .state.running != null))]
        | sort_by(.metadata.creationTimestamp) | last | .metadata.name // empty' <<< "$pods")"
      if [ -z "$pod" ]; then
        printf '%s: no Running Ready modelserver (stopped or not ready); inference not queried.\n' "$node"
        continue
      fi
      printf '%s (%s): ' "$node" "$pod"
      if ! models="$(k -n llm-d exec "$pod" -c modelserver --pod-running-timeout=5s -- curl -fsS --connect-timeout 2 --max-time 5 http://localhost:8000/v1/models)"; then
        echo 'Ornith load state unavailable (/v1/models query failed).'
        continue
      fi
      if ! state="$(jq -er '(.data | map(select(.id == "ornith")) | first) as $model
        | if $model == null then "not listed" else ($model.status.value // "unknown") end' <<< "$models")"; then
        echo 'Ornith load state unavailable (invalid /v1/models response).'
        continue
      fi
      printf 'ornith=%s\n' "$state"
      if [ "$state" != loaded ]; then
        echo '  slots: not queried (Ornith is not confirmed loaded).'
        continue
      fi
      if ! slots="$(k -n llm-d exec "$pod" -c modelserver --pod-running-timeout=5s -- curl -fsS --connect-timeout 2 --max-time 5 'http://localhost:8000/slots?model=ornith')"; then
        echo '  slots: unavailable (/slots query failed; activity unknown).'
        continue
      fi
      jq -er 'if type != "array" then error("invalid slot response")
        elif length == 0 then "  slots: none (no activity reported)"
        else .[] | "  slot \(.id): \(if .is_processing == true then "busy" elif .is_processing == false then "idle" else "unknown" end) ctx=\(.n_ctx) prompt=\(.n_prompt_tokens) processed=\(.n_prompt_tokens_processed) cached=\(.n_prompt_tokens_cache) (\(if .n_prompt_tokens > 0 then ((100 * .n_prompt_tokens_cache / .n_prompt_tokens) | floor | tostring) + "%" else "n/a" end))"
        end' <<< "$slots" || echo '  slots: unavailable (invalid response; activity unknown).'
    done
    echo 'Busy/idle and prompt counters are sampled slot state, not measured GPU load or throughput.'

# Save lessons before a normal stop/restart; review before capturing with chezmoi.
contribute-learning-save pod="":
    #!/usr/bin/env bash
    set -euo pipefail
    pods="{{pod}}"
    if [ -z "$pods" ]; then
      pods="$(kubectl -n contribute get pods -l app.kubernetes.io/name=contribute -o json | jq -r '.items[] | select(any(.status.containerStatuses[]?; .name == "contribute" and .state.running != null)) | .metadata.name')"
    fi
    [ -n "$pods" ] || exit 0
    dest="$HOME/.omp/profiles/ghost/agent/managed-skills"
    mkdir -p "$dest"
    # ponytail: additive capture from every contributor; prune obsolete lessons during review.
    # Two pods changing the same skill would overwrite each other: stop instead of losing a lesson.
    stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
    for pod in $pods; do
      kubectl -n contribute exec -c contribute "$pod" -- mkdir -p /home/hive/.omp/agent/managed-skills
      mkdir "$stage/$pod"
      kubectl -n contribute cp -c contribute "$pod:/home/hive/.omp/agent/managed-skills/." "$stage/$pod"
    done
    set -- $pods
    if [ $# -gt 1 ]; then
      for s in "$stage/$1"/*; do
        [ -e "$s" ] && [ -e "$stage/$2/${s##*/}" ] && ! diff -rq "$s" "$stage/$2/${s##*/}" >/dev/null &&
          { echo "Skill ${s##*/} differs between $1 and $2; save one with pod=<name>, reconcile, then retry." >&2; exit 1; }
      done
    fi
    for pod in $pods; do cp -r "$stage/$pod/." "$dest"; done
    printf 'Saved learned skills to %s; review before chezmoi add.\n' "$dest"
