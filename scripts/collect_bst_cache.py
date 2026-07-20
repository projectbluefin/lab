#!/usr/bin/env python3
"""Collect BuildStream / remote-execution cache usage across both cache
drives (ghost, exo-0) and write docs/data/bst-cache.json.

Two independent cache backends currently exist:
  - bazel-remote ("bst-artifact-server", argo/bst-artifact-server), pinned to
    ghost only, backing /var/mnt/ghost-data/bst-artifact-cache. Exposes a
    live JSON /status endpoint with real byte counts.
  - Buildbarn's 2-shard CAS/AC (buildbarn/storage-0, storage-1), scheduled
    across ghost + exo-0 via podAntiAffinity. Each shard uses fixed-size
    block-device-backed storage (always fully preallocated on disk), so
    there is no direct "bytes used" gauge; usage is derived from the
    allocations_total/releases_total counters times the known block size
    (see manifests/buildbarn-config.yaml: 20GiB/35 blocks for CAS,
    20MiB/33 blocks for AC).

All cluster reads go through the Kubernetes API server's service/pod proxy
subresource (`kubectl get --raw .../proxy/...`) rather than requiring direct
network reachability to ClusterIPs or new NodePorts/manifests, matching the
existing kubectl-based reachability pattern in refresh_factory_stats.py.
Any read that fails (cluster unreachable, pod not scheduled) yields a row
with state: "unavailable" and an explicit state_reason -- never invented
values, per docs/data/page-contracts.md.
"""
import json
import subprocess
import datetime

OUT_PATH = "docs/data/bst-cache.json"

# Buildbarn block-device-backed storage constants (manifests/buildbarn-config.yaml)
CAS_CAPACITY_BYTES = 20 * 1024 * 1024 * 1024
CAS_BLOCKS = 8 + 24 + 3
AC_CAPACITY_BYTES = 20 * 1024 * 1024
AC_BLOCKS = 8 + 24 + 1


def run_raw(path):
    try:
        out = subprocess.check_output(
            ["kubectl", "get", "--raw", path],
            text=True, stderr=subprocess.DEVNULL, timeout=12,
        )
        return out
    except Exception:
        return None


def run_json_raw(path):
    out = run_raw(path)
    if out is None:
        return None
    try:
        return json.loads(out)
    except Exception:
        return None


def parse_metric_value(metrics_text, metric_name, labels):
    """Pull a single Prometheus metric value matching all given labels."""
    if not metrics_text:
        return None
    for line in metrics_text.splitlines():
        if not line.startswith(metric_name + "{"):
            continue
        if all(f'{k}="{v}"' in line for k, v in labels.items()):
            try:
                return float(line.rsplit(" ", 1)[1])
            except (IndexError, ValueError):
                continue
    return None


def node_for_pod(namespace, pod):
    doc = run_json_raw(f"/api/v1/namespaces/{namespace}/pods/{pod}")
    if not doc:
        return None
    return doc.get("spec", {}).get("nodeName")


def main():
    now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    rows = []

    # -- bazel-remote (ghost only) --------------------------------------
    # bst-artifact-server is nodeSelector-pinned to ghost (manifests/bst-artifact-server.yaml)
    status = run_json_raw("/api/v1/namespaces/argo/services/bst-artifact-server:8080/proxy/status")
    if status:
        used = status.get("CurrSize")
        capacity = status.get("MaxSize")
        rows.append({
            "id": "bazel-remote-ghost",
            "node": "ghost",
            "drive": "/var/mnt/ghost-data",
            "cache_backend": "bazel-remote",
            "storage_type": "cas",
            "used_bytes": used,
            "capacity_bytes": capacity,
            "percent": round(100 * used / capacity, 3) if used is not None and capacity else None,
            "source_url": "http://192.168.1.102:32746 (kubectl proxy: services/bst-artifact-server:8080/proxy/status)",
            "collected_at": now,
            "derivation": "live /status endpoint (CurrSize / MaxSize) via kubectl API-server service proxy",
            "state": "available",
            "state_reason": None,
        })
    else:
        rows.append({
            "id": "bazel-remote-ghost",
            "node": "ghost",
            "drive": "/var/mnt/ghost-data",
            "cache_backend": "bazel-remote",
            "storage_type": "cas",
            "used_bytes": None,
            "capacity_bytes": None,
            "percent": None,
            "source_url": None,
            "collected_at": now,
            "derivation": None,
            "state": "unavailable",
            "state_reason": "cluster unreachable or bst-artifact-server not responding on /status",
        })

    # -- Buildbarn 2-shard CAS/AC (ghost + exo-0) ------------------------
    for pod in ("storage-0", "storage-1"):
        node = node_for_pod("buildbarn", pod)
        metrics = run_raw(f"/api/v1/namespaces/buildbarn/pods/{pod}:9980/proxy/metrics")
        for storage_type, capacity, block_count in (
            ("cas", CAS_CAPACITY_BYTES, CAS_BLOCKS),
            ("ac", AC_CAPACITY_BYTES, AC_BLOCKS),
        ):
            row_id = f"buildbarn-{pod}-{storage_type}"
            if node is None or metrics is None:
                rows.append({
                    "id": row_id,
                    "node": None,
                    "drive": None,
                    "cache_backend": "buildbarn",
                    "storage_type": storage_type,
                    "used_bytes": None,
                    "capacity_bytes": capacity,
                    "percent": None,
                    "source_url": None,
                    "collected_at": now,
                    "derivation": None,
                    "state": "unavailable",
                    "state_reason": f"cluster unreachable or {pod} not scheduled/responding on diagnostics port 9980",
                })
                continue

            allocations = parse_metric_value(
                metrics, "buildbarn_blobstore_block_device_backed_block_allocator_allocations_total",
                {"storage_type": storage_type},
            )
            releases = parse_metric_value(
                metrics, "buildbarn_blobstore_block_device_backed_block_allocator_releases_total",
                {"storage_type": storage_type},
            )
            block_size = capacity / block_count
            used = None
            if allocations is not None and releases is not None:
                used = max(0.0, allocations - releases) * block_size
                used = min(used, float(capacity))

            rows.append({
                "id": row_id,
                "node": node,
                "drive": "/var/mnt/ghost-data",
                "cache_backend": "buildbarn",
                "storage_type": storage_type,
                "used_bytes": used,
                "capacity_bytes": capacity,
                "percent": round(100 * used / capacity, 3) if used is not None else None,
                "source_url": f"kubectl proxy: pods/{pod}:9980/proxy/metrics (namespace buildbarn)",
                "collected_at": now,
                "derivation": (
                    "(allocations_total - releases_total) * block_size, where block_size = "
                    f"{capacity} bytes / {block_count} blocks (manifests/buildbarn-config.yaml); "
                    "block-device-backed storage is fixed-size on disk, so this is an estimate "
                    "of logical fill, not physical allocation"
                ),
                "state": "available" if used is not None else "unavailable",
                "state_reason": None if used is not None else "allocator counters not readable from /metrics",
            })

    doc = {
        "schema_version": 1,
        "_meta": {
            "page": "index",
            "description": "BuildStream / remote-execution cache usage across both cache drives (ghost, exo-0)",
            "generated_at": now,
            "starter_artifact": False,
            "status": "live",
        },
        "summary_metrics": [],
        "rows": rows,
    }

    with open(OUT_PATH, "w") as f:
        json.dump(doc, f, indent=2)
        f.write("\n")


if __name__ == "__main__":
    main()
