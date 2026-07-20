# Documentation map

This directory contains the repository's durable operating knowledge.

The architecture and maintenance contract is [`DOCUMENTATION-OVERHAUL.md`](DOCUMENTATION-OVERHAUL.md).

## Load by task

- **Agent instructions:** [`../AGENTS.md`](../AGENTS.md)
- **Skill discovery:** [`skills/README.md`](skills/README.md)
- **Operations and failure recovery:** [`operations/README.md`](operations/README.md)
- **Commands:** [`reference/commands.md`](reference/commands.md)
- **Workflow contracts:** [`reference/WORKFLOWS.md`](reference/WORKFLOWS.md)
- **Data contracts:** [`reference/page-contracts.md`](reference/page-contracts.md)
- **Canonical terms:** [`reference/ubiquitous-language.md`](reference/ubiquitous-language.md)
- **Architecture decisions:** [`adr/README.md`](adr/README.md)

## Documentation layers

| Layer | Purpose | Load when |
|---|---|---|
| `skills/` | Task-triggered procedures | You are changing or debugging a subsystem |
| `ops/` | Stable operational procedures and failure modes | You are operating or recovering the lab |
| `reference/` | Contracts, commands, and terminology | You need an exact fact or interface |
| `adr/` | Durable architecture decisions | A design choice or trade-off is involved |
| `specs/` | Explicit design material | A proposal or design artifact is active |

Read the smallest applicable document. Do not load every skill or reference
file at session start. Keep one canonical source for each project fact and link
to it instead of copying it.

## Authoring rules

- Use standard Markdown and relative links.
- Put the answer, prerequisites, and verification command near the top.
- Keep skill entry points concise; defer long examples and background material.
- Do not record credentials, host-specific secrets, incident scratch notes, or
  generated output as evergreen documentation.
- Run `python3 scripts/validate-docs.py` after documentation changes.
