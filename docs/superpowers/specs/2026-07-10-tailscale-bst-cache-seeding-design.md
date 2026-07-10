# Tailscale-Ephemeral BST Cache Seeding from Ghost

Date: 2026-07-10
Status: design / reference implementation

## Assumptions

1. The target repo follows a standard Universal Blue layout with seeding workflows under `.github/workflows/` (e.g. `build-iso.yml`, `generate-iso.yml`, or `seed-*.yml`). The snippet below is designed to be pasted into such a workflow as a separate job.
2. Ghost runs a **dedicated CI tailnet** instance separate from the production/home tailnet. The ephemeral runner joins only this CI tailnet and can see only Ghost (and other ephemeral runners, which are short-lived).
3. Ghost already compiles BST artifacts and can run `bst artifact push` against a configured cache server.
4. `cache.projectbluefin.io` authenticates pushes with mTLS client certificates (the standard BuildStream remote-cache pattern). The private key and certificate live only on the GitHub Actions runner as repository secrets.
5. The runner uses the same `bst2` OCI image that the lab uses for BST builds, or has `buildbox-casd` and `bst` installed by another mechanism.
6. The seeding step is **best-effort / opportunistic**. It must never fail the parent workflow.

## Security model

The design is built around the principle that **the GitHub Actions runner is untrusted**. It runs on hardware GitHub controls and executes arbitrary workflow code. Therefore:

- The runner is confined to a **dedicated CI tailnet** with only one persistent peer: Ghost.
- The runner cannot see or reach the production tailnet, home devices, or any cluster subnet.
- The runner can only call Ghost's seed webhook on a single port.
- The runner cannot pull data from Ghost; it can only ask Ghost to push artifacts to it.
- Ghost authenticates each seed request with a short-lived signed JWT and Tailscale peer identity.
- Ghost never holds the upstream cache signing key; the runner alone signs and pushes to `cache.projectbluefin.io`.
- Runner `--accept-routes` is disabled so it cannot learn cluster routes, and Tailscale SSH is disabled for the CI tag.

## 1. Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ GitHub Actions seeding workflow                                             │
│                                                                             │
│  ┌─────────────────────┐   ephemeral OAuth/Tailscale join                  │
│  │   ubuntu-latest     │ ──────────────────────────────────────▶ CI tailnet│
│  │   runner job        │   tagged tag:ci-cache-seeder                      │
│  │                     │   --accept-routes=false                           │
│  └─────────────────────┘   cannot see production tailnet                    │
│            │                                                                │
│            │ 2. start buildbox-casd on :11002 (CI tailnet only)            │
│            │                                                                │
│            ▼                                                                │
│  ┌─────────────────────┐                                                    │
│  │  buildbox-casd      │◀──── grpc://gh-seed-<run_id>.<ci-tailnet>.ts.net  │
│  │  (local cache)      │        pushed by Ghost (ACL: only Ghost may reach)│
│  └─────────────────────┘                                                    │
│            │                                                                │
│            │ 3. bst artifact push to cache.projectbluefin.io                │
│            │    using runner-only client cert + key                         │
│            ▼                                                                │
│  ┌─────────────────────┐                                                    │
│  │ cache.projectbluefin.io (Hetzner CAS)                                   │
│  └─────────────────────┘                                                    │
└─────────────────────────────────────────────────────────────────────────────┘
                                    ▲
                                    │ CI tailnet only
                                    │
┌─────────────────────────────────────────────────────────────────────────────┐
│ Ghost cluster (k3s)                                                         │
│                                                                             │
│  ┌─────────────────────┐   second tailscaled instance                      │
│  │  tailscale-ci       │   on CI tailnet only                              │
│  │  (tag:ghost-cluster-ci)                                                  │
│  └─────────────────────┘                                                    │
│            │                                                                │
│            ▼                                                                │
│  ┌─────────────────────┐                                                    │
│  │  seed-webhook       │◀──── POST from runner                             │
│  │  (ci-tailnet:18080) │     signed JWT + MagicDNS + ACL-restricted        │
│  └─────────────────────┘                                                    │
│            │                                                                │
│            │ verifies JWT + peer tag, then pushes                          │
│            ▼                                                                │
│  ┌─────────────────────┐                                                    │
│  │  bst artifact push  │──── grpc://<runner>:11002                          │
│  │  (no upstream cache  │     unsigned local push                           │
│  │   credentials)      │                                                    │
│  └─────────────────────┘                                                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Why this flow is safe

