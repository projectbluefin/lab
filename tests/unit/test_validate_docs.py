"""Unit tests for scripts/validate-docs.py."""

from __future__ import annotations

import importlib.util
from pathlib import Path

repo_root = Path(__file__).resolve().parents[2]
script_path = repo_root / "scripts" / "validate-docs.py"
spec = importlib.util.spec_from_file_location("validate_docs", script_path)
validate_docs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validate_docs)


class TestValidateDocs:
    def test_validate_frontmatter_valid(self, tmp_path):
        doc = tmp_path / "valid.md"
        doc.write_text("---\nname: test-skill\ndescription: A test skill\n---\n# Content\n", encoding="utf-8")
        assert validate_docs.validate_frontmatter(doc) == []

    def test_validate_frontmatter_missing_opener(self, tmp_path):
        doc = tmp_path / "no_opener.md"
        doc.write_text("name: test\ndescription: desc\n---\n", encoding="utf-8")
        errors = validate_docs.validate_frontmatter(doc)
        assert len(errors) == 1
        assert "missing YAML frontmatter opener" in errors[0]

    def test_validate_frontmatter_missing_closer(self, tmp_path):
        doc = tmp_path / "no_closer.md"
        doc.write_text("---\nname: test\ndescription: desc\n", encoding="utf-8")
        errors = validate_docs.validate_frontmatter(doc)
        assert len(errors) == 1
        assert "missing YAML frontmatter closer" in errors[0]

    def test_validate_frontmatter_missing_required_fields(self, tmp_path):
        doc = tmp_path / "no_fields.md"
        doc.write_text("---\nfoo: bar\n---\n", encoding="utf-8")
        errors = validate_docs.validate_frontmatter(doc)
        assert any("frontmatter missing 'name'" in e for e in errors)
        assert any("frontmatter missing 'description'" in e for e in errors)

    def test_validate_size(self, tmp_path):
        doc = tmp_path / "doc.md"
        doc.write_text("line 1\nline 2\nline 3\n", encoding="utf-8")
        assert validate_docs.validate_size(doc, 5) == []
        errors = validate_docs.validate_size(doc, 2)
        assert len(errors) == 1
        assert "exceeds 2" in errors[0]

    def test_collect_md_files(self):
        files = validate_docs.collect_md_files()
        assert isinstance(files, set)
        assert Path("README.md") in files or Path("AGENTS.md") in files

    def test_validate_links_valid_and_external(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validate_docs, "ROOT", tmp_path)
        target = tmp_path / "target.md"
        target.write_text("# Target", encoding="utf-8")
        source = tmp_path / "source.md"
        source.write_text(
            "[target](target.md)\n"
            "[target with anchor](target.md#section)\n"
            "[root target](/target.md)\n"
            "[self anchor](#section)\n"
            "[http](https://example.com)\n"
            "[mailto](mailto:dev@example.com)\n",
            encoding="utf-8",
        )
        errors = validate_docs.validate_links({Path("source.md")})
        assert errors == []

    def test_validate_links_broken(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validate_docs, "ROOT", tmp_path)
        source = tmp_path / "source.md"
        source.write_text("[broken](nonexistent.md)\n[broken root](/no/such/file.md)", encoding="utf-8")
        errors = validate_docs.validate_links({Path("source.md")})
        assert len(errors) == 2
        assert any("broken link to nonexistent.md" in e for e in errors)
        assert any("broken link to /no/such/file.md" in e for e in errors)

    def test_find_forbidden(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validate_docs, "ROOT", tmp_path)
        clean = tmp_path / "clean.md"
        clean.write_text("Normal safe text\n", encoding="utf-8")
        dirty = tmp_path / "dirty.md"
        dirty.write_text("Hello jorge@example.com\nIP 192.168.1.5\ncopilot-config\n", encoding="utf-8")

        assert validate_docs.find_forbidden({Path("clean.md")}) == []
        hits = validate_docs.find_forbidden({Path("dirty.md")})
        assert len(hits) == 3
        assert any("jorge@" in h for h in hits)
        assert any(r"192\.168\." in h for h in hits)
        assert any("copilot-config" in h for h in hits)

    def test_validate_router_missing_router(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validate_docs, "ROOT", tmp_path)
        errors = validate_docs.validate_router()
        assert errors == ["docs/SKILL.md: missing skill router (common doc structure)"]

    def test_validate_router_structure(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validate_docs, "ROOT", tmp_path)
        docs_dir = tmp_path / "docs"
        skills_dir = docs_dir / "skills" / "my-skill"
        skills_dir.mkdir(parents=True)
        skill_md = skills_dir / "SKILL.md"
        skill_md.write_text("---\nname: my-skill\ndescription: demo\n---\n", encoding="utf-8")

        router = docs_dir / "SKILL.md"
        router.write_text("## Read order\n## Skill index\n- [my-skill](skills/my-skill/SKILL.md)\n", encoding="utf-8")

        agents = tmp_path / "AGENTS.md"
        agents.write_text("See docs/SKILL.md for skills.\n", encoding="utf-8")

        monkeypatch.setattr(validate_docs, "skills", lambda: [skill_md])
        errors = validate_docs.validate_router()
        assert errors == []

    def test_validate_router_missing_skill_link(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validate_docs, "ROOT", tmp_path)
        docs_dir = tmp_path / "docs"
        skills_dir = docs_dir / "skills" / "my-skill"
        skills_dir.mkdir(parents=True)
        skill_md = skills_dir / "SKILL.md"
        skill_md.write_text("---\nname: my-skill\ndescription: demo\n---\n", encoding="utf-8")

        router = docs_dir / "SKILL.md"
        router.write_text("## Read order\n## Skill index\n", encoding="utf-8")

        agents = tmp_path / "AGENTS.md"
        agents.write_text("See docs/SKILL.md for skills.\n", encoding="utf-8")

        monkeypatch.setattr(validate_docs, "skills", lambda: [skill_md])
        errors = validate_docs.validate_router()
        assert len(errors) == 1
        assert "skill index does not list skills/my-skill/SKILL.md" in errors[0]

    def test_main_passes(self, monkeypatch):
        monkeypatch.setattr(validate_docs, "skills", list)
        monkeypatch.setattr(validate_docs, "collect_md_files", lambda: set())
        monkeypatch.setattr(validate_docs, "validate_links", lambda files: [])
        monkeypatch.setattr(validate_docs, "validate_router", list)
        monkeypatch.setattr(validate_docs, "find_forbidden", lambda files: [])
        assert validate_docs.main() == 0

    def test_main_fails_on_errors(self, monkeypatch):
        monkeypatch.setattr(validate_docs, "skills", list)
        monkeypatch.setattr(validate_docs, "collect_md_files", lambda: set())
        monkeypatch.setattr(validate_docs, "validate_links", lambda files: ["broken link"])
        monkeypatch.setattr(validate_docs, "validate_router", list)
        monkeypatch.setattr(validate_docs, "find_forbidden", lambda files: [])
        assert validate_docs.main() == 1
