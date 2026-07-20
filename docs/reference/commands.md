# Commands

This is the short command router. The [`Justfile`](../../Justfile) is the
source of truth for recipe names and arguments.

## Validation

```bash
just lint
python3 scripts/validate-docs.py
```

## Routine operations

```bash
just list-workflows
just logs
just list-vms
just argocd-status
just argocd-sync
```

## Test execution

```bash
just run-tests
just run-tests-tag testing
just run-tests-matrix
just run-flatcar-smoke
```

## Detailed selector and failure triage

Use [`agent-cheatsheet.md`](agent-cheatsheet.md) for the complete command
selector and symptom-to-next-command table. Use [`../ops/RUNBOOK.md`](../ops/RUNBOOK.md)
for recovery decisions. Do not copy live command values into this router when
the Justfile or a workflow owns them.