- **Network isolation**: The runner joins a CI-only tailnet. It cannot see the production tailnet, home devices, or any cluster subnet. Ghost joins both tailnets via separate tailscaled instances.
- **ACL lockdown**: The runner can reach only Ghost on port `18080`. Ghost can reach only the runner on port `11002`. No other cross-node traffic is permitted.
- **No route learning**: The runner starts with `--accept-routes=false` and advertises no routes. It cannot learn cluster routes or expose fake ones.
- **Push-only data flow**: The runner can only *ask* Ghost to push artifacts. It cannot pull data from Ghost or enumerate cluster services.
- **Request authentication**: Every seed request carries a short-lived signed JWT generated with a GitHub secret. Ghost verifies the signature and expiry and checks the peer's Tailscale tags via `tailscale whois`.
- **Credential isolation**: Ghost never holds the `cache.projectbluefin.io` signing key. The runner alone signs and pushes to the upstream cache.
- **Ephemeral lifecycle**: The runner is deleted from the CI tailnet shortly after the job ends, and its hostname includes the run ID so it cannot be confused with a persistent node.

## 2. Tailscale setup from scratch

### 2.1. Create a dedicated CI tailnet

Create a **separate tailnet** from your production/home tailnet. This is the strongest isolation available:

1. In the Tailscale admin console, create a new tailnet (e.g. `mytailnet-ci.ts.net`).
2. Enable **MagicDNS** in the DNS tab.
3. Enable **Device Approval** so new nodes must be approved before they can communicate.

### 2.2. Add Ghost to the CI tailnet with a second tailscaled instance

Do **not** add your production Ghost node to the CI tailnet. Run a second tailscaled instance dedicated to CI traffic:

```bash
# Create state/socket paths for the CI instance
sudo mkdir -p /var/lib/tailscale-ci /run/tailscale-ci

# Start the CI tailscaled
sudo tailscaled --state=/var/lib/tailscale-ci/tailscaled.state \
                --socket=/run/tailscale-ci/tailscaled.sock \
                --tun=userspace-networking &

# Join the CI tailnet
sudo -E tailscale --socket=/run/tailscale-ci/tailscaled.sock up \
  --hostname ghost-cluster-ci \
  --advertise-tags tag:ghost-cluster-ci \
  --accept-routes=false \
  --ssh=false
```

Alternatively, run it as a systemd service:

