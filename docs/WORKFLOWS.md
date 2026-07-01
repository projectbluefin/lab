# Workflow Reference

## Table of Contents
- [Pipelines](#pipelines)
  - [bluefin-qa-pipeline](#bluefin-qa-pipeline)
  - [dakota-qa-pipeline](#dakota-qa-pipeline)
  - [knuckle-qa-pipeline](#knuckle-qa-pipeline)
- [Supporting Templates](#supporting-templates)
- [Nightly Schedule](#nightly-schedule)
- [Ghost-Heavy-Compute Mutex](#ghost-heavy-compute-mutex)
- [Disk Paths](#disk-paths)
- [Resource Profiles](#resource-profiles)

## Pipelines

### bluefin-qa-pipeline
- **Purpose:** End-to-end Bluefin validation from golden disk check/build through ephemeral VM boot and GNOME test suites.
- **Parameters:** `image`, `image-tag`, `namespace`, `suites`, `variant`, `ssh-key-secret`, `branch`.
- **DAG:** `ensure-disk` → `provision` → `run-smoke` → `run-developer` → `run-software`; `onExit: teardown`.
- **Resource profile:** ContainerDisk build (`build-containerdisk/install-to-disk`, `build-containerdisk/convert-and-push`), VM clone/boot (`reflink-disk`, `wait-for-vm-ready`), and `run-gnome-tests`.
- **Just recipe:** `run-tests`, `run-tests-tag`, `run-tests-matrix`.

### dakota-qa-pipeline
- **Purpose:** Validate the existing Dakota containerDisk by provisioning ephemeral VMs and running selected suites in parallel lanes.
- **Parameters:** `image`, `image-tag`, `containerdisk-tag`, `namespace`, `suites`, `variant`, `ssh-key-secret`, `branch`, `vm-memory`.
- **DAG:** `assert-cd` → parallel `test-lane` items (`smoke`, `developer`, `system`); `onExit: cleanup-orphan-vms`.
- **Resource profile:** Per-suite VM provision + `run-gnome-tests`; fails fast if containerDisk is missing from Zot.
- **Just recipe:** `run-dakota-qa`.

### knuckle-qa-pipeline
- **Purpose:** Build the Knuckle installer ISO/binary, provision a blank VM, run the headless installer in-cluster, boot the installed system, rediscover SSH reachability, and run smoke tests.
- **Parameters:** `branch`, `namespace`, `suite`, `ssh-key-secret`, `tests-branch`.
- **DAG:** `clone-source` → `build-installer` → `provision-target-vm` → `boot-installer` → `wait-install-complete` → `transition-to-installed` → `discover-ip` → `run-smoke-tests`; `onExit: teardown` (`teardown-vm` → `cleanup-installer-artifacts`).
- **Resource profile:** Installer build on ghost, several small control pods for ignition/install orchestration, then `run-gnome-tests` against the installed VM.
- **Just recipe:** None currently; submit the WorkflowTemplate directly or use the nightly CronWorkflow.

## Supporting Templates

| Template | Role |
| --- | --- |
| `build-containerdisk` | Shared containerDisk builder. Flow: `check` (zot existence) → `install-to-disk` → `convert-and-push`. Builds a KubeVirt containerDisk from a bootc image and pushes to the local registry. Replaces the old `bib-build-and-push` hostDisk approach. |
| `provision-bluefin-vm` | Shared Bluefin/Dakota VM bring-up. `reflink-disk` clones `disk.raw`, `create-vm` defines a 4 vCPU / 8 GiB KubeVirt VM, and `wait-for-vm-ready` returns the pod IP once SSH is reachable. |
| `teardown-bluefin-vm` | Shared Bluefin/Dakota/Knuckle VM cleanup. Deletes the KubeVirt VM and removes the matching hostDisk from the pipeline test root. |
| `run-gnome-tests` | Shared test runner. Clones `projectbluefin/testsuite` (the single source of truth), waits for SSH, installs test dependencies, copies `tests/<suite>`, and runs `behave`. GUI suites (smoke) run via `qecore-headless` inside the VM. The `common` suite runs from the runner container with `VM_IP`/`SSH_KEY` exported so its SSH steps reach the VM directly — do NOT run common via qecore-headless. The `system` suite runs inside the VM without a display. |
| `run-incluster-tests` | Shared in-cluster pytest runner. Git-syncs `lab`, runs a pytest module against a live k8s workload, emits JUnit XML. |
| `dakota-build-pipeline` | Dakota BST build path. Builds `oci/bluefin.bst` and `oci/bluefin-nvidia.bst` in parallel via in-cluster buildbox-casd and pushes tags to local Zot. |

## Nightly Schedule

| CronWorkflow | Time (UTC) | Pipeline | Parameters |
| --- | --- | --- | --- |
| `nightly-smoke` | 02:00 | `bluefin-qa-pipeline` | `image=ghcr.io/projectbluefin/bluefin`, `image-tag=testing`, `namespace=bluefin-test`, `suites=smoke,developer,system` |
| `nightly-smoke-lts` | 02:30 | `bluefin-qa-pipeline` | `image=ghcr.io/projectbluefin/bluefin-lts`, `image-tag=lts-testing`, `namespace=bluefin-lts-test`, `suites=smoke,developer,system` |
| `nightly-dakota` | 03:00 | `dakota-qa-pipeline` | `variant=default`, `branch=main`, `namespace=bluefin-test`, `suites=smoke` |
| `nightly-knuckle` | 03:30 | `knuckle-qa-pipeline` | `branch=main`, `namespace=knuckle-test`, `suite=smoke`, `tests-branch=main` |

## Ghost-Heavy-Compute Mutex

`ghost-heavy-compute` serialises host-local heavy work for shared disk builders.

Templates that hold it:
- `build-containerdisk/install-to-disk`
- `build-containerdisk/convert-and-push`
- `knuckle-qa-pipeline/build-installer`

In practice this means Bluefin/Dakota containerDisk work, Dakota BST builds, and Knuckle installer builds queue instead of racing each other.

## Disk Paths

| Pipeline | Golden root | Test root | Notes |
| --- | --- | --- | --- |
| Bluefin | `/var/tmp/bluefin-golden` | `/var/tmp/bluefin-test` | Golden disk at `<golden-root>/<image-tag>/disk.raw`; reflinked VM disks at `<test-root>/<vm-name>.raw`. |
| Dakota | `/var/tmp/dakota-golden` | `/var/tmp/dakota-test` | Dakota overrides both containerDisk/provision paths so its OCI-derived disks stay separate from Bluefin. |
| Knuckle | N/A | `/var/tmp/knuckle-test` | Stores installer ISO, ignition, QA config, helper binary, and installed target disk under the same root. |

## Resource Profiles

Pod resource requests/limits used by workflow steps:

| Template | CPU req/limit | Memory req/limit |
| --- | --- | --- |
| `build-containerdisk/check` | 100m / 500m | 128Mi / 512Mi |
| `build-containerdisk/install-to-disk` | 4 / 8 | 8Gi / 16Gi |
| `build-containerdisk/convert-and-push` | 2 / 4 | 4Gi / 8Gi |
| `reflink-disk` | 100m / 500m | 128Mi / 512Mi |
| `wait-for-vm-ready` | 100m / 500m | 128Mi / 256Mi |
| `run-gnome-tests` | 1 / 2 | 1Gi / 2Gi |
| `delete-hostdisk` | 50m / 200m | 64Mi / 128Mi |
| `dakota-build-pipeline/bst-build` | 4 / 8 | 14Gi / 28Gi |
| `knuckle resolve-source` | 100m / 500m | 128Mi / 256Mi |
| `knuckle build-installer` | 4 / 4 | 8Gi / 8Gi |
| `knuckle write-ignition` | 100m / 500m | 128Mi / 256Mi |
| `knuckle prepare-target-disk` | 500m / 2 | 512Mi / 2Gi |
| `knuckle boot-installer` | 1 / 2 | 1Gi / 2Gi |
| `knuckle wait-install-complete` | 250m / 1 | 256Mi / 512Mi |
| `knuckle transition-to-installed` | 250m / 1 | 256Mi / 512Mi |
| `knuckle discover-installed-ip` | 250m / 1 | 256Mi / 512Mi |
| `knuckle cleanup-installer-artifacts` | 50m / 200m | 64Mi / 128Mi |
