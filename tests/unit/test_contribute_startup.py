import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture
def launcher(tmp_path):
    recipe = ROOT / "docs/skills/hive-contribute/contribute.just"
    home = tmp_path / "home"
    profile = home / ".omp/profiles/ghost/agent"
    (profile / "skills").mkdir(parents=True)
    (profile / "APPEND_SYSTEM.md").write_text("Existing owner instructions.\n")
    registration = tmp_path / "registration.env"
    registration.write_text("HIVE_HUB=wss://example.invalid/contribute\nHIVE_REGISTRATION_TOKEN=existing-token\n")
    config = tmp_path / "config.json"
    config.write_text(json.dumps({"data": {"mcp.json": "{}", "tooling.md": "guidance\n"}}))
    binaries = tmp_path / "bin"
    binaries.mkdir()
    calls = tmp_path / "calls"
    calls.touch()
    scripts = {
        "kubectl": """case "$*" in
  *'get sts/contribute-reviewed'*) printf '%s' "$STATEFULSET" ;;
  *'get configmap contribute-omp'*) cat "$BUNDLE_CONFIG" ;;
  *'get pods'*) printf '%s' "$PODS" ;;
  *'has-session'*) exit 0 ;;
  *) exit 99 ;;
esac""",
        "gh": "exit 97",
        "curl": "exit 98",
        "tmux": """case "$*" in
  'has-session -t =hive-lab') exit 0 ;;
  *list-panes*) printf '%s\\n' '%0 exo-0 0 worker-1' '%1 ghost 0 worker-0' ;;
esac""",
    }

    def stub(name, script):
        executable = binaries / name
        executable.write_text(f'#!/bin/sh\nprintf "%s %s\\n" "{name}" "$*" >> "$CALLS"\n{script}\n')
        executable.chmod(0o755)
        return executable

    for name, script in scripts.items():
        stub(name, script)

    def run(action="on", replicas=0, mode="dual-review-v1", **overrides):
        pods = [
            {"metadata": {"name": name}, "spec": {"nodeName": node}, "status": {"phase": "Running"}}
            for name, node in (("worker-0", "ghost"), ("worker-1", "exo-0"))
        ]
        sts = {"spec": {"replicas": replicas, "template": {"metadata": {
            "annotations": {"bluefin.io/contributor-mode": mode}
        }}}}
        environment = {**os.environ, "HOME": str(home), "XDG_RUNTIME_DIR": str(tmp_path),
                       "HIVE_CONTRIBUTE_REGISTRATION": str(registration), "BUNDLE_CONFIG": str(config),
                       "PATH": f"{binaries}:{os.environ['PATH']}", "CALLS": str(calls),
                       "STATEFULSET": json.dumps(sts), "PODS": json.dumps({"items": pods if replicas else []}),
                       **overrides}
        if "TMUX" not in overrides:
            environment.pop("TMUX", None)
        calls.write_text("")
        result = subprocess.run(
            ["just", "--justfile", str(recipe), "contribute", *([action] if action else [])],
            env=environment, capture_output=True, text=True, timeout=20,
        )
        return result, calls.read_text()

    return run, registration, config, stub


def test_missing_safe_tool_assets_do_not_rotate_registration(launcher):
    run, registration, _, _ = launcher
    original = registration.read_text()
    result, calls = run()
    assert result.returncode != 0
    assert "get configmap contribute-omp" in calls, result.stderr
    assert not any(command in calls for command in ("gh auth token", "curl ", "scale "))
    assert registration.read_text() == original


@pytest.mark.parametrize(("replicas", "mode"), [(0, None), (1, None), (1, "dual-review-v1"), (3, "dual-review-v1")])
def test_on_refuses_legacy_or_unexpected_replica_state(launcher, replicas, mode):
    run, _, _, _ = launcher
    result, calls = run(replicas=replicas, mode=mode)
    assert result.returncode != 0
    assert "get sts/contribute-reviewed" in calls
    assert not any(command in calls for command in ("gh auth token", "curl ", "scale ", "rollout ", "configmap "))


@pytest.mark.parametrize("action", ["on", ""])
def test_already_on_attaches_without_startup_side_effects(launcher, action):
    run, _, _, _ = launcher
    result, calls = run(action=action, replicas=2)
    assert result.returncode == 0, result.stderr
    assert "tmux attach-session -t =hive-lab" in calls
    assert not any(command in calls for command in ("gh auth token", "curl ", "scale ", "rollout ", "configmap "))


