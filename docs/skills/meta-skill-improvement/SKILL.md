---
name: meta-skill-improvement
description: >
  Add, refactor, validate, or retire repository skills and durable agent
  documentation. Use when a task reveals reusable knowledge or when a skill
  needs maintenance.
---

# Skill improvement

Use this skill to keep the repository's agent-facing documentation accurate,
small, discoverable, and reusable.

## When to Use

- A task reveals a workaround, invariant, or non-obvious procedure.
- A skill contains stale facts, duplicated policy, or a broken link.
- A new recurring task has no suitable skill.
- A `SKILL.md` is approaching its size budget.
- A tool or API documented by a skill has changed.

## When NOT to Use

- A one-off implementation detail belongs in the code or commit history.
- An unresolved incident belongs in the issue tracker or a temporary incident
  record, not an evergreen skill.
- A design decision belongs in `docs/adr/` as well as any affected skill.

## Core Process

1. Identify the durable lesson. Ask: would knowing this before the task have
   prevented trial and error?
2. Find the closest existing skill in [`../README.md`](../README.md). Update it
   instead of creating a duplicate.
3. Keep the entry point focused on triggers, procedure, red flags, and
   verification. Move long examples, schemas, and troubleshooting matrices to
   a supporting file.
4. Use standard YAML front matter with `name` and `description`. Names and
   directories use lowercase kebab-case.
5. Make the description specific enough for an agent to select the skill
   without loading its body.
6. Link to canonical facts instead of copying them. Prefer source files and
   exact commands over prose summaries of live configuration.
7. If the change affects architecture, ownership, or a durable trade-off,
   write or update an ADR.
8. Update [`../README.md`](../README.md) when adding, renaming, or retiring a
   skill.
9. Run the verification checks before handoff.

## Progressive loading rules

- Metadata is the discovery layer.
- `SKILL.md` is the task procedure.
- Supporting files are deferred resources and must be linked from the skill.
- Keep `SKILL.md` below 300 lines where possible and below 500 lines always.
- Add a table of contents to supporting files longer than 300 lines.
- Do not require an agent to load unrelated skills.

## Common Rationalizations

| Rationalization | Corrective action |
|---|---|
| "This workaround is obvious." | If it required investigation, record the durable rule. |
| "I will update the skill later." | Update it in the same change while the evidence is available. |
| "A second document is easier than editing the existing one." | Find the canonical owner first; avoid parallel facts. |
| "The file is only slightly too long." | Move deferred material before the entry point becomes expensive to load. |

## Red Flags

- A skill duplicates the root agent instructions.
- A skill contains current issue numbers, completed work, or session dates.
- A skill states a live image, tag, hostname, or workflow value without a
  verification command or source link.
- A new skill overlaps an existing skill's scope.
- A reference file is not linked from its parent skill.
- A skill grows because background explanation was added instead of a focused
  procedure.
- A client-specific instruction is presented as a community-wide standard.

## Verification

```bash
python3 scripts/validate-docs.py
```

Also verify, as applicable:

- [ ] The skill has valid front matter with `name` and `description`.
- [ ] The directory and name use kebab-case.
- [ ] The description includes concrete use triggers.
- [ ] The entry point has When to Use, When NOT to Use, Core Process, Red
      Flags, and Verification sections.
- [ ] Every relative link resolves.
- [ ] Supporting files are linked and are no larger than necessary.
- [ ] Project-specific facts are checked against source.
- [ ] The skill index reflects the current catalog.
- [ ] No secrets, private host details, or transient state were added.

## Sources

- [Anthropic Skills](https://github.com/anthropics/skills) — progressive disclosure and skill structure
- [Vercel Agent Skills](https://github.com/vercel-labs/agent-skills) — per-skill directory layout
- [AGENTS.md](https://agents.md/) — repository agent instruction format

## Related references

- [`../../README.md`](../../README.md) — documentation map
- [`../_template/SKILL.md`](../_template/SKILL.md) — new-skill template
- [`../../adr/README.md`](../../adr/README.md) — architecture decision records
