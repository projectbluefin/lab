---
name: agent-cheatsheet-pr-queue
description: >
  PR queue / verification report notes extracted from the agent cheatsheet.
---

# PR Queue Mode — Vanguard Lab Strike Report


Mandatory gate for `knuckle`, `dakota`, and this repo's PRs.

1. Run the lab loop end-to-end — `just run-tests-tag testing` minimum, `just run-tests-matrix` for high-risk changes.
2. Collect **real evidence** using CLI tools:
   - Workflow status/steps → `argo get -n argo <name>` / `argo list -n argo`
   - Log output → `argo logs -n argo <name>`
   - Pod state → `kubectl get pods -n argo`
   - VMI state only for VM-backed lanes → `kubectl get vmi -A`
3. Keep the PR comment minimal: what ran, pass/fail, and blockers only.
4. Only then apply `agent-tested` and approve / queue.

Hard exit checklist:

- [ ] Real lab evidence exists for the lane under test.
- [ ] Evidence was collected via CLI tools (`argo`, `kubectl`).
- [ ] The entire loop was tested, not isolated commands.
- [ ] A canonical Vanguard report with real data is posted on the PR.
- [ ] Any blocker is filed as an issue in the owning repo.

---
