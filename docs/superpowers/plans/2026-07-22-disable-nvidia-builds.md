# Disable NVIDIA Builds Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Disable NVIDIA variants in the distributed Dakota and COSMIC build workflows while retaining successful default image builds and documenting the resulting build boundary.

**Architecture:** Remove NVIDIA DAG tasks and NVIDIA-specific workflow descriptions/outputs from the distributed build templates. Keep the default build task and existing cache/registry behavior unchanged. Update only documentation and tests that claim the distributed workflows build NVIDIA variants; retain historical/product catalog references unless they describe these specific workflows.

**Tech Stack:** Argo Workflow YAML, Markdown, Python pytest, Git history/data inspection.

## Global Constraints

- Preserve the existing default (`bluefin`, `cosmic`) build paths.
- Do not alter unrelated uncommitted BuildStream/cache changes.
- Do not remove product-level NVIDIA image metadata unless it directly describes the disabled distributed workflows.
- Validate YAML/workflow structure and run the focused unit tests plus the site build if practical.

---

### Task 1: Remove NVIDIA workflow branches

**Files:**
- Modify: `argo/workflow-templates/dakota-build-pipeline.yaml`
- Modify: `argo/workflow-templates/cosmic-build-pipeline.yaml`

**Interfaces:**
- Consumes: Existing Argo DAG/task definitions.
- Produces: Distributed workflows that build and export only their default image variant.

- [ ] **Step 1: Inspect the full DAG task blocks and descriptions**

Run:
```bash
sed -n '1,130p' argo/workflow-templates/dakota-build-pipeline.yaml
sed -n '1,125p' argo/workflow-templates/cosmic-build-pipeline.yaml
```

Expected: identify the description/output claims and the `build-bluefin-nvidia` / `build-cosmic-nvidia` task blocks.

- [ ] **Step 2: Edit the workflow descriptions and remove only NVIDIA tasks**

For Dakota, describe one default build/export (`dakota:testing`) and remove the `build-bluefin-nvidia` DAG task. For COSMIC, describe the default `cosmic` build/export and remove the `build-cosmic-nvidia` DAG task. Leave shared templates, cache configuration, parameters, and default tasks unchanged.

- [ ] **Step 3: Verify no disabled task or output remains in these workflows**

Run:
```bash
rg -n -i "nvidia|build-bluefin-nvidia|build-cosmic-nvidia" argo/workflow-templates/dakota-build-pipeline.yaml argo/workflow-templates/cosmic-build-pipeline.yaml
```

Expected: no matches.

- [ ] **Step 4: Parse the YAML**

Run:
```bash
python3 - <<'PY'
import yaml
for path in [
    'argo/workflow-templates/dakota-build-pipeline.yaml',
    'argo/workflow-templates/cosmic-build-pipeline.yaml',
]:
    with open(path) as f:
        doc = yaml.safe_load(f)
    assert doc['kind'] == 'WorkflowTemplate'
    assert doc['spec']['templates']
    print(path, 'OK')
PY
```

Expected: both files print `OK`.

### Task 2: Update documentation and regression tests

**Files:**
- Modify: `docs/ops/lab-operations.md`
- Modify: `docs/reference/agent-cheatsheet.md`
- Modify: `docs/reference/workflow-reference.md`
- Modify: `docs/reference/WORKFLOWS.md`
- Modify: `docs/skills/cluster-tooling/buildstream.md`
- Modify: `tests/unit/test_workflow_defaults.py`

**Interfaces:**
- Consumes: The workflow behavior from Task 1 and existing build evidence in `docs/data/history/build-runs.ndjson` / `docs/data/factory-stats.json`.
- Produces: Documentation that says the distributed Dakota/COSMIC workflows build default variants only, plus tests that prevent NVIDIA tasks from returning.

- [ ] **Step 1: Locate claims tied to these workflows**

Run:
```bash
rg -n -C 2 -i "bluefin \+ nvidia|bluefin-nvidia|cosmic-nvidia|variant.*nvidia|dakota-nvidia" docs/ops/lab-operations.md docs/reference/agent-cheatsheet.md docs/reference/workflow-reference.md docs/reference/WORKFLOWS.md docs/skills/cluster-tooling/buildstream.md tests/unit/test_workflow_defaults.py
```

- [ ] **Step 2: Update only workflow-specific documentation**

Change command descriptions and workflow references from “default + NVIDIA” to “default only.” Add one concise note that NVIDIA image variants remain outside these distributed workflows and are not part of the clean-build validation path. Do not rewrite historical build matrices or product catalog entries that describe separately published upstream variants.

- [ ] **Step 3: Replace the NVIDIA-positive unit assertion with a default-only regression assertion**

In `tests/unit/test_workflow_defaults.py`, retain the existing Dakota workflow loading/fixture pattern and assert that the workflow contains the default build task and `oci/bluefin.bst`, while asserting that `build-bluefin-nvidia`, `oci/bluefin-nvidia.bst`, and `dakota-nvidia` are absent. Add the equivalent COSMIC absence assertion if the test module already loads that workflow; otherwise add a focused test using the existing YAML-loading style.

- [ ] **Step 4: Run focused tests**

Run:
```bash
pytest -q tests/unit/test_workflow_defaults.py
```

Expected: all tests pass.

### Task 3: Measure and report distributed build duration

**Files:**
- Read: `docs/data/history/build-runs.ndjson`
- Read: `docs/data/factory-stats.json`
- Read: `docs/skills/cluster-tooling/buildstream.md`

**Interfaces:**
- Consumes: Recorded run timestamps/durations and the verified build notes.
- Produces: A precise report distinguishing recorded distributed workflow duration from local fallback duration when available.

- [ ] **Step 1: Query recorded build-run entries**

Run:
```bash
python3 - <<'PY'
import json
from pathlib import Path
for line in Path('docs/data/history/build-runs.ndjson').read_text().splitlines():
    row = json.loads(line)
    text = json.dumps(row).lower()
    if 'dakota' in text or 'cosmic' in text or 'buildstream' in text or 'build-barn' in text:
        print(json.dumps(row, sort_keys=True))
PY
```

- [ ] **Step 2: Check the verified narrative for elapsed-time evidence**

Run:
```bash
rg -n -i "duration|minutes|min|elapsed|build took|distributed|verified" docs/skills/cluster-tooling/buildstream.md docs/data/history/build-runs.ndjson docs/data/factory-stats.json
```

- [ ] **Step 3: Report only measured evidence**

State the distributed build duration with its source and timestamp. If the repository contains only start/end timestamps or only a qualitative note, calculate from the timestamps or report that no exact distributed duration is recorded; do not infer it from the local fallback or workflow timeout.

### Task 4: Validate final change set

**Files:**
- Read: all modified files from Tasks 1–2

- [ ] **Step 1: Run workflow regression tests and documentation validation**

Run:
```bash
pytest -q tests/unit/test_workflow_defaults.py
python3 scripts/validate-docs.py
```

Expected: both commands pass.

- [ ] **Step 2: Run the site build if dependencies are available**

Run:
```bash
npm run build
```

Expected: successful production build.

- [ ] **Step 3: Review the diff without staging unrelated work**

Run:
```bash
git diff --check
git diff --stat
git diff -- argo/workflow-templates docs/ops/lab-operations.md docs/reference docs/skills/cluster-tooling/buildstream.md tests/unit/test_workflow_defaults.py
```

Expected: only the approved NVIDIA-disable/docs/test changes are included in the reviewed paths; unrelated pre-existing worktree changes remain untouched.
