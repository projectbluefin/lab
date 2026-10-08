"""Static contract for run-chairlift-wayland-tests (ChairLift's live Wayland lane)."""

import re
import subprocess
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
TEMPLATE = ROOT / "argo/workflow-templates/run-chairlift-wayland-tests.yaml"


def _doc():
    return yaml.safe_load(TEMPLATE.read_text(encoding="utf-8"))


def _template():
    return next(t for t in _doc()["spec"]["templates"] if t["name"] == "run-chairlift-wayland-tests")


def _runner():
    return _template()["script"]["source"]


def _heredoc(source, tag):
    match = re.search(rf"<<'{tag}'[^\n]*\n(.*?)\n\s*{tag}\n", source, re.S)
    assert match, f"heredoc {tag} not found"
    return match.group(1)


def _bash_parses(script):
    result = subprocess.run(["bash", "-n"], input=script, text=True, capture_output=True)
    assert result.returncode == 0, result.stderr


def test_parameters_are_the_lane_contract():
    names = [p["name"] for p in _doc()["spec"]["arguments"]["parameters"]]
    assert names == [
        "chairlift-repo",
        "chairlift-branch",
        "image",
        "image-tag",
        "behave-tags",
        "goose-isolation-check",
        "answer-quality",
    ]
    defaults = {p["name"]: p["value"] for p in _doc()["spec"]["arguments"]["parameters"]}
    assert defaults["image"] == "ghcr.io/projectbluefin/dakota"
    assert defaults["image-tag"] == "testing"


def test_runs_on_ghost_behind_the_container_qa_semaphore():
    template = _template()
    assert template["nodeSelector"]["kubernetes.io/hostname"] == "ghost"
    keys = [s["configMapKeyRef"]["key"] for s in template["synchronization"]["semaphores"]]
    assert keys == ["ghost-container-qa"]
    assert template["activeDeadlineSeconds"] <= 7200


def test_builds_the_requested_branch_with_the_e2e_target():
    build = _template()["initContainers"][0]
    script = build["args"][0]
    assert 'git clone --depth 1 --branch "${CHAIRLIFT_BRANCH}" "${CHAIRLIFT_REPO}"' in script
    assert "CGO_ENABLED=0 make build-e2e schemas" in script
    # ChairLift's whole CI gate runs on the untouched checkout, with the
    # golangci-lint CI pins, verified by checksum, before any lane tool exists.
    assert script.index("make ci >") < script.index("build/lane/navjson")
    assert 'echo "${LINT_SHA256}  /tmp/${LINT_TGZ}" | sha256sum -c -' in script
    assert "troubleshoot.ProfileAt(os.Args[2])" in script
    # The install goes through ChairLift's own Setup, not hand-written brew.
    assert "troubleshoot.Setup(troubleshoot.Detect()," in script
    assert "navigation.Items()" in script
    # Installed the way the Homebrew cask lays ChairLift out.
    assert "install -m 0755 data/chairlift-wrapper.sh /workspace/lane/install/chairlift-wrapper" in script
    _bash_parses(script)


def test_boots_the_run_container_tests_wayland_target():
    source = _runner()
    setup = _heredoc(source, "NESTED_SETUP")
    assert "--systemd=always" in source and "--privileged" in source
    # Rule 12: headless shell, never DRM master, Shell.Eval kept.
    assert "--mode=%i --unsafe-mode --headless --virtual-monitor 1920x1080" in setup
    assert "loginctl enable-linger bluefin-test" in setup
    assert 'chage -d "$(date +%Y-%m-%d)" bluefin-test' in setup
    assert "--volume /etc/resolv.conf:/etc/resolv.conf:ro" in source
    assert "qecore-headless \\\n  --session-type wayland" in source
    assert "systemctl --global mask brew-preinstall.service" in setup
    _bash_parses(source)
    _bash_parses(setup)


def test_wayland_only_never_x11():
    text = TEMPLATE.read_text(encoding="utf-8")
    for banned in ("Xvfb", "xvfb", "GDK_BACKEND=x11", "xwd", "DISPLAY=:"):
        assert banned not in text, banned
    runner = _heredoc(_runner(), "RUNNER")
    assert "GDK_BACKEND=wayland" in runner
    assert "unset DISPLAY XAUTHORITY" in runner
    # Goose's window comes up exactly as ChairLift launches it: no Electron
    # platform hint, and no DISPLAY to fall back to (unset above).
    assert "ELECTRON_OZONE_PLATFORM_HINT" not in runner
    assert "org.gnome.Shell.Screenshot" in runner
    _bash_parses(runner)


def test_no_shell_modal_holds_the_keyboard():
    source = _runner()
    setup = _heredoc(source, "NESTED_SETUP")
    assert "gsettings set org.gnome.shell welcome-dialog-last-shown-version" in setup
    runner = _heredoc(source, "RUNNER")
    assert "'Main.modalCount'" in runner


