# BuildStream fast-fail policy for lab workflows

The lab's BuildStream pipelines should fail fast once a run is known to be unusable instead of spending hours waiting in the shared `bst-build` queue or retrying dead-end pods.

## What changed

- Build preflight now runs before the workflow acquires the shared `bst-build` semaphore.
- A workflow that fails the BuildBarn/USB4 gate exits immediately instead of burning queue time.
- Build pods now use bounded per-pod deadlines and limited retries so failures
  surface quickly without killing cache-cold bootstrap builds. Dakota and
  Bluefin Server allow one retry, and none when an element's own build failed
  and BuildStream cached the failure (`bst-build.sh` exit 3): that retry would
  re-pull ~900 artifacts and fail the same way. See
  [Failed builds and Argo retries](../skills/cluster-tooling/buildstream.md#2-buildstream-client-config).

## Why this matters

The previous behavior let workflows:

- wait for hours behind the shared semaphore before the gate even ran;
- retry long-tail BuildStream failures multiple times;
- consume queue slots for runs that were already known to be invalid.

This policy is intentionally conservative: if a run cannot be admitted to the distributed BuildBarn path, or if a pod is clearly stuck in a long-tail failure, the workflow should end quickly and let the next run start.

## Templates updated

- `argo/workflow-templates/dakota-build-pipeline.yaml`
- `argo/workflow-templates/bluefin-server-build-pipeline.yaml`
