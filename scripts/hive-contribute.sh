#!/usr/bin/env bash
# Start a hive contributor session using the local llama.cpp model on ghost.
# Usage: ./scripts/hive-contribute.sh
#
# Requires: just, gh (brew install just gh), goose (already installed)
set -euo pipefail

HIVE_DIR="${HIVE_DIR:-/tmp/hive}"
HIVE_REPO="https://github.com/kubestellar/hive"
HIVE_BRANCH="v2"

# ── Guardrails injected into every task via goose Top-of-Mind ──────────
export GOOSE_MOIM_MESSAGE_TEXT="HIVE CONTRIBUTOR RULES — OBEY ON EVERY TURN:
1. COMMENTS: ONE comment per issue/PR maximum. If you have already commented, EDIT that existing comment — never post a new one. Always check first: gh api repos/OWNER/REPO/issues/NUMBER/comments --jq '[.[] | select(.user.login == \"castrojo\")]'
2. NO MERGES: Never merge or approve a PR. Open PRs for human review only. Merging is reserved for human maintainers.
3. NO SPAM: No 'I am working on this', no status updates, no progress reports. Only comment when you have a concrete, complete result.
4. CONSERVATIVE: Propose changes over making them. When uncertain, post findings and a suggested fix — do not push code.
5. CLOSING ISSUES: Only close an issue if a PR you opened was already merged by a maintainer and directly resolves the reported problem.
6. SCOPE: Work only on the assigned repo/issue. Do not open issues or comment in unrelated repos."

# ── Local llama.cpp endpoint (ghost, OpenAI-compatible) ────────────────
export HIVE_HUB=wss://hosted-projectbluefin-knuckle-gjvq.hive.kubestellar.io/contribute
export GOOSE_PROVIDER=openai
export OPENAI_BASE_URL=http://192.168.1.102:30800/v1
export OPENAI_API_KEY=local
export GOOSE_MODEL=Qwen/Qwen3.6-35B-A3B

# ── Clone/refresh hive repo ────────────────────────────────────────────
if [[ ! -d "$HIVE_DIR/.git" ]]; then
  echo "Cloning hive → $HIVE_DIR"
  git clone -q -b "$HIVE_BRANCH" "$HIVE_REPO" "$HIVE_DIR"
else
  echo "Updating hive in $HIVE_DIR"
  git -C "$HIVE_DIR" pull -q --ff-only
fi


# ── First-time setup (idempotent) ──────────────────────────────────────
cd "$HIVE_DIR"
just contribute-setup goose

# ── Connect ────────────────────────────────────────────────────────────
echo ""
echo "Starting hive contributor (local mode, Qwen/Qwen3.6-35B-A3B on ghost)..."
just contribute-hive goose local
