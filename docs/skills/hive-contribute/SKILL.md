---
name: hive-contribute
description: >
  Run the projectbluefin/contribute Hive contributor (OMP in tmux) on the lab,
  driven by two local llm-d models: a worker on exo-0 and an advisor on ghost.
  Use when turning contribution on/off, attaching to watch it, changing the
  models or their flags, or debugging why the contributor is idle.
---

# hive-contribute

## Shape

| Piece | Where | Manifest |
|---|---|---|
| Worker model: `ornith-ai/Ornith-1.5-35B-A3B-GGUF` `Ornith-1.5-35B-Q6_K.gguf`, 100k ctx | `llm-d/llm-d-modelserver`, exo-0, NodePort 30800 | `manifests/llm-d.yaml` |
| Advisor model: `unsloth/Qwen3.8-27B-GGUF` `Qwen3.8-27B-UD-Q6_K.gguf` + MTP draft, 64k ctx | `llm-d/llm-d-advisor`, ghost, NodePort 30801 | `manifests/llm-d.yaml` |
| Contributor: `ghcr.io/projectbluefin/contribute:stable` | `contribute/contribute`, any node | `manifests/contribute.yaml` |

One model per node, one GPU each. OMP roles are pinned by the `contribute-omp`
ConfigMap (`roles.yml`, layered last via `PI_CONFIG_FILES`): `default` and
`smol` → worker, `advisor` → advisor. No cloud provider keys enter the pod.

## Use

The recipes live in `~/.justfile`, so `just contribute-on` works from any directory. This repo's Justfile has `set fallback`, which reaches them too. If chezmoi manages that file, merge recipe updates there: a direct download replaces machine-specific registration defaults. Otherwise install or refresh it:

```bash
curl -fsSL https://raw.githubusercontent.com/projectbluefin/lab/main/docs/skills/hive-contribute/contribute.just -o ~/.justfile
```

```bash
just contribute-on       # write Secret, scale models + contributor to 1, wait, attach to tmux
just contribute-attach   # re-attach (detach: C-b d; the contributor keeps running)
just contribute-status
just contribute-off      # scale all three to 0; frees both GPUs for video jobs
just contribute-learning-export # snapshot learned skills for review, not automatic promotion
```

The k8s MCP `resources_scale` tool can toggle the same three Deployments.
Scaling up through MCP reuses the last Secret that `contribute-on` wrote.

`contribute-on` sends, every time, with no login step:

- `gh auth token` → `GH_TOKEN`. Hive contributors fork and open PRs as themselves (`gh-wrapper.sh`).
- `~/.config/hive/contributor.bluefin.env`: the Hive registration (knuckle hive). Its token is reissued with your gh identity on every `contribute-on` (`POST /api/contribute/reissue-token`, what upstream's `contribute-move` does) and written back to the file. That invalidates any other client using this registration.
- `~/.omp/profiles/ghost/agent/config.yml`: isolated Ghost OMP settings, shipped with `modelRoles` rewritten (`yq`) to the lab models. The workstation's default OMP settings are not changed.
- `~/.omp/profiles/ghost/agent/skills/local-model-*/SKILL.md` and `APPEND_SYSTEM.md`: curated capacity, Hive, learning and workflow skills plus additive guidance, staged into the `contribute-skills` ConfigMap.

Override the paths with `HIVE_CONTRIBUTE_REGISTRATION` and `HIVE_CONTRIBUTE_OMP_CONFIG`; `KUBECONFIG` defaults to `~/.kube/bluespeed.yaml`.

## Rules

- Keep `replicas: 1` for the model Deployments in git, so each new
  WaitForFirstConsumer PVC binds on first sync. `contribute` stays at 0 in git
  because its Secret only exists after `contribute-on`. `testing-lab-infra` ignores
  `/spec/replicas` for all three. That app is applied by hand:
  `kubectl apply -f argocd/infra-application.yaml`.
- Don't run the workstation appliance under the same registration while the
  lab is on.
- While the models are up they hold both GPUs, and `llm-d-preempt` evicts GPU
  video jobs. Turn contribution off to hand the GPUs back.
- `argo-cluster-read` excludes secrets, so Argo workflow pods cannot read the
  contribute Secret. Keep it that way.
- Pinned llama.cpp build: `server-vulkan-b11151`. It is the first build with
  `--reasoning-budget`, `--spec-draft-n-max` and `draft-mtp`. Ornith's official
  GGUF has no MTP head, so the worker runs without speculative decoding.

## Context, skills and learning

The Ghost profile and curated library are maintained in the owner's chezmoi
source. Create/apply that profile before `contribute-on`; missing skill files
fail startup rather than silently launching an uninformed agent. Keep secrets
and registration files outside dotfiles.

`seed-home` installs the four curated skills into OMP's native skill directory
and links user-level `AGENTS.md` to the Hive export at `~/agent.md`. Hive fetches
the export before starting OMP. User-level discovery keeps knowledge available
when tasks move into cloned repositories; the Hive assignment prompt and the
assigned repository's own instructions still determine scope and policy.
Use `omp --mode rpc --no-session` and `get_state` to inspect the actual system
prompt without spending a model turn. Do not attach to or type into another
operator's tmux session as a diagnostic probe.

Model `contextWindow` values must match the server contexts (worker 102400,
advisor 65536), not OMP's generic default. Updating these values is not a
reason to change the model's quantization or inference runtime.

Ghost enables `autolearn.enabled` and `autolearn.autoContinue`. Eligible stops
can trigger a capture turn; that does not guarantee a useful skill is written.
Only verified reusable lessons belong in managed skills. Curated/project policy
outranks learned shortcuts. No model-weight training takes place.

Only managed skills use the `contribute-learned-skills` PVC. Credentials,
sessions and workspaces stay on `emptyDir`; they are not made persistent merely
to retain procedural memory. Do not replace a busy pod without resolving its
active lease and preserving necessary work first.

`just contribute-learning-export` copies the managed library to a dated local
review directory under `~/.local/state/ghost-omp/learned-skills`. Review for
accuracy and secrets, then capture approved files into the isolated Ghost
profile with chezmoi. Never automatically sync raw transcripts, databases,
credentials or every learned hypothesis into dotfiles.
