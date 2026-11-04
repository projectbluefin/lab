---
name: hive-contribute
description: >
  Run the projectbluefin/contribute Hive contributor (OMP in tmux) on the lab,
  driven by local llm-d Ornith models on exo-0 (worker) and ghost (advisor).
  Use when turning contribution on/off, attaching to watch it, changing the
  models or their flags, or debugging why the contributor is idle.
metadata:
  context7-sources:
    - /can1357/oh-my-pi
    - /ggml-org/llama.cpp
    - /websites/cli_github_manual
---

# hive-contribute

## Shape

| Piece | Where | Manifest |
|---|---|---|
| llama-server (`ornith-ai/Ornith-1.5-35B-A3B-GGUF` `Ornith-1.5-35B-Q6_K.gguf`, 100k ctx); flags in ConfigMap `llm-d-presets` (`presets.ini`) | `llm-d/llm-d-exo-0`, exo-0, NodePort 30800 (worker) | `manifests/llm-d.yaml` |
| Same Ornith model as dedicated advisor | `llm-d/llm-d-ghost`, ghost, NodePort 30801 (advisor) | `manifests/llm-d.yaml` |
| Contributor: `ghcr.io/projectbluefin/contribute:stable` | StatefulSet `contribute/contribute`, 1 replica when on: `contribute-0` on exo-0 | `manifests/contribute.yaml` |

exo-0 runs the Ornith worker model and ghost runs the Ornith advisor model.
Ornith implements; the advisor reviews concurrently per turn via OMP `advisor.reviewMode: turn` with `advisor.syncBacklog: "off"`.
OMP roles are configured by `contribute-on`: `default` and `smol` → `ornith` on exo-0,
`advisor` → `ornith` on ghost (or optionally Luna via `advisor="luna"`).

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
just contribute-on       # write Secret, scale models to 1, warm up, scale contributor to 1 and attach
just contribute          # alias of contribute-on
just contribute-attach   # attach to contribute-0 (detach: C-b d)
just contribute-status
just contribute-off      # scale everything to 0; frees both GPUs for video jobs
just contribute-learning-save # save learned skills to the isolated Ghost profile
```

`contribute-on` runs a warm-up gate before starting the contributor: it ensures
`ornith` is loaded and responsive on both nodes.

The k8s MCP `resources_scale` tool can toggle the same Deployments and StatefulSet.
Scaling up through MCP reuses the last Secret that `contribute-on` wrote and skips the warm-up.

`contribute-on` sends, every time, with no login step:

- `gh auth token` → `GH_TOKEN`. Hive contributors fork and open PRs as themselves (`gh-wrapper.sh`).
- The registration file (`HIVE_CONTRIBUTE_REGISTRATION`; the repo default is `~/.config/hive/contributor.bluefin.env`, the owner's chezmoi default is `contributor.tunaos-lab.env`). Its `HIVE_HUB` picks the hive, so it decides whose work the lab takes. Its token is reissued with your gh identity on every `contribute-on` (`POST /api/contribute/reissue-token`, what upstream's `contribute-move` does) and written back to the file. That invalidates any other client using this registration. The recipe reissues only the first hub, so keep one hub per file.
- `~/.omp/profiles/ghost/agent/config.yml`: isolated Ghost OMP settings, shipped with `modelRoles` rewritten (`yq`) to the lab models. The default workstation OMP configuration remains unchanged.
- `skills/`, `managed-skills/` and `APPEND_SYSTEM.md` from that profile: bundled into the existing Secret, then copied into the container's native OMP agent directory by `seed-home`. The bundle ships the `ponytail` skill (profile `skills/ponytail` symlinks the omp ponytail plugin's skill; tar dereferences it), and `APPEND_SYSTEM.md` makes it always-on.

Override the paths with `HIVE_CONTRIBUTE_REGISTRATION` and `HIVE_CONTRIBUTE_OMP_CONFIG`; `KUBECONFIG` defaults to `~/.kube/bluespeed.yaml`.

Every `contribute-on` launch reads `mcp.json` from the `contribute-omp`
ConfigMap and includes it in the existing context bundle. `seed-home` extracts
it into `~/.omp/agent/mcp.json`. It enables the Bluefin knowledge server
(`https://mcp.projectbluefin.io/mcp`) and Context7
(`https://mcp.context7.com/mcp`) over HTTP, independently of workstation or
Ghost-profile MCP settings. No workstation API keys are copied. The canonical
configuration lives in `manifests/contribute.yaml`; both local and Luna
launches use it. Changing this ConfigMap does not restart the contributor.

