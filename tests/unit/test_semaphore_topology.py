"""Semaphore topology rules for argo/ (spec.parallelism is not inherited through templateRef)."""

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
LIMITS = {
    k: int(v)
    for k, v in yaml.safe_load((ROOT / "manifests/workflow-semaphores.yaml").read_text())["data"].items()
}
DOCS = [
    doc
    for path in sorted((ROOT / "argo").rglob("*.yaml"))
    for doc in yaml.safe_load_all(path.read_text())
    if isinstance(doc, dict) and isinstance(doc.get("spec"), dict)
]
TEMPLATES = [(doc["metadata"]["name"], t) for doc in DOCS for t in doc["spec"].get("templates") or []]


def refs(sync):
    return [s["configMapKeyRef"] for s in (sync or {}).get("semaphores") or [] if "configMapKeyRef" in s]


def test_semaphore_keys_exist_in_workflow_semaphores():
    blocks = [d["spec"].get("synchronization") for d in DOCS] + [t.get("synchronization") for _, t in TEMPLATES]
    for ref in (r for block in blocks for r in refs(block)):
        assert ref["name"] == "workflow-semaphores" and ref["key"] in LIMITS, ref


def test_fanout_parallelism_stays_under_semaphore_limit():
    holders = {f"{wf}/{t['name']}": [r["key"] for r in refs(t.get("synchronization"))] for wf, t in TEMPLATES}
    checked = 0
    for wf, template in TEMPLATES:
        tasks = (template.get("dag") or {}).get("tasks") or [s for g in template.get("steps") or [] for s in g]
        for task in tasks:
            ref = task.get("templateRef")
            if not ref or not any(k in task for k in ("withItems", "withParam", "withSequence")):
                continue
            for key in holders.get(f"{ref['name']}/{ref['template']}", []):
                checked += 1
                parallelism = template.get("parallelism")
                assert parallelism is not None and parallelism < LIMITS[key], (
                    f"{wf}/{template['name']} fans out to {key} without parallelism < {LIMITS[key]}"
                )
    assert checked, "expected at least one semaphore fan-out (dakota-qa-pipeline)"
