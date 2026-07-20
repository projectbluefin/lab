# Operations

Use this directory for stable procedures and failure recovery. Load the skill
for the subsystem first when changing infrastructure; use these documents for
operator execution.

- [`../ops/bootstrap.md`](../ops/bootstrap.md) — one-time setup
- [`../ops/lab-operations.md`](../ops/lab-operations.md) — routine operations
- [`../ops/RUNBOOK.md`](../ops/RUNBOOK.md) — symptom-based recovery
- [`../ops/architecture.md`](../ops/architecture.md) — current static topology
- [`../ops/k3s-tuning.md`](../ops/k3s-tuning.md) — tuning constraints
- [`../ops/merge-queue.md`](../ops/merge-queue.md) — repository merge operations

The existing `docs/ops/` paths remain canonical during migration. New
procedures should be added there until the path migration is completed; do not
create a second copy under `docs/operations/`.
