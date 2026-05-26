# bluefin-test-suite Justfile
# GitOps policy:
#   - WorkflowTemplate changes go via git push to main; ArgoCD auto-syncs.
#   - Do NOT kubectl apply templates directly. Do NOT SSH to ghost or exo-1.
#   - Workflow submission and monitoring: use these just targets or Argo MCP tools.
#   - Cluster bootstrap (setup-ssh-secret, setup-argocd) runs once from workstation.

image       := env_var_or_default("BLUEFIN_IMAGE", "ghcr.io/ublue-os/bluefin:latest")
image_tag   := env_var_or_default("BLUEFIN_IMAGE_TAG", "latest")
test_branch := env_var_or_default("BLUEFIN_TEST_BRANCH", "main")
gnomeos_image_url := env_var_or_default("GNOMEOS_IMAGE_URL", "https://os.gnome.org/download/latest/installer_x86_64.iso")
gnomeos_image_sha256 := env_var_or_default("GNOMEOS_IMAGE_SHA256", "")
gnomeos_image_format := env_var_or_default("GNOMEOS_IMAGE_FORMAT", "raw")
gnomeos_namespace := env_var_or_default("GNOMEOS_NAMESPACE", "gnomeos-test")
gnomeos_console_hook := env_var_or_default("GNOMEOS_CONSOLE_HOOK_CONFIG_MAP", "")
upstream_terminal_repo := env_var_or_default("UPSTREAM_TERMINAL_REPO", "https://github.com/modehnal/GNOMETerminalAutomation.git")
upstream_terminal_ref := env_var_or_default("UPSTREAM_TERMINAL_REF", "main")
argo_ns     := "argo"

# List all available recipes
default:
    @just --list

# ── Bootstrap (run once) ─────────────────────────────────────────────────────

# Create bluefin-test-ssh-key secret in argo namespace (idempotent)
# The secret is read by bib-disk-configure via secretKeyRef — no pubkey env var needed.
setup-ssh-secret:
    #!/usr/bin/env bash
    set -euo pipefail
    if kubectl get secret bluefin-test-ssh-key -n {{ argo_ns }} &>/dev/null; then
        echo "✓ bluefin-test-ssh-key already exists"
        kubectl get secret bluefin-test-ssh-key -n {{ argo_ns }} \
            -o jsonpath="{.data.id_ed25519\.pub}" | base64 -d | ssh-keygen -lf - \
            && echo "(fingerprint above)"
        exit 0
    fi
    ssh_key=$(mktemp)
    ssh-keygen -t ed25519 -f "${ssh_key}" -N "" -C "bluefin-test-suite@ghost" >/dev/null
    kubectl create secret generic bluefin-test-ssh-key \
        --from-file=id_ed25519="${ssh_key}" \
        --from-file=id_ed25519.pub="${ssh_key}.pub" \
        -n {{ argo_ns }}
    shred -u "${ssh_key}" "${ssh_key}.pub"
    echo "✓ SSH secret created"

# Deploy the ArgoCD Application that auto-syncs argo/workflow-templates from git (run once)
# After this, template changes take effect on git push — no kubectl apply needed.
setup-argocd:
    kubectl apply -f argocd/application.yaml -n argocd
    @echo "✓ ArgoCD Application deployed — syncs argo/workflow-templates from main automatically"

# ── Template management (GitOps — prefer git push over manual sync) ──────────

# Force ArgoCD to sync now instead of waiting for the next poll interval
argocd-sync:
    argocd app sync testing-lab testing-lab-infra --timeout 120
    argocd app wait testing-lab --health --timeout 120
    argocd app wait testing-lab-infra --health --timeout 120

# Show ArgoCD sync status for the test suite
argocd-status:
    argocd app get testing-lab
    argocd app get testing-lab-infra

# ── Disk image management ────────────────────────────────────────────────────

# Pre-build golden disk for a given tag (idempotent — skips if disk already exists)
# Pubkey is injected from the bluefin-test-ssh-key secret automatically.
# Usage: just ensure-disk
# Usage: just ensure-disk lts
ensure-disk tag=image_tag:
    argo submit --from workflowtemplate/bib-build-and-push \
        -p image="ghcr.io/ublue-os/bluefin:{{ tag }}" \
        -p image-tag="{{ tag }}" \
        -n {{ argo_ns }} \
        --watch

