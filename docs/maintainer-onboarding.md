# Maintainer onboarding: using `ghost-runners`

`ghost-runners` is a projectbluefin organization-wide, Kubernetes container-mode
Actions Runner Controller (ARC) scale set. It lets maintainers run GitHub
Actions jobs on the Ghost cluster without keeping fat runner pods idle. Heavy
work is offloaded to Argo Workflows; the GitHub Actions runner pod stays small.

## What you get

- `runs-on: ghost-runners` in any workflow job.
- Each job runs in its own container on the cluster.
- Large builds (BST, ISO, container images) can be submitted as Argo Workflows
  so the cluster supplies the CPU/memory, not the Actions runner pod.

## Who can use it

Only repositories in the `projectbluefin` organization that are covered by the
`bluefin-ghost-arc` GitHub App installation can schedule `ghost-runners` jobs.

You also need permission to edit workflow files in the repository you are
working in. In practice this means:

- Organization owners and repository admins can always add workflows.
- Maintainers with **Write** access can edit `.github/workflows/` and push
  branches.
- Outside collaborators and read-only members cannot schedule jobs on the
  runner because they cannot modify workflows in the org repos.

## Authentication model

Three layers enforce that only projectbluefin maintainers can use this runner.

1. **GitHub App installation scope**
   - The `bluefin-ghost-arc` GitHub App is installed on the `projectbluefin`
     organization.
   - GitHub only allows repos covered by that installation to request runners
     from the `ghost-runners` scale set.
   - If a repository is removed from the App installation, its workflows can no
     longer schedule `ghost-runners` jobs.

