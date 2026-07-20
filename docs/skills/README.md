# Skill index

Load only the skill that matches the task. Each skill entry point is a
`SKILL.md`; supporting material is loaded only when that skill links to it.

| Task | Load |
|---|---|
| Author, lint, or debug Argo workflows | [`argo-workflows/SKILL.md`](argo-workflows/SKILL.md) |
| Build or troubleshoot dashboard pages | [`astro-dashboard-pages/SKILL.md`](astro-dashboard-pages/SKILL.md) |
| Build or troubleshoot the image under test | [`bluefin-server/SKILL.md`](bluefin-server/SKILL.md) |
| Change CI workflows or validation | [`ci-tooling/SKILL.md`](ci-tooling/SKILL.md) |
| Operate cluster add-ons, storage, registries, or nodes | [`cluster-tooling/SKILL.md`](cluster-tooling/SKILL.md) |
| Review downstream image changes | [`dakota-pr-review/SKILL.md`](dakota-pr-review/SKILL.md) |
| Onboard or recover a Flatcar node | [`flatcar-node-onboarding/SKILL.md`](flatcar-node-onboarding/SKILL.md) |
| Design dashboard UI or visual components | [`frontend-design/SKILL.md`](frontend-design/SKILL.md) |
| Operate ArgoCD and GitOps reconciliation | [`gitops-argocd/SKILL.md`](gitops-argocd/SKILL.md) |
| Provision or debug KubeVirt VMs | [`kubevirt-vms/SKILL.md`](kubevirt-vms/SKILL.md) |
| Add or improve a skill | [`meta-skill-improvement/SKILL.md`](meta-skill-improvement/SKILL.md) |
| Author or debug GUI and system tests | [`test-authoring/SKILL.md`](test-authoring/SKILL.md) |

## Progressive loading

1. Read this index to select one skill.
2. Read that skill's `SKILL.md`.
3. Load a linked supporting document only when the current procedure requires
   it.
4. Run the skill's verification commands before handoff.

## Skill contract

Every skill directory uses a kebab-case name and contains `SKILL.md` with YAML
front matter containing `name` and `description`. The description must state
when an agent should load the skill. Long examples, schemas, and troubleshooting
material belong in supporting files rather than the entry point.

Use [`_template/SKILL.md`](_template/SKILL.md) when adding a skill. See
[`meta-skill-improvement/SKILL.md`](meta-skill-improvement/SKILL.md) for the
maintenance and write-back procedure.
