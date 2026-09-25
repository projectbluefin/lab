# Hive contributor on the lab (projectbluefin/lab docs/skills/hive-contribute/SKILL.md).
# Works from any directory; repo justfiles with `set fallback` also reach it.

export KUBECONFIG := env("KUBECONFIG", home_directory() / ".kube/bluespeed.yaml")

# Start both local models and the Hive contributor, then attach to it.
contribute-on:
    #!/usr/bin/env bash
    set -euo pipefail
    reg="${HIVE_CONTRIBUTE_REGISTRATION:-$HOME/.config/hive/contributor.bluefin.env}"
    omp="${HIVE_CONTRIBUTE_OMP_CONFIG:-$HOME/.omp/agent/config.yml}"
    tok="$(mktemp)"; trap 'rm -f "$tok"' EXIT
    printf %s "$(gh auth token)" > "$tok"
    # Refresh the Hive registration token with the gh identity (upstream's contribute-move call),
    # so a stale or reissued-elsewhere token never needs a manual repair. The lab is the one contributor.
    hub="$(sed -nE 's/^HIVE_HUB=//p' "$reg" | head -1)"; http="https://${hub#wss://}"; http="${http%/contribute}"
    new="$(curl -fsS --max-time 20 -X POST "${http}/api/contribute/reissue-token" -H "Authorization: Bearer $(cat "$tok")" | jq -er .registration_token)"
    sed -i "s|^HIVE_REGISTRATION_TOKEN=.*|HIVE_REGISTRATION_TOKEN=${new}|" "$reg"
    # Ship your config.yml with the roles pinned to the lab models, so Hive and OMP both see the truth.
    cfg="$(mktemp)"; trap 'rm -f "$tok" "$cfg"' EXIT
    yq '.modelRoles = {"default": "lab-worker/ornith", "smol": "lab-worker/ornith", "advisor": "lab-advisor/qwen-advisor"}' "$omp" > "$cfg"
    kubectl -n contribute create secret generic contribute \
      --from-file=GH_TOKEN="$tok" --from-file=contributor.env="$reg" --from-file=config.yml="$cfg" \
      --dry-run=client -o yaml | kubectl apply -f - >/dev/null
    kubectl -n llm-d scale deploy/llm-d-modelserver deploy/llm-d-advisor --replicas=1 >/dev/null
    kubectl -n contribute scale deploy/contribute --replicas=1 >/dev/null
    kubectl -n contribute rollout restart deploy/contribute >/dev/null
    echo "Waiting for the models (first start downloads ~50 GB) and the contributor..."
    kubectl -n llm-d rollout status deploy/llm-d-modelserver --timeout=60m
    kubectl -n llm-d rollout status deploy/llm-d-advisor --timeout=60m
    kubectl -n contribute rollout status deploy/contribute --timeout=10m
    until kubectl -n contribute exec deploy/contribute -- tmux has-session -t contributor 2>/dev/null; do sleep 2; done
    just contribute-attach

# Re-attach to the contributor's tmux session (detach: C-b d; it keeps running).
contribute-attach:
    kubectl -n contribute exec -it deploy/contribute -- env TERM=xterm-256color COLORTERM=truecolor tmux attach -t contributor

# Stop the contributor and free both GPUs.
contribute-off:
    kubectl -n contribute scale deploy/contribute --replicas=0
    kubectl -n llm-d scale deploy/llm-d-modelserver deploy/llm-d-advisor --replicas=0

# Replica state of the contributor and both models.
contribute-status:
    kubectl -n llm-d get deploy,pods -o wide
    kubectl -n contribute get deploy,pods -o wide
