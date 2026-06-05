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

# ── Patch relay: prepend guardrails to every task prompt ───────────────
python3 - << 'PYEOF'
import sys, os
relay_path = os.environ.get("HIVE_DIR", "/tmp/hive") + "/bin/contributor-relay.sh"
relay = open(relay_path).read()

guardrail_js = (
    "const GUARDRAILS = ["
    "'HIVE CONTRIBUTOR RULES — OBEY BEFORE EVERY ACTION:\\n',"
    "'1. COMMENTS: ONE comment per issue/PR max. If you already commented, EDIT it — never post a new one.\\n',"
    "'2. NO MERGES: Never merge or approve a PR. Open PRs for human review only.\\n',"
    "'3. NO SPAM: No status updates or progress comments. Only comment with a concrete complete result.\\n',"
    "'4. CONSERVATIVE: Propose over act. When uncertain, comment with findings and suggested fix only.\\n',"
    "'5. CLOSING: Only close an issue if your PR was already merged by a maintainer and directly fixes it.\\n',"
    "'6. SCOPE: Work only on the assigned repo/issue.\\n',"
    "].join('');\n"
)

old = "      const taskPrompt = msg.prompt || `Work on ${msg.kind} ${msg.repo}#${msg.number}: ${msg.title}`;"
new = guardrail_js + "      const taskPrompt = GUARDRAILS + (msg.prompt || `Work on ${msg.kind} ${msg.repo}#${msg.number}: ${msg.title}`);"

old_review = "        const reviewPrompt = `Check your open PRs on ${completedRepo} for review comments. ` +"
new_review = "        const reviewPrompt = GUARDRAILS + `Check your open PRs on ${completedRepo} for review comments. ` +"

if "GUARDRAILS" in relay:
    print("relay already patched")
    sys.exit(0)

assert old in relay, "taskPrompt anchor not found — hive v2 may have changed, check relay manually"
assert old_review in relay, "reviewPrompt anchor not found"
relay = relay.replace(old, new).replace(old_review, new_review)
open(relay_path, "w").write(relay)
print("relay patched with guardrails")
PYEOF

# ── First-time setup (idempotent) ──────────────────────────────────────
cd "$HIVE_DIR"
just contribute-setup goose

# ── Connect ────────────────────────────────────────────────────────────
echo ""
echo "Starting hive contributor (local mode, Qwen/Qwen3.6-35B-A3B on ghost)..."
just contribute-hive goose local