```ini
# /etc/systemd/system/tailscaled-ci.service
[Unit]
Description=Tailscale CI tailnet
After=network.target

[Service]
ExecStart=/usr/sbin/tailscaled --state=/var/lib/tailscale-ci/tailscaled.state --socket=/run/tailscale-ci/tailscaled.sock --tun=userspace-networking
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

Then configure the seed-webhook service on Ghost to bind only to the CI tailnet interface (e.g. `100.x.y.z` from `tailscale --socket=/run/tailscale-ci/tailscaled.sock ip -4`).

### 2.3. Define ACL tags on the CI tailnet

In the CI tailnet admin console → Access Controls (ACL), use a default-deny policy:

```json
{
  "tagOwners": {
    "tag:ghost-cluster-ci": ["autogroup:admin"],
    "tag:ci-cache-seeder":  ["autogroup:admin"]
  },
  "acls": [
    {
      "action": "accept",
      "src": ["tag:ci-cache-seeder"],
      "dst": ["tag:ghost-cluster-ci:18080"],
      "proto": "tcp"
    },
    {
      "action": "accept",
      "src": ["tag:ghost-cluster-ci"],
      "dst": ["tag:ci-cache-seeder:11002"],
      "proto": "tcp"
    }
  ],
  "nodeAttrs": [
    {
      "target": ["tag:ci-cache-seeder"],
      "attr": ["funnel", false]
    }
  ],
  "ssh": [
    {
      "action": "check",
      "src": ["autogroup:member"],
      "dst": ["tag:ghost-cluster-ci"],
      "users": ["autogroup:nonroot", "root"]
    }
  ]
}
```

Explanation:

- `tag:ci-cache-seeder` may call Ghost's seed webhook on TCP port `18080` only.
- `tag:ghost-cluster-ci` may push BST artifacts to the runner's local cache on TCP port `11002` only.
- The runner cannot use Tailscale Funnel.
- The runner cannot SSH into Ghost (SSH is only for tailnet members to Ghost, and the runner is not a member).
- No other cross-tag traffic is allowed.

### 2.4. Create an OAuth client for GitHub Actions

1. In the **CI tailnet** admin console → **OAuth clients** → **Generate OAuth client**.
2. Scopes: select **`auth_keys`** with **Write** permission.
3. Tags: add `tag:ci-cache-seeder`.
4. Copy the **Client ID** and **Client secret**.

### 2.5. Add GitHub repository secrets

In the target repo (the one containing the seeding workflow):

| Secret name | Value |
|-------------|-------|
| `TS_OAUTH_CLIENT_ID` | CI tailnet OAuth client ID |
| `TS_OAUTH_SECRET` | CI tailnet OAuth client secret |
| `CACHE_PROJECTBLUEFIN_IO_CERT` | PEM client certificate for `cache.projectbluefin.io` |
| `CACHE_PROJECTBLUEFIN_IO_KEY` | PEM private key for the above certificate |
| `GHOST_SEED_JWT_SECRET` | A strong random secret used to sign/verify the seed-request JWT |

### 2.6. Ghost-side Tailscale identity

Ghost's CI instance is tagged `tag:ghost-cluster-ci` and lives only in the CI tailnet. The production tailnet is completely separate.

## 3. Workflow YAML

Paste this as a new job named `seed-bst-cache` into the seeding workflow. It is intentionally independent of the main build job and runs in parallel with it.

```yaml
  seed-bst-cache:
    name: Seed BST cache from Ghost
    runs-on: ubuntu-latest
    # Non-blocking: a failure here must not block PR merge or release.
    continue-on-error: true
    timeout-minutes: 30
    permissions:
      contents: read
    env:
      # CI tailnet name, e.g. mytailnet-ci.ts.net
      TS_TAILNET: ${{ vars.TS_TAILNET_CI }}
      # Hostname the ephemeral runner will register in the CI tailnet.
      TS_RUNNER_HOSTNAME: gh-seed-${{ github.run_id }}-${{ github.run_attempt }}
      # Ghost webhook endpoint on the CI tailnet.
      GHOST_WEBHOOK_URL: https://ghost-cluster-ci.${{ vars.TS_TAILNET_CI }}:18080/seed
      # OCI image that contains buildbox-casd and bst.
      BST_IMAGE: ghcr.io/projectbluefin/bst2:latest

    steps:
      - name: Checkout
        uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7
        with:
          persist-credentials: false

      - name: Join CI Tailscale tailnet (ephemeral)
        id: tailscale
        # Pinned to tailscale/github-action@v2 as requested.
        uses: tailscale/github-action@4e4c49acaa9818630ce0bd7a564372c17e33fb4d # v2
        with:
          oauth-client-id: ${{ secrets.TS_OAUTH_CLIENT_ID }}
          oauth-secret: ${{ secrets.TS_OAUTH_SECRET }}
          tags: tag:ci-cache-seeder
          hostname: ${{ env.TS_RUNNER_HOSTNAME }}
          # Do not learn cluster routes; do not enable Tailscale SSH on this node.
          args: --accept-routes=false --ssh=false
          version: 1.58.2
        continue-on-error: true

      - name: Verify CI tailnet connectivity
        id: tailscale-ping
        if: steps.tailscale.outcome == 'success'
        shell: bash
        run: |
          set +e
          timeout 120 tailscale ping "ghost-cluster-ci.${TS_TAILNET}"
          rc=$?
          if [[ $rc -ne 0 ]]; then
            echo "::warning::Ghost is not reachable over Tailscale; skipping cache seeding."
            exit 0
          fi
          echo "status=ok" >> "$GITHUB_OUTPUT"
        continue-on-error: true

      - name: Write cache client certificate
        id: write-certs
        if: steps.tailscale-ping.outcome == 'success' && steps.tailscale-ping.outputs.status == 'ok'
        shell: bash
        env:
          CERT: ${{ secrets.CACHE_PROJECTBLUEFIN_IO_CERT }}
          KEY: ${{ secrets.CACHE_PROJECTBLUEFIN_IO_KEY }}
        run: |
          set +e
          if [[ -z "${CERT}" || -z "${KEY}" ]]; then
            echo "::warning::Cache signing credentials missing; skipping cache seeding."
            exit 0
          fi
          mkdir -p "$HOME/.config/buildstream"
          printf '%s\n' "${CERT}" > "$HOME/.config/buildstream/cache.crt"
          printf '%s\n' "${KEY}"  > "$HOME/.config/buildstream/cache.key"
          chmod 600 "$HOME/.config/buildstream/cache.key"
          echo "status=ok" >> "$GITHUB_OUTPUT"
        continue-on-error: true

      - name: Start local BuildStream cache server
        id: start-casd
        if: steps.write-certs.outputs.status == 'ok'
        shell: bash
        run: |
          set +e
          mkdir -p /tmp/bst-seed-cache
          # Listen on all interfaces so the CI tailnet IP can reach it.
          # The ACL on the CI tailnet is the only thing that allows Ghost in.
          docker run -d --rm \
            --name bst-seed-casd \
            --network host \
            -v /tmp/bst-seed-cache:/cache \
            ${{ env.BST_IMAGE }} \
            buildbox-casd --listen 0.0.0.0:11002 --cas-cache /cache
          rc=$?
          if [[ $rc -ne 0 ]]; then
            echo "::warning::Failed to start local cache server; skipping cache seeding."
            exit 0
          fi
          # Give casd a moment to bind.
          sleep 2
          if ! ss -tlnp | grep -q ':11002'; then
            echo "::warning::Local cache server did not bind to :11002; skipping."
            docker logs bst-seed-casd || true
            docker stop bst-seed-casd || true
            exit 0
          fi
          echo "status=ok" >> "$GITHUB_OUTPUT"
          echo "runner_hostname=${TS_RUNNER_HOSTNAME}.${TS_TAILNET}" >> "$GITHUB_OUTPUT"
        continue-on-error: true

      - name: Sign seed request JWT
        id: sign-jwt
        if: steps.start-casd.outputs.status == 'ok'
        shell: bash
        env:
          JWT_SECRET: ${{ secrets.GHOST_SEED_JWT_SECRET }}
        run: |
          set +e
          if [[ -z "${JWT_SECRET}" ]]; then
            echo "::warning::JWT secret missing; skipping cache seeding."
            exit 0
          fi

          b64url() { openssl base64 -e -A | tr '+/' '-_' | tr -d '='; }
          HEADER=$(printf '%s' '{"alg":"HS256","typ":"JWT"}' | b64url)
          EXP=$(($(date +%s) + 300))
          PAYLOAD=$(printf '{"run_id":"%s","run_attempt":"%s","runner_hostname":"%s","exp":%d}' \
            "${{ github.run_id }}" \
            "${{ github.run_attempt }}" \
            "${TS_RUNNER_HOSTNAME}.${TS_TAILNET}" \
            "${EXP}" | b64url)
          SIG=$(printf '%s' "${HEADER}.${PAYLOAD}" | openssl dgst -sha256 -hmac "${JWT_SECRET}" -binary | b64url)
          JWT="${HEADER}.${PAYLOAD}.${SIG}"

          # Pass the token to the next step via GitHub Actions environment file
          # so we never need to interpolate a step output inside a shell script.
          echo "SEED_TOKEN=${JWT}" >> "$GITHUB_ENV"
          echo "status=ok" >> "$GITHUB_OUTPUT"
        continue-on-error: true

      - name: Request BST artifacts from Ghost
        id: request-seed
        if: steps.sign-jwt.outputs.status == 'ok'
        shell: bash
        run: |
          set +e
          if [[ -z "${SEED_TOKEN}" ]]; then
            echo "::warning::Seed token not available; skipping cache seeding."
            exit 0
          fi
          RUNNER_HOST="${TS_RUNNER_HOSTNAME}.${TS_TAILNET}"
          PAYLOAD=$(jq -n \
            --arg run_id "${{ github.run_id }}" \
            --arg run_attempt "${{ github.run_attempt }}" \
            --arg runner_hostname "${RUNNER_HOST}" \
            --arg elements "oci/bluefin.bst" \
            '{run_id: $run_id, run_attempt: $run_attempt, runner_hostname: $runner_hostname, elements: ($elements | split(","))}')

          echo "::notice::Requesting seeding from Ghost for runner ${RUNNER_HOST} ..."
          HTTP_STATUS=$(curl -s -o /tmp/seed-response.json -w '%{http_code}' \
            --max-time 30 \
            --connect-timeout 10 \
            -H "Content-Type: application/json" \
            -H "X-Seed-Token: ${SEED_TOKEN}" \
            -d "${PAYLOAD}" \
            "${GHOST_WEBHOOK_URL}")

          if [[ "${HTTP_STATUS}" != "200" && "${HTTP_STATUS}" != "202" ]]; then
            echo "::warning::Ghost seed webhook returned HTTP ${HTTP_STATUS}; skipping upstream cache push."
            cat /tmp/seed-response.json || true
            exit 0
          fi

          echo "::notice::Ghost accepted seed request."
          echo "status=ok" >> "$GITHUB_OUTPUT"
          # If Ghost returns the list of artifact refs it pushed, capture them.
          jq -r '.artifact_refs // [] | join(",")' /tmp/seed-response.json > /tmp/seed-artifact-refs.txt
        continue-on-error: true

      - name: Wait for Ghost to push artifacts
        id: wait-push
        if: steps.request-seed.outputs.status == 'ok'
        shell: bash
        timeout-minutes: 20
        run: |
          set +e
          RUNNER_HOST="${TS_RUNNER_HOSTNAME}.${TS_TAILNET}"
          echo "::notice::Waiting for Ghost to push BST artifacts to ${RUNNER_HOST}:11002 ..."

          # Poll the local casd for non-empty cache. Adjust sleep/iterations to taste.
          for i in $(seq 1 120); do
            if [[ -n "$(find /tmp/bst-seed-cache -type f 2>/dev/null | head -1)" ]]; then
              echo "::notice::Artifacts detected in local cache."
              echo "status=ok" >> "$GITHUB_OUTPUT"
              exit 0
            fi
            sleep 10
          done

          echo "::warning::Timed out waiting for Ghost artifact push; skipping upstream cache push."
        continue-on-error: true

      - name: Push artifacts to cache.projectbluefin.io
        id: push-cache
        if: steps.wait-push.outputs.status == 'ok'
        shell: bash
        timeout-minutes: 20
        run: |
          set +e
          RUNNER_HOST="${TS_RUNNER_HOSTNAME}.${TS_TAILNET}"

          cat > /tmp/buildstream-push.conf <<EOF
          scheduler:
            network-retries: 3
          artifacts:
            override-project-caches: false
            servers:
            # Read from the local cache Ghost just populated.
            - url: grpc://${RUNNER_HOST}:11002
              push: false
            # Push to the upstream Bluefin cache using runner-only credentials.
            - url: https://cache.projectbluefin.io:11001
              push: true
              client-cert: $HOME/.config/buildstream/cache.crt
              client-key:  $HOME/.config/buildstream/cache.key
          EOF

          # Push the element(s) Ghost was asked to seed. The --deps all option
          # ensures all built dependencies are uploaded.
          docker run -d --rm --network host \
            -v /tmp/bst-seed-cache:/cache:ro \
            -v /tmp/buildstream-push.conf:/etc/buildstream/buildstream.conf:ro \
            -v "$HOME/.config/buildstream/cache.crt:/certs/cache.crt:ro" \
            -v "$HOME/.config/buildstream/cache.key:/certs/cache.key:ro" \
            ${{ env.BST_IMAGE }} \
            bst --config /etc/buildstream/buildstream.conf \
                --no-interactive \
                artifact push --deps all \
                $(cat /tmp/seed-artifact-refs.txt | tr ',' ' ')

          rc=$?
          if [[ $rc -ne 0 ]]; then
            echo "::warning::Upstream cache push returned ${rc}; seeding incomplete."
            exit 0
          fi
          echo "::notice::Successfully pushed BST artifacts to cache.projectbluefin.io."
        continue-on-error: true

      - name: Teardown (Tailscale logout / stop cache server)
        if: always()
        shell: bash
        run: |
          set +e
          echo "::notice::Tearing down ephemeral cache seeder..."
          docker stop bst-seed-casd >/dev/null 2>&1 || true
          docker rm   bst-seed-casd >/dev/null 2>&1 || true
          sudo tailscale logout >/dev/null 2>&1 || true
          echo "::notice::Teardown complete."
        continue-on-error: true