def test_cold_start_warms_both_models_before_reissuing_once(launcher, tmp_path):
    run, registration, config, stub = launcher
    config.write_text(json.dumps({"data": {
        "mcp.json": "{}", "tooling.md": "guidance\n", "safe-tools.js": "// hook\n",
        "checkout.py": "# hook\n", "verify.py": "# hook\n",
    }}))
    stub("gh", "printf fixture-token")
    stub("curl", """printf '%s' '{"registration_token":"replacement-token"}'""")
    stub("kubectl", """case "$*" in
  *'get sts/contribute-reviewed'*) printf '%s' "$STATEFULSET" ;;
  *'get configmap contribute-omp'*) cat "$BUNDLE_CONFIG" ;;
  *'get pods'*)
    if [ -e "$STARTED" ]; then printf '%s' "$READY_PODS"; else printf '{"items":[]}'; fi ;;
  *'has-session'*) exit 0 ;;
  *'scale sts/contribute-reviewed --current-replicas=0 --replicas=2'*) touch "$STARTED" ;;
  *'scale deploy/'*|*'rollout status'*) exit 0 ;;
  *'/slots?model=ornith'*) printf '[{"n_ctx":%s,"speculative":true}]' "${SLOT_CONTEXT:-262144}" ;;
  *'v1/chat/completions'*) printf '{"usage":{"completion_tokens":1}}' ;;
  *'create secret'*)
    for arg do
      case "$arg" in --from-file=ghost-context.tgz=*) tar -tzf "${arg#--from-file=ghost-context.tgz=}" > "$BUNDLE_CONTENTS" ;; esac
    done
    printf 'secret YAML' ;;
  *'apply -f -'*) cat >/dev/null ;;
  *) exit 99 ;;
esac""")
    ready = {"items": [
        {"metadata": {"name": pod}, "spec": {"nodeName": node},
         "status": {"phase": "Running", "conditions": [{"type": "Ready", "status": "True"}]}}
        for node, pod in (("ghost", "worker-0"), ("exo-0", "worker-1"))
    ]}
    home = tmp_path / "home"
    approved = home / ".omp/profiles/ghost/agent/managed-skills/approved/SKILL.md"
    pending = home / ".local/state/hive/lab/review-pending-skills/unapproved/SKILL.md"
    for skill in (approved, pending):
        skill.parent.mkdir(parents=True)
        skill.write_text("Test lesson\n")
    contents = tmp_path / "bundle-contents"
    options = dict(READY_PODS=json.dumps(ready), BUNDLE_CONTENTS=str(contents))
    result, calls = run(STARTED=str(tmp_path / "started"), **options)
    assert result.returncode == 0, result.stderr
    assert calls.count("/api/contribute/reissue-token") == 1
    rotation = calls.index("gh auth token")
    for node in ("exo-0", "ghost"):
        assert calls.index(f"exec deploy/llm-d-{node}") < rotation
    assert rotation < calls.index("scale sts/contribute-reviewed --current-replicas=0 --replicas=2")
    assert "rollout restart" not in calls
    assert "HIVE_REGISTRATION_TOKEN=replacement-token\n" in registration.read_text()
    assert "managed-skills/approved/SKILL.md" in contents.read_text()
    assert "unapproved" not in contents.read_text()
    result, calls = run(SLOT_CONTEXT="102400", STARTED=str(tmp_path / "must-not-start"), **options)
    assert result.returncode != 0
    assert "262144" in result.stderr
    assert "reissue-token" not in calls
    assert not (tmp_path / "must-not-start").exists()
    assert registration.read_text().endswith("HIVE_REGISTRATION_TOKEN=replacement-token\n")


def test_models_starts_native_context_without_hive_credentials(launcher):
    run, registration, _, stub = launcher
    previous = registration.read_text()
    stub("kubectl", """case "$*" in
  *'get sts/contribute-reviewed'*) printf '%s' "$STATEFULSET" ;;
  *'get pods'*) printf '%s' "$PODS" ;;
  *'scale deploy/'*|*'rollout status'*) exit 0 ;;
  *'/slots?model=ornith'*) printf '[{"n_ctx":262144,"speculative":true}]' ;;
  *'v1/chat/completions'*) printf '{"usage":{"completion_tokens":1}}' ;;
  *) exit 99 ;;
esac""")
    result, calls = run("models", mode=None)
    assert result.returncode == 0, result.stderr
    assert set(line.split("exec deploy/")[1].split()[0] for line in calls.splitlines()
               if "v1/chat/completions" in line) == {"llm-d-exo-0", "llm-d-ghost"}
    assert "gh auth token" not in calls and "reissue-token" not in calls
    assert "scale sts/contribute" not in calls
    assert registration.read_text() == previous


def test_off_refuses_shutdown_without_a_safe_drain(launcher):
    run, _, _, _ = launcher
    result, calls = run(action="off", replicas=2)
    assert result.returncode != 0
    assert "drain" in result.stderr
    assert "scale " not in calls


