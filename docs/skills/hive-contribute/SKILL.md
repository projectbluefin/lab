---
name: hive-contribute
description: >
  Use when launching, attaching to, draining, or troubleshooting Hive
  contributors on the lab, changing their Ornith configuration, or verifying
  that a dedicated review gates publication.
metadata:
  context7-sources:
    - /can1357/oh-my-pi
    - /ggml-org/llama.cpp
    - /websites/cli_github_manual
    - /git/htmldocs
    - /casey/just
---

# hive-contribute

## When to Use

Use when launching, observing, or stopping the lab contributors or changing
their local Ornith model, publication gate, or deterministic tooling.

## When NOT to Use

Do not start another Hive relay merely to inspect context or test tools.
General cluster administration belongs to the relevant cluster operations skill.

## Core Process

1. Use the entrypoints below; preserve the separate workstation profile.
2. Exercise changed contracts in a disposable native OMP session first.
3. Follow the verification checks before activating changes at a clean start.

## Shape

| Piece | Where | Manifest |
|---|---|---|
| `llm-d/llm-d-exo-0`, exo-0, NodePort 30800 | Ornith worker and dedicated reviewer on exo-0 | `manifests/llm-d.yaml` |
| `llm-d/llm-d-ghost`, ghost, NodePort 30801 | Ornith worker and dedicated reviewer on ghost | `manifests/llm-d.yaml` |
| StatefulSet `contribute/contribute-reviewed` | One contributor per node with hard pod anti-affinity; `spec.nodeName` seeds that node's model URL | `manifests/contribute.yaml` |

Both lanes use `lab-worker/ornith`, served with one slot per node. A lane
reviews only after its worker has handed off a commit; no per-turn advisor or
cross-node requests. Both pods share one Hive registration but set distinct
`HIVE_SESSION` values from their pod names. Separate sessions allow separate
leases; the hub still decides whether each session is eligible for work.

## Use

The canonical recipes are in `docs/skills/hive-contribute/contribute.just`.
The workstation's chezmoi `dot_justfile` delegates there, keeping its Reef
registration default (`~/.config/hive/contributor.tunaos-lab.env`). The Lab
Justfile's `set fallback := true` lets Just find that home file when invoked
from the repo. Point `lab_contribute` in the wrapper at the checkout containing
the dual-review launcher. Do not replace the home Justfile with the raw Lab recipe.

```bash
just contribute on       # warm both nodes and launch the pair, or attach to a running pair
just contribute models   # warm both native-window servers without Hive registration or leases
just contribute attach   # attach without making changes on the cluster
just contribute status   # node/pod state and Ornith slots, no inference
just contribute off      # release both GPUs once contributors are drained and stopped
just contribute-learning-save
```

`contribute-on`, `contribute-attach`, `contribute-status`, and `contribute-off`
are still accepted. The `hive-lab` session puts exo-0 on the left and ghost on
the right, independent of pod ordinal. Input is not synchronized. Detach
with `Ctrl-b d`; it does not stop either worker. `HIVE_CONTRIBUTE_NO_ATTACH=1`
waits for both lanes without opening tmux. Run `attach` to reconnect a dead pane.

A clean `on` requires a StatefulSet explicitly marked `dual-review-v1`. Do
not add that annotation to bypass the gate: it certifies that the deployed
image enforces review and credential isolation. `on` validates the context
bundle and checks a single 262,144-token MTP slot and one generated token on
each node before reissuing one registration token. No long prompt runs at startup.
If both workers are already running, `on` just attaches without rotating
credentials. Supply one hub per file; override with `HIVE_CONTRIBUTE_REGISTRATION`.

`off` must not race a new assignment. An idle status sample is not a drain
acknowledgment. `off` checks every relay, requests a drain, waits for current
leases to settle, saves skills, then scales down contributors followed by
the models. If a relay lacks drain support, `off` refuses to scale while any
contributor replica or pod remains. Export transcripts before shutdown.

Every `contribute-on` launch reads `mcp.json` from the `contribute-omp`
ConfigMap and includes it in the existing context bundle. `seed-home` extracts
it into `~/.omp/agent/mcp.json`. It enables the Bluefin knowledge server
(`https://mcp.projectbluefin.io/mcp`) and Context7
from the ConfigMap, without copying workstation provider credentials into the
OMP bundle. `seed-home` restores curated skills and the system appendix from
the Ghost profile, and shares only the lessons in its approved `managed-skills`.

