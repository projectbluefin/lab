#!/usr/bin/env python3
"""Assert the dakota build pipeline can be narrowed to a single variant.

The four-variant fan-out cannot schedule on a two-node grid: the extra
`bst-build` pods hit their own anti-affinity and their retries sit
`Unschedulable: 0/2 nodes are available: 2 node(s) didn't match pod
anti-affinity rules`, which stalls the whole workflow. A rebase of one machine
only needs `oci/bluefin.bst`, so `-p variants=default` must produce exactly one
build task -- and that only holds while every optional task carries the gate and
both entrypoints forward the parameter.

Usage: check_dakota_variants.py [path to dakota-build-pipeline.yaml]
"""

import re
import sys
from pathlib import Path

import yaml

DEFAULT_PATH = Path("argo/workflow-templates/dakota-build-pipeline.yaml")
GATE = "{{inputs.parameters.variants}} == all"
ENTRYPOINTS = ("build", "build-warmup")
CORE = "build-core"


def step_args(step):
    return {p["name"] for p in ((step.get("arguments") or {}).get("parameters") or [])}


def main(path):
    doc = yaml.safe_load(path.read_text())
    spec = doc["spec"]
    templates = {t["name"]: t for t in spec["templates"]}
    failures = []

    declared = {p["name"] for p in spec.get("arguments", {}).get("parameters", [])}
    if "variants" not in declared:
        failures.append("workflow does not declare a `variants` parameter")

    # Every caller of build-core must forward the parameter, or the gate below
    # evaluates against an unset value.
    for name in ENTRYPOINTS:
        template = templates.get(name)
        if template is None:
            failures.append(f"missing entrypoint template `{name}`")
            continue
        calls = [
            step
            for group in template.get("steps", [])
            for step in group
            if step.get("template") == CORE
        ] + [
            task
            for task in (template.get("dag") or {}).get("tasks", [])
            if task.get("template") == CORE
        ]
        if not calls:
            failures.append(f"`{name}` never calls `{CORE}`")
        for call in calls:
            if "variants" not in step_args(call):
                failures.append(f"`{name}` calls `{CORE}` without forwarding `variants`")

    core = templates.get(CORE)
    if core is None:
        failures.append(f"missing `{CORE}` template")
        return report(failures)

    core_inputs = {p["name"] for p in (core.get("inputs") or {}).get("parameters", [])}
    if "variants" not in core_inputs:
        failures.append(f"`{CORE}` does not accept `variants`")

    build_tasks = [
        task
        for task in (core.get("dag") or {}).get("tasks", [])
        if task["name"].startswith("build-bluefin")
    ]
    ungated = [t["name"] for t in build_tasks if t.get("when") != GATE]
    if ungated != ["build-bluefin"]:
        failures.append(
            "with variants=default exactly one build task must remain; "
            f"ungated tasks are {ungated or 'none'}"
        )
    gated = [t["name"] for t in build_tasks if t.get("when") == GATE]
    if not gated:
        failures.append("no optional variant is gated; variants=all would build one image")

    failures += check_build_export_options(path)

    return report(failures, len(build_tasks), gated)


def check_build_export_options(path):
    """Build and export must resolve the same BuildStream cache key.

    Every `-o` option feeds the cache key, so exporting with a different option
    set than the build used checks out a *different* artifact - stale content if
    one was ever cached, a miss otherwise. This caught `-o x86_64_v3 true` on
    the checkout while the build ran with dakota's default of false.
    """
    text = path.read_text()
    failures = []
    build = re.search(r"build \"\$\{ELEMENT\}\"", text)
    checkout = re.search(r"artifact checkout \"\$\{ELEMENT\}\"", text)
    if not build or not checkout:
        failures.append("could not locate the build and checkout invocations")
        return failures

    def options(end):
        start = text.rfind("bst --config", 0, end)
        return set(re.findall(r"-o (\S+) (\S+)", text[start:end]))

    build_opts, checkout_opts = options(build.start()), options(checkout.start())
    if build_opts != checkout_opts:
        failures.append(
            "build and export disagree on BuildStream options: "
            f"build={sorted(build_opts) or 'none'} export={sorted(checkout_opts) or 'none'}"
        )
    return failures


def report(failures, total=None, gated=None):
    if failures:
        for f in failures:
            print(f"ERROR: {f}", file=sys.stderr)
        return 1
    print(
        f"✓ dakota variants: {total} build task(s), "
        f"{len(gated)} gated behind variants=all, 1 always built"
    )
    return 0


if __name__ == "__main__":
    target = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_PATH
    sys.exit(main(target))
