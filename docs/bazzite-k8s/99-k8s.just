# k3s toggle for bazzite — gaming machine, k8s on demand only
# Install: mkdir -p ~/.config/ublue-os/just && cp 99-k8s.just ~/.config/ublue-os/just/

[group('k8s')]
[doc('Toggle k3s agent on/off — start when cluster compute needed, stop before gaming')]
toggle-k8s:
    #!/usr/bin/env bash
    set -euo pipefail
    if systemctl is-active --quiet k3s-agent 2>/dev/null; then
        echo "🎮 Stopping k3s agent (gaming mode)..."
        sudo systemctl stop k3s-agent
        sudo systemctl disable k3s-agent
        echo "✓ k3s agent stopped and disabled"
    else
        echo "☸️  Starting k3s agent..."
        sudo systemctl enable --now k3s-agent
        echo "✓ k3s agent started — node will appear in cluster within ~30s"
    fi
    echo ""
    systemctl is-active k3s-agent 2>/dev/null && echo "Status: RUNNING" || echo "Status: STOPPED"