```

### Notes on the YAML

- Every step has `continue-on-error: true`. The final teardown step uses `if: always()`.
- The job itself has `continue-on-error: true` as a belt-and-suspenders guard.
- Long steps carry `timeout-minutes` (20 minutes for wait + upstream push).
- `BST_IMAGE` should be replaced with the exact digest-pinned image used by the lab (e.g. `192.168.1.102:30500/bst2:<sha>` if the runner can reach the lab registry, or a public `ghcr.io/projectbluefin/bst2:<sha>`). Pulling from the lab registry over the CI tailnet is fine because the runner can only reach Ghost, not the rest of the cluster.
- The `elements` value in the webhook payload is a comma-separated list of top-level elements whose artifacts should be seeded. It can be sourced from workflow inputs or a matrix.
- The `--accept-routes=false --ssh=false` arguments prevent the runner from learning cluster routes or enabling Tailscale SSH on the CI node.

## 4. Ghost-side sketch

Ghost needs a small long-lived service that can receive a seed request from the ephemeral runner and then push BST artifacts to the runner's local cache.

### 4.1. Webhook service (recommended)

Run a minimal HTTP service on Ghost, bound **only** to the CI tailnet interface (`100.x.y.z` from the CI tailscaled instance). It exposes one endpoint:

```
POST /seed
X-Seed-Token: <signed-JWT>
Content-Type: application/json