The worker and reviewer run in separate containers without GitHub or Hive
credentials. Only the relay mounts the registration and holds the lease's
publication token; it shares Git bundles with the worker and talks to the
reviewer over a separate private Unix socket. Disable pod process sharing
and service-account token mounts. Never put the workstation token in the
startup Secret: it authenticates the reissue request only.

The ConfigMap also ships `safe-tools.js`, `checkout.py`, and `verify.py` through
the context bundle into `~/.omp/agent/hooks/pre/`. New OMP sessions discover
the extension there. The recipe appends `tooling.md` to the Ghost profile's
`APPEND_SYSTEM.md` in the bundle without editing the profile itself.

- `contribute_checkout` accepts a Git URL, a new absolute destination, an
  optional ref and positive depth (default 1). It fetches that ref into a
  detached checkout and returns its absolute `cwd` and commit. Existing
  destinations are rejected; no fork, push, deletion, retry, or global Git
  interception occurs. Extra history and fork setup remain explicit Git work.
- Every native `bash` call must supply an existing absolute `cwd`; missing,
  relative, nonexistent and non-directory values are blocked before execution.
  Commands are not rewritten and no directory is silently inferred.
- `contribute_verify` takes `cwd`, a `runner` (`unittest` or `junit`), and exact
  `required` test IDs. Unittest loads the supplied `tests` selectors (defaulting
  to `required`) with the selected `python` interpreter. JUnit runs an explicit
  argv `command` containing one `{report}` placeholder and checks its fresh
  report. Required tests must actually pass; missing, skipped, duplicate JUnit
  IDs, failed processes or failed reports reject verification. Unrelated skips
  remain allowed.
- Optional `setup` is a list of caller-selected argv commands, executed once
  in order before testing. Any setup failure stops verification. Use the
  repository's supported isolated environment and declared dependencies;
  neither package manager nor packages are guessed or automatically installed.

