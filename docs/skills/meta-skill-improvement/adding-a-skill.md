---
name: adding-a-skill
description: >
  How to add a new skill to the lab docs. Use when a recurring task needs
  its own guidance file or when the skill discovery surface changes.
---

# Adding a New Skill

## When to add a skill

Create a new skill when a domain keeps needing the same explanations, checklists,
or troubleshooting steps, and the existing skill surface does not cover it clearly.
Do not create a skill for one-off tasks.

## Creating the skill

1. Copy the template:
   ```bash
   cp docs/skills/_template/SKILL.md docs/skills/<name>/SKILL.md
   ```
2. Fill in the frontmatter `name` and `description`.
3. Replace the bracketed guide text with concrete rules, process steps, and
   verification checklists.
4. Keep `SKILL.md` under 300 lines. Extract deep-dive sections to sibling
   `.md` files if the skill needs more detail.
5. Add the skill to `docs/skills/README.md`.
6. If the skill is part of the standard agent routing table, add a row to
   `agents.md` as well.

## Validation

Run the docs validator before committing:

```bash
python3 scripts/validate-docs.py
```

It checks skill frontmatter, internal links, forbidden patterns, and line budgets.