{
  "run_id": "1234567890",
  "run_attempt": "1",
  "runner_hostname": "gh-seed-1234567890-1.mytailnet-ci.ts.net",
  "elements": ["oci/bluefin.bst"]
}
```

The service **must** perform all of the following checks before pushing anything:

1. **JWT signature and expiry**: Verify the `X-Seed-Token` header value with HMAC-SHA256 using the shared `GHOST_SEED_JWT_SECRET`. Reject expired tokens (the runner generates them with a 5-minute expiry).
2. **Claim consistency**: The JWT claims `run_id`, `run_attempt`, and `runner_hostname` must match the JSON payload.
3. **Tailscale peer identity**: Verify the source IP is a Tailscale peer on the CI tailnet and has tag `tag:ci-cache-seeder` using `tailscale --socket=/run/tailscale-ci/tailscaled.sock whois <ip>`.
4. **Hostname allowlist**: The `runner_hostname` must match the expected pattern `gh-seed-<run_id>-<run_attempt>.<ci-tailnet>.ts.net`.
5. Generate a transient BuildStream config pointing artifact push at `grpc://<runner_hostname>:11002`.
6. Run `bst artifact push --deps all <element>...` for each requested element.
7. Return `200 OK` with `{"artifact_refs": [...]}` on success, or `202 Accepted` if the push is dispatched asynchronously.

