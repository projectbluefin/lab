"""Exercise the generated colord build command with real ELF dependencies."""

import ast
import os
import shutil
import subprocess
from pathlib import Path

import pytest
import yaml


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml"
TOOLS = ("bash", "cc", "ninja", "jobserver_pool.py")


def _colord_build_command():
    workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
    source = next(
        template["script"]["source"]
        for template in workflow["spec"]["templates"]
        if "GBM_JUNCTION_PATCH" in template.get("script", {}).get("source", "")
    )
    preparation = source.split("<<'GBM_JUNCTION_PATCH'\n", 1)[1].split(
        "\nGBM_JUNCTION_PATCH", 1
    )[0]
    assignment = next(
        node
        for node in ast.parse(preparation).body
        if isinstance(node, ast.Assign)
        and any(isinstance(target, ast.Name) and target.id == "patch5_content"
                for target in node.targets)
    )
    patch = ast.literal_eval(assignment.value)
    additions = "\n".join(
        line[1:] for line in patch.splitlines()
        if line.startswith("+") and not line.startswith("+++")
    )
    return yaml.safe_load(additions)["meson-build"]


@pytest.mark.skipif(
    any(shutil.which(tool) is None for tool in TOOLS),
    reason="requires the real BuildStream runner jobserver, C compiler, and Ninja",
)
@pytest.mark.parametrize("inherited_path", [None, "", "existing"])
def test_generated_command_loads_build_tree_and_inherited_libraries(tmp_path, inherited_path):
    # Keep the libraries absent until Ninja builds them, as in a clean build.
    build = tmp_path / "_builddir"
    libraries = build / "lib/colord"
    libraries.mkdir(parents=True)
    existing = tmp_path / "existing"
    existing.mkdir()
    (tmp_path / "inherited.c").write_text("int inherited(void) { return 2; }\n")
    (tmp_path / "colord.c").write_text("int colord(void) { return 40; }\n")
    (tmp_path / "client.c").write_text(
        '#include <stdio.h>\n'
        'int colord(void); int inherited(void);\n'
        'int main(void) { printf("%d\\n", colord() + inherited()); return 0; }\n'
    )
    # Empty/unset-path cases have both dependencies in the bounded build tree.
    inherited_dir = existing if inherited_path == "existing" else libraries
    (build / "build.ninja").write_text(
        "rule compile\n  command = cc -o $out $in $args\n"
        "rule generate\n  command = ./client > result\n"
        "build lib/colord/libcolord.so.2: compile ../colord.c\n"
        "  args = -shared -fPIC -Wl,-soname,libcolord.so.2\n"
        f"build {inherited_dir}/libinherited.so.1: compile ../inherited.c\n"
        "  args = -shared -fPIC -Wl,-soname,libinherited.so.1\n"
        "build client: compile ../client.c | lib/colord/libcolord.so.2 "
        f"{inherited_dir}/libinherited.so.1\n"
        f"  args = -Llib/colord -L{inherited_dir} -l:libcolord.so.2 -l:libinherited.so.1\n"
        "build result: generate | client\ndefault result\n"
    )
    env = dict(os.environ, JOBS="2")
    env.pop("LD_LIBRARY_PATH", None)
    if inherited_path is not None:
        env["LD_LIBRARY_PATH"] = str(existing) if inherited_path == "existing" else ""
    result = subprocess.run(
        ["bash", "-euc", _colord_build_command()],
        cwd=tmp_path, env=env, capture_output=True, text=True, check=False, timeout=30,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert (build / "result").read_text() == "42\n"
