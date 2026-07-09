# Distributed BST Builds over USB4 with Ethernet Cache-Only Fallback

Date: 2026-07-09
Status: approved

## Problem

Buildbarn remote execution (RE) fans out action input roots across nodes. Over
the ghost<->exo-0 USB4 link (40 Gbps) this is the intended distributed-build
path. Over the 1 GbE LAN it saturates the network and starves the control
plane ("buildstream over ethernet will crash the network"). The USB4 link is
also unreliable at the EC/PD level (see RUNBOOK.md 2026-07-08/09 incidents),
so the pipeline must degrade gracefully: full RE when the link is up,
Buildbarn as artifact/source cache only when it is down.

## Decisions

- Ethernet mode is **cache-only**: bst builds locally in the workflow pod;
  Buildbarn is used only as artifact push + source cache (bounded transfers).
- Mid-build link drop: the build step **retries and forces cache-only** on any
  retry attempt, reusing artifacts already pushed to the cache.
- Link state detection is a **node-annotation DaemonSet** (Approach A):
  single source of truth, GitOps-managed, reusable by other pipelines.

## Components

### 1. usb4-link-monitor DaemonSet (`manifests/usb4-link-monitor.yaml`)

- Namespace `kube-system`, nodeAffinity to hostnames `ghost`, `exo-0`.
- `hostNetwork: true`, image `cgr.dev/chainguard/kubectl:latest-dev`.
- Every 15 s evaluates: `/sys/class/net/thunderbolt0/operstate == up` AND
  `carrier == 1` AND TCP probe `/dev/tcp/<peer-tb-ip>/10250` (kubelet).
  Peer map: ghost -> 10.99.0.2, exo-0 -> 10.99.0.1.
- Patches its own node annotation `lab.projectbluefin.io/usb4-link: up|down`
  only on state change.
- ServiceAccount `usb4-link-monitor` + ClusterRole (`nodes` get/patch).

### 2. BuildStream config split (`manifests/buildstream-remote-cache-config.yaml`)

- `dakota-buildstream.conf` becomes the shared baseline with Buildbarn artifact
  writes and upstream read-only fallback caches (`gbm.gnome.org`,
  `cache.freedesktop-sdk.io`, `cache.projectbluefin.io`).
- The per-project override block keeps `override-project-caches: false` so the
  project can still reuse upstream bootstrap artifacts when the USB4 link is
  down or when Buildstream is running in cache-only mode.
- New key `remote-execution.conf`: the per-project RE snippet body the pipeline
  appends when RE mode is selected.
- For checkout trees that track upstream `gnome-build-meta` or
  `freedesktop-sdk` patch queues, the workflow mirrors those patch queues into
  the checkout before the build so the artifact keys match the upstream cache
  identities.

### 3. Pipeline mode selection (`argo/workflow-templates/dakota-build-pipeline.yaml`)

- New workflow parameter `build-mode`: `auto` (default) | `cache-only` | `re`.
- New `detect-build-mode` DAG task (kubectl image) runs first, outputs `mode`:
  - `re` if `build-mode=re`, or `build-mode=auto` AND both node annotations
    are `up`; else `cache-only`.
- `bst-build` consumes `mode`; inside the step, RE is used only when
  `mode == re` AND `{{retries}} == 0` — any retry forces cache-only.
- Existing `retryStrategy: limit 2` already covers the retry path.

### 4. Network layer (no changes)

Table-40 policy routes ride on the NM thunderbolt0 profile; when the link
drops NM withdraws them and flannel host-gw ethernet routes take over
automatically. Cache traffic (bounded) is safe on 1 GbE.

## Error handling

- Annotation missing/unreadable -> treated as `down` -> cache-only (fail safe).
- Detection step failure -> workflow fails fast before builds start.
- Link drops mid-build -> bst RE actions fail -> step retries cache-only.
- Monitor pod dead -> annotation goes stale; stale `up` with dead link is the
  worst case and is bounded by the bst retry -> cache-only path.

## Testing

1. Link up: submit dakota build, verify generated config contains RE block,
   actions execute on bb workers on both nodes.
2. `build-mode=cache-only`: verify no RE block, artifacts still push/pull.
3. Pause a monitor pod, set annotation `down`, verify `auto` picks cache-only.