Example JWT + peer verification in Python:

```python
import hmac, hashlib, base64, json, subprocess, time

JWT_SECRET = "..."  # from Ghost config, matches GITHUB_SECRET GHOST_SEED_JWT_SECRET

def b64url_decode(s: str) -> bytes:
    pad = 4 - len(s) % 4
    return base64.urlsafe_b64decode(s + "=" * pad)

def verify_jwt(token: str, expected_run_id: str, expected_attempt: str, expected_hostname: str) -> bool:
    try:
        header_b64, payload_b64, sig_b64 = token.split(".")
        expected_sig = base64.urlsafe_b64encode(
            hmac.new(JWT_SECRET.encode(), f"{header_b64}.{payload_b64}".encode(), hashlib.sha256).digest()
        ).decode().rstrip("=")
        if not hmac.compare_digest(sig_b64, expected_sig):
            return False
        payload = json.loads(b64url_decode(payload_b64))
        if payload.get("exp", 0) < time.time():
            return False
        return (
            payload.get("run_id") == expected_run_id
            and payload.get("run_attempt") == expected_attempt
            and payload.get("runner_hostname") == expected_hostname
        )
    except Exception:
        return False

def peer_tag_ok(src_ip: str, allowed_tag: str = "tag:ci-cache-seeder") -> bool:
    out = subprocess.run(
        ["tailscale", "--socket=/run/tailscale-ci/tailscaled.sock", "whois", "--json", src_ip],
        capture_output=True, text=True, check=False,
    )
    if out.returncode != 0:
        return False
    data = json.loads(out.stdout)
    tags = data.get("Node", {}).get("Tags", [])
    return allowed_tag in tags
```