2. **Repository write access**
   - A workflow file containing `runs-on: ghost-runners` must be committed to
     `.github/workflows/` in the repo.
   - GitHub restricts who can modify files under `.github/workflows/` to users
     with repository **Write** (or **Maintain**/**Admin**) permissions.
   - This means an arbitrary user cannot point a fork or external repo at the
     runner; only maintainers who can write to the repo can add the label.

3. **ARC listener authentication**
   - The ARC listener pod in the cluster authenticates to GitHub using the
     GitHub App private key stored in the `arc-github-secret` Kubernetes
     secret.
   - GitHub only delivers jobs to the listener for repos authorized by the App
     installation.
   - Runner pods run in the `arc-runners` namespace with a dedicated service
     account and RBAC that only allows Argo Workflow submission; they cannot
     access other cluster resources.

## Adding the runner to a repository

Edit or create a workflow file under `.github/workflows/`:

```yaml
name: My heavy job

on:
  push:
    branches: [main]
  pull_request:

jobs:
  build:
    runs-on: ghost-runners
    container:
      image: ghcr.io/projectbluefin/arc-runner:latest
    steps:
      - uses: actions/checkout@v4

      - name: Inspect available tooling
        run: |
          argo version
          kubectl version --client
```

### Required: declare a `container:`

Container mode requires every job to declare a container image. The
`ghcr.io/projectbluefin/arc-runner:latest` image contains `argo`, `kubectl`,
`jq`, `skopeo`, and other tooling used by projectbluefin workflows. If you omit
`container:`, the job will fail to start.

### Offload heavy work to Argo

Keep the runner pod small by submitting the actual build as an Argo Workflow and
waiting for it:

```yaml
jobs:
  warm-cache:
    runs-on: ghost-runners
    container:
      image: ghcr.io/projectbluefin/arc-runner:latest
    steps:
      - uses: actions/checkout@v4

      - name: Submit BST warm-cache build
        run: |
          argo submit --from workflowtemplate/dakota-buildstream-warm-cache \
            --parameter cache-key="${{ github.sha }}" \
            --wait
```

The `--wait` flag keeps the GitHub Actions job alive and streams status until
the Argo Workflow finishes. The cluster supplies the real CPU/memory; the
runner pod just coordinates.

## Bluefin Maintainers who want to use the Runners on their personal repos

The `ghost-runners` scale set is bound to the `projectbluefin` organization, so it
cannot serve repositories outside that org. A maintainer can reuse the same
cluster infrastructure for personal repos by adding a second scale set tied to
their personal GitHub account.

### What the maintainer does

1. Open personal **Settings → Applications → GitHub Apps**.
2. Install the `bluefin-ghost-arc` app on the personal account (or select
   specific repos).
3. Note the **installation ID** from the URL (`/settings/installations/<ID>`).

### What a cluster admin does

Create a new secret for the personal installation. The app ID and private key
are the same; only the installation ID changes:

```bash
kubectl create secret generic arc-github-secret-personal \
  --namespace arc-runners \
  --from-literal=github_app_id=4099840 \
  --from-literal=github_app_installation_id="<PERSONAL_INSTALLATION_ID>" \
  --from-literal=github_app_private_key="$(cat /path/to/bluefin-ghost-arc.pem)"
```

Add a new ArgoCD Application, for example
`argocd/arc-runners-personal-app.yaml`:

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: arc-runners-personal
  namespace: argocd
spec:
  project: default
  source:
    repoURL: ghcr.io/actions/actions-runner-controller-charts
    chart: gha-runner-scale-set
    targetRevision: 0.9.3
    helm:
      values: |
        githubConfigUrl: "https://github.com/<GITHUB_USERNAME>"
        githubConfigSecret: arc-github-secret-personal
        runnerScaleSetName: ghost-runners-personal
        minRunners: 0
        maxRunners: 6
        controllerServiceAccount:
          namespace: arc-systems
          name: arc-systems-gha-rs-controller
        containerMode:
          type: kubernetes
          kubernetesModeWorkVolumeClaim:
            accessModes: ["ReadWriteOnce"]
            storageClassName: local-path
            resources:
              requests:
                storage: 50Gi
        template:
          spec:
            serviceAccountName: arc-runner-workflow-submitter
            containers:
              - name: runner
                image: ghcr.io/projectbluefin/arc-runner:latest
                command: ["/home/runner/run.sh"]
  destination:
    server: https://kubernetes.default.svc
    namespace: arc-runners
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
```

### What the maintainer puts in their personal repo

```yaml
jobs:
  build:
    runs-on: ghost-runners-personal
    container:
      image: ghcr.io/projectbluefin/arc-runner:latest
    steps:
      - uses: actions/checkout@v4
      - run: argo version
```

### Security note

The personal scale set uses the same runner image and cluster RBAC as the org
scale set, so it has the same privileges (Argo Workflow submit/read in the
`argo` namespace). Only extend this to trusted Bluefin maintainers.

## Verifying a repo can use the runner

Open a PR with a workflow that uses `runs-on: ghost-runners`. If the repository
is not covered by the GitHub App installation, GitHub will report:

```
This workflow request was rejected because the repository is not authorized
by the GitHub App installation.
```

If the repository is authorized but no runner is available, the job will stay
in the "Queued" state until a runner pod starts. With `minRunners: 0`, the
first job may take 30-60 seconds while a pod is created.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Job fails immediately with "could not find a runner" | Repository not in the GitHub App installation | Ask an org owner to add the repo to the `bluefin-ghost-arc` installation. |
| Job queued for a long time | No runner pod available | Wait for the cluster to scale a runner up, or check `kubectl get pods -n arc-runners`. |
| `argo` command not found | `container:` image missing tooling | Use `ghcr.io/projectbluefin/arc-runner:latest`. |
| Permission denied on `argo submit` | Step pod service account lacks RBAC | File an issue in `projectbluefin/lab`; do not add broad permissions yourself. |

## Getting help

- Reference workflow: `.github/workflows/example-container-mode-build.yml`
- Cluster operations guide: `docs/agent-cheatsheet.md`
- ARC patterns and red flags: `docs/skills/ci-tooling.md`
- Open an issue in `projectbluefin/lab` for access or infrastructure problems.
