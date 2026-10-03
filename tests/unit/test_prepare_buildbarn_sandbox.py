"""Behavioral tests for the action-local BuildBarn sandbox installer."""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import errno
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import sys
from tempfile import TemporaryDirectory
from threading import Barrier, Event
from types import SimpleNamespace

import pytest

repo_root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(repo_root / "scripts"))

import prepare_buildbarn_sandbox as sandbox


def make_action(build_directory: Path, name: str = "action") -> Path:
    action = build_directory / name
    (action / "root").mkdir(parents=True)
    (action / "tmp").mkdir()
    return action


@pytest.fixture
def build_directory(tmp_path):
    directory = tmp_path / "build"
    directory.mkdir()
    return directory


def mode(path: Path) -> int:
    return stat.S_IMODE(path.stat().st_mode)


def test_creates_action_local_sticky_directory_with_restrictive_umask(build_directory):
    action = make_action(build_directory)
    (action / "root").chmod(0o750)
    original_umask = os.umask(0o077)
    try:
        sandbox.SandboxPreparer(build_directory).prepare("action/tmp")
    finally:
        os.umask(original_umask)
    assert mode(action / "root/dev/shm") == 0o1777
    assert mode(action / "root") == 0o750
    assert list((action / "tmp").iterdir()) == []
    assert not (build_directory / "dev").exists()


def test_preserves_dev_permissions_and_shm_contents_on_repeated_calls(build_directory):
    action = make_action(build_directory)
    dev = action / "root/dev"
    dev.mkdir(mode=0o700)
    shm = dev / "shm"
    shm.mkdir(mode=0o700)
    marker = shm / "sem.existing"
    marker.write_text("preserved")
    inode = shm.stat().st_ino
    preparer = sandbox.SandboxPreparer(build_directory)
    preparer.prepare("action/tmp")
    preparer.prepare("action/tmp")
    assert mode(dev) == 0o700
    assert mode(shm) == 0o1777
    assert shm.stat().st_ino == inode
    assert marker.read_text() == "preserved"


def test_concurrent_actions_are_isolated_and_duplicate_installs_are_safe(build_directory):
    names = [f"action-{number}" for number in range(12)]
    actions = [make_action(build_directory, name) for name in names]
    preparer = sandbox.SandboxPreparer(build_directory)
    with ThreadPoolExecutor(max_workers=12) as executor:
        list(executor.map(preparer.prepare, [f"{name}/tmp" for name in names] * 3))
    directories = [action / "root/dev/shm" for action in actions]
    assert len({directory.stat().st_ino for directory in directories}) == 12
    for number, directory in enumerate(directories):
        assert mode(directory) == 0o1777
        (directory / "sem.identical-name").write_text(str(number))
    for number, directory in enumerate(directories):
        assert (directory / "sem.identical-name").read_text() == str(number)
    assert set(build_directory.iterdir()) == set(actions)


@pytest.mark.parametrize("temporary_directory", [
    "", "/action/tmp", "../tmp", "./tmp", "action/../tmp", "action//tmp",
    "action/tmp/", "action/tmp/nested", "nested/action/tmp", "action/root",
    "action", "action\x00/tmp", "action/tmp\x00", None,
])
def test_rejects_invalid_relative_paths_without_mutation(build_directory, temporary_directory):
    action = make_action(build_directory)
    with pytest.raises(ValueError, match="temporary_directory"):
        sandbox.SandboxPreparer(build_directory).prepare(temporary_directory)
    assert not (action / "root/dev").exists()


@pytest.mark.parametrize("missing", ["action", "action/root", "action/tmp"])
def test_missing_worker_layout_is_not_created(build_directory, missing):
    action = make_action(build_directory)
    shutil.rmtree(build_directory / missing)
    with pytest.raises(FileNotFoundError):
        sandbox.SandboxPreparer(build_directory).prepare("action/tmp")
    assert not (action / "root/dev").exists()
    assert not (build_directory / missing).exists()


@pytest.mark.parametrize("component", ["action", "action/root", "action/tmp", "action/root/dev", "action/root/dev/shm"])
def test_rejects_symlinks_at_every_action_component(build_directory, component):
    action = make_action(build_directory)
    (action / "root/dev/shm").mkdir(parents=True)
    outside = build_directory.parent / "outside"
    (outside / "root/dev/shm").mkdir(parents=True)
    (outside / "tmp").mkdir()
    (outside / "root/dev/shm").chmod(0o700)
    marker = outside / "untouched"
    marker.write_text("outside")
    expected_modes = {path: mode(path) for path in outside.rglob("*")}
    replacement = build_directory / component
    shutil.rmtree(replacement)
    replacement.symlink_to(outside, target_is_directory=True)
    with pytest.raises(OSError) as raised:
        sandbox.SandboxPreparer(build_directory).prepare("action/tmp")
    assert raised.value.errno in (errno.ENOTDIR, errno.ELOOP)
    assert marker.read_text() == "outside"
    assert {path: mode(path) for path in outside.rglob("*")} == expected_modes