The same ConfigMap's `tooling.md` is appended to the profile's
`APPEND_SYSTEM.md` in the bundle, without editing the workstation profile.
It establishes shallow initial checkouts with on-demand history, explicit
per-call working directories, native scoped investigation tools, and complete
acceptance-context reads. Repository policy still wins. GitHub CLI forwards
clone flags after `--`: `gh repo clone OWNER/REPO PATH -- --depth=1`
([source](https://cli.github.com/manual/gh_repo_clone)).
Native edits need the current non-raw read snapshot and `PUT`/`CUT` operations;
copied old numbered lines are not a patch. Dependency-gated regression tests
must execute in the repository's supported environment, not merely skip.

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
- Pinned llama.cpp build: `server-vulkan-b11151`. Its native MTP path reuses
  target weights; enable it with `spec-type = draft-mtp` and
  `spec-draft-n-max = 1` in the Ornith preset. The cached official Q6_K file
  contains a draft head (`blk.40.*`); without MTP the server ignores those
  tensors. Do not set a separate draft-model path: that loads another model.
  Verify `/slots?model=ornith` reports `speculative: true` and compare the
  response's `timings.predicted_per_second` and draft acceptance counters.
  Source: [pinned MTP initialization](https://github.com/ggml-org/llama.cpp/blob/b11151/common/speculative.cpp#L2532-L2588).
- Benchmark only on an isolated node: disable the advisor with `/advisor off`,
  unload its router model, and run the same binary and cached GGUF on a separate
  loopback port. Keep context, batching, sampling and prompts identical between
  configurations. Stop the disposable server and reload the router model before
  `/advisor on`. Never benchmark against the busy worker. Code-quality probes
  need an output allowance above the 3600-token reasoning budget; a shorter
  allowance can consume every token in reasoning and return no final code.

## Choosing the hive

Hives are separate hubs, each with its own registration. hivecommons-hosted
hives appear in `https://hive.hivecommons.dev/api/saas/my-hives` (what
`hive-contribute hives` reads); self-hosted forks do not. tunaOS runs its own
spokes, for example `reef` (`wss://reef.tunaos.org/contribute`, the `tuna-os`
org; layout in `hanthor/dotfiles` `talos-k8s/hive/README.md`).

To join a hub where your GitHub account is already registered (check
`GET <hub>/api/contribute/me` with your gh token as bearer), `register` will not
hand the token back. Reissue it instead:
`POST <hub>/api/contribute/reissue-token` (bearer = `gh auth token`) returns
`registration_token` and `contributor_id`. Write them with the hub into a
registration file (`HIVE_REGISTRATION_TOKEN`, `HIVE_HUB`, `CONTRIBUTOR_ID`,
`AGENT_BACKEND=omp`, mode 600) and point `HIVE_CONTRIBUTE_REGISTRATION` at it.
`hivectl hives add` does the same for multi-hub files, but needs podman and only
edits `~/.config/hive/contributor.env`.

## Failure modes

- `/advisor status` shows `[no model]` although `modelRoles.advisor` is set:
  compare the selected role with `kubectl -n contribute exec contribute-0 -c contribute -- omp models lab-advisor --json`.
  Local mode requires `lab-advisor/ornith`; a catalog that still lists only
  `lab-advisor/qwen-advisor` cannot resolve it. The catalog in
  `manifests/contribute.yaml` must register `ornith` at
  `http://llm-d-ghost.llm-d.svc.cluster.local:8000/v1`.
  Wait until the active contributor finishes or is idle before pushing the
  manifest changes to git: ArgoCD auto-syncs `testing-lab-infra` on push, and
  the pod template diff (removing `POD_NAME` and updating `seed-home`) triggers
  an immediate rolling restart under Kubernetes default `RollingUpdate`, while
  `manifests/llm-d.yaml` removes init containers from the model servers.
  Push to `main` only when the active task completes, then run
  `just contribute-on` to seed the refreshed `models.yml` and attach.
  `seed-home` copies `models.yml` at pod initialization: syncing the ConfigMap
  alone does not update the running pod's `~/.omp/agent/models.yml`.
  Do not overwrite workstation settings or start another Hive relay to debug it.
- `400 model name=ornith is not running` in a contributor pane: the router
  evicted a model as idle between admitting a request and proxying it, because
  requests for the other model arrived at the same time. It persists in b11151
  despite upstream #29217. OMP does not retry a 400, so the session stops.
  Unstick it with `kubectl -n contribute exec contribute-<n> -c contribute -- tmux send-keys -t contributor '<continue message>' Enter`.
  Avoid overlapping requests for the two models: never send manual requests to
  a router while its contributor works.
- `contribute-on` needs `jq`, `yq`, `gh` and `kubectl` as real binaries on PATH.
  A shell function or alias works interactively and fails in a fresh shell.
  If it fails after the reissue POST, just rerun it: the reissue authenticates
  with gh, so the discarded token is replaced.
- Applying `argocd/infra-application.yaml` with new Deployment names before the
  manifests reach `main` removes the old names from the ignore list, so
  self-heal scales the old Deployments back up until the sync prunes them.
  Push the manifests first, or apply the app right after the push.

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
model context limits (`ornith` 102400), and that
`curl <node>:3080x/v1/models` lists the Ornith id. No inference call is needed
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
