#!/usr/bin/env python3
"""Validate lab documentation structure and internal links."""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SKILL_MAX_LINES = 300
SKILL_HARD_MAX_LINES = 500
REFERENCE_MAX_LINES = 500
FORBIDDEN = [
    re.compile(r"jorge@"),
    re.compile(r"192\.168\."),
    re.compile(r"copilot-config"),
]


def skills() -> list[Path]:
    return list((ROOT / "docs" / "skills").glob("*/SKILL.md"))


def validate_entry_points() -> list[str]:
    errors: list[str] = []
    for name in ("AGENTS.md", "agents.md"):
        if not (ROOT / name).exists():
            errors.append(f"missing root agent entry point: {name}")
    if not (ROOT / "docs" / "skills" / "README.md").exists():
        errors.append("missing docs/skills/README.md skill manifest")
    return errors


def validate_skill_manifest(files: set[Path]) -> list[str]:
    manifest = ROOT / "docs" / "skills" / "README.md"
    if not manifest.exists():
        return []
    errors: list[str] = []
    pattern = r"\]\(([^)]+/SKILL\.md)\)"
    targets = set(re.findall(pattern, manifest.read_text(encoding="utf-8")))
    for target in targets:
        if not (manifest.parent / target).exists():
            errors.append(f"{manifest.relative_to(ROOT)}: missing skill target {target}")

    expected = {
        p.parent.name + "/SKILL.md"
        for p in skills()
        if p.parent.name != "_template"
    }
    missing = sorted(expected - targets)
    for target in missing:
        errors.append(f"{manifest.relative_to(ROOT)}: skill is not indexed: {target}")
    return errors


def validate_frontmatter(path: Path) -> list[str]:
    errors: list[str] = []
    text = path.read_text(encoding="utf-8")
    if not text.startswith("---\n"):
        errors.append(f"{path}: missing YAML frontmatter opener")
        return errors
    end = text.find("\n---\n", 4)
    if end == -1:
        errors.append(f"{path}: missing YAML frontmatter closer")
        return errors
    fm = text[4:end]
    if "name:" not in fm:
        errors.append(f"{path}: frontmatter missing 'name'")
    if "description:" not in fm:
        errors.append(f"{path}: frontmatter missing 'description'")
    return errors


def validate_size(path: Path, max_lines: int) -> list[str]:
    lines = path.read_text(encoding="utf-8").splitlines()
    if len(lines) > max_lines:
        return [f"{path}: {len(lines)} lines exceeds {max_lines}"]
    return []


def collect_md_files() -> set[Path]:
    files: set[Path] = set()
    for p in (ROOT / "docs").rglob("*.md"):
        files.add(p.relative_to(ROOT))
    for p in ROOT.glob("*.md"):
        files.add(p.relative_to(ROOT))
    for p in (ROOT / "argo").rglob("*.md"):
        files.add(p.relative_to(ROOT))
    for p in (ROOT / ".github").rglob("*.md"):
        files.add(p.relative_to(ROOT))
    return files


LINK_RE = re.compile(r"\[([^\]]*)\]\(([^)]+)\)")


def validate_links(files: set[Path]) -> list[str]:
    errors: list[str] = []
    md_paths = {f: ROOT / f for f in files}
    for p in md_paths:
        text = md_paths[p].read_text(encoding="utf-8")
        for m in LINK_RE.finditer(text):
            target = m.group(2).strip()
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            base = target.split("#", 1)[0]
            if not base:
                continue
            if base.startswith("/"):
                resolved = ROOT / base.lstrip("/")
            else:
                resolved = (ROOT / p).parent / base
            if not resolved.exists():
                errors.append(f"{p}: broken link to {target}")
    return errors


def find_forbidden(files: set[Path]) -> list[str]:
    hits: list[str] = []
    for p in files:
        text = (ROOT / p).read_text(encoding="utf-8", errors="ignore")
        for idx, line in enumerate(text.splitlines(), 1):
            for pat in FORBIDDEN:
                if pat.search(line):
                    hits.append(f"{p}:{idx}: forbidden pattern '{pat.pattern}'")
    return hits


def main() -> int:
    errors: list[str] = []
    warnings: list[str] = []

    errors.extend(validate_entry_points())

    for skill in skills():
        errors.extend(validate_frontmatter(skill))
        line_count = len(skill.read_text(encoding="utf-8").splitlines())
        if line_count > SKILL_HARD_MAX_LINES:
            errors.append(f"{skill}: {line_count} lines exceeds hard limit {SKILL_HARD_MAX_LINES}")
        elif line_count > SKILL_MAX_LINES:
            warnings.append(f"{skill}: {line_count} lines exceeds advisory limit {SKILL_MAX_LINES}")

    files = collect_md_files()
    errors.extend(validate_skill_manifest(files))
    for ref_dir in (ROOT / "docs" / "reference", ROOT / "docs" / "ops"):
        if ref_dir.exists():
            for p in ref_dir.glob("*.md"):
                if p.name == "RUNBOOK.md":
                    continue
                warnings.extend(f"{p}: {len(p.read_text(encoding='utf-8').splitlines())} lines exceeds {REFERENCE_MAX_LINES}"
                                for _ in [0] if len(p.read_text(encoding="utf-8").splitlines()) > REFERENCE_MAX_LINES)

    errors.extend(validate_links(files))
    warnings.extend(find_forbidden(files))

    if warnings:
        print("Warnings:")
        for w in warnings:
            print(f"  {w}")
    if errors:
        print("Errors:")
        for e in errors:
            print(f"  {e}")
        return 1
    print("Documentation passes validation.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
