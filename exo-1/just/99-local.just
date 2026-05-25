# ~/.config/ublue-os/just/99-local.just
# Local ujust recipes for exo-1.
#
# Deploy: mkdir -p ~/.config/ublue-os/just && cp this file ~/.config/ublue-os/just/99-local.just
# ── k8s-mode ─────────────────────────────────────────────────────────────────
# Start the k3s agent and hold a systemd sleep inhibitor for the duration.
# Refuses to start if not on ethernet — wifi is unreliable for cluster membership.
#
# Usage:  ujust k8s-mode
# Exit:   Ctrl+C  →  stops k3s agent, releases sleep inhibitor

# ─────────────────────────────────────────────────────────────────────────────
[group('local')]
k8s-mode:
    #!/usr/bin/env bash
    set -euo pipefail

    # ── 1. Require ethernet ───────────────────────────────────────────────────
    # Enumerate physical (non-virtual, non-wireless) interfaces that are up.
    ETH_UP=""
    for iface in $(ip -o link show \
                   | awk -F': ' '$2 !~ /^(lo|wl|docker|virbr|cni|flannel|veth|tun|tap|podman|br-)/{print $2}' \
                   | sed 's/@.*//'
                  ); do
        state=$(cat "/sys/class/net/${iface}/operstate" 2>/dev/null || echo "unknown")
        if [[ "$state" == "up" ]]; then
            ETH_UP="$iface"
            break
        fi
    done

    if [[ -z "$ETH_UP" ]]; then
        echo "❌  No ethernet interface is up."
        echo "    Plug in before starting k8s mode — wifi is not reliable enough"
        echo "    for cluster membership."
        exit 1
    fi

    echo "✓  Ethernet:  $ETH_UP"

    # ── 2. Start k3s agent ───────────────────────────────────────────────────
    echo "→  Starting k3s agent..."
    sudo systemctl start k3s-agent.service
    echo "✓  k3s agent: running"

    # ── 3. Cleanup on exit ───────────────────────────────────────────────────
    # Trap fires on Ctrl+C (INT), kill (TERM), or normal exit.
    cleanup() {
        local rc=$?
        echo ""
        echo "→  Leaving k8s mode..."
        sudo systemctl stop k3s-agent.service \
            && echo "✓  k3s agent stopped" \
            || echo "⚠  k3s agent stop returned non-zero (may already be stopped)"
        # Kill the inhibitor background job to release the lock
        if [[ -n "${INHIBIT_PID:-}" ]]; then
            kill "$INHIBIT_PID" 2>/dev/null || true
            echo "✓  Sleep inhibitor released"
        fi
        exit $rc
    }
    trap cleanup EXIT INT TERM

    # ── 4. Hold systemd sleep inhibitor ──────────────────────────────────────
    # systemd-inhibit holds the named lock for the lifetime of its child
    # process.  We run 'sleep infinity' in the background so the lock is held
    # until cleanup() kills it.
    #
    # --what=sleep:handle-lid-switch
    #   sleep             — blocks explicit suspend/hibernate
    #   handle-lid-switch — blocks the lid-close auto-suspend action
    #   (idle is intentionally omitted: screen can still blank)
    #
    # --mode=block  — actively prevent the inhibited action (not just delay it)
    systemd-inhibit \
        --what=sleep:handle-lid-switch \
        --who="k8s-mode (ujust)" \
        --why="k3s agent running — suspend would drop cluster membership" \
        --mode=block \
        sleep infinity &
    INHIBIT_PID=$!

    # ── 5. Status ─────────────────────────────────────────────────────────────
    echo ""
    echo "k8s mode active"
    echo "  ethernet : $ETH_UP"
    echo "  sleep    : inhibited (sleep + lid-close)"
    echo "  exit     : Ctrl+C to stop k3s and allow sleep"
    echo ""

    # Show this node's presence in the cluster if kubectl is available
    if command -v kubectl &>/dev/null; then
        kubectl get node "$(hostname)" 2>/dev/null \
            || echo "  (node not yet visible — may take a few seconds to register)"
    fi
    echo ""

    # Block until Ctrl+C or the inhibitor process dies
    wait "$INHIBIT_PID" || true
