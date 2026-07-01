# Flatcar Kernel Lifecycle Design (exo-0 7.1 canary)

Date: 2026-06-27

## Goal

Finish the lab design for upstream Flatcar kernel intake so exo-0 validates Linux 7.1 first, then the same release is safely consumable by other Flatcar nodes through the cluster-hosted update stack.

If exo-0 is being reinstalled, treat the old node object as disposable and wait for the replacement to rejoin before running the canary gate.

## Scope

- Keep Nebraska and payload hosting on-cluster.
- Keep rollout operationally simple for a 3-node lab.
- Cover the full lifecycle: detect → build → register → canary gate → promote/rollback.

## Chosen approach

Single update group (`GROUP=stable`) for all Flatcar nodes, with a **policy-level canary gate**:

1. New kernel candidate is built and registered.
2. exo-0 must pass a 24h health window.
3. Candidate is promoted as the active stable target.
4. Failure in gate keeps last-known-good target active.

This avoids multi-ring config complexity while preserving staged risk control.

## Architecture

### Existing components (kept)

- `manifests/flatcar-update-nebraska.yaml`: Nebraska with `-host-flatcar-packages`
- `manifests/flatcar-update-configurator.yaml`: writes `/etc/flatcar/update.conf`
- `argo/workflow-templates/flatcar-kernel-build.yaml`: builds payload and registers package
- `manifests/flatcar-kernel-poller.yaml`: detects upstream stable kernel changes

### Lifecycle states

1. **Detect**
   - Poll `kernel.org/releases.json` for stable version drift.
2. **Build/Register**
   - Run `flatcar-kernel-build` with detected kernel version.
   - Register package metadata in Nebraska (`url` base path + `filename`).
3. **Canary gate (exo-0, 24h)**
   - exo-0 node remains Ready.
   - `flatcar-update` namespace remains healthy.
   - at least one successful exo-0 update/reboot validation run.
4. **Promote**
   - Keep candidate as active stable package target for all nodes.
5. **Rollback**
   - Re-point to previous known-good package/version and re-trigger update checks.

## Promotion gate definition

The 24h gate is **pass/fail**:

- Pass: all three checks hold for the window.
- Fail: any check fails once; candidate is blocked.

Gate output should be explicit in workflow logs:

- `gate_status=pass` or `gate_status=fail`
- first failing signal and timestamp

## Debugging flow (ponytail style)

Use the shortest path first:

```bash
# 1. Is exo-0 on 7.1?
kubectl get node exo-0 -o jsonpath='{.status.nodeInfo.kernelVersion}{"\n"}'

# 2. Is the package in Nebraska?
curl -s "http://192.168.1.102:30802/api/v1/apps/e96281a6-d1af-4bde-9a0a-97b76e56dc57/packages" | jq '.[-5:]'

# 3. Is update.conf pointed to Nebraska?
POD=$(kubectl get pods -n flatcar-update -l app=flatcar-update-configurator -o wide | awk '/exo-0/ {print $1; exit}')
kubectl exec -n flatcar-update "$POD" -- nsenter --target 1 --mount -- cat /etc/flatcar/update.conf
```

## Error handling and rollback

- Build failure: candidate never registered; no rollout change.
- Registration failure: payload exists but package unavailable to clients; retry register step only.
- Gate failure: candidate blocked; keep previous stable.
- Promotion failure: no config mutation until promotion write succeeds.

No silent fallback. Every failure path emits a concrete reason.

## Test plan

1. Force a candidate build (7.1.x).
2. Validate exo-0 reaches target kernel and reboots cleanly.
3. Hold 24h gate; verify no unhealthy periods in required signals.
4. Promote and confirm other Flatcar nodes resolve same package.
5. Run one rollback drill to previous package.

## Documentation updates linked to this design

- `docs/skills/flatcar-node-onboarding.md`
- `docs/agent-cheatsheet.md`

## References

- Flatcar update server configuration and `update.conf` (`SERVER`, `GROUP`):
  - https://www.flatcar.org/docs/latest/nebraska/managing-updates
  - https://www.flatcar.org/docs/latest/setup/releases/update-conf
  - https://www.flatcar.org/docs/latest/setup/releases/switching-channels
- Argo CronWorkflow concurrency semantics (`Forbid`):
  - https://github.com/argoproj/argo-workflows/blob/main/docs/cron-workflows.md