def test_suite_isolation_matches_the_fixture_harness():
    source = _runner()
    runner = _heredoc(source, "RUNNER")
    # A shipped config.yml outranks every fixture; the suite refuses it.
    assert "--tmpfs /usr/share/chairlift:ro,notmpcopyup" in source
    assert "destination=/proc/cmdline,ro" in source
    for setting in ("GSETTINGS_BACKEND=memory", "GDK_DEBUG=no-portals", "GTK_A11Y=atspi", "CHAIRLIFT_REQUIRE_ATSPI=1"):
        assert setting in runner, setting
    assert "--require-hashes -r /workspace/chairlift/test/e2e/requirements-atspi.txt" in source


def test_fails_closed_and_cleans_up():
    source = _runner()
    assert "trap on_exit EXIT" in source
    assert "trap 'exit 143' TERM" in source
    assert "trap 'exit 130' INT" in source
    assert 'podman rm --force "${TARGET_NAME}"' in source
    assert "0 scenarios executed" in source
    assert "sys.exit(1)" in source


def test_engine_check_runs_chairlifts_own_paths():
    runner = _heredoc(_runner(), "RUNNER")
    # ChairLift installs the packages itself, with no Homebrew on PATH and
    # no `brew shellenv`, the way a direct launch runs it.
    assert "PATH=/usr/bin:/bin" in runner
    assert '"${LANE}/bin/troubleshootcheck" setup' in runner
    assert '"${LANE}/bin/troubleshootcheck" command-path' in runner
    assert 'eval "$("${BREW}" shellenv)"' not in runner
    assert '"${BREW}" install' not in runner
    assert '"${LANE}/bin/troubleshootcheck" agent-mode' in runner
    assert '"${LANE}/bin/troubleshootcheck" model' in runner
    # No lane fallbacks: PullModel must succeed, and nothing is trusted or
    # aliased on ChairLift's behalf.
    for fallback in ("troubleshootcheck alias", "not found in daemon stored models", '"${BREW}" trust', "brew trust"):
        assert fallback not in runner, fallback
    build = _template()["initContainers"][0]["args"][0]
    assert "/usr/bin/true" not in build
    for call in ("aistack.Enable(ctx)", "aistack.WaitHealthy(", "aistack.PullModel(", "aistack.ConfigureActiveModel("):
        assert call in build, call
    # Goose is launched by ChairLift itself: a cold --ask-bluefin, and the
    # distro menu's exact command, both with GNOME Shell's environment.
    assert 'observe_launch askbluefin "${BIN}/chairlift" --ask-bluefin' in runner
    assert 'observe_launch menu bash -c "${BIN}/chairlift-wrapper --ask-bluefin"' in runner
    assert 'env -i "${SHELL_ENV[@]}" PATH="${MENU_PATH}"' in runner
    assert "grep -v linuxbrew" in runner
    # Nothing fakes the provider: llmman hands Goose its environment.
    for fake in ("GOOSE_PROVIDER=openai", "OPENAI_HOST=http://127.0.0.1:9", "OPENAI_API_KEY=x"):
        assert fake not in runner, fake
    assert 'launch goose --model bluefin-active -- run --text' in runner
    assert "systemctl --failed --no-legend --plain" in runner
    source = _runner()
    for check in (
        "setup_ran_without_brew_on_path",
        "chairlift_setup_exit_zero",
        "goose_desktop_resolves_after_setup",
        "agent_mode_enabled_on_stock_host",
        "llmman_formula_trusted",
        "llmmanorg_tap_not_trusted",
        "pullmodel_succeeded_and_alias_set",
        "command_env_path_has_brew_bin_first",
        "_launcher_exit_zero",
        "_env_goose_path_root",
        "_env_openai_host_agent_mode",
        "_lock_in_profile",
        "menu_same_goose_env_as_ask_bluefin",
        "linux_tools_tool_called",
        "search_knowledge_tool_called",
        "only_linux_tools_and_bluefin_knowledge_enabled",
        "goose_data_or_state_under_path_root",
        "nothing_in_home_config_goose",
    ):
        assert check in source, check


def test_screenshots_use_chairlifts_own_wayland_capture():
    source = _runner()
    assert "/workspace/chairlift/test/e2e/wayland_session.sh" in source
    assert "/workspace/chairlift/test/e2e/capture_walkthrough.sh" in source
    assert "CHAIRLIFT_WAYLAND_SIZE=900x700" in source
    assert "views: reset group built" in source
    # Byproducts stay behind; only the page PNGs are deliverable.
    assert "-name '[0-9]-*.png'" in source
    assert 'CI_RC}" != "0"' in source


def test_relaunch_while_goose_runs_reuses_the_session():
    runner = _heredoc(_runner(), "RUNNER")
    assert 'KEEP_GOOSE=1 observe_launch askbluefin "${BIN}/chairlift" --ask-bluefin' in runner
    assert 'bash -c "${BIN}/chairlift-wrapper --ask-bluefin"' in runner
    assert "mains_before=" in runner and "mains_after=" in runner
    source = _runner()
    for check in ("relaunch_while_running_exit_zero", "relaunch_no_second_goose", "relaunch_goose_window_exists",
                  "relaunch_goose_window_raised"):
        assert check in source, check
