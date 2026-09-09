"""Unit tests for the per-app cluster resource collector.

Covers the pure transform layer of ``scripts/collect_app_resources.py``:
``parse_cpu_cores``, ``parse_mem_mib`` and ``get_app_for_pod``. These three
functions convert raw Kubernetes quantity strings and pod metadata into the
per-application CPU/memory rollup written to ``docs/data/``. A silent unit
misparse (``n`` vs ``m``, ``Ki`` vs ``Mi``) or a namespace attribution
regression would ship wrong numbers to the dashboard with no visible failure,
so the parsing contract is pinned here.
"""

from __future__ import annotations

import sys
from pathlib import Path

repo_root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(repo_root / "scripts"))

import collect_app_resources as collector  # noqa: E402


class TestParseCpuCores:
    def test_empty_and_none_are_zero(self):
        assert collector.parse_cpu_cores("") == 0.0
        assert collector.parse_cpu_cores(None) == 0.0

    def test_nanocores(self):
        assert collector.parse_cpu_cores("500000000n") == 0.5

    def test_microcores(self):
        assert collector.parse_cpu_cores("1500u") == 0.0015

    def test_millicores(self):
        assert collector.parse_cpu_cores("250m") == 0.25

    def test_plain_cores(self):
        assert collector.parse_cpu_cores("2") == 2.0
        assert collector.parse_cpu_cores("0.5") == 0.5

    def test_surrounding_whitespace_tolerated(self):
        assert collector.parse_cpu_cores("  250m  ") == 0.25

    def test_non_string_input_is_coerced(self):
        assert collector.parse_cpu_cores(3) == 3.0

    def test_garbage_is_zero_not_an_exception(self):
        assert collector.parse_cpu_cores("abc") == 0.0
        assert collector.parse_cpu_cores("m") == 0.0

    def test_zero_string_is_zero(self):
        assert collector.parse_cpu_cores("0") == 0.0


class TestParseMemMib:
    def test_empty_and_none_are_zero(self):
        assert collector.parse_mem_mib("") == 0.0
        assert collector.parse_mem_mib(None) == 0.0

    def test_kibibytes(self):
        assert collector.parse_mem_mib("2048Ki") == 2.0

    def test_mebibytes_pass_through(self):
        assert collector.parse_mem_mib("512Mi") == 512.0

    def test_gibibytes(self):
        assert collector.parse_mem_mib("2Gi") == 2048.0

    def test_tebibytes(self):
        assert collector.parse_mem_mib("1Ti") == 1024.0 * 1024.0

    def test_bare_number_is_treated_as_bytes(self):
        assert collector.parse_mem_mib(str(1024 * 1024)) == 1.0

    def test_garbage_is_zero_not_an_exception(self):
        assert collector.parse_mem_mib("lots") == 0.0
        assert collector.parse_mem_mib("Mi") == 0.0

    def test_suffix_precedence_mi_before_bare(self):
        # "1024Mi" must not be read as 1024 bytes.
        assert collector.parse_mem_mib("1024Mi") == 1024.0


class TestGetAppForPod:
    def test_workflow_label_wins_over_namespace(self):
        labels = {"workflows.argoproj.io/workflow": "run-abc"}
        assert collector.get_app_for_pod("run-abc-123", "argo", labels) == "testing-lab"
        assert (
            collector.get_app_for_pod("run-abc-123", "buildbarn", labels) == "testing-lab"
        )

    def test_arc_namespaces(self):
        assert collector.get_app_for_pod("runner-1", "arc-runners", {}) == "arc-runners"
        assert (
            collector.get_app_for_pod("listener-1", "arc-systems", {}) == "arc-systems"
        )

    def test_argo_control_plane_pods(self):
        assert (
            collector.get_app_for_pod("argo-server-abc", "argo", {}) == "argo-workflows"
        )
        assert (
            collector.get_app_for_pod(
                "argo-workflows-workflow-controller-xyz", "argo", {}
            )
            == "argo-workflows"
        )

    def test_other_argo_namespace_pods_are_infra(self):
        assert (
            collector.get_app_for_pod("k8sgpt-77d", "argo", {}) == "testing-lab-infra"
        )
        assert (
            collector.get_app_for_pod("homelab-access-1", "argo", {})
            == "testing-lab-infra"
        )

    def test_update_namespaces(self):
        assert (
            collector.get_app_for_pod("agent-1", "flatcar-update", {})
            == "flatcar-update"
        )
        assert (
            collector.get_app_for_pod("agent-1", "system-upgrade", {})
            == "flatcar-update"
        )

    def test_infra_namespaces(self):
        for namespace in ("buildbarn", "local-registry", "cdi", "kubevirt"):
            assert (
                collector.get_app_for_pod("pod-1", namespace, {}) == "testing-lab-infra"
            )

    def test_argocd_instance_label_fallback(self):
        labels = {"argocd.argoproj.io/instance": "testing-lab"}
        assert collector.get_app_for_pod("pod-1", "default", labels) == "testing-lab"

    def test_kubernetes_instance_label_fallback(self):
        labels = {"app.kubernetes.io/instance": "arc-runners"}
        assert collector.get_app_for_pod("pod-1", "default", labels) == "arc-runners"

    def test_argocd_label_takes_precedence_over_kubernetes_label(self):
        labels = {
            "argocd.argoproj.io/instance": "testing-lab",
            "app.kubernetes.io/instance": "arc-runners",
        }
        assert collector.get_app_for_pod("pod-1", "default", labels) == "testing-lab"

    def test_unknown_instance_label_is_not_attributed(self):
        labels = {"argocd.argoproj.io/instance": "some-other-app"}
        assert collector.get_app_for_pod("pod-1", "default", labels) is None

    def test_unattributed_pod_returns_none(self):
        assert collector.get_app_for_pod("pod-1", "kube-system", {}) is None
