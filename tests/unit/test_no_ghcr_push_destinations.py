"""No Argo push destination (registry parameter or env) defaults to ghcr.io."""

import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]


def named_values(node):
    if isinstance(node, dict):
        if isinstance(node.get("name"), str) and ("value" in node or "default" in node):
            yield node["name"], str(node.get("value", node.get("default")))
        for child in node.values():
            yield from named_values(child)
    elif isinstance(node, list):
        for child in node:
            yield from named_values(child)


def test_push_destinations_never_default_to_ghcr():
    destinations = [
        (path.name, name, value)
        for path in sorted((ROOT / "argo").rglob("*.yaml"))
        for doc in yaml.safe_load_all(path.read_text(encoding="utf-8"))
        for name, value in named_values(doc)
        if re.search(r"registry|dest", name, re.I)
    ]
    assert destinations, "expected registry parameters in argo/"
    for entry in destinations:
        assert "ghcr.io" not in entry[2], entry
