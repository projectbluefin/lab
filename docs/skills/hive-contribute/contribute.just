# Hive contributor on the lab (projectbluefin/lab docs/skills/hive-contribute/SKILL.md).
# Works from any directory; repo justfiles with `set fallback` also reach it.

export KUBECONFIG := env("KUBECONFIG", home_directory() / ".kube/bluespeed.yaml")

alias contribute := contribute-on

# Start both nodes' models and both Hive contributors, then attach to contributor 0.
contribute-on: contribute-learning-save
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
    # Ship your config.yml with the roles pinned to the lab models, so Hive and OMP both see the truth.
    cfg="$(mktemp)"; bundle="$(mktemp)"; trap 'rm -f "$tok" "$cfg" "$bundle"' EXIT
    mkdir -p "$profile/managed-skills"
    tar -czhf "$bundle" -C "$profile" skills managed-skills APPEND_SYSTEM.md
    yq '.modelRoles = {"default": "lab-worker/ornith", "smol": "lab-worker/ornith", "advisor": "lab-advisor/qwen-advisor"}' "$omp" > "$cfg"
    kubectl -n contribute create secret generic contribute \
      --from-file=GH_TOKEN="$tok" --from-file=contributor.env="$reg" --from-file=config.yml="$cfg" \
      --from-file=ghost-context.tgz="$bundle" \
      --dry-run=client -o yaml | kubectl apply -f - >/dev/null
    kubectl -n llm-d scale deploy/llm-d-exo-0 deploy/llm-d-ghost --replicas=1 >/dev/null
    echo "Warming both models on each node (first start downloads ~70 GB per node) before starting contributors..."
    kubectl -n llm-d rollout status deploy/llm-d-exo-0 --timeout=60m
    kubectl -n llm-d rollout status deploy/llm-d-ghost --timeout=60m
    # Warm-up gate: download + load both models on each node; ornith ends resident.
    warm() {
      for m in qwen-advisor ornith; do
        kubectl -n llm-d exec "deploy/llm-d-$1" -- curl -fsS --max-time 3600 localhost:8000/v1/chat/completions \
          -H 'Content-Type: application/json' \
          -d "{\"model\":\"$m\",\"messages\":[{\"role\":\"user\",\"content\":\"ok\"}],\"max_tokens\":1}" >/dev/null
      done
    }
    warm exo-0 & a=$!; warm ghost & b=$!
    wait "$a"; wait "$b"
    kubectl -n contribute scale sts/contribute --replicas=2 >/dev/null
    kubectl -n contribute rollout restart sts/contribute >/dev/null
    kubectl -n contribute rollout status sts/contribute --timeout=10m
    for n in 0 1; do
      until kubectl -n contribute exec "contribute-$n" -- tmux has-session -t contributor 2>/dev/null; do sleep 2; done
    done
    just contribute-attach

# Re-attach to a contributor's tmux session: 0 = exo-0, 1 = ghost (detach: C-b d; it keeps running).
contribute-attach n="0":
    kubectl -n contribute exec -it contribute-{{n}} -- env TERM=xterm-256color COLORTERM=truecolor tmux attach -t contributor

# Stop both contributors and free both GPUs.
contribute-off: contribute-learning-save
    kubectl -n contribute scale sts/contribute --replicas=0
    kubectl -n llm-d scale deploy/llm-d-exo-0 deploy/llm-d-ghost --replicas=0

# Replica state of both contributors and both nodes' models.
contribute-status:
    kubectl -n llm-d get deploy,pods -o wide
    kubectl -n contribute get sts,pods -o wide

# Save lessons before a normal stop/restart; review before capturing with chezmoi.
contribute-learning-save pod="":
    #!/usr/bin/env bash
    set -euo pipefail
    pods="{{pod}}"
    if [ -z "$pods" ]; then
      pods="$(kubectl -n contribute get pods -l app.kubernetes.io/name=contribute --field-selector=status.phase=Running -o jsonpath='{.items[*].metadata.name}')"
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
