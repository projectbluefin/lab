---
name: hive-contribute
description: >
  Run the projectbluefin/contribute Hive contributor (OMP in tmux) on the lab,
  driven by two local llm-d models: a worker on exo-0 and an advisor on ghost.
  Use when turning contribution on/off, attaching to watch it, changing the
  models or their flags, or debugging why the contributor is idle.
metadata:
  context7-sources:
    - /can1357/oh-my-pi
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

The recipes live in `~/.justfile`, so `just contribute-on` works from any directory. This repo's Justfile has `set fallback`, which reaches them too. If chezmoi manages that file, merge updates there: downloading a replacement overwrites machine-specific registration defaults. Otherwise install or refresh it:

```bash
curl -fsSL https://raw.githubusercontent.com/projectbluefin/lab/main/docs/skills/hive-contribute/contribute.just -o ~/.justfile
```

```bash
just contribute-on       # write Secret, scale models + contributor to 1, wait, attach to tmux
just contribute-attach   # re-attach (detach: C-b d; the contributor keeps running)
just contribute-status
just contribute-off      # scale all three to 0; frees both GPUs for video jobs
just contribute-learning-save # save learned skills to the isolated Ghost profile
```

The k8s MCP `resources_scale` tool can toggle the same three Deployments.
Scaling up through MCP reuses the last Secret that `contribute-on` wrote.

`contribute-on` sends, every time, with no login step:

- `gh auth token` → `GH_TOKEN`. Hive contributors fork and open PRs as themselves (`gh-wrapper.sh`).
- `~/.config/hive/contributor.bluefin.env`: the Hive registration (knuckle hive). Its token is reissued with your gh identity on every `contribute-on` (`POST /api/contribute/reissue-token`, what upstream's `contribute-move` does) and written back to the file. That invalidates any other client using this registration.
- `~/.omp/profiles/ghost/agent/config.yml`: isolated Ghost OMP settings, shipped with `modelRoles` rewritten (`yq`) to the lab models. The default workstation OMP configuration remains unchanged.
- `skills/`, `managed-skills/` and `APPEND_SYSTEM.md` from that profile: bundled into the existing Secret, then copied into the container's native OMP agent directory by `seed-home`.

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

## Context and learned skills

The owner maintains the Ghost profile and curated library in chezmoi. Native
skill paths are `~/.omp/agent/skills/<name>/SKILL.md` inside the container.
`APPEND_SYSTEM.md` adds guidance without replacing OMP's built-in prompt.
User-level `AGENTS.md` links to Hive's `~/agent.md` export, keeping that knowledge
visible when a task moves into a cloned repository. Project instructions and
skills remain authoritative over generic or learned guidance.

`contribute-learning-save` copies managed skills back to the separate Ghost
profile before normal on/off operations; startup restores them from the same
Secret bundle. Failed saves stop the operation instead of discarding lessons.
Review lessons for accuracy and secrets before `chezmoi add`. Capture is
additive, so prune obsolete lessons during review. Unexpected pod loss can
still lose unsaved lessons; no persistent volume is introduced.

## Verification

Check the actual prompt with a disposable OMP `--mode rpc --no-session`
instance and `get_state`, not a second process sharing an active session's
state. Confirm skill names, a distinctive Hive knowledge phrase, and matching
model context limits (worker 102400, advisor 65536). No inference call is needed
to verify context construction. Never use a copied Hive entrypoint to lease
additional work merely for a smoke check.

`get_state.data.systemPrompt` is an array of prompt blocks, not one string.
Inspect all blocks. Check that project `.omp/skills` and restored
`~/.omp/agent/managed-skills` entries appear alongside the curated library.
For a persistence check, save a disposable marker through the real recipe,
delete the disposable consumer, restore into a replacement and inspect its
OMP prompt; remove the marker from both the profile and Secret afterward.

Red flags: replacing the workstation profile, starting a second Hive relay
to inspect context, or treating an unsaved file in an ephemeral pod as durable.