@pytest.mark.parametrize("component", ["root", "tmp", "root/dev", "root/dev/shm"])
def test_rejects_non_directory_layout_components(build_directory, component):
    action = make_action(build_directory)
    target = action / component
    if target.exists():
        target.rmdir()
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("not a directory")
    with pytest.raises(OSError) as raised:
        sandbox.SandboxPreparer(build_directory).prepare("action/tmp")
    assert raised.value.errno == errno.ENOTDIR
    assert target.read_text() == "not a directory"


def test_rejects_symlink_swapped_between_mkdir_and_open(build_directory, monkeypatch):
    action = make_action(build_directory)
    outside = build_directory.parent / "outside"
    outside.mkdir()
    original_open = os.open

    def racing_open(name, flags, **kwargs):
        if name == "dev":
            dev = action / "root/dev"
            dev.rmdir()
            dev.symlink_to(outside, target_is_directory=True)
        return original_open(name, flags, **kwargs)

    monkeypatch.setattr(sandbox.os, "open", racing_open)
    with pytest.raises(OSError):
        sandbox.SandboxPreparer(build_directory).prepare("action/tmp")
    assert list(outside.iterdir()) == []


@pytest.mark.parametrize("component", ["build", "parent"])
def test_rejects_symlink_in_configured_build_path(tmp_path, component):
    real = tmp_path / "real"
    real.mkdir()
    make_action(real)
    alias = tmp_path / "alias"
    alias.symlink_to(real if component == "build" else tmp_path, target_is_directory=True)
    configured = alias if component == "build" else alias / "real"
    preparer = sandbox.SandboxPreparer(configured)
    with pytest.raises(OSError):
        preparer.check_readiness()
    with pytest.raises(OSError):
        preparer.prepare("action/tmp")
    assert not (real / "action/root/dev").exists()


def test_readiness_checks_access_without_creating_actions(build_directory, monkeypatch):
    preparer = sandbox.SandboxPreparer(build_directory)
    preparer.check_readiness()
    assert list(build_directory.iterdir()) == []
    monkeypatch.setattr(sandbox.os, "access", lambda *args, **kwargs: False)
    with pytest.raises(PermissionError, match="read, write, and search"):
        preparer.check_readiness()
    assert list(build_directory.iterdir()) == []


@pytest.fixture
def protocol():
    grpc, request_type, empty_type = sandbox.load_protocol()
    return SimpleNamespace(grpc=grpc, request_type=request_type, empty_type=empty_type)


@pytest.fixture
def grpc_server(build_directory, protocol):
    preparer = sandbox.SandboxPreparer(build_directory)
    # A short path keeps the Unix socket below sockaddr_un's length limit.
    with TemporaryDirectory(prefix="bbpreparer-") as socket_directory:
        listen = f"unix:{socket_directory}/s"
        with ThreadPoolExecutor(max_workers=12) as executor:
            server = sandbox.create_server(preparer, listen, executor)
            server.start()
            try:
                with protocol.grpc.insecure_channel(listen) as channel:
                    protocol.grpc.channel_ready_future(channel).result(timeout=5)
                    install = channel.unary_unary(f"/{sandbox.SERVICE}/InstallTemporaryDirectory")
                    readiness = channel.unary_unary(f"/{sandbox.SERVICE}/CheckReadiness")
                    yield SimpleNamespace(
                        protocol=protocol, preparer=preparer, listen=listen,
                        install=install, readiness=readiness,
                    )
            finally:
                server.stop(0).wait()


def test_grpc_matches_upstream_wire_schema_and_returns_empty(build_directory, grpc_server):
    action = make_action(build_directory)
    # Manually encoded string field #1 avoids testing a matching wrong schema.
    assert grpc_server.install(b"\x0a\x0aaction/tmp", timeout=3) == b""
    assert grpc_server.readiness(b"", timeout=3) == b""
    assert mode(action / "root/dev/shm") == 0o1777
    request = grpc_server.protocol.request_type(temporary_directory="action/tmp")
    assert request.SerializeToString() == b"\x0a\x0aaction/tmp"
    assert request.DESCRIPTOR.full_name == "buildbarn.tmp_installer.InstallTemporaryDirectoryRequest"


