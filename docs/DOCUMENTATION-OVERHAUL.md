# Documentation architecture

This repository uses a small agent entry point, a task router, and deferred
skill references. The design follows the open `AGENTS.md` format and standard
Markdown. MCP itself standardizes discoverable resources and prompts but does
not prescribe a repository `docs/skills/` layout.

## Loading model

1. Load [`../AGENTS.md`](../AGENTS.md).
2. Select one entry from [`skills/README.md`](skills/README.md).
3. Load that skill's `SKILL.md`.
4. Load linked supporting material only when the procedure requires it.
5. Run the skill's verification commands.

Each skill directory uses a kebab-case name and contains `SKILL.md`. Long
examples, schemas, and troubleshooting tables belong in supporting files.
Entry points should remain below 300 lines and must never exceed 500 lines.

## Canonical documentation layers

| Layer | Canonical purpose |
|---|---|
| `AGENTS.md` | Agent navigation, commands, boundaries, completion checks |
| `README.md` | Public project purpose and operating model |
| `CONTRIBUTING.md` | Human contribution workflow |
| `SECURITY.md` | Security policy and reporting |
| `docs/skills/` | Task-triggered procedures |
| `docs/ops/` | Stable operator procedures and recovery |
| `docs/reference/` | Commands, contracts, and terminology |
| `docs/adr/` | Durable architecture decisions |

One fact has one canonical home. Other documents link to it instead of copying
it. Generated dashboard output is not documentation source.

## Maintenance

Update the closest existing skill when work reveals a reusable rule. Add a new
skill only when no existing scope fits. Keep session notes, issue backlogs,
completed work, secrets, host-specific credentials, and transient state out of
skills. Run:

```bash
python3 scripts/validate-docs.py
```

Documentation CI validates entry points, skill front matter, manifest
coverage, local relative-link targets, and size budgets. It does not validate
external URLs or Markdown anchors. Architecture or ownership changes require
an ADR.

## Sources

- [AGENTS.md](https://agents.md/) — open repository agent-instruction format
- [MCP resources](https://modelcontextprotocol.io/specification/2025-11-25/server/resources) — discover/read resource model
- [MCP prompts](https://modelcontextprotocol.io/specification/2025-11-25/server/prompts) — prompt metadata and discovery
- [Anthropic Skills](https://github.com/anthropics/skills) — progressive disclosure and skill structure
- [Vercel Agent Skills](https://github.com/vercel-labs/agent-skills) — per-skill directory layout