### 4.2. BuildStream push config generated by the webhook

```yaml
scheduler:
  network-retries: 3
artifacts:
  override-project-caches: false
  servers:
  - url: grpc://gh-seed-<run_id>-<attempt>.<ci-tailnet>.ts.net:11002
    push: true
```

Ghost uses this config to run:

```bash
bst --config /tmp/seed-to-runner.conf --no-interactive artifact push --deps all oci/bluefin.bst
```

No signing credentials for `cache.projectbluefin.io` are present on Ghost.

### 4.3. Polling fallback

If you prefer not to run an inbound webhook service on Ghost, an alternative is:

1. Give Ghost a Tailscale **API access token** (read-only on devices).
2. A CronJob on Ghost polls `https://api.tailscale.com/api/v2/tailnet/<tailnet>/devices` every 2–5 minutes.
3. When it sees a device tagged `tag:ci-cache-seeder` with hostname prefix `gh-seed-`, it pushes artifacts to that device's MagicDNS name.
4. The runner still runs the cache server and the upstream push steps.

The webhook approach is preferred because it is event-driven and avoids polling delays and Tailscale API quota.

## 5. Failure modes and why each are safe

| Failure | What happens | Why the parent workflow still succeeds |
|---------|--------------|----------------------------------------|
| **Tailscale OAuth secret missing or invalid** | `tailscale/github-action` step fails. `steps.tailscale.outcome` is not `success`, so all dependent steps are skipped. | The job has `continue-on-error: true`. |
| **Runner joins wrong tailnet** | The OAuth client is scoped to the CI-only tailnet. Even if misconfigured, the runner cannot see the production tailnet. | Separate tailnets provide fail-closed isolation. |
| **Compromised runner tries to scan the CI tailnet** | ACL default-deny allows the runner only to reach Ghost on port `18080`. It cannot reach other CI-tailnet nodes or subnets. | ACLs are enforced by the coordination server; the runner has no path. |
| **Ghost offline or unreachable** | `tailscale ping` fails; or the webhook POST times out / returns non-2xx. The step exits 0 after logging a warning. | `continue-on-error: true` on every step; no downstream step depends on the failed one. |
| **Forged seed request to Ghost** | Ghost rejects the request: JWT signature/expiry mismatch, claim mismatch, or peer tag is not `tag:ci-cache-seeder`. | Authentication is layered; no artifacts are pushed. |
| **Network partition mid-transfer** | `bst artifact push` on Ghost or the runner times out after `network-retries`. The runner's wait/push step hits `timeout-minutes`. | Timeouts are explicit and caught by `continue-on-error`. |
| **Seeding takes too long** | `wait-push` and `push-cache` each have `timeout-minutes: 20`. | Job-level `timeout-minutes: 30` is the backstop. |
| **Local cache server fails to start** | `start-casd` checks `ss -tlnp` and bails if port 11002 is not bound. | Dependent steps are skipped; `continue-on-error` absorbs the failure. |
| **Upstream cache.projectbluefin.io push fails** | `push-cache` logs a warning and exits 0. | Explicit `continue-on-error: true`. |
| **Teardown/logout fails** | Final step uses `if: always()` and `set +e` for every subcommand. | `continue-on-error: true` guarantees success. |
| **Secret leak in logs** | Certificates are written to `$HOME/.config/buildstream` and never echoed. The JWT secret is used only for HMAC and is masked by GitHub Actions. | Standard GitHub Actions secret masking applies to `${{ secrets.* }}`. |