def test_readiness_saturation_queues_action_installation(build_directory, grpc_server, monkeypatch):
    action = make_action(build_directory)
    occupied = Barrier(sandbox.MAX_WORKERS + 1)
    release = Event()

    def readiness():
        occupied.wait(timeout=5)
        assert release.wait(timeout=5)

    monkeypatch.setattr(grpc_server.preparer, "check_readiness", readiness)
    probes = [grpc_server.readiness.future(b"", timeout=10) for _ in range(sandbox.MAX_WORKERS)]
    installation = None
    try:
        occupied.wait(timeout=5)
        installation = grpc_server.install.future(b"\x0a\x0aaction/tmp", timeout=10)
        # All execution threads are busy. The action must wait, not receive
        # RESOURCE_EXHAUSTED because health probes occupy the RPC limit.
        with pytest.raises(grpc_server.protocol.grpc.FutureTimeoutError):
            installation.result(timeout=0.2)
    finally:
        release.set()
    for probe in probes:
        assert probe.result(timeout=5) == b""
    assert installation.result(timeout=5) == b""
    assert mode(action / "root/dev/shm") == 0o1777


@pytest.mark.parametrize("payload", [b"", b"\x0a\x06../tmp", b"\x0a\xff"])
def test_grpc_invalid_requests_fail_closed(build_directory, grpc_server, payload):
    action = make_action(build_directory)
    with pytest.raises(grpc_server.protocol.grpc.RpcError) as raised:
        grpc_server.install(payload, timeout=3)
    assert raised.value.code() == grpc_server.protocol.grpc.StatusCode.INVALID_ARGUMENT
    assert raised.value.details()
    assert not (action / "root/dev").exists()


def test_grpc_missing_layout_fails_closed(grpc_server):
    with pytest.raises(grpc_server.protocol.grpc.RpcError) as raised:
        grpc_server.install(b"\x0a\x0aaction/tmp", timeout=3)
    assert raised.value.code() == grpc_server.protocol.grpc.StatusCode.FAILED_PRECONDITION
    assert "sandbox preparation failed" in raised.value.details()


@pytest.mark.parametrize("error_number,status", [
    (errno.EACCES, "PERMISSION_DENIED"), (errno.EPERM, "PERMISSION_DENIED"),
    (errno.ENOSPC, "RESOURCE_EXHAUSTED"), (errno.EDQUOT, "RESOURCE_EXHAUSTED"),
    (errno.ENOENT, "FAILED_PRECONDITION"), (errno.ELOOP, "FAILED_PRECONDITION"),
    (errno.EROFS, "FAILED_PRECONDITION"), (errno.EIO, "UNAVAILABLE"),
])
@pytest.mark.parametrize("operation", ["prepare", "check_readiness"])
def test_grpc_filesystem_errors_have_clear_statuses(grpc_server, monkeypatch, error_number, status, operation):
    def fail(*args):
        raise OSError(error_number, "filesystem unavailable")

    monkeypatch.setattr(grpc_server.preparer, operation, fail)
    rpc = grpc_server.install if operation == "prepare" else grpc_server.readiness
    payload = b"\x0a\x0aaction/tmp" if operation == "prepare" else b""
    with pytest.raises(grpc_server.protocol.grpc.RpcError) as raised:
        rpc(payload, timeout=3)
    assert raised.value.code() == getattr(grpc_server.protocol.grpc.StatusCode, status)
    assert "filesystem unavailable" in raised.value.details()


def test_grpc_readiness_rejects_malformed_empty_message(grpc_server):
    with pytest.raises(grpc_server.protocol.grpc.RpcError) as raised:
        grpc_server.readiness(b"\xff", timeout=3)
    assert raised.value.code() == grpc_server.protocol.grpc.StatusCode.INVALID_ARGUMENT


def test_cli_probe_exercises_actual_readiness_and_reports_failure(build_directory, grpc_server):
    assert sandbox.main(["--listen", grpc_server.listen, "--check"]) == 0
    build_directory.rmdir()
    assert sandbox.main(["--listen", grpc_server.listen, "--check"]) == 1


@pytest.mark.parametrize("listen", ["localhost:8980", "unix:relative", "unix:/tmp/../s", "unix:///tmp/s"])
def test_server_rejects_noncanonical_or_nonunix_listen_addresses(build_directory, listen):
    with ThreadPoolExecutor(max_workers=1) as executor:
        with pytest.raises(ValueError):
            sandbox.create_server(sandbox.SandboxPreparer(build_directory), listen, executor)
    assert sandbox.main(["--listen", listen, "--check"]) == 1


@pytest.mark.parametrize("signum", [signal.SIGTERM, signal.SIGINT])
def test_server_shuts_down_cleanly_on_signals(build_directory, protocol, signum):
    with TemporaryDirectory(prefix="bbpreparer-") as socket_directory:
        listen = f"unix:{socket_directory}/s"
        process = subprocess.Popen([
            sys.executable, str(repo_root / "scripts/prepare_buildbarn_sandbox.py"),
            "--build-directory", str(build_directory), "--listen", listen,
        ], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            with protocol.grpc.insecure_channel(listen) as channel:
                protocol.grpc.channel_ready_future(channel).result(timeout=5)
                process.send_signal(signum)
                stdout, stderr = process.communicate(timeout=5)
            assert process.returncode == 0, stderr
            assert stdout == ""
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate(timeout=5)
