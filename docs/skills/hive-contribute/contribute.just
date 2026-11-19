# Hive contributor on the lab (projectbluefin/lab docs/skills/hive-contribute/SKILL.md).
# Works from any directory; repo justfiles with `set fallback` also reach it.

export KUBECONFIG := env("KUBECONFIG", home_directory() / ".kube/bluespeed.yaml")

# Start or view the pair; detach from tmux without stopping either worker.
contribute $action="on":
    #!/usr/bin/env bash
    set -euo pipefail
    die() { printf '%s\n' "$*" >&2; exit 1; }
    k() { timeout --kill-after=2s 20s kubectl --request-timeout=15s "$@"; }
    contributor_pods() { k -n contribute get pods -l app.kubernetes.io/name=contribute -o json; }
    workload=sts/contribute-reviewed

    start_models() {
      k -n llm-d scale deploy/llm-d-exo-0 deploy/llm-d-ghost --replicas=1 >/dev/null
      echo 'Starting both Ornith servers; checking the native slot and one generated token...'
      warm() {
        local node="$1" deployment="deploy/llm-d-$1"
        timeout --kill-after=2s 310s kubectl -n llm-d rollout status "$deployment" --timeout=5m || return
        k -n llm-d exec "$deployment" -- curl -fsS --max-time 10 'http://localhost:8000/slots?model=ornith' |
          jq -e 'length == 1 and .[0].n_ctx == 262144 and .[0].speculative == true' >/dev/null ||
          die "$node: expected one 262144-token Ornith slot with MTP. Check the synced Lab preset and model rollout."
        timeout --kill-after=2s 40s kubectl -n llm-d exec "$deployment" -- curl -fsS --max-time 30 http://localhost:8000/v1/chat/completions \
          -H 'Content-Type: application/json' \
          -d '{"model":"ornith","messages":[{"role":"user","content":"ok"}],"chat_template_kwargs":{"enable_thinking":false},"max_tokens":1}' |
          jq -e '.usage.completion_tokens == 1' >/dev/null || die "$node: Ornith inference check failed."
        echo "$node: Ornith ready (262144 tokens, MTP active)."
      }
      local a b failed=0
      warm exo-0 & a=$!
      warm ghost & b=$!
      wait "$a" || failed=1
      wait "$b" || failed=1
      [ "$failed" = 0 ] || die 'Ornith readiness failed; no contributor was started.'
    }

    case "$action" in
      status) exec just --justfile {{quote(justfile())}} contribute-status ;;
      on|models|attach|off) ;;
      *) die "Usage: just contribute [on|models|attach|status|off]" ;;
    esac

    # Serialize local launches and pane reconciliation; don't hold the lock while viewing.
    umask 077
    lock_dir="${XDG_RUNTIME_DIR:-$HOME/.cache}"
    mkdir -p "$lock_dir"
    exec 9>"$lock_dir/hive-lab.lock"
    flock -n 9 || die "Another contributor command is running; retry when it finishes."

    if [ "$action" != attach ]; then
      sts="$(k -n contribute get "$workload" -o json)"
      replicas="$(jq -er '.spec.replicas | numbers' <<< "$sts")"
      if [ "$action" = models ]; then
        [ "$replicas" = 0 ] && contributor_pods | jq -e '.items == []' >/dev/null ||
          die 'Refusing to warm models while a Hive contributor is running or stopping.'
        start_models
        exit
      fi
      if [ "$action" = off ]; then
        pods="$(contributor_pods)"
        if [ "$replicas" != 0 ]; then
          # A legacy idle snapshot is not a drain acknowledgment. Check every
          # replica before touching any marker, and keep the lock through scale-down.
          jq -e --argjson count "$replicas" '.items | length == $count and all(.[]; .metadata.deletionTimestamp == null and .status.phase == "Running")' <<< "$pods" >/dev/null ||
            die "Refusing to stop: cannot drain while a contributor replica is missing or starting."
          membership() { jq -c '[.items[] | [.metadata.name, .metadata.uid]] | sort'; }
          original="$(membership <<< "$pods")"
          mapfile -t workers < <(jq -r '.items[].metadata.name' <<< "$pods")
          drain_state() {
            k -n contribute exec "$1" -c relay -- cat /home/hive/.config/hive/relay-status.json |
              jq -er '.operator_drain | select(. == "open" or . == "draining" or . == "drained")'
          }
          for pod in "${workers[@]}"; do
            drain_state "$pod" >/dev/null ||
              die "Refusing to stop: $pod does not report a supported drain capability."
          done
          for pod in "${workers[@]}"; do
            k -n contribute exec "$pod" -c relay -- touch /tmp/hive-operator-drain
          done
          echo 'Waiting for contributor leases to drain...'
          deadline=$((SECONDS + 600))
          while :; do
            current="$(contributor_pods)"
            [ "$(membership <<< "$current")" = "$original" ] ||
              die "Contributor pods changed during drain; rerun just contribute off."
            all_drained=1
            for pod in "${workers[@]}"; do
              [ "$(drain_state "$pod")" = drained ] || all_drained=0
            done
            [ "$all_drained" = 0 ] || break
            [ "$SECONDS" -lt "$deadline" ] ||
              die "Drain still in progress; existing leases are untouched. Rerun just contribute off once they finish."
            sleep 2
          done
          just --justfile {{quote(justfile())}} contribute-learning-save
          current="$(contributor_pods)"
          [ "$(membership <<< "$current")" = "$original" ] ||
            die "Contributor pods changed while saving skills; rerun just contribute off."
          k -n contribute scale "$workload" --current-replicas="$replicas" --replicas=0
          deadline=$((SECONDS + 120))
          while :; do
            pods="$(contributor_pods)"
            if jq -e '.items == []' <<< "$pods" >/dev/null; then break; fi
            [ "$SECONDS" -lt "$deadline" ] || die "Contributor pods are still stopping; GPUs left on."
            sleep 2
          done
        fi
        jq -e '.items == []' <<< "$pods" >/dev/null ||
          die "Refusing to stop GPUs: contributor pods are still present after drain."
        k -n llm-d scale deploy/llm-d-exo-0 deploy/llm-d-ghost --replicas=0
        exit
      fi
      jq -e '.spec.template.metadata.annotations["bluefin.io/contributor-mode"] == "dual-review-v1"' <<< "$sts" >/dev/null ||
        die "Refusing launch: the live StatefulSet lacks the verified dedicated reviewer runtime (dual-review-v1). Use 'just contribute models' for models only."
      case "$replicas" in
        0) ;;
        2) action=attach ;;
        *) die "Refusing launch: unexpected desired replica count ($replicas); no in-place takeover." ;;
      esac
    fi

    command -v tmux >/dev/null || die "Install tmux on the workstation first."
    if [ "$action" = on ]; then
      pods="$(contributor_pods)"
      jq -e '.items == []' <<< "$pods" >/dev/null ||
        die "Refusing launch: old contributor pods are still present. Wait for them to exit."
      reg="${HIVE_CONTRIBUTE_REGISTRATION:-$HOME/.config/hive/contributor.bluefin.env}"
      profile="$HOME/.omp/profiles/ghost/agent"
      context="$(mktemp -d)"
      trap 'rm -rf "$context"' EXIT
      mkdir -p "$profile/managed-skills" "$context/hooks/pre"
      k -n contribute get configmap contribute-omp -o json > "$context/config.json"
      jq -er '.data["mcp.json"] | strings | select(length > 0)' "$context/config.json" > "$context/mcp.json"
      jq -e 'type == "object"' "$context/mcp.json" >/dev/null
      { cat "$profile/APPEND_SYSTEM.md"; printf '\n'; jq -er '.data["tooling.md"] | strings | select(length > 0)' "$context/config.json"; } > "$context/APPEND_SYSTEM.md"
      for asset in safe-tools.js checkout.py verify.py; do
        jq -er --arg name "$asset" '.data[$name] | strings | select(length > 0)' "$context/config.json" > "$context/hooks/pre/$asset"
      done
      tar -czhf "$context/ghost-context.tgz" -C "$profile" skills managed-skills -C "$context" APPEND_SYSTEM.md mcp.json hooks
      cat > "$context/config.yml" <<'EOF'
    modelRoles:
      default: lab-worker/ornith
      smol: lab-worker/ornith
    advisor:
      enabled: false
    async:
      enabled: false
    task:
      batch: false
    autolearn:
      enabled: true
    EOF
      hub="$(awk '/^HIVE_HUB=/{sub(/^HIVE_HUB=/, ""); print; exit}' "$reg")"
      [[ "$hub" == wss://* ]] || die "Missing or invalid HIVE_HUB in registration."
      grep -q '^HIVE_REGISTRATION_TOKEN=' "$reg" || die "Registration token field is missing."

      start_models

      # Recheck after the potentially long warm-up before touching the one registration.
      sts="$(k -n contribute get "$workload" -o json)"
      jq -e '.spec.replicas == 0 and .spec.template.metadata.annotations["bluefin.io/contributor-mode"] == "dual-review-v1"' <<< "$sts" >/dev/null ||
        die "StatefulSet changed during warm-up; refusing to reissue registration."
      pods="$(contributor_pods)"
      jq -e '.items == []' <<< "$pods" >/dev/null || die "Contributor pods appeared during warm-up."
      token="$(timeout --kill-after=2s 20s gh auth token)"
      [ -n "$token" ] || die "gh returned an empty token."
      http="https://${hub#wss://}"
      new="$(curl -fsS --max-time 20 -X POST "${http%/contribute}/api/contribute/reissue-token" -H "Authorization: Bearer $token" |
        jq -er '.registration_token | strings | select(length > 0 and (test("[[:space:]]") | not))')"
      HIVE_NEW_TOKEN="$new" awk '/^HIVE_REGISTRATION_TOKEN=/{ $0="HIVE_REGISTRATION_TOKEN=" ENVIRON["HIVE_NEW_TOKEN"] } 1' "$reg" > "$context/contributor.env"
      cat "$context/contributor.env" > "$reg"
      k -n contribute create secret generic contribute \
        --from-file=contributor.env="$context/contributor.env" \
        --from-file=config.yml="$context/config.yml" --from-file=ghost-context.tgz="$context/ghost-context.tgz" \
        --dry-run=client -o yaml | k apply -f - >/dev/null
      rm -rf "$context"
      trap - EXIT
      k -n contribute scale "$workload" --current-replicas=0 --replicas=2 >/dev/null
      # Check remote session readiness directly, even if only one pod can schedule.
      deadline=$((SECONDS + 600))
    else
      deadline=$SECONDS
    fi

    find_lanes() {
      local pods listing node pod
      exo= ghost=
      pods="$(contributor_pods)" || return
      listing="$(jq -r --arg action "$action" '.items[]
        | select(.metadata.deletionTimestamp == null and .status.phase == "Running")
        | select($action != "on" or any(.status.conditions[]?; .type == "Ready" and .status == "True"))
        | select(.spec.nodeName == "exo-0" or .spec.nodeName == "ghost")
        | [.spec.nodeName, .metadata.name] | @tsv' <<< "$pods")" || return
      while IFS=$'\t' read -r node pod; do
        case "$node" in
          exo-0) [ -z "$exo" ] || continue ;;
          ghost) [ -z "$ghost" ] || continue ;;
          *) continue ;;
        esac
        if k -n contribute exec "$pod" -c contribute --pod-running-timeout=5s -- tmux has-session -t =contributor 2>/dev/null; then
          if [ "$node" = exo-0 ]; then exo="$pod"; else ghost="$pod"; fi
        fi
      done <<< "$listing"
    }

    until find_lanes && [ -n "$exo" ] && [ -n "$ghost" ]; do
      [ "$SECONDS" -lt "$deadline" ] || break
      sleep 2
    done
    if [ "$action" = on ] || [ "${HIVE_CONTRIBUTE_NO_ATTACH:-0}" = 1 ]; then
      [ -n "$exo" ] && [ -n "$ghost" ] || die 'Both reviewer-backed workers did not become ready; run just contribute status.'
      printf 'Dedicated-review workers ready: exo-0=%s ghost=%s\n' "$exo" "$ghost"
    fi
    [ "${HIVE_CONTRIBUTE_NO_ATTACH:-0}" != 1 ] || exit 0
    session="=hive-lab"
    window="hive-lab:lanes"
    if ! tmux has-session -t "$session" 2>/dev/null; then
      left="$(tmux new-session -d -P -F '#{pane_id}' -s hive-lab -n lanes -x 120 -y 32 sleep infinity 9>&-)"
      tmux set-window-option -t "$window" remain-on-exit on
      right="$(tmux split-window -d -h -P -F '#{pane_id}' -t "$left" sleep infinity 9>&-)"
      tmux set-option -p -t "$left" @hive_lane exo-0
      tmux set-option -p -t "$right" @hive_lane ghost
      tmux set-window-option -t "$window" pane-border-status top
      tmux set-window-option -t "$window" pane-border-format ' #{@hive_lane} '
    fi
    panes="$(tmux list-panes -t "$window" -F '#{pane_id} #{@hive_lane} #{pane_dead} #{@hive_pod}')"
    tmux set-window-option -t "$window" synchronize-panes off
    while read -r pane node dead attached_pod; do
      case "$node" in
        exo-0) pod="$exo" ;;
        ghost) pod="$ghost" ;;
        *) continue ;;
      esac
      if [ -z "$pod" ]; then
        printf '%s: remote lane unavailable; rerun just contribute attach to reconnect.\n' "$node" >&2
        if [ -z "$attached_pod" ] && [ "$dead" = 0 ]; then
          tmux respawn-pane -k -t "$pane" bash -c 'printf "%s: lane unavailable. Reconnect with just contribute attach.\n" "$1"' bash "$node" 9>&-
        fi
      elif [ "$dead" = 1 ] || [ -z "$attached_pod" ]; then
        tmux respawn-pane -k -t "$pane" bash -c '
          KUBECONFIG="$1" kubectl -n contribute exec -it "$2" -c contribute --pod-running-timeout=5s \
            -- env TERM=xterm-256color COLORTERM=truecolor tmux attach -t =contributor
          printf "\n%s: disconnected. Reconnect with just contribute attach.\n" "$3"
        ' bash "$KUBECONFIG" "$pod" "$node" 9>&-
        tmux set-option -p -t "$pane" @hive_pod "$pod"
      fi
    done <<< "$panes"
    exec 9>&-
    if [ -n "${TMUX:-}" ]; then
      exec tmux switch-client -t "$session"
    else
      exec tmux attach-session -t "$session"
    fi

contribute-on: (contribute "on")

contribute-attach: (contribute "attach")

contribute-off: (contribute "off")

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
    pods={{quote(pod)}}
    if [ -z "$pods" ]; then
      pods="$(kubectl -n contribute get pods -l app.kubernetes.io/name=contribute -o json | jq -r '.items[] | select(any(.status.containerStatuses[]?; .name == "contribute" and .state.running != null)) | .metadata.name')"
    fi
    [ -n "$pods" ] || exit 0
    dest="$HOME/.local/state/hive/lab/review-pending-skills"
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
    printf 'Saved lessons to %s; review and promote selected skills to the profile managed-skills directory before sharing.\n' "$dest"
