"""bst-build.sh decides whether a failed bst-build-re pod is worth an Argo retry.

The fixtures are trimmed `bst build` output from real lab runs. Exit 3 is the
code bst-build-re's retryStrategy does not retry.
"""

import os
import stat
import subprocess
from pathlib import Path

import pytest
import yaml


ROOT = Path(__file__).resolve().parents[2]
CONFIG_MAP = ROOT / "manifests/buildstream-remote-cache-config.yaml"
NO_RETRY_EXIT = 3

# dakota-build-pipeline-wg4t7: the kernel configure check failed. BuildStream
# cached the failed artifact, then failed the element.
KERNEL_CONFIG_FAILURE = """\
[--:--:--][07ff04b7][   build:core/linux-ubuntu.bst         ] START   Waiting for the remote build to complete
[00:00:46][07ff04b7][   build:core/linux-ubuntu.bst         ] SUCCESS Waiting for the remote build to complete
[00:00:56][07ff04b7][   build:core/linux-ubuntu.bst         ] FAILURE Running commands
[--:--:--][07ff04b7][   build:core/linux-ubuntu.bst         ] START   Caching artifact
[00:00:00][07ff04b7][   build:core/linux-ubuntu.bst         ] SUCCESS Caching artifact
[00:02:14][07ff04b7][   build:core/linux-ubuntu.bst         ] FAILURE Command failed

    Printing the last 20 lines from log file:
    grep -q '^CONFIG_CRYPTO_ZSTD=y$' .config' failed with exitcode 1
    [00:00:56] FAILURE core/linux-ubuntu.bst: Running commands
    [--:--:--] START   [07ff04b7] core/linux-ubuntu.bst: Caching artifact
    [00:00:00] SUCCESS [07ff04b7] core/linux-ubuntu.bst: Caching artifact
    [00:02:14] FAILURE [07ff04b7] core/linux-ubuntu.bst: Command failed

[--:--:--][07ff04b7][    push:core/linux-ubuntu.bst         ] START   bluefin/core-linux-ubuntu/07ff04b7-push.20261004-002039.log
[00:00:05][07ff04b7][    push:core/linux-ubuntu.bst         ] SUCCESS bluefin/core-linux-ubuntu/07ff04b7-push.20261004-002039.log
[00:28:05][        ][    main:core activity                 ] FAILURE Build

Failure Summary
    core/linux-ubuntu.bst:
    [00:02:14][07ff04b7][   build:core/linux-ubuntu.bst         ] FAILURE Command failed
"""

# dakota-bling-8jhzb: a worker CAS capture hit its 60 s deadline while the
# integration commands ran. Nothing was cached; a retry can succeed.
WORKER_DEADLINE_FAILURE = """\
[--:--:--][1cfaaafa][   build:gnome-build-meta.bst:core-deps/folks.bst] START   Caching artifact
[00:00:00][1cfaaafa][   build:gnome-build-meta.bst:core-deps/folks.bst] SUCCESS Caching artifact
[00:01:28][1cfaaafa][   build:gnome-build-meta.bst:core-deps/folks.bst] SUCCESS gnome/core-deps-folks/1cfaaafa-build.20261003-233056.log
[00:01:15][9bf58aca][   build:bluefin/shell-extensions/syncthing-toggle.bst] FAILURE Running commands
[00:01:15][9bf58aca][   build:bluefin/shell-extensions/syncthing-toggle.bst] FAILURE Integrating sandbox
[00:01:21][9bf58aca][   build:bluefin/shell-extensions/syncthing-toggle.bst] FAILURE Deadline Exceeded
[00:21:22][        ][    main:core activity                 ] FAILURE Build
"""

# dakota-kernel-recc-9l6bd retry: replaying a cached failure under the
# storage-service CAS crashed before the element ran any command.
REPLAY_BUG = """\
[--:--:--][b82c3a16][   build:core/linux-ubuntu.bst         ] START   bluefin/core-linux-ubuntu/b82c3a16-build.20261004-020619.log
[00:00:00][b82c3a16][   build:core/linux-ubuntu.bst         ] BUG     Build
    FileNotFoundError: [Errno 2] No such file or directory: '/root/.cache/buildstream/cas/objects/36/af8e4d9b'
[00:02:28][        ][    main:core activity                 ] FAILURE Build
"""

# A failed element's caching lines are keyed by its own cache key: another
# element that cached successfully does not make a grid failure deterministic.
DIFFERENT_ELEMENTS = """\
[00:00:00][aaaaaaaa][   build:a.bst] SUCCESS Caching artifact
[00:00:09][bbbbbbbb][   build:b.bst] FAILURE Failed to determine missing blobs
"""


@pytest.fixture
def run_build(tmp_path):
    script = tmp_path / "bst-build.sh"
    script.write_text(yaml.safe_load(CONFIG_MAP.read_text())["data"]["bst-build.sh"])
    fake_bst = tmp_path / "bin/bst"
    fake_bst.parent.mkdir()
    fake_bst.write_text('#!/bin/sh\ncat "$BST_OUTPUT"\necho "$*" > "$BST_ARGS"\nexit "$BST_STATUS"\n')
    fake_bst.chmod(fake_bst.stat().st_mode | stat.S_IEXEC)

    def run(output, status):
        (tmp_path / "out.log").write_text(output)
        env = {
            **os.environ,
            "PATH": f"{fake_bst.parent}:{os.environ['PATH']}",
            "BST_OUTPUT": str(tmp_path / "out.log"),
            "BST_ARGS": str(tmp_path / "args"),
            "BST_STATUS": str(status),
        }
        result = subprocess.run(
            ["bash", str(script), "--no-interactive", "build", "oci/bluefin.bst"],
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
        return result, (tmp_path / "args").read_text().strip()

    return run


def test_cached_element_failure_is_not_retried(run_build):
    result, _ = run_build(KERNEL_CONFIG_FAILURE, 255)

    assert result.returncode == NO_RETRY_EXIT
    assert "core/linux-ubuntu.bst" in result.stderr
    assert "CONFIG_CRYPTO_ZSTD" in result.stdout


@pytest.mark.parametrize("output", [WORKER_DEADLINE_FAILURE, REPLAY_BUG, DIFFERENT_ELEMENTS])
def test_failure_without_cached_failed_artifact_keeps_bst_exit_code(run_build, output):
    result, _ = run_build(output, 255)

    assert result.returncode == 255


def test_success_passes_arguments_and_exits_zero(run_build):
    result, args = run_build(KERNEL_CONFIG_FAILURE, 0)

    assert result.returncode == 0
    assert args == "--no-interactive build oci/bluefin.bst"


def test_rebuilds_instead_of_replaying_cached_failures():
    config = yaml.safe_load(yaml.safe_load(CONFIG_MAP.read_text())["data"]["dakota-buildstream.conf"])

    assert config["build"]["retry-failed"] is True
