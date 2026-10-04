# Zot candidate promotion

The writable Zot keeps anonymous pulls working while authenticated writes are
prepared. `manifests/zot-writable.yaml` currently mounts `config.json`;
`config-authenticated.json` is the activation-ready policy and is deliberately
not active until every writer has migrated.

## Candidate contract

- A lane is independent and promotes only its own repository.
- Candidate tags are immutable: `candidate-<40-or-64-lowercase-hex>`.
- Call `zot-candidate-lifecycle/candidate-preflight` before a build pushes a
  candidate. It rejects an existing tag and requires at least 20 GiB and 10%
  free on the Zot data filesystem.
- After that lane's QA succeeds, call
  `zot-candidate-lifecycle/promote-candidate` with the pushed digest.
- Promotion fetches the manifest back by digest, verifies its SHA-256, copies
  the same registry digest to `:testing`, verifies the target digest, and
  attaches `application/vnd.projectbluefin.lab.promotion-evidence.v1+json`.
- A failed lane does not block another lane from validating or promoting.

Zot retention (`manifests/zot-writable.yaml`) applies to `dakota` and
`dakota-nvidia` and keeps only tags matching `^testing$`,
`^candidate-<40|64 hex>$` (ten most recently pushed), and `^<40|64 hex>$` (ten
most recently pushed). Every other tag in those repositories, for example
`:ubuntu-kernel` or `:pr-123`, is deleted at the next GC after its 24h delay,
along with untagged manifests and their referrers. To keep a one-off image,
push it under its commit SHA or to a repository outside the policy. GC runs
every 24h and at registry start, so a Zot restart applies retention at once.

## Secret contracts and activation gate

Credentials are operator-managed and never stored in git:

| Namespace | Secret | Contract |
|---|---|---|
| `local-registry` | `zot-auth` | key `htpasswd`; contains `zot-writer` and `zot-metrics` bcrypt entries |
| `argo` | `zot-writer-auth` | type `kubernetes.io/dockerconfigjson`; key `.dockerconfigjson` |

Do not activate authentication until all of these are true:

1. Provision both secrets.
2. Migrate every existing `:30500` writer to the mounted auth file. The new
   lifecycle template already supports it; the current Dakota build/export
   DAG does not.
3. Change the registry config mount to `config-authenticated.json`, set the
   `zot-auth` volume to `optional: false`, and set writer auth volumes to
   `optional: false` in the same GitOps rollout.
4. Verify anonymous `/v2/` and image pulls, authenticated push/delete, and
   denied anonymous writes before removing the rollout gate.

## Remaining Dakota DAG integration

For each Dakota lane: derive `candidate-${commit-sha}`, call
`candidate-preflight`, push only that candidate, capture its digest, run QA
against `repository@digest`, then call `promote-candidate` after that lane
succeeds. Remove the current direct `:testing` push only in that DAG refactor.
