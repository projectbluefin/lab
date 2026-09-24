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
      -p build-mode=re \
      -p variants={{ variants }} \
      {{ if commit_sha != "" { "-p commit-sha=" + commit_sha } else { "" } }} \
      -n {{ argo_ns }} --watch
# Re-run the Dakota poller for the current testing SHA without bypassing BST admission.
force-dakota-poll:
    argo submit --from cronworkflow/dakota-commit-poller \
      -p force=true \
      -n {{ argo_ns }} --watch

# Compatibility alias for older docs/callers.
run-dakota-validate ref="testing" repo="https://github.com/projectbluefin/dakota.git" variants="all":
    just run-bst-build {{ ref }} {{ repo }} {{ variants }}

# Compatibility alias for older docs/callers.
run-dakota-build ref="testing" repo="https://github.com/projectbluefin/dakota.git" variants="all":
    just run-bst-build {{ ref }} {{ repo }} {{ variants }}

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

# Run the isolated operator-only RECC baseline.
# Usage: just run-recc-baseline mode=cache-only cache-policy=both recc-provider=components/buildbox.bst
run-recc-baseline *args:
    #!/usr/bin/env bash
    set -euo pipefail
    MODE="buildstream-only"
    RUN_ID=""
    CACHE_POLICY="cold"
    RECC_PROVIDER="freedesktop-sdk.bst:components/buildbox.bst"
    for arg in {{ args }}; do
      case "${arg}" in
        mode=*) MODE="${arg#mode=}" ;;
        run-id=*) RUN_ID="${arg#run-id=}" ;;
        cache-policy=*) CACHE_POLICY="${arg#cache-policy=}" ;;
        recc-provider=*) RECC_PROVIDER="${arg#recc-provider=}" ;;
        *)
          echo "Unsupported run-recc-baseline argument: ${arg}" >&2
          exit 2
          ;;
      esac
    done
    exec argo submit --from workflowtemplate/recc-baseline-pipeline \
      -p mode="${MODE}" \
      -p run-id="${RUN_ID}" \
      -p cache-policy="${CACHE_POLICY}" \
      -p recc-provider="${RECC_PROVIDER}" \
      -n {{ argo_ns }} --watch

# ── Validation ───────────────────────────────────────────────────────────────

# Validate the reusable BuildStream OCI rechunk transform.
test-rechunk:
    bash -n scripts/rechunk_bst_image.sh
    python -m pytest -q tests/unit/test_rechunk_bst_image.py

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
    @python3 scripts/check_semaphore_topology.py argo/
    @echo "Checking dakota build variants stay narrowable..."
    @python3 scripts/check_dakota_variants.py
    @echo "✓ All manifests valid"

# Run the Python checks over the trees declared in .python-scope — the single
# source of truth also consumed by .github/workflows/lint.yaml (syntax, blocking)
# and .github/workflows/ci.yml (ruff, advisory). Keeps local == CI.
check-python:
    #!/usr/bin/env bash
    set -euo pipefail
    mapfile -t TREES < <(grep -vE '^[[:space:]]*(#|$)' .python-scope)
    echo "Python check scope (.python-scope): ${TREES[*]}"
    echo "Validating Python syntax..."
    find "${TREES[@]}" -name '*.py' -print0 | xargs -0 python3 -m py_compile
    echo "✔ syntax OK"
    echo "Linting with ruff (advisory)..."
    python3 -m ruff check "${TREES[@]}" || echo "WARNING: ruff reported findings (advisory, not a gate)"