# Patch an existing golden disk's SSH config (no SSH to node required)
# Use after secret rotation or when SSH auth fails on an existing disk.
# Usage: just patch-disk
# Usage: just patch-disk lts
patch-disk tag=image_tag:
    argo submit --from workflowtemplate/patch-golden-disk \
        -p image-tag="{{ tag }}" \
        -n {{ argo_ns }} \
        --watch

# ── Test execution ───────────────────────────────────────────────────────────

# Run smoke tests against latest (or BLUEFIN_IMAGE_TAG)
run-tests:
    argo submit argo/bluefin-smoke-test.yaml \
        -p image="{{ image }}" \
        -p image-tag="{{ image_tag }}" \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run smoke tests against a specific tag
# Usage: just run-tests-tag lts
run-tests-tag tag:
    argo submit argo/bluefin-smoke-test.yaml \
        -p image="ghcr.io/ublue-os/bluefin:{{ tag }}" \
        -p image-tag="{{ tag }}" \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run matrix tests (latest + lts in parallel)
# Optional: PR_TITLE, PR_NUMBER, and BLUEFIN_TEST_BRANCH env vars
run-tests-matrix:
    #!/usr/bin/env bash
    set -euo pipefail
    PR_TITLE="${PR_TITLE:-}"
    PR_NUMBER="${PR_NUMBER:-}"
    argo submit argo/bluefin-test-matrix.yaml \
        -p pr-title="${PR_TITLE}" \
        -p pr-number="${PR_NUMBER}" \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run smoke + developer suites on a fresh VM
# Usage: just run-developer-tests
# Usage: just run-developer-tests lts
run-developer-tests tag=image_tag:
    #!/usr/bin/env bash
    set -euo pipefail
    NAMESPACE="bluefin-test"
    VARIANT="bluefin"
    if [[ "{{ tag }}" == "lts" ]]; then
        NAMESPACE="bluefin-lts-test"
        VARIANT="lts"
    fi
    argo submit --from workflowtemplate/bluefin-qa-pipeline \
        -p image="ghcr.io/ublue-os/bluefin:{{ tag }}" \
        -p image-tag="{{ tag }}" \
        -p namespace="${NAMESPACE}" \
        -p suites="smoke,developer" \
        -p variant="${VARIANT}" \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run smoke + developer + software suites on a fresh VM
# Usage: just run-software-tests
# Usage: just run-software-tests lts
run-software-tests tag=image_tag:
    #!/usr/bin/env bash
    set -euo pipefail
    NAMESPACE="bluefin-test"
    VARIANT="bluefin"
    if [[ "{{ tag }}" == "lts" ]]; then
        NAMESPACE="bluefin-lts-test"
        VARIANT="lts"
    fi
    argo submit --from workflowtemplate/bluefin-qa-pipeline \
        -p image="ghcr.io/ublue-os/bluefin:{{ tag }}" \
        -p image-tag="{{ tag }}" \
        -p namespace="${NAMESPACE}" \
        -p suites="smoke,developer,software" \
        -p variant="${VARIANT}" \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run smoke against persistent titan VMs (no BIB build, instant start)
run-titan-smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    IP_LATEST=$(kubectl get vmi titan-bluefin -n bluefin-test \
        -o jsonpath='{.status.interfaces[0].ipAddress}' 2>/dev/null)
    IP_LTS=$(kubectl get vmi titan-lts -n bluefin-lts-test \
        -o jsonpath='{.status.interfaces[0].ipAddress}' 2>/dev/null)
    : "${IP_LATEST:?titan-bluefin VMI not found or has no IP}"
    : "${IP_LTS:?titan-lts VMI not found or has no IP}"
    echo "titan-bluefin: ${IP_LATEST}"
    echo "titan-lts:     ${IP_LTS}"
    argo submit --from workflowtemplate/bluefin-titan-smoke \
        -p vm-ip-latest="${IP_LATEST}" \
        -p vm-ip-lts="${IP_LTS}" \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run developer suite against persistent titan VMs
