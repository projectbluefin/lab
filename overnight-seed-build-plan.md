# Overnight Dakota Seed Build — Next Steps

## User requirements
1. Update all docs/templates that still reference `dakota:latest`; they must reference `dakota:testing` (the only production tag).
2. Bump BuildStream concurrency to 16 threads / fetchers / builders in `dakota-build-pipeline.yaml`.
3. Evaluate whether the old run (`dakota-build-pipeline-z7m77`) local cache can be copied to seed a new build.
4. Kick off a new Dakota seed build overnight with the improved parallel + proxy + threaded template.

## Preparations already landed on `main`
- `manifests/bst-cache-proxy.yaml` deployed a unified pull-through HTTP cache for BuildStream remotes.
- `argo/workflow-templates/dakota-build-pipeline.yaml` now routes all HTTP artifact/source cache traffic through `bst-cache-proxy`.

## Remaining work

### 1. Fix `dakota:latest` references
Files known to contain the string:
- `/agents.md:255` — `image-poll-dakota.yaml` label says "poll dakota:latest digest".
- Search `argo/` for any template still defaulting to `latest` (e.g., `image-poll-dakota.yaml`, `dakota-container-qa-pipeline`). A previous commit already changed `dakota-qa-pipeline` default to `testing`.
- Search `docs/` and `common/` for stale references.
Action: replace user-facing references with `:testing` and, where a workflow genuinely needs `latest`, add a comment explaining why or delete the workflow.

### 2. Concurrency bump
In `argo/workflow-templates/dakota-build-pipeline.yaml`, change the two `bst` invocations:
- `bst --config "${BST_CONF}" --no-interactive build "${ELEMENT}"`
- `bst --config "${BST_CONF}" --no-interactive -o x86_64_v3 true artifact checkout ...`
Add: `--fetchers 16 --builders 16 --pushers 16 --max-jobs 16`
(Adjust down if BuildBarn workers or driver memory prove unstable.)

### 3. Copy old-run cache for seeding
- Old run: `dakota-build-pipeline-z7m77` (Running, ~4h14m, build-bluefin lane).
- Bound PVC: `dakota-build-pipeline-z7m77-bst-cache-dakota` (200Gi, `local-path`, RWO).
- The second (`-dakota-nvidia`) PVC exists but is still Pending.
- BuildStream has already pushed fetched artifacts to BuildBarn (`push: true`), so the main overnight speedup should come from BuildBarn/cache-proxy warming, not from the local PVC.
- **Copy feasibility:** `local-path` storage class does not support snapshots/cloning. To copy the local cache we would need to stop `z7m77`, mount its PVC in a Job, and copy `/root/.cache/dakota` to a new PVC or hostPath seed. This is possible but heavy (potentially hundreds of GB) and would delay the overnight start.
- **Decision:** unless the copy can complete quickly, do not block the overnight run on it. Use BuildBarn + proxy as the warm cache.

### 4. Kick off overnight seed build
- `argo stop -n argo dakota-build-pipeline-z7m77` to free the `bst-build` semaphore and resources.
- `argo submit -n argo dakota-iteration2-manifest.yaml` (or a fresh manifest using the updated template) with:
  - `ref: testing`
  - `build-mode: re`
  - `lock-key: bst-build-manual` (to avoid blocking production runs)
- The background `drive-dakota-loop.sh` is currently polling `z7m77`; after it is stopped, the script detects terminal phase, records failure/cancel, and would not auto-submit iteration 2. Either update the script or submit the new run manually.
- Update `dakota-iteration1-report.md` with the cancellation reason.

## Risks
- Cancelling `z7m77` loses ~4h of cold-fetch CPU, but BuildBarn retains pushed artifacts.
- New proxy may have teething issues; verify it returns 200/404 before submitting the build.
- `local-path` clone not supported; if we later decide we must copy the old PVC, plan extra time.

## Proof target
After the overnight run succeeds, run:
`skopeo inspect --tls-verify=false docker://192.168.1.102:30500/dakota:testing`
and ensure the Created timestamp and digest are fresh.
