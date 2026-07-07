# Bluefin Integration

This homelab instance is the CI backend for Project Bluefin. Every image publish
results in a full acceptance test run with zero human intervention; screenshots from
passing runs appear automatically in Bluefin GitHub Releases.

---

## Images Under Test

| Image | Tag | Namespace | Trigger |
|---|---|---|---|
| `ghcr.io/projectbluefin/bluefin` | `testing` | `bluefin-test` | Nightly 02:00 UTC + digest change |
| `ghcr.io/projectbluefin/bluefin-lts` | `testing` | `bluefin-lts-test` | Nightly 02:30 UTC + digest change |
| `ghcr.io/frostyard/snow` | `latest` | `snosi-test` | Every 3 hours + digest change |
| `ghcr.io/projectbluefin/dakota` | latest BST build | `bluefin-test` | Nightly 03:00 UTC + BST build |

`:testing` is the only production branch tested continuously. `:stable` runs are
supported but not scheduled by default. Never use `:latest` or date tags in
automation — `bluefin-lts` has no `:latest` tag.

---

## Test Suites

All Bluefin image test scenarios live in **[`projectbluefin/testsuite`](https://github.com/projectbluefin/testsuite)** — the single source of truth. The lab's `run-gnome-tests` WorkflowTemplate clones `testsuite` (main or a branch) and runs qecore-headless + behave against a real KubeVirt VM. The same test code also runs in GitHub Actions (`e2e.yml`) against a QEMU VM.

Each pipeline run executes one or more suites via the `suites` parameter (comma-separated).

### smoke
GNOME Shell acceptance tests. Validates the desktop is functional after boot.

- Activities button opens and closes the overview
- `dash-to-dock` is present and responds to AT-SPI
- `blur-my-shell` and app-indicator extensions are loaded (Bluefin-specific)
- Top-bar clock and quick-settings accessible via Shell.Eval JS (GNOME Shell 50)
- Screenshot captured at end of each scenario

### developer
Bluefin developer tooling validation. Validates the tools Bluefin ships for developers.

- Homebrew: `brew` in PATH, `brew install` resolves packages
- Podman: `podman run hello-world` succeeds
- Distrobox: `distrobox list` runs without error
- Dev mode (`ujust enable-dev-mode`): systemd service activated, no fatal journal entries
- Ptyxis terminal opens and accepts input

### common
Atomic OS contract and system health tests. Validates Bluefin's immutable-image guarantees.

- `bootc status` reports a known good deployment
- `/usr` is mounted read-only
- XDG portal health + integration
- Flatpak model and state
- polkit rules, shell environment, ujust recipes
- GSettings/dconf defaults, desktop entries, signing assertions

### flatcar (separate pipeline)
Flatcar OS substrate tests. Not part of the Bluefin image pipelines; runs via
`flatcar-smoke-test.yaml`.

---

## Image-Poll Trigger

Hourly CronWorkflows (`image-poll-bluefin-testing`, `image-poll-lts-testing`) call
the `image-poller` WorkflowTemplate. Each run:

1. Pulls the current digest for the target image from ghcr.io
2. Reads the last-known digest from a ConfigMap (`image-polling-state` in `argo`)
3. If digests match: exits cleanly (no test run)
4. If digest changed: updates the ConfigMap, then submits a `bluefin-qa-pipeline` run

This means every new Bluefin image publish triggers a test run within one hour,
automatically, with no human action.

---

## Screenshot Pipeline

`run-gnome-tests` captures a desktop PNG at the end of each test scenario via
`qecore-headless`. The screenshot pipeline:

1. PNGs written to the test VM at the path the test runner specifies
2. SCPed from the VM to the workflow pod's working directory
3. Pod pushes to the OCI registry via `oras push`:
   ```
   ghcr.io/projectbluefin/testsuite/desktop-screenshot:<slug>-<suite>-latest
   ```
   where `<slug>` is the image tag (e.g. `testing`, `lts-testing`)
4. `publish-to-pages.yml` in [projectbluefin/testsuite](https://github.com/projectbluefin/testsuite)
   runs every 2h, pulls each `*-latest` tag, and pushes the PNG to GitHub Pages at:
   ```
   https://projectbluefin.github.io/testsuite/screenshots/<slug>-<suite>-latest.png
   ```
5. `reusable-release.yml` in [projectbluefin/actions](https://github.com/projectbluefin/actions)
   reads the GitHub Pages URL and embeds the screenshot directly in the GitHub Release body

The result: a Bluefin release is published → tests pass → the release notes
automatically contain a desktop screenshot from a real VM boot.

---

## Triggering a Test Run Manually

```bash
# Smoke suite against bluefin:testing (latest tag)
just run-tests

# Smoke suite against bluefin-lts:testing
just run-tests-tag lts

# Full suite (smoke + developer + system)
just run-tests-full

# Submit a named workflow directly
argo submit argo/bluefin-smoke-test.yaml --watch
```

The `just` wrappers are thin shims around `argo submit`. See `Justfile` for the
full parameter set.

---

## PR Label Trigger (`test-on-lab`)

The `pr-label-poller` CronWorkflow runs every 5 minutes and scans the
`projectbluefin` org for open PRs labeled `test-on-lab`. For each matching PR it
has not already processed (idempotency tracked via GitHub commit status), it:

1. Identifies the target repo (bluefin, bluefin-lts, dakota, etc.)
2. Dispatches the appropriate WorkflowTemplate with the PR's branch/SHA as parameters
3. Sets a `pending` commit status on the PR SHA
4. On workflow completion, sets `success` or `failure` commit status

To trigger: add the `test-on-lab` label to a PR in the `projectbluefin` org.
The poller picks it up within 5 minutes.

---

## Dakota

Dakota runs through `dakota-qa-pipeline` rather than `bluefin-qa-pipeline`. The
pipeline runs BST validate → BST build → provisions a VM from the built artifact →
runs `smoke` and `system` suites. Dakota PRs can also use the `test-on-lab` label
via the PR label poller.