run-titan-developer:
    #!/usr/bin/env bash
    set -euo pipefail
    IP_LATEST=$(kubectl get vmi titan-bluefin -n bluefin-test \
        -o jsonpath='{.status.interfaces[0].ipAddress}' 2>/dev/null)
    IP_LTS=$(kubectl get vmi titan-lts -n bluefin-lts-test \
        -o jsonpath='{.status.interfaces[0].ipAddress}' 2>/dev/null)
    : "${IP_LATEST:?titan-bluefin VMI not found or has no IP}"
    : "${IP_LTS:?titan-lts VMI not found or has no IP}"
    echo "titan-bluefin: ${IP_LATEST}"
    echo "titan-lts:     ${IP_LTS}"
    argo submit --from workflowtemplate/bluefin-titan-smoke \
        -p vm-ip-latest="${IP_LATEST}" \
        -p vm-ip-lts="${IP_LTS}" \
        -p suite="developer" \
        -p issue-title="titan developer run" \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run software suite against persistent titan VMs
run-titan-software:
    #!/usr/bin/env bash
    set -euo pipefail
    IP_LATEST=$(kubectl get vmi titan-bluefin -n bluefin-test \
        -o jsonpath='{.status.interfaces[0].ipAddress}' 2>/dev/null)
    IP_LTS=$(kubectl get vmi titan-lts -n bluefin-lts-test \
        -o jsonpath='{.status.interfaces[0].ipAddress}' 2>/dev/null)
    : "${IP_LATEST:?titan-bluefin VMI not found or has no IP}"
    : "${IP_LTS:?titan-lts VMI not found or has no IP}"
    echo "titan-bluefin: ${IP_LATEST}"
    echo "titan-lts:     ${IP_LTS}"
    argo submit --from workflowtemplate/bluefin-titan-smoke \
        -p vm-ip-latest="${IP_LATEST}" \
        -p vm-ip-lts="${IP_LTS}" \
        -p suite="software" \
        -p issue-title="titan software run" \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run Flatcar smoke tests
run-flatcar-smoke:
    argo submit argo/flatcar-smoke-test.yaml \
        -n {{ argo_ns }} \
        --watch

# Run GNOME OS ingest/boot/access spike
run-gnomeos-spike:
    argo submit argo/gnomeos-access-spike.yaml \
        -p image-url="{{ gnomeos_image_url }}" \
        -p image-sha256="{{ gnomeos_image_sha256 }}" \
        -p image-format="{{ gnomeos_image_format }}" \
        -p namespace="{{ gnomeos_namespace }}" \
        -p console-hook-config-map="{{ gnomeos_console_hook }}" \
        -n {{ argo_ns }} \
        --watch

# Run the public Red Hat GNOME Terminal upstream suite against persistent titan VMs
run-upstream-terminal-tests:
    #!/usr/bin/env bash
    set -euo pipefail
    IP_LATEST=$(kubectl get vmi titan-bluefin -n bluefin-test \
        -o jsonpath='{.status.interfaces[0].ipAddress}' 2>/dev/null)
    IP_LTS=$(kubectl get vmi titan-lts -n bluefin-lts-test \
        -o jsonpath='{.status.interfaces[0].ipAddress}' 2>/dev/null)
    : "${IP_LATEST:?titan-bluefin VMI not found or has no IP}"
    : "${IP_LTS:?titan-lts VMI not found or has no IP}"
    echo "titan-bluefin: ${IP_LATEST}"
    echo "titan-lts:     ${IP_LTS}"
    argo submit argo/upstream-gnome-terminal-titan.yaml \
        -p vm-ip-latest="${IP_LATEST}" \
        -p vm-ip-lts="${IP_LTS}" \
        -p upstream-suite-repo="{{ upstream_terminal_repo }}" \
        -p upstream-suite-ref="{{ upstream_terminal_ref }}" \
        -n {{ argo_ns }} \
        --watch

