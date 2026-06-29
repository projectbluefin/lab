# Bootc Image Expansion Design (lab ingest wave 1)

Date: 2026-06-28

## Goal

Expand lab image ingestion so the requested streams are continuously polled and automatically validated with a full QA matrix on digest change.

## Scope

- Add/expand polling for:
  - bluefin
  - bluefin-lts
  - aurora
  - bazzite
  - kinoite
  - fedora bootc (stable + testing)
  - dakota
  - akmods
  - ublue main streams: `bluefin:main`, `aurora:main`, `bazzite:main`
- Track `stable`, `testing`, and `latest` where available.
- On digest change, trigger **full matrix** validation (`smoke,common,developer,software,system`).

Out of scope:
- Auto-discovery of arbitrary tags/images.
- Refactoring all existing pollers into a new generic engine in this wave.

## Chosen approach

Use a fast GitOps expansion:

1. Add explicit CronWorkflows per requested stream.
2. Extend digest state keys in `image-polling-state`.
3. Reuse current image-poller workflow behavior and wiring.
4. Keep one-stream-per-cron isolation.

This is the lowest-risk path and matches existing lab operations patterns.

## Architecture

### Existing components (kept)

- `argo/workflow-templates/image-poller.yaml`
- Existing `manifests/image-poll-*.yaml` CronWorkflows
- `manifests/image-polling-state.yaml`
- Existing QA pipeline templates used by image-poller triggers

### New/updated components

- Add new/expanded `manifests/image-poll-*.yaml` resources for the requested streams/tags.
- Add matching keys to `manifests/image-polling-state.yaml`.
- Ensure each new stream triggers full-matrix suites when digest changes.

## Data flow

1. CronWorkflow polls one stream digest.
2. If digest unchanged:
   - exit success
   - do not trigger QA workflow
3. If digest changed:
   - update that stream key in `image-polling-state`
   - submit QA workflow with full matrix suites
   - include stream/tag metadata for traceability

## Scheduling and overlap policy

- Keep `concurrencyPolicy: Forbid` on pollers to prevent overlapping runs for the same stream.
- Keep each stream independent so one bad source does not block others.

## Error handling

- Poll failures fail the poll workflow and leave prior digest state unchanged.
- QA submission failures are explicit failures (no success-shaped fallback).
- No silent retries beyond workflow-level retry semantics already in templates.

## Validation plan

1. `just lint`
2. Confirm ArgoCD app health (`testing-lab-infra`) after merge/sync.
3. Manually submit 1-2 new pollers:
   - verify no-change path (no matrix workflow)
   - verify changed-digest path (matrix workflow appears)
4. Verify state ConfigMap mutates only for changed streams.

## Rollout

1. Merge manifest updates to `main` (GitOps).
2. Wait for ArgoCD reconcile.
3. Smoke-check the newly added streams in Argo list/logs.
4. Monitor for first digest-change cycle and full-matrix trigger behavior.

## Risks and mitigations

- **Risk:** too many simultaneous matrix runs during broad upstream churn.
  - **Mitigation:** per-stream `Forbid`, keep independent stream schedules, tune cadence if backlog appears.
- **Risk:** unsupported/nonexistent tag on a source stream.
  - **Mitigation:** isolate by stream; one failing poller does not block others.
- **Risk:** state drift from manual mutations.
  - **Mitigation:** GitOps-owned manifests and explicit state-key mapping.

## References

- Argo CronWorkflow options (`concurrencyPolicy`, schedules):
  - https://github.com/argoproj/argo-workflows/blob/main/docs/cron-workflows.md
- Argo metrics behavior for concurrency policy:
  - https://github.com/argoproj/argo-workflows/blob/main/docs/metrics.md