def test_off_saves_both_workers_before_releasing_gpus(launcher, tmp_path):
    run, _, _, stub = launcher
    stub("kubectl", """case "$*" in
  *'get sts/contribute-reviewed'*) printf '%s' "$STATEFULSET" ;;
  *'scale sts/contribute-reviewed --current-replicas=2 --replicas=0'*) touch "$STOPPED" ;;
  *'get pods'*)
    if [ -e "$STOPPED" ]; then printf '{"items":[]}'; else printf '%s' "$PODS"; fi ;;
  *'cat /home/hive/.config/hive/relay-status.json'*)
    printf '{"operator_drain":"%s"}' "$(if [ -e "$STOPPED.$5" ]; then echo drained; else echo open; fi)" ;;
  *'touch /tmp/hive-operator-drain'*) touch "$STOPPED.$5" ;;
  *' cp '*)
    for arg do dest=$arg; done
    mkdir -p "$dest/new-lesson" && printf 'Captured lesson\\n' > "$dest/new-lesson/SKILL.md" ;;
  *'mkdir -p'*|*'scale deploy/'*) exit 0 ;;
  *) exit 99 ;;
esac""")
    pods = {"items": [{
        "metadata": {"name": name}, "spec": {"nodeName": node},
        "status": {"phase": "Running", "containerStatuses": [{"name": "contribute", "state": {"running": {}}}]},
    } for name, node in (("worker-0", "ghost"), ("worker-1", "exo-0"))]}
    result, calls = run("off", replicas=2, STOPPED=str(tmp_path / "stopped"), PODS=json.dumps(pods))
    assert result.returncode == 0, result.stderr
    assert calls.count("touch /tmp/hive-operator-drain") == 2
    contributor_stop = calls.index("scale sts/contribute-reviewed --current-replicas=2 --replicas=0")
    assert calls.index("worker-0:/home/hive/.omp/agent/managed-skills/.") < contributor_stop
    assert calls.index("worker-1:/home/hive/.omp/agent/managed-skills/.") < contributor_stop
    assert contributor_stop < calls.index("scale deploy/llm-d-exo-0 deploy/llm-d-ghost --replicas=0")
    home = tmp_path / "home"
    assert (home / ".local/state/hive/lab/review-pending-skills/new-lesson/SKILL.md").is_file()
    assert not (home / ".omp/profiles/ghost/agent/managed-skills/new-lesson").exists()


def test_attach_switches_an_existing_tmux_client_and_reports_missing_lane(launcher):
    run, _, _, _ = launcher
    result, calls = run(action="attach", TMUX="existing", PODS=json.dumps({"items": [{
        "metadata": {"name": "worker-1"}, "spec": {"nodeName": "exo-0"}, "status": {"phase": "Running"}
    }]}))
    assert result.returncode == 0, result.stderr
    assert "ghost: remote lane unavailable" in result.stderr
    assert "tmux switch-client -t =hive-lab" in calls
    assert "respawn-pane" not in calls


@pytest.mark.skipif(shutil.which("tmux") is None, reason="requires tmux")
def test_real_tmux_reconnect_preserves_healthy_pane(launcher, tmp_path):
    run, _, _, stub = launcher
    socket = tmp_path / "tmux.sock"
    tmux = shutil.which("tmux")
    stub("kubectl", """case "$*" in
  *'get pods'*) printf '%s' "$PODS" ;;
  *'has-session'*) exit 0 ;;
  *'attach -t'*) printf 'attached %s\\n' "$*"; exec sleep infinity ;;
  *) exit 99 ;;
esac""")
    # Suppress only the final client attachment (pytest has no controlling TTY).
    stub("tmux", f"""[ "$1" != attach-session ] || exit 0
exec '{tmux}' -S '{socket}' -f /dev/null "$@" """)

    def command(*args):
        return subprocess.check_output([tmux, "-S", str(socket), *args], text=True, timeout=5)

    def pane_rows():
        return [row.split() for row in command(
            "list-panes", "-t", "hive-lab:lanes", "-F",
            "#{pane_left} #{@hive_lane} #{pane_pid} #{@hive_pod}"
        ).splitlines()]

    try:
        ghost_only = {"items": [{"metadata": {"name": "worker-0"}, "spec": {"nodeName": "ghost"},
                                 "status": {"phase": "Running"}}]}
        result, _ = run(action="attach", PODS=json.dumps(ghost_only))
        assert result.returncode == 0, result.stderr
        rows = pane_rows()
        assert rows[0][:2] == ["0", "exo-0"]
        assert rows[1][1::2] == ["ghost", "worker-0"]
        assert "exo-0: remote lane unavailable" in result.stderr
        result, _ = run(action="attach", replicas=2)
        assert result.returncode == 0, result.stderr
        recovered = pane_rows()
        assert recovered[0][-1] == "worker-1"
        assert recovered[1] == rows[1]
        assert command("show-options", "-wv", "-t", "hive-lab:lanes", "synchronize-panes").strip() == "off"
        assert command("list-sessions", "-F", "#{session_name}").strip() == "hive-lab"
    finally:
        subprocess.run([tmux, "-S", str(socket), "kill-server"], capture_output=True, timeout=5)
