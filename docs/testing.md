# Testing

Testing is layered. Use the narrowest layer that proves the change, then run
broader checks when the change crosses a boundary.

## Local validation

```bash
python3 scripts/validate-docs.py
just lint
```

Dashboard changes additionally require:

```bash
npm ci
npm run build
```

## Test procedures

- Workflow authoring: [`skills/argo-workflows/SKILL.md`](skills/argo-workflows/SKILL.md)
- Test authoring and debugging: [`skills/test-authoring/SKILL.md`](skills/test-authoring/SKILL.md)
- Cluster-backed validation: [`ops/lab-operations.md`](ops/lab-operations.md)
- Exact workflow parameters: [`reference/WORKFLOWS.md`](reference/WORKFLOWS.md)
- Command selection: [`reference/agent-cheatsheet.md`](reference/agent-cheatsheet.md)

## Evidence

A test claim must identify the command or workflow, the input digest or
revision when relevant, and the observable result. Do not present generated
lab evidence as a general release guarantee without reading the release
verdict definition and data contract.
