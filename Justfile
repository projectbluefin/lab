# bluefin-test-suite Justfile
# GitOps policy:
#   - WorkflowTemplate changes go via git push to main; ArgoCD auto-syncs.
#   - Do NOT kubectl apply templates directly.
#   - Workflow submission and monitoring: use these just targets (argo/kubectl CLI).
#   - These recipes are the canonical interface for all routine lifecycle operations.
#   - Agents use these recipes or call argo/kubectl directly. No MCP required.
#   - ssh jorge@ghost is permitted for OS-level tasks only (k3s restart, systemd, brew).
#   - No recipe SSHes to ghost; do NOT add workstation SSH hops.
#   - Cluster bootstrap (setup-argocd) runs once from workstation.

argo_ns   := "argo"

# List all available recipes
default:
    @just --list

# ── Bootstrap (run once) ─────────────────────────────────────────────────────

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

# ── Cleanup ──────────────────────────────────────────────────────────────────

# Delete orphaned VMs in test namespaces
delete-vms:
    kubectl delete vm --all -n bluefin-test --ignore-not-found

# Delete all test workflows
delete-workflows:
    argo delete --all -n {{ argo_ns }} || true

# Full teardown of in-flight resources
teardown:
    just delete-vms
    just delete-workflows

# ── KubeStellar ──────────────────────────────────────────────────────────────

# Verify the GitOps-managed KubeStellar platform and final smoke acceptance gate
run-kubestellar-verify wec-name="ghost":
    argo submit --from workflowtemplate/kubestellar-platform-verify \
      -p wec-name="{{ wec-name }}" \
      -n {{ argo_ns }} --wait --log

# ── Dakota BST builds ────────────────────────────────────────────────────────

# Run Dakota BST pipeline (default bluefin variant only; NVIDIA disabled)
# Usage: just run-bst-build
# Usage: just run-bst-build testing https://github.com/projectbluefin/dakota.git
# Usage: just run-bst-build testing "" default   # only oci/bluefin.bst
# Usage: just run-bst-build testing "" default <commit-sha>
#   variants=all (default) builds every variant; variants=default builds just
#   the plain image, which is what a single-machine rebase needs and what fits
#   on a two-node grid.
run-bst-build ref="testing" repo="https://github.com/projectbluefin/dakota.git" variants="all" commit_sha="":
    argo submit --from workflowtemplate/dakota-build-pipeline \
      -p ref={{ ref }} \
      -p repo={{ repo }} \
      -p variants={{ variants }} \
      {{ if commit_sha != "" { "-p commit-sha=" + commit_sha } else { "" } }} \
      -n {{ argo_ns }} --watch
# Re-run the Dakota poller for the current testing SHA without bypassing BST admission.
force-dakota-poll:
    argo submit --from cronworkflow/dakota-commit-poller \
      -p force=true \
      -n {{ argo_ns }} --watch

# Full Dakota QA pipeline: container-only suite fan-out against the published Dakota image.
run-dakota-qa branch="main" variant="dakota":
    argo submit --from workflowtemplate/dakota-qa-pipeline \
      -p variant={{ variant }} \
      -p branch={{ branch }} \
      -n {{ argo_ns }} --watch

# Legacy Dakota containerized smoke lane: run behave suites directly inside the OCI
# image with explicit image/variant overrides.
run-dakota-container-qa image-tag="testing" variant="dakota":
    argo submit --from workflowtemplate/dakota-container-qa-pipeline \
      -p image=192.168.1.102:30500/{{ variant }} \
      -p image-tag={{ image-tag }} \
      -p variant={{ variant }} \
      -n {{ argo_ns }} --watch

# Promote one immutable Zot candidate to :testing after its lane passes QA.
# Usage: just run-zot-promotion dakota-testing dakota candidate-<sha> sha256:<digest>
run-zot-promotion lane repository candidate_tag expected_digest:
    argo submit --from workflowtemplate/zot-candidate-lifecycle \
      -p lane={{ lane }} \
      -p repository={{ repository }} \
      -p candidate-tag={{ candidate_tag }} \
      -p expected-digest={{ expected_digest }} \
      -n {{ argo_ns }} --watch

