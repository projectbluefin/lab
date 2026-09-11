"""Unit tests for scripts/validate-docs.py.

`scripts/validate-docs.py` validates the documentation structure, skill frontmatter,
and internal markdown links. It runs as a required check in `.github/workflows/docs.yml`.
A regression in link checking, frontmatter parsing, forbidden pattern detection, or skill
routing would cause the documentation validation gate to fail open silently.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "validate-docs.py"

_spec = importlib.util.spec_from_file_location("validate_docs", SCRIPT)
assert _spec is not None and _spec.loader is not None
validator = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(validator)


# ==============================================================================
# Module loading and constants
# ==============================================================================


class TestModuleContract:
    def test_constants_values(self):
        assert validator.SKILL_MAX_LINES == 300
        assert validator.REFERENCE_MAX_LINES == 500

    def test_forbidden_patterns_match_expected(self):
        patterns = [p.pattern for p in validator.FORBIDDEN]
        assert r"jorge@" in patterns
        assert r"192\.168\." in patterns
        assert r"copilot-config" in patterns

        # Verify actual matching behaviour
        assert any(p.search("contact jorge@projectbluefin.io") for p in validator.FORBIDDEN)
        assert any(p.search("internal ip 192.168.1.100") for p in validator.FORBIDDEN)
        assert any(p.search("path to copilot-config file") for p in validator.FORBIDDEN)

        # Verify safe strings are not matched
        assert not any(p.search("contact jorge-admin at projectbluefin.io") for p in validator.FORBIDDEN)
        assert not any(p.search("10.0.0.1 or 172.16.0.1") for p in validator.FORBIDDEN)
        assert not any(p.search("copilot_settings or plain copilot") for p in validator.FORBIDDEN)


# ==============================================================================
# skills()
# ==============================================================================


class TestSkills:
    def test_skills_empty_when_no_skills_dir(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        assert validator.skills() == []

    def test_skills_finds_nested_skill_md(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        skill_a = tmp_path / "docs" / "skills" / "skill-a" / "SKILL.md"
        skill_b = tmp_path / "docs" / "skills" / "skill-b" / "SKILL.md"
        ignored_txt = tmp_path / "docs" / "skills" / "skill-c" / "README.md"
        ignored_root = tmp_path / "docs" / "skills" / "SKILL.md"

        skill_a.parent.mkdir(parents=True)
        skill_b.parent.mkdir(parents=True)
        ignored_txt.parent.mkdir(parents=True)

        skill_a.write_text("---\nname: a\ndescription: b\n---\n")
        skill_b.write_text("---\nname: c\ndescription: d\n---\n")
        ignored_txt.write_text("not a skill")
        ignored_root.write_text("not in subfolder")

        found = sorted(validator.skills())
        assert found == sorted([skill_a, skill_b])


# ==============================================================================
# validate_frontmatter()
# ==============================================================================


class TestValidateFrontmatter:
    def test_valid_frontmatter_returns_no_errors(self, tmp_path):
        skill = tmp_path / "SKILL.md"
        skill.write_text(
            "---\n"
            "name: gitops-argocd\n"
            "description: Operational guide for ArgoCD\n"
            "---\n"
            "# GitOps with ArgoCD\n"
        )
        assert validator.validate_frontmatter(skill) == []

    def test_missing_opener(self, tmp_path):
        skill = tmp_path / "SKILL.md"
        skill.write_text(
            "name: gitops\n"
            "description: Operational guide\n"
            "---\n"
        )
        errors = validator.validate_frontmatter(skill)
        assert len(errors) == 1
        assert f"{skill}: missing YAML frontmatter opener" in errors[0]

    def test_opener_without_newline(self, tmp_path):
        skill = tmp_path / "SKILL.md"
        skill.write_text(
            "---name: gitops\n"
            "description: Operational guide\n"
            "---\n"
        )
        errors = validator.validate_frontmatter(skill)
        assert len(errors) == 1
        assert "missing YAML frontmatter opener" in errors[0]

    def test_missing_closer(self, tmp_path):
        skill = tmp_path / "SKILL.md"
        skill.write_text(
            "---\n"
            "name: gitops\n"
            "description: Operational guide\n"
        )
        errors = validator.validate_frontmatter(skill)
        assert len(errors) == 1
        assert f"{skill}: missing YAML frontmatter closer" in errors[0]

    def test_missing_name_field(self, tmp_path):
        skill = tmp_path / "SKILL.md"
        skill.write_text(
            "---\n"
            "description: Operational guide\n"
            "---\n"
            "# Content\n"
        )
        errors = validator.validate_frontmatter(skill)
        assert len(errors) == 1
        assert f"{skill}: frontmatter missing 'name'" in errors[0]

    def test_missing_description_field(self, tmp_path):
        skill = tmp_path / "SKILL.md"
        skill.write_text(
            "---\n"
            "name: gitops\n"
            "---\n"
            "# Content\n"
        )
        errors = validator.validate_frontmatter(skill)
        assert len(errors) == 1
        assert f"{skill}: frontmatter missing 'description'" in errors[0]

    def test_missing_both_name_and_description(self, tmp_path):
        skill = tmp_path / "SKILL.md"
        skill.write_text(
            "---\n"
            "author: jorge\n"
            "version: 1\n"
            "---\n"
            "# Content\n"
        )
        errors = validator.validate_frontmatter(skill)
        assert len(errors) == 2
        assert any("frontmatter missing 'name'" in e for e in errors)
        assert any("frontmatter missing 'description'" in e for e in errors)

    def test_fields_in_body_do_not_satisfy_frontmatter(self, tmp_path):
        skill = tmp_path / "SKILL.md"
        skill.write_text(
            "---\n"
            "description: A valid description\n"
            "---\n"
            "name: body-level-name\n"
        )
        errors = validator.validate_frontmatter(skill)
        assert len(errors) == 1
        assert "frontmatter missing 'name'" in errors[0]


# ==============================================================================
# validate_size()
# ==============================================================================


class TestValidateSize:
    def test_file_within_limit_returns_no_errors(self, tmp_path):
        f = tmp_path / "doc.md"
        f.write_text("line 1\nline 2\nline 3\n")
        assert validator.validate_size(f, max_lines=5) == []

    def test_file_exactly_at_limit_returns_no_errors(self, tmp_path):
        f = tmp_path / "doc.md"
        f.write_text("line 1\nline 2\nline 3\n")
        assert validator.validate_size(f, max_lines=3) == []

    def test_file_exceeding_limit_returns_error(self, tmp_path):
        f = tmp_path / "doc.md"
        f.write_text("line 1\nline 2\nline 3\nline 4\n")
        errors = validator.validate_size(f, max_lines=3)
        assert len(errors) == 1
        assert f"{f}: 4 lines exceeds 3" in errors[0]

    def test_empty_file_returns_no_errors(self, tmp_path):
        f = tmp_path / "empty.md"
        f.write_text("")
        assert validator.validate_size(f, max_lines=10) == []


# ==============================================================================
# collect_md_files()
# ==============================================================================


class TestCollectMdFiles:
    def test_collects_md_from_all_four_expected_scopes(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)

        # Valid targets
        doc1 = tmp_path / "docs" / "index.md"
        doc2 = tmp_path / "docs" / "nested" / "guide.md"
        root_md = tmp_path / "README.md"
        argo_md = tmp_path / "argo" / "workflows.md"
        github_md = tmp_path / ".github" / "PULL_REQUEST_TEMPLATE.md"

        # Targets to ignore
        doc_txt = tmp_path / "docs" / "notes.txt"
        scripts_md = tmp_path / "scripts" / "helper.md"
        tests_md = tmp_path / "tests" / "unit" / "test.md"
        root_nested_md = tmp_path / "other" / "doc.md"

        for p in [doc1, doc2, root_md, argo_md, github_md, doc_txt, scripts_md, tests_md, root_nested_md]:
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text("# doc\n")

        collected = validator.collect_md_files()
        expected = {
            Path("docs/index.md"),
            Path("docs/nested/guide.md"),
            Path("README.md"),
            Path("argo/workflows.md"),
            Path(".github/PULL_REQUEST_TEMPLATE.md"),
        }
        assert collected == expected


# ==============================================================================
# validate_links()
# ==============================================================================


class TestValidateLinks:
    def test_valid_relative_link_succeeds(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        docs = tmp_path / "docs"
        docs.mkdir(parents=True)
        target = docs / "target.md"
        target.write_text("# Target\n")
        source = docs / "source.md"
        source.write_text("See [Target](target.md) for details.\n")

        assert validator.validate_links({Path("docs/source.md")}) == []

    def test_valid_parent_relative_link_succeeds(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        docs = tmp_path / "docs" / "sub"
        docs.mkdir(parents=True)
        target = tmp_path / "README.md"
        target.write_text("# Root README\n")
        source = docs / "guide.md"
        source.write_text("See [Root](../../README.md) for details.\n")

        assert validator.validate_links({Path("docs/sub/guide.md")}) == []

    def test_valid_root_relative_link_succeeds(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        docs = tmp_path / "docs"
        docs.mkdir(parents=True)
        target = docs / "target.md"
        target.write_text("# Target\n")
        source = docs / "source.md"
        source.write_text("See [Target](/docs/target.md) for details.\n")

        assert validator.validate_links({Path("docs/source.md")}) == []

    def test_external_schemes_and_anchors_are_skipped(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        source = tmp_path / "docs" / "source.md"
        source.parent.mkdir(parents=True)
        source.write_text(
            "[Web](https://example.com)\n"
            "[Http](http://example.com)\n"
            "[Mail](mailto:user@example.com)\n"
            "[Anchor](#heading-name)\n"
            "[Empty Anchor](#)\n"
        )
        assert validator.validate_links({Path("docs/source.md")}) == []

    def test_relative_link_with_anchor_resolves_base_file(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        docs = tmp_path / "docs"
        docs.mkdir(parents=True)
        target = docs / "target.md"
        target.write_text("# Target Section\n")
        source = docs / "source.md"
        source.write_text("See [Section](target.md#target-section).\n")

        assert validator.validate_links({Path("docs/source.md")}) == []

    def test_broken_relative_link_reports_error(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        docs = tmp_path / "docs"
        docs.mkdir(parents=True)
        source = docs / "source.md"
        source.write_text("See [Missing](nonexistent.md).\n")

        errors = validator.validate_links({Path("docs/source.md")})
        assert len(errors) == 1
        assert "docs/source.md: broken link to nonexistent.md" in errors[0]

    def test_broken_root_relative_link_reports_error(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        docs = tmp_path / "docs"
        docs.mkdir(parents=True)
        source = docs / "source.md"
        source.write_text("See [Missing](/docs/missing.md).\n")

        errors = validator.validate_links({Path("docs/source.md")})
        assert len(errors) == 1
        assert "docs/source.md: broken link to /docs/missing.md" in errors[0]

    def test_broken_link_preserves_anchor_in_error_message(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        docs = tmp_path / "docs"
        docs.mkdir(parents=True)
        source = docs / "source.md"
        source.write_text("See [Broken](bad.md#some-anchor).\n")

        errors = validator.validate_links({Path("docs/source.md")})
        assert len(errors) == 1
        assert "docs/source.md: broken link to bad.md#some-anchor" in errors[0]

    def test_multiple_links_only_flags_broken_ones(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        docs = tmp_path / "docs"
        docs.mkdir(parents=True)
        target = docs / "exists.md"
        target.write_text("# Exists\n")
        source = docs / "source.md"
        source.write_text(
            "[Good](exists.md)\n"
            "[External](https://bluefin.io)\n"
            "[Bad 1](missing1.md)\n"
            "[Bad 2](missing2.md)\n"
        )

        errors = validator.validate_links({Path("docs/source.md")})
        assert len(errors) == 2
        assert any("broken link to missing1.md" in e for e in errors)
        assert any("broken link to missing2.md" in e for e in errors)


# ==============================================================================
# find_forbidden()
# ==============================================================================


class TestFindForbidden:
    def test_clean_file_yields_no_forbidden_hits(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        doc = tmp_path / "docs" / "doc.md"
        doc.parent.mkdir(parents=True)
        doc.write_text("This is completely clean text.\nNo secrets here.\n")

        assert validator.find_forbidden({Path("docs/doc.md")}) == []

    def test_detects_forbidden_patterns_with_line_numbers(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        doc = tmp_path / "docs" / "doc.md"
        doc.parent.mkdir(parents=True)
        doc.write_text(
            "Line 1: clean\n"
            "Line 2: contact jorge@bluefin.io\n"
            "Line 3: clean\n"
            "Line 4: node ip is 192.168.1.55\n"
            "Line 5: see copilot-config section\n"
        )

        hits = validator.find_forbidden({Path("docs/doc.md")})
        assert len(hits) == 3
        assert "docs/doc.md:2: forbidden pattern 'jorge@'" in hits[0]
        assert "docs/doc.md:4: forbidden pattern '192\\.168\\.'" in hits[1]
        assert "docs/doc.md:5: forbidden pattern 'copilot-config'" in hits[2]


# ==============================================================================
# validate_router()
# ==============================================================================


class TestValidateRouter:
    def _setup_base_env(self, tmp_path):
        docs = tmp_path / "docs"
        docs.mkdir(parents=True, exist_ok=True)
        skill_dir = docs / "skills" / "deploy"
        skill_dir.mkdir(parents=True, exist_ok=True)
        (skill_dir / "SKILL.md").write_text(
            "---\nname: deploy\ndescription: Deploy skill\n---\n"
        )

        (tmp_path / "AGENTS.md").write_text(
            "# Agents\nSee docs/SKILL.md for available skills.\n"
        )

        router = docs / "SKILL.md"
        router.write_text(
            "# Skills Router\n\n"
            "## Read order\n1. First read this.\n\n"
            "## Skill index\n- [Deploy](skills/deploy/SKILL.md)\n"
        )
        return router

    def test_valid_router_passes_without_errors(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        self._setup_base_env(tmp_path)
        assert validator.validate_router() == []

    def test_missing_skill_router_file(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        errors = validator.validate_router()
        assert errors == ["docs/SKILL.md: missing skill router (common doc structure)"]

    def test_missing_read_order_section(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        router = self._setup_base_env(tmp_path)
        router.write_text(
            "# Skills Router\n\n"
            "## Skill index\n- [Deploy](skills/deploy/SKILL.md)\n"
        )
        errors = validator.validate_router()
        assert "docs/SKILL.md: missing '## Read order' section" in errors

    def test_missing_skill_index_section(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        router = self._setup_base_env(tmp_path)
        router.write_text(
            "# Skills Router\n\n"
            "## Read order\n1. First read this.\n"
        )
        errors = validator.validate_router()
        assert "docs/SKILL.md: missing '## Skill index' section" in errors

    def test_agents_md_missing_router_pointer(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        self._setup_base_env(tmp_path)
        (tmp_path / "AGENTS.md").write_text("# Agents without pointer\n")

        errors = validator.validate_router()
        assert "AGENTS.md: must route skill discovery to docs/SKILL.md" in errors

    def test_skill_not_listed_in_skill_index(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        self._setup_base_env(tmp_path)
        unlisted = tmp_path / "docs" / "skills" / "unlisted-skill" / "SKILL.md"
        unlisted.parent.mkdir(parents=True)
        unlisted.write_text("---\nname: unlisted\ndescription: Unlisted skill\n---\n")

        errors = validator.validate_router()
        assert "docs/SKILL.md: skill index does not list skills/unlisted-skill/SKILL.md" in errors

    def test_underscore_prefixed_templates_are_exempt(self, tmp_path, monkeypatch):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        self._setup_base_env(tmp_path)
        template = tmp_path / "docs" / "skills" / "_template" / "SKILL.md"
        template.parent.mkdir(parents=True)
        template.write_text("---\nname: template\ndescription: Template\n---\n")

        # Even though _template/SKILL.md is not in docs/SKILL.md, it should not produce an error
        assert validator.validate_router() == []


# ==============================================================================
# main() integration
# ==============================================================================


class TestMain:
    def _create_minimal_valid_repo(self, tmp_path):
        docs = tmp_path / "docs"
        docs.mkdir(parents=True, exist_ok=True)
        (docs / "reference").mkdir(parents=True, exist_ok=True)
        (docs / "ops").mkdir(parents=True, exist_ok=True)

        skill_dir = docs / "skills" / "my-skill"
        skill_dir.mkdir(parents=True, exist_ok=True)
        (skill_dir / "SKILL.md").write_text(
            "---\nname: my-skill\ndescription: Test skill\n---\n# Content\n"
        )

        (tmp_path / "AGENTS.md").write_text("See docs/SKILL.md\n")
        (docs / "SKILL.md").write_text(
            "## Read order\nOrder\n\n## Skill index\n- [Skill](skills/my-skill/SKILL.md)\n"
        )
        (tmp_path / "README.md").write_text("# Lab\n")

    def test_main_returns_zero_on_clean_tree(self, tmp_path, monkeypatch, capsys):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        self._create_minimal_valid_repo(tmp_path)

        exit_code = validator.main()
        captured = capsys.readouterr()

        assert exit_code == 0
        assert "Documentation passes validation." in captured.out
        assert "Errors:" not in captured.out

    def test_main_returns_one_on_validation_errors(self, tmp_path, monkeypatch, capsys):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        self._create_minimal_valid_repo(tmp_path)

        # Introduce a broken link in README.md
        (tmp_path / "README.md").write_text("[Missing link](missing.md)\n")

        exit_code = validator.main()
        captured = capsys.readouterr()

        assert exit_code == 1
        assert "Errors:" in captured.out
        assert "README.md: broken link to missing.md" in captured.out

    def test_main_warnings_do_not_fail_gate(self, tmp_path, monkeypatch, capsys):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        self._create_minimal_valid_repo(tmp_path)

        # Introduce a warning (forbidden pattern) in README.md without any errors
        (tmp_path / "README.md").write_text("Host: 192.168.1.1\n")

        exit_code = validator.main()
        captured = capsys.readouterr()

        assert exit_code == 0
        assert "Warnings:" in captured.out
        assert "forbidden pattern '192\\.168\\.'" in captured.out
        assert "Documentation passes validation." in captured.out
        assert "Errors:" not in captured.out

    def test_main_oversized_skill_warning(self, tmp_path, monkeypatch, capsys):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        self._create_minimal_valid_repo(tmp_path)

        skill_md = tmp_path / "docs" / "skills" / "my-skill" / "SKILL.md"
        long_content = "---\nname: my-skill\ndescription: Test skill\n---\n" + ("line\n" * 305)
        skill_md.write_text(long_content)

        exit_code = validator.main()
        captured = capsys.readouterr()

        assert exit_code == 0
        assert "Warnings:" in captured.out
        assert "lines exceeds 300" in captured.out

    def test_main_runbook_exemption_from_reference_size_check(self, tmp_path, monkeypatch, capsys):
        monkeypatch.setattr(validator, "ROOT", tmp_path)
        self._create_minimal_valid_repo(tmp_path)

        # RUNBOOK.md in reference directory exceeds 500 lines but should be exempt
        runbook = tmp_path / "docs" / "reference" / "RUNBOOK.md"
        runbook.write_text("line\n" * 550)

        exit_code = validator.main()
        captured = capsys.readouterr()

        assert exit_code == 0
        assert "RUNBOOK.md" not in captured.out


# ==============================================================================
# Live repository sanity check
# ==============================================================================


class TestLiveRepositoryContract:
    def test_live_repository_router_is_valid(self):
        """The actual projectbluefin/lab documentation router passes all checks."""
        assert validator.validate_router() == []

    def test_live_repository_all_skills_have_valid_frontmatter(self):
        """Every live skill in docs/skills/ passes frontmatter validation."""
        live_skills = validator.skills()
        assert len(live_skills) > 0
        for skill in live_skills:
            errors = validator.validate_frontmatter(skill)
            assert errors == [], f"Frontmatter error in {skill}: {errors}"
