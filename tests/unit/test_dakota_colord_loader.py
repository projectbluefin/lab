"""Exercise the generated colord build command with real ELF dependencies."""

import ast
import io
import os
import shutil
import subprocess
from pathlib import Path

import pytest
import yaml


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml"
TOOLS = ("bash", "cc", "ninja", "jobserver_pool.py")


def _generated_patch(variable):
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
        and any(isinstance(target, ast.Name) and target.id == variable
                for target in node.targets)
    )
    return ast.literal_eval(assignment.value)


def _colord_build_command():
    additions = "\n".join(
        line[1:] for line in _generated_patch("patch5_content").splitlines()
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


@pytest.mark.parametrize("cpuinfo, expected", [
    (None, None),
    ("processor\t: 0\nmodel name\t: Example CPU\n", "Example CPU"),
    ("processor\t: 0\n", None),
])
def test_generated_mozjs_patch_handles_optional_cpuinfo(tmp_path, monkeypatch, cpuinfo, expected):
    # Minimal upstream source tree; neither a shared clone nor network access is required.
    element = tmp_path / "elements/sdk/mozjs.bst"
    element.parent.mkdir(parents=True)
    element.write_text(
        "kind: manual\n\nsources:\n- kind: tar\n  url: mozilla:source.tar.xz\n"
        "  ref: pinned\n- kind: patch\n  path: patches/mozjs/bmo1973993-fix-installed-headers.patch\n"
        "- kind: patch\n  path: patches/mozjs/bmo1973994-fix-os-dependent-headers.patch\n"
        "- kind: patch\n  path: patches/mozjs/python-3.14.patch\n\nbuild-depends:\n"
        "- (@): include/clang-for-recc.yml\n"
    )
    (tmp_path / "patches/mozjs").mkdir(parents=True)
    outer = tmp_path / "outer.patch"
    outer.write_text(_generated_patch("patch6_content"))
    subprocess.run(["git", "apply", "--check", str(outer)], cwd=tmp_path,
                   check=True, capture_output=True, timeout=10)
    subprocess.run(["git", "apply", str(outer)], cwd=tmp_path,
                   check=True, capture_output=True, timeout=10)
    source = tmp_path / "python/mozbuild/mozbuild/telemetry.py"
    source.parent.mkdir(parents=True)
    source.write_text('''import os

def cpu_brand_linux():
    """
    Read the CPU brand string out of /proc/cpuinfo on Linux.
    """
    with open("/proc/cpuinfo") as f:
        for line in f:
            if line.startswith("model name"):
                _, brand = line.split(": ", 1)
                return brand.rstrip()
    return None
''')
    subprocess.run(["patch", "--batch", "--fuzz=0", "-p1", "--input",
                    str(tmp_path / "patches/mozjs/proc-cpuinfo.patch")], cwd=tmp_path,
                   check=True, capture_output=True, timeout=10)
    namespace = {}
    exec(compile(source.read_text(), str(source), "exec"), namespace)
    original_exists, original_open = os.path.exists, open

    def cpuinfo_exists(path):
        return cpuinfo is not None if path == "/proc/cpuinfo" else original_exists(path)

    def cpuinfo_open(path, *args, **kwargs):
        if path != "/proc/cpuinfo":
            return original_open(path, *args, **kwargs)
        if cpuinfo is None:
            raise FileNotFoundError(path)
        return io.StringIO(cpuinfo)

    with monkeypatch.context() as context:
        context.setattr(os.path, "exists", cpuinfo_exists)
        context.setattr("builtins.open", cpuinfo_open)
        assert namespace["cpu_brand_linux"]() == expected