## 6. Operational checklist

- [ ] A dedicated CI tailnet exists and Ghost joins it via a second `tailscaled` instance (`tailscaled-ci.service`).
- [ ] Ghost's production tailnet is never used for CI runner traffic.
- [ ] ACL on the CI tailnet allows only `tag:ci-cache-seeder` → `tag:ghost-cluster-ci:18080` and `tag:ghost-cluster-ci` → `tag:ci-cache-seeder:11002`.
- [ ] `tag:ci-cache-seeder` cannot use Funnel, SSH, or accept routes.
- [ ] OAuth client has only the `auth_keys` write scope and tag `tag:ci-cache-seeder`.
- [ ] GitHub secrets `TS_OAUTH_CLIENT_ID`, `TS_OAUTH_SECRET`, `CACHE_PROJECTBLUEFIN_IO_CERT`, `CACHE_PROJECTBLUEFIN_IO_KEY`, and `GHOST_SEED_JWT_SECRET` are set.
- [ ] Ghost webhook service binds only to the CI tailnet interface and verifies JWT signature/expiry, claim consistency, peer tag `tag:ci-cache-seeder`, and hostname pattern.
- [ ] Ghost webhook service runs as an unprivileged user with no access to production cluster credentials.
- [ ] The seeding job is **not** marked as a required status check in branch protection.
- [ ] `BST_IMAGE` in the workflow points to the correct, digest-pinned image.
- [ ] Audit logs on Ghost record every seed request, peer identity, and push outcome.

## 7. Ponytail review: do we need Tailscale at all?

The Tailscale bridge is heavy. Before committing to it, two simpler options were evaluated.

### 7.1. GitOps-cronned seed build using ghcr.io as a go-between

This was the user's first instinct. It does **not** work for BST artifacts.

BuildStream artifact caches use the ContentAddressableStorage (CAS) protocol plus a remote-asset index (`cache.projectbluefin.io` exposes this interface). ghcr.io is an OCI registry. `bst artifact push` cannot target an OCI registry, and OCI image layers are not interchangeable with CAS blobs. Storing tarred artifact sets in ghcr.io as generic blobs would add packaging/unpackaging glue and lose CAS deduplication.

Verdict: ghcr.io can carry final OCI images, not intermediate BST artifacts, so it cannot replace the runner-side CAS server.

### 7.2. ARC runner on Ghost (`runs-on: ghost-runners`)

The lab already runs Actions Runner Controller on Ghost. A seeding workflow that targets `ghost-runners` would:

- Run an ephemeral runner pod inside the cluster.
- Reach the local Buildbarn cache without any network bridge.
- Hold the `cache.projectbluefin.io` client certificate as a Kubernetes secret.
- Push directly from the pod.

This removes Tailscale, the second tailnet, the JWT webhook, and inbound port exposure entirely. It is the lazier solution if you are willing to let the seed step execute on-cluster rather than on a GitHub-hosted runner.

### 7.3. When Tailscale is the right remaining choice

Keep the Tailscale design only if all three of these are true:

1. The build must run on Ghost (hardware/acceleration advantage or artifact locality).
2. The runner must be GitHub-hosted, not an on-cluster ARC runner.
3. Ghost must not hold the upstream cache signing credentials.

If any of those constraints can relax, use the simpler option.

### 7.4. What cannot be cut from this design

Given the frozen constraints:

- A dedicated CI tailnet is required for "no one can see my cluster".
- JWT + peer-tag verification is required because the runner is untrusted and sits on the tailnet.
- `buildbox-casd` on the runner is required because BST needs a CAS server to push artifacts into.
- `continue-on-error` and timeouts are required because seeding is best-effort.

The design is minimal for those constraints, but the constraints themselves are not minimal.