These helpers are mechanical gates, not a claim that tests cover the task or
that the chosen working directory is semantically correct. Native read/edit
syntax and complete acceptance-context reads still require model judgment.
These are tool-level contracts: arbitrary Git/test commands and Hive completion
signals are not intercepted. JUnit evidence comes from the caller's trusted
test runner; it is not an attestation against deliberately fabricated reports.
New extension modules are not assumed hot-loadable in OMP 18.12.6: install
them in the existing startup bundle, verify a disposable native session, and
activate on a clean contributor start without abandoning a live task.
Source: [pinned hook discovery and interception](https://github.com/can1357/oh-my-pi/blob/v18.12.6/docs/hooks.md).
Use an ESM `export default` factory for the pinned native loader. Local
`file://` checkouts use Git's built-in upload-pack command because the slim
image may omit its standalone executable.

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
- Keep the Ornith preset's `ctx-size` and the OMP catalog's `contextWindow`
  in sync. Check `GET /props?model=ornith` (`default_generation_settings.n_ctx`)
  and `GET /slots?model=ornith` on each endpoint. Canary changes with a cold
  prompt beyond the old window, not just a successful model load. Check
  `/proc/meminfo` and GPU driver counters separately: CPU and GPU share RAM.
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

- `on` refuses to launch against the legacy runtime: do not patch the
  capability annotation or directly scale up. Wait for a pinned, verified
  review-capable image and a drained rollout. Kubernetes cannot add
  `volumeClaimTemplates` to the existing StatefulSet. Sync the replacement
  under `contribute-reviewed` only after the old replica is stopped; do not
  force recreation of a StatefulSet with active work.
- Review silence, invalid output, a mismatched commit, timeout, lost lease,
  or a second negative review must never publish. A passing result binds
  publication to the reviewed SHA; it does not trust a pane verdict or a
  branch name that the worker can still move.
- Protected publishing requires a Hub-authorized direct-push GitHub issue
  or a single fix to an existing same-repository PR. A fork route, unsupported
  forge, or unverified base branch fails closed. Check the hub's actual
  assignment routes before enabling it; a connected relay is not evidence
  that the hub can issue a publishable lease.
- A disposable OMP catalog with unknown reasoning support probes multiple
  effort levels on first use. Set the reviewer-only model's catalog
  `reasoning: false` to avoid those extra requests; the llama.cpp preset
  controls Ornith's native reasoning. Confirm the adapter sends exactly one
  inference request using a disposable endpoint, not a busy GPU.
- `400 model name=ornith is not running` in a contributor pane: the router
  evicted a model as idle between admitting a request and proxying it, because
  requests for the other model arrived at the same time. It persists in b11151
  despite upstream #29217. OMP does not retry a 400, so the session stops.
  Unstick it with `kubectl -n contribute exec contribute-<n> -c contribute -- tmux send-keys -t contributor '<continue message>' Enter`.
  Avoid overlapping requests for the two models: never send manual requests to
  a router while its contributor works.
- The recipe uses `jq`, `gh`, `kubectl`, `flock`, and `tmux` executables on
  `PATH`. If startup fails after the reissue, check that there are no active
  pods before retrying; reissue invalidates the previous registration token.
- Applying `argocd/infra-application.yaml` with new Deployment names before the
  manifests reach `main` removes the old names from the ignore list, so
  self-heal scales the old Deployments back up until the sync prunes them.
  Push the manifests first, or apply the app right after the push.

## Context and learned skills

The owner maintains the Ghost profile and curated library in chezmoi. Native
skill paths are `~/.omp/agent/skills/<name>/SKILL.md` inside the container.
`APPEND_SYSTEM.md` adds guidance without replacing OMP's built-in prompt.
User-level `AGENTS.md` points to the relay's read-only Hive knowledge export
so it stays visible after switching repositories. Project instructions and
skills remain authoritative over generic or learned guidance.

`contribute-learning-save` captures lessons in
`~/.local/state/hive/lab/review-pending-skills`, not in the startup bundle.
Review them for accuracy and secrets, then copy selected skill directories into
the profile's `managed-skills` to share them on the next launch. Differing
versions of the same lesson stop capture; select a pod explicitly and reconcile.

Worker and reviewer transcripts are ephemeral; export them before shutdown
(`~/.omp/agent/sessions` in `contribute`, `~/.omp/review-sessions` in
`reviewer`). The relay's evidence volume is for compact redacted review records only.
Auto-Learn is enabled for workers only. A dedicated reviewer starts with
no Auto-Learn or worker skills. Auto-Learn writes managed skills, never
Ornith weights; review captured lessons before restoring them on both nodes.

## Verification

Check the actual prompt with a disposable OMP `--mode rpc --no-session`
instance and `get_state`, not a second process sharing an active session's
state. Confirm skill names, a distinctive Hive knowledge phrase, and matching
worker model context limits (`ornith` 262144), and that
`curl <node>:3080x/v1/models` lists the Ornith id. No inference call is needed
to verify context construction. Never use a copied Hive entrypoint to lease
additional work merely for a smoke check.

`get_state.data.systemPrompt` is an array of prompt blocks, not one string.
Inspect all blocks. Check that project `.omp/skills` and restored
`~/.omp/agent/managed-skills` entries appear alongside the curated library.
For a persistence check, save a disposable marker through the real recipe,
delete the disposable consumer, restore into a replacement and inspect its
OMP prompt; remove the marker from both the profile and Secret afterward.

- [ ] `dumpTools` registers both helper names in a disposable native session.
- [ ] Invalid-cwd bash cannot write a file; valid-cwd bash executes.
- [ ] A real checkout returns the requested commit and depth.
- [ ] Required regression skips and missing tests fail the gate.
- [ ] Startup bundle errors do not reissue registration.
- [ ] Existing owner context and credentials remain unchanged.
- [ ] Distinct Ready sessions have overlapping leases and each uses only its local GPU.
- [ ] Reject, timeout, revoke, stale SHA and failed publication cannot produce a success verdict.
- [ ] Review evidence survives pod replacement; model processes cannot read publication credentials.

## Common Rationalizations

- "A zero test-process exit proves the regression passed" — require its exact
  executed test ID and passing outcome, not a skip or absent report entry.
- "Editing an extension file activates it" — verify native discovery on a new
  session; do not claim a live reload without observing execution.


## Red Flags

Replacing the workstation profile, starting a second Hive relay to inspect
context, treating an unsaved ephemeral file as durable, or claiming a guard is
active before observing its execution.