# Run the in-cluster BuildStream build pipeline for bluefin-server
# Usage: just run-bluefin-server-build
run-bluefin-server-build ref="main" repo="https://github.com/projectbluefin/server.git":
    argo submit --from workflowtemplate/bluefin-server-build-pipeline \
      -p ref={{ ref }} \
      -p repo={{ repo }} \
      -n {{ argo_ns }} --watch

# ── Validation ───────────────────────────────────────────────────────────────

# Apply bootstrap WorkflowTemplates to the cluster (run once during initial setup)
apply-bootstrap:
    kubectl apply -f argo/bootstrap/ -n {{ argo_ns }}
    @echo "✓ Bootstrap templates applied — run individual templates with: argo submit --from workflowtemplate/<name> -n argo --wait --log"

# Lint all Argo YAML manifests.
# WorkflowTemplates are linted together (--offline) so cross-file templateRef
# references (e.g. dakota-commit-poller → dakota-build-pipeline) resolve without needing
# the Argo server to have the new templates already synced.
lint:
    @echo "Linting argo/workflow-templates/ (offline, cross-file refs)..."
    @argo lint --offline argo/workflow-templates/
    @echo "✔ workflow-templates: no linting errors found!"
    @echo "Linting argo/bootstrap/ (offline)..."
    @argo lint --offline argo/bootstrap/
    @echo "✔ bootstrap: no linting errors found!"
    @echo "Checking semaphore topology..."
    @python3 -m pytest -q tests/unit/test_semaphore_topology.py
    @echo "Checking dakota build variants stay narrowable..."
    @python3 scripts/check_dakota_variants.py
    @echo "✓ All manifests valid"

# Python syntax + advisory ruff over scripts and tests (matches CI).
check-python:
    find scripts tests -name '*.py' -print0 | xargs -0 python3 -m py_compile
    python3 -m ruff check scripts tests || echo "WARNING: ruff reported findings (advisory, not a gate)"

# ── hive-contribute (docs/skills/hive-contribute/SKILL.md) ───────────────────

# Start both local models and the Hive contributor (ships GH token, Hive registration, OMP config).
contribute-on:
    #!/usr/bin/env bash
    set -euo pipefail
    token="${HIVE_CONTRIBUTE_GH_TOKEN:-$HOME/.config/hive-contribute/gh-token}"
    reg="${HIVE_CONTRIBUTE_REGISTRATION:-$HOME/.config/hive/contributor.bluefin.env}"
    omp="${HIVE_CONTRIBUTE_OMP_CONFIG:-$HOME/.omp/agent/config.yml}"
    [[ -s "$token" ]] || { echo "Missing $token: create a classic token with scopes public_repo,read:org at"; echo "  https://github.com/settings/tokens/new?scopes=public_repo,read:org&description=hive-contribute"; echo "then: install -Dm600 /dev/stdin $token"; exit 1; }
    for f in "$reg" "$omp"; do [[ -s "$f" ]] || { echo "Missing $f"; exit 1; }; done
    kubectl -n contribute create secret generic contribute \
      --from-file=GH_TOKEN="$token" --from-file=contributor.env="$reg" --from-file=config.yml="$omp" \
      --dry-run=client -o yaml | kubectl apply -f - >/dev/null
    kubectl -n llm-d scale deploy/llm-d-modelserver deploy/llm-d-advisor --replicas=1
    kubectl -n contribute scale deploy/contribute --replicas=1
    kubectl -n contribute rollout restart deploy/contribute >/dev/null
    echo "Models load in a few minutes on first start. Watch: just contribute-attach"

# Stop the contributor and free both GPUs.
contribute-off:
    kubectl -n contribute scale deploy/contribute --replicas=0
    kubectl -n llm-d scale deploy/llm-d-modelserver deploy/llm-d-advisor --replicas=0

# Attach to the contributor's tmux session (detach: C-b d).
contribute-attach:
    kubectl -n contribute exec -it deploy/contribute -- tmux attach -t contributor

# Replica state of the contributor and both models.
contribute-status:
    kubectl -n llm-d get deploy,pods -o wide
    kubectl -n contribute get deploy,pods -o wide