# Run the public Red Hat GNOME Terminal upstream suite against a fresh Bluefin VM
# Usage: just run-upstream-terminal-tests-fresh
# Usage: just run-upstream-terminal-tests-fresh lts
run-upstream-terminal-tests-fresh tag=image_tag:
    #!/usr/bin/env bash
    set -euo pipefail
    NAMESPACE="bluefin-test"
    IMAGE="ghcr.io/ublue-os/bluefin:{{ tag }}"
    if [[ "{{ tag }}" == "lts" ]]; then
        NAMESPACE="bluefin-lts-test"
    fi
    argo submit argo/upstream-gnome-terminal-bluefin.yaml \
        -p image="${IMAGE}" \
        -p image-tag="{{ tag }}" \
        -p namespace="${NAMESPACE}" \
        -p upstream-suite-repo="{{ upstream_terminal_repo }}" \
        -p upstream-suite-ref="{{ upstream_terminal_ref }}" \
        -n {{ argo_ns }} \
        --watch

# Run the first in-cluster homelab substrate lane
run-homelab-substrate:
    argo submit argo/homelab-substrate.yaml \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run the shared k8s-first service-catalog workflow
run-service-catalog-smoke lane="media":
    argo submit argo/bluefin-service-catalog-smoke.yaml \
        -p lane="{{ lane }}" \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run the media lane in the shared service-catalog workflow
run-service-media:
    just run-service-catalog-smoke media

# Run the non-media lane in the shared service-catalog workflow
run-service-nonmedia:
    just run-service-catalog-smoke nonmedia

# Run the first in-cluster HTTPS access probe lane
run-homelab-access:
    argo submit argo/homelab-access-probe.yaml \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run the auth-gated variant of the homelab access probe
run-homelab-auth:
    argo submit argo/homelab-auth-probe.yaml \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run the first in-cluster restore drill
run-homelab-restore:
    argo submit argo/homelab-restore-drill.yaml \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# Run the in-cluster storage persistence lane
run-homelab-storage:
    argo submit argo/homelab-storage.yaml \
        -p branch="{{ test_branch }}" \
        -n {{ argo_ns }} \
        --watch

# ── Observation ─────────────────────────────────────────────────────────────

# List all test workflows
list-workflows:
    argo list -n {{ argo_ns }}

# Tail logs from the most recent workflow
logs:
    argo logs -n {{ argo_ns }} @latest

# List VMs in all test namespaces
list-vms:
    @echo "=== bluefin-test ===" && kubectl get vm -n bluefin-test 2>/dev/null || true
    @echo "=== bluefin-lts-test ===" && kubectl get vm -n bluefin-lts-test 2>/dev/null || true
    @echo "=== flatcar-test ===" && kubectl get vm -n flatcar-test 2>/dev/null || true
    @echo "=== gnomeos-test ===" && kubectl get vm -n gnomeos-test 2>/dev/null || true

# ── Cleanup ──────────────────────────────────────────────────────────────────

# Delete orphaned VMs in test namespaces.
# SAFE BY DEFAULT: skips persistent titan VMs (app=titan-bluefin / titan-lts),
# skips knuckle-test (managed elsewhere). To force-include everything, the
# cluster CronWorkflow `orphan-vm-cleanup` runs every 2h with the same safety
# rules — use it instead of running this manually.
delete-vms:
    #!/usr/bin/env bash
    set -euo pipefail
    for NS in bluefin-test bluefin-lts-test flatcar-test gnomeos-test; do
        while IFS= read -r VM; do
            [[ -z "${VM}" ]] && continue
            APP=$(kubectl get vm "${VM}" -n "${NS}" \
                -o jsonpath='{.metadata.labels.app}' 2>/dev/null || true)
            if [[ "${APP}" == titan-* ]]; then
                echo "SKIP ${NS}/${VM}: titan VM (app=${APP})"
                continue
            fi
            echo "DELETE ${NS}/${VM}"
            kubectl delete vm "${VM}" -n "${NS}" --ignore-not-found --wait=false
        done < <(kubectl get vm -n "${NS}" -o name 2>/dev/null | sed 's|.*/||')
    done

# Delete all test workflows
delete-workflows:
    argo delete --all -n {{ argo_ns }} || true

# Full teardown of in-flight resources
teardown:
    just delete-vms
    just delete-workflows

# ── Validation ───────────────────────────────────────────────────────────────

# Lint all Argo workflow templates and workflows together
lint:
    argo lint --offline argo/workflow-templates/*.yaml argo/*.yaml
    @echo "✓ All manifests valid"
