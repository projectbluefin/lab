---
name: dakota-pr-review
description: >
  Review Dakota pull requests through the lab-backed workflow: dispatch the Dakota
  QA run, keep maintainer decisions tied to fresh evidence, and hand off a clear
  operator summary. Use when a Dakota PR needs lab validation or triage.
metadata:
  context7-sources:
    - /argoproj/argo-workflows
    - /websites/github_en_actions
---

# Dakota PR Review — lab Skill

## When to Use

- Reviewing a PR in `projectbluefin/dakota`
- Deciding whether a Dakota PR is mergeable from the lab side
- Triaging a Dakota PR that failed, stalled, or never got a lab run

## When NOT to Use

- Reviewing non-Dakota repositories → `ci-tooling.md` or `argo-workflows.md`
- Merging a PR directly without a linked lab run
- Reworking the Dakota build pipeline itself → `argo-workflows.md`

---

## Core Process

1. Treat Dakota PRs as lab-backed changes, not just code-review items.
2. Let the PR poller dispatch `dakota-pr-batch-pipeline` for open Dakota PRs labeled `test-on-lab`; if it does not, trigger the workflow manually against the PR number or branch.
3. Track the workflow in Argo and keep the run linked to the PR.
4. Report the outcome as pass/fail with blockers, not a vague “looks fine.”
5. Return to maintainers with a short summary and the next action.

## Maintainer Policy

- Maintainers own merge and hold decisions; operators do not merge.
- A Dakota PR should stay in review until a fresh lab run is attached and the result is understood.
- If the workflow is missing, failing, or inconclusive, hold the PR and request a rerun or follow-up.
- Merge only when the evidence is green and no unresolved blocker remains.

## Operator Flow

1. Confirm the PR number, target branch, and commit SHA.
2. Check whether the PR poller already dispatched `dakota-pr-batch-pipeline`; if not, trigger it manually.
3. Watch the workflow to completion and capture the relevant logs or artifacts.
4. Report back with the PR, workflow link, outcome, and any blocker or next step.
5. If the PR needs changes, leave the review open and hand the exact failure back to the author or maintainer.

## Common Rationalizations

- "The PR is fine; I don't need a lab run." — Dakota changes still need fresh evidence before a maintainer decision.
- "The poller can be skipped if I am already looking at the PR." — The cluster-backed review lane is the point of the workflow; skipping it leaves the evidence gap unresolved.
- "I can just merge and fix later." — The workflow exists to keep review and evidence linked; merging without a fresh run is a regression risk.

## Red Flags

- No linked Dakota workflow for a PR that is otherwise ready to merge
- Workflow fails before the Dakota tests run
- Operator report lacks evidence or a workflow link
- PR is merged while the latest lab run is still failing or missing

## Verification

- [ ] The PR has a fresh Dakota workflow run attached
- [ ] The operator report includes the workflow link and outcome
- [ ] Maintainer decision is based on the evidence, not a guess
