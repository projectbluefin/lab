---
name: bluefin-server
description: >
  The Bluefin Server lab loop on ghost: dev-key BuildStream builds of the
  release image set (bluefin-server-build-pipeline) and the unattended USB
  installer boot test (bluefin-server-boot-test). Use when building a
  projectbluefin/server branch in the lab, iterating on the installer or
  image, or debugging those two WorkflowTemplates.
---

# Bluefin Server — lab Skill

## When to Use

- Building a `projectbluefin/server` branch in the lab before pushing it.
- Iterating on the image set or the USB installer without waiting on GitHub
  Actions.
- Debugging `bluefin-server-build-pipeline`, `bluefin-server-boot-test`, or the
  server leg of `bst-commit-poller`.

## When NOT to Use

- Release builds and signing: GitHub Actions (`build.yml` in
  projectbluefin/server) is the only place the release keys exist and the only
  thing that publishes releases.
- KubeVirt VM mechanics in general → `kubevirt-vms.md`.

## The model: lab on dev keys, releases on GitHub Actions

| | Lab (ghost) | GitHub Actions |
|---|---|---|
| Keys | Fixed dev set, Secret `argo/bluefin-server-dev-boot-keys` | Release keys (`BOOT_KEYS_TARBALL`, `SYSUPDATE_SIGNING_KEY`) |
| Cache | BuildBarn CAS (artifacts), persistent | none between runs |
| Sandbox | local bubblewrap in a 16-28 CPU pod | hosted runner, 4 vCPU |
| Output | Zot `:30500/bluefin-server-image:latest` | GitHub Release `vYY.MM.<run>` + ghcr.io |
| Use | fast iteration, installer boot test | what users install |

Iterate in the lab, push to GitHub, and the push to `main` publishes the
release. Lab images are dev-signed and never published.

The dev set is fixed on purpose: the FSDK kernel's cache key includes the
module certificate (`components/linux-module-cert.bst` override), so a per-run
key would rebuild the kernel (about 1 h 45 min) every time. With the fixed set,
the kernel and everything signed with it stay in the CAS.

Builds run in the pod's local sandbox, not on BuildBarn remote execution:
bb_runner chroots into the action's input root without `/proc`, and the FSDK
kernel's objtool (`read_stack_limit` opens `/proc/self/maps`) and bootstrap Go
fail there. BuildBarn still stores and serves the artifacts.

## Core Process

### Build a branch

```bash
just run-bluefin-server-build ref=<branch>
```

The pipeline builds `oci/bluefin-server-image.bst` (OS DDI, UKIs, netboot ESP,
USB installer, sysexts, signed `SHA256SUMS`) and pushes the export as a
`FROM scratch` image. Take files out with
`podman create` + `podman export | tar -x`.

### Commit gate and installer testing

`bst-commit-poller` builds every new `main` commit and reports a "Lab build"
status on it. `bluefin-server-boot-test` still targets the removed Flatcar
installer and is not chained: the offline USB installer needs an unattended
path that works under KubeVirt (plaintext `.cred` files in the stick's
`/loader/credentials/` are not applied), tracked in projectbluefin/server#265.
Until then, test the installer interactively: pull the image set from Zot and
boot `bluefin-server-installer_<ver>.raw` in QEMU/OVMF, or in a KubeVirt VM and
take screenshots with `virtctl vnc screenshot`.

### Rotate or recreate the dev keys

The Secret is created out of band (it is not a release secret, but keys do not
go in git here). From a projectbluefin/server checkout:

```bash
bash scripts/gen-dev-keys.sh --force
tar -C files/boot-keys -czf /tmp/dev-keys.tgz .
kubectl -n argo create secret generic bluefin-server-dev-boot-keys \
  --from-file=keys.tgz=/tmp/dev-keys.tgz --dry-run=client -o yaml | kubectl apply -f -
```

A new module certificate means one cold kernel build on the next run.

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "Copy the release keys to the lab so lab images match releases." | Release keys stay in GitHub Actions only. The lab exists to iterate; releases come from `main`. |
| "Generate fresh dev keys per build, like PR CI does." | That changes the kernel's cache key every run and rebuilds it for ~1 h 45 min. |

## Red Flags

- `Specified path 'files/boot-keys' does not exist` → the dev-keys Secret is
  missing or not mounted.
- The kernel (`freedesktop-sdk.bst:components/linux.bst`) builds on every run →
  the dev key set changed or the CAS was wiped.
- Boot test waits the full timeout for power-off → the installer prompted
  (a new prompt, or the credential drop-in no longer matches the image's
  `ExecStart=`); read the installer serial log.

## Verification

- [ ] `argo get -n argo @latest` shows `bst-build-re` Succeeded and
      `bluefin-server-image:latest` pushed.
- [ ] `bluefin-server-boot-test` Succeeded: the target has an ESP and a
      `bluefin_usr_<ver>` slot, and the installed disk reached a systemd target.
