import ast
import types
from pathlib import Path


WORKFLOW_TEMPLATE = Path("argo/workflow-templates/k8sgpt-on-demand.yaml")


def _extract_script_source() -> str:
    lines = WORKFLOW_TEMPLATE.read_text(encoding="utf-8").splitlines()
    block = []
    in_source = False

    for line in lines:
        if not in_source:
            if line.strip() == "source: |":
                in_source = True
            continue
    
        if not line.strip():
            block.append("")
            continue
        if not line.startswith(" " * 10):
            break
        block.append(line[10:])

    assert block, "failed to extract inline Python from k8sgpt-on-demand workflow"
    return "\n".join(block)


def _load_script_module() -> types.ModuleType:
    source = _extract_script_source()
    tree = ast.parse(source, filename=str(WORKFLOW_TEMPLATE))
    module = types.ModuleType("k8sgpt_on_demand")
    filtered_body = [
        node
        for node in tree.body
        if isinstance(node, (ast.Import, ast.ImportFrom, ast.FunctionDef))
    ]
    exec(
        compile(ast.Module(body=filtered_body, type_ignores=[]), str(WORKFLOW_TEMPLATE), "exec"),
        module.__dict__,
    )
    return module


def test_workflow_exposes_normalize_results_helper():
    module = _load_script_module()

    assert hasattr(module, "normalize_results")


def test_normalize_results_treats_null_results_as_empty_list():
    module = _load_script_module()

    normalized = module.normalize_results(
        {"status": "OK", "problems": 0, "results": None},
        "argocd/argocd-dex-server",
    )

    assert normalized["results"] == []


def test_normalize_results_filters_only_ignored_services():
    module = _load_script_module()

    normalized = module.normalize_results(
        {
            "results": [
                {"kind": "Service", "name": "argocd/argocd-dex-server"},
                {"kind": "Service", "name": "bluefin-test/api"},
                {"kind": "Pod", "name": "broken-pod"},
            ]
        },
        "argocd/argocd-dex-server",
    )

    assert normalized["results"] == [
        {"kind": "Service", "name": "bluefin-test/api"},
        {"kind": "Pod", "name": "broken-pod"},
    ]
