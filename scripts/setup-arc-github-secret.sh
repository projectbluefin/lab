#!/usr/bin/env bash
# Setup script for the ARC GitHub App secret.
#
# A contributor with GitHub org admin access and the downloaded private key
# can run this to create the arc-github-secret in the arc-runners namespace.
#
# GitHub does not let you download an existing private key. If the key is lost,
# generate a new one from the app settings page. The new .pem downloads once.

set -euo pipefail

NAMESPACE="arc-runners"
SECRET_NAME="arc-github-secret"
APP_SLUG="bluefin-ghost-arc"

if ! command -v gh >/dev/null 2>&1; then
  echo "ERROR: gh CLI is required." >&2
  exit 1
fi

if ! command -v kubectl >/dev/null 2>&1; then
  echo "ERROR: kubectl is required." >&2
  exit 1
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "ERROR: gh CLI is not authenticated. Run 'gh auth login' first." >&2
  exit 1
fi

echo "Fetching GitHub App details for ${APP_SLUG}..."
APP_JSON=$(gh api "/apps/${APP_SLUG}")
APP_ID=$(echo "${APP_JSON}" | jq -r '.id')

echo "Fetching installation for ${APP_SLUG}..."
INSTALL_JSON=$(gh api "/orgs/projectbluefin/installations" --jq ".installations[] | select(.app_slug == \"${APP_SLUG}\")")
INSTALLATION_ID=$(echo "${INSTALL_JSON}" | jq -r '.id')

echo ""
echo "App ID:          ${APP_ID}"
echo "Installation ID: ${INSTALLATION_ID}"
echo ""

echo "Download a new private key from:"
echo "  https://github.com/organizations/projectbluefin/settings/apps/${APP_SLUG}"
echo "Then choose Private keys -> Generate a private key. The .pem downloads once."
echo ""

read -rp "Path to the downloaded GitHub App private key (.pem file): " PEM_PATH
PEM_PATH="${PEM_PATH/#\~/$HOME}"

if [[ ! -f "${PEM_PATH}" ]]; then
  echo "ERROR: File not found: ${PEM_PATH}" >&2
  exit 1
fi

if ! grep -qE 'BEGIN (RSA )?PRIVATE KEY' "${PEM_PATH}"; then
  echo "ERROR: ${PEM_PATH} does not look like a PEM private key." >&2
  echo "       Make sure you downloaded the .pem, not a token or fingerprint." >&2
  exit 1
fi

echo ""
echo "Creating secret ${SECRET_NAME} in namespace ${NAMESPACE}..."
kubectl create secret generic "${SECRET_NAME}" \
  --namespace "${NAMESPACE}" \
  --from-literal=github_app_id="${APP_ID}" \
  --from-literal=github_app_installation_id="${INSTALLATION_ID}" \
  --from-file=github_app_private_key="${PEM_PATH}" \
  --dry-run=client -o yaml | kubectl apply -f -

echo ""
echo "Done. Verify with:"
echo "  kubectl get secret ${SECRET_NAME} -n ${NAMESPACE}"
echo "  kubectl logs -n arc-systems deployment/arc-systems-gha-rs-controller --tail=20"
