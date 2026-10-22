---
name: hive-contribute
description: >
  Run the projectbluefin/contribute Hive contributor (OMP in tmux) on the lab,
  driven by two local llm-d models served serially on both exo-0 and ghost.
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
| llama-server router (`--models-max 1`) serving `ornith` (`ornith-ai/Ornith-1.5-35B-A3B-GGUF` `Ornith-1.5-35B-Q6_K.gguf`, 100k ctx) and `qwen-advisor` (`unsloth/Qwen3.8-27B-GGUF` `Qwen3.8-27B-UD-Q6_K.gguf` + MTP draft, 64k ctx); per-model flags in ConfigMap `llm-d-presets` (`presets.ini`) | `llm-d/llm-d-exo-0`, exo-0, NodePort 30800 | `manifests/llm-d.yaml` |
| Same router and models | `llm-d/llm-d-ghost`, ghost, NodePort 30801 | `manifests/llm-d.yaml` |
| Contributor: `ghcr.io/projectbluefin/contribute:stable` | StatefulSet `contribute/contribute`, 2 replicas when on: `contribute-0` → exo-0 models, `contribute-1` → ghost models | `manifests/contribute.yaml` |

Each node holds one model at a time; the router swaps on request (LRU). Ornith
implements; Qwen reviews once at the end via OMP `advisor.reviewMode: agent-end`
in the Ghost profile `config.yml` (no per-turn advisor). A Qwen concern sends
Ornith back, which costs a model swap and a full re-prefill of Ornith's context.
OMP roles are pinned by the `contribute-omp` ConfigMap (`roles.yml`, layered
last via `PI_CONFIG_FILES`): `default` and `smol` → `ornith`, `advisor` →
`qwen-advisor`, on the pod's own node. No cloud provider keys enter the pod.

Both pods share one Hive registration: `HIVE_SESSION` is the pod name, and
relays with distinct sessions hold independent tasks under one account
(upstream hivecommons/hive v5 `contributor-relay.md`, "Running multiple
backends under one account").

## Use

The recipes live in `~/.justfile`, so `just contribute-on` works from any directory. This repo's Justfile has `set fallback`, which reaches them too. If chezmoi manages that file, merge updates there: downloading a replacement overwrites machine-specific registration defaults. Otherwise install or refresh it:

```bash
curl -fsSL https://raw.githubusercontent.com/projectbluefin/lab/main/docs/skills/hive-contribute/contribute.just -o ~/.justfile
```

```bash
just contribute-on       # write Secret, scale models to 1 and contributor to 2, warm up, attach to tmux
just contribute          # alias of contribute-on
just contribute-attach 1 # re-attach to contribute-0 (default) or 1 (detach: C-b d; it keeps running)
just contribute-status
just contribute-off      # scale everything to 0; frees both GPUs for video jobs
just contribute-learning-save # save learned skills to the isolated Ghost profile
```

`contribute-on` runs a warm-up gate: it loads `qwen-advisor` then `ornith` on
both nodes before starting the contributor. The first start downloads ~70 GB
per node.

The k8s MCP `resources_scale` tool can toggle the same Deployments and StatefulSet.
Scaling up through MCP reuses the last Secret that `contribute-on` wrote and skips the warm-up.

`contribute-on` sends, every time, with no login step:

- `gh auth token` → `GH_TOKEN`. Hive contributors fork and open PRs as themselves (`gh-wrapper.sh`).
- `~/.config/hive/contributor.bluefin.env`: the Hive registration (knuckle hive). Its token is reissued with your gh identity on every `contribute-on` (`POST /api/contribute/reissue-token`, what upstream's `contribute-move` does) and written back to the file. That invalidates any other client using this registration.
- `~/.omp/profiles/ghost/agent/config.yml`: isolated Ghost OMP settings, shipped with `modelRoles` rewritten (`yq`) to the lab models. The default workstation OMP configuration remains unchanged.
- `skills/`, `managed-skills/` and `APPEND_SYSTEM.md` from that profile: bundled into the existing Secret, then copied into the container's native OMP agent directory by `seed-home`. The bundle ships the `ponytail` skill (profile `skills/ponytail` symlinks the omp ponytail plugin's skill; tar dereferences it), and `APPEND_SYSTEM.md` makes it always-on.

Override the paths with `HIVE_CONTRIBUTE_REGISTRATION` and `HIVE_CONTRIBUTE_OMP_CONFIG`; `KUBECONFIG` defaults to `~/.kube/bluespeed.yaml`.

## Rules

- Keep `replicas: 1` for `llm-d-exo-0` and `llm-d-ghost` in git, so each
  WaitForFirstConsumer PVC binds on first sync (`llm-d-advisor-cache` is a
  historical name, kept so ArgoCD does not prune cached models). The
  `contribute` StatefulSet stays at 0 in git because its Secret only exists
  after `contribute-on`. `testing-lab-infra` ignores `/spec/replicas` for all
  three. That app is applied by hand: `kubectl apply -f argocd/infra-application.yaml`.
- Don't run the workstation appliance under the same registration while the
  lab is on.
- While the routers are up they hold both GPUs, and `llm-d-preempt` evicts GPU
  video jobs. Turn contribution off to hand the GPUs back.
- `argo-cluster-read` excludes secrets, so Argo workflow pods cannot read the
  contribute Secret. Keep it that way.
- Preset INI values cannot start with whitespace (llama.cpp `common/preset.cpp`
  skips `ws` after `=`), so Ornith's reasoning-budget message lost its leading space.
- Pinned llama.cpp build: `server-vulkan-b11151`. It is the first build with
  `--reasoning-budget`, `--spec-draft-n-max` and `draft-mtp`, and the router-mode
  build in use. Ornith's official GGUF has no MTP head, so it runs without
  speculative decoding.

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
model context limits (`ornith` 102400, `qwen-advisor` 65536), and that
`curl <node>:3080x/v1/models` lists both ids. No inference call is needed
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
