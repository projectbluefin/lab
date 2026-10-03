#!/usr/bin/env python3
"""Prepare action-local /dev/shm through bb_runner's temporary-directory hook.

Run with --build-directory /worker/build --listen unix:/run/buildbarn/sandbox/preparer.sock.
The worker owns action cleanup; this helper never mounts or accesses host /dev/shm.
Runtime dependencies (grpcio and protobuf) must already be present in the image.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from contextlib import ExitStack, contextmanager
import errno
import logging
import os
import signal
import time

SERVICE = "buildbarn.tmp_installer.TemporaryDirectoryInstaller"
DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
MAX_WORKERS = 12


def _absolute_components(path: str) -> list[str]:
    if not isinstance(path, str) or not path.startswith("/"):
        raise ValueError("path must be absolute")
    components = path[1:].split("/")
    if any(component in ("", ".", "..") for component in components) or "\x00" in path:
        raise ValueError("path must not contain empty, dot, or parent components or NUL")
    return components


@contextmanager
def _directory(name: str, parent_fd: int | None = None):
    fd = os.open(name, DIRECTORY_FLAGS, dir_fd=parent_fd)
    try:
        yield fd
    finally:
        os.close(fd)


class SandboxPreparer:
    """Use descriptor-relative operations only, rejecting all symlink traversal."""

    def __init__(self, build_directory: str):
        self.components = _absolute_components(os.fspath(build_directory))

    @contextmanager
    def _build_directory(self):
        with ExitStack() as stack:
            fd = stack.enter_context(_directory("/"))
            for component in self.components:
                fd = stack.enter_context(_directory(component, fd))
            yield fd

    def check_readiness(self) -> None:
        with self._build_directory() as fd:
            if not os.access(".", os.R_OK | os.W_OK | os.X_OK, dir_fd=fd, effective_ids=True):
                raise PermissionError(errno.EACCES, "build directory requires read, write, and search access")

    def prepare(self, temporary_directory: str) -> None:
        if not isinstance(temporary_directory, str) or "\x00" in temporary_directory:
            raise ValueError("temporary_directory must be a relative <action>/tmp path without NUL")
        components = temporary_directory.split("/")
        if (
            len(components) != 2
            or components[0] in ("", ".", "..")
            or components[1] != "tmp"
        ):
            raise ValueError("temporary_directory must be exactly a relative <action>/tmp path")
        with self._build_directory() as build_fd, _directory(components[0], build_fd) as action_fd:
            # Both directories must already be worker-created siblings. Validate
            # tmp before mutating root, even though shm does not use its contents.
            with _directory("tmp", action_fd), _directory("root", action_fd) as root_fd:
                self._mkdir("dev", root_fd, 0o755)
                with _directory("dev", root_fd) as dev_fd:
                    self._mkdir("shm", dev_fd, 0o1777)
                    with _directory("shm", dev_fd) as shm_fd:
                        # Correct for the process umask and preexisting modes,
                        # without changing dev, root, or any other action.
                        os.fchmod(shm_fd, 0o1777)

    @staticmethod
    def _mkdir(name: str, parent_fd: int, mode: int) -> None:
        try:
            os.mkdir(name, mode, dir_fd=parent_fd)
        except FileExistsError:
            # The following O_DIRECTORY | O_NOFOLLOW open validates its type.
            pass


def load_protocol():
    """Construct the pinned upstream proto schema without protoc or downloads."""
    import grpc
    from google.protobuf import descriptor_pb2, descriptor_pool, empty_pb2, message_factory

    pool = descriptor_pool.DescriptorPool()
    pool.AddSerializedFile(empty_pb2.DESCRIPTOR.serialized_pb)
    definition = descriptor_pb2.FileDescriptorProto(
        name="buildbarn/tmp_installer/tmp_installer.proto",
        package="buildbarn.tmp_installer",
        syntax="proto3",
        dependency=["google/protobuf/empty.proto"],
    )
    request = definition.message_type.add(name="InstallTemporaryDirectoryRequest")
    request.field.add(
        name="temporary_directory",
        number=1,
        label=descriptor_pb2.FieldDescriptorProto.LABEL_OPTIONAL,
        type=descriptor_pb2.FieldDescriptorProto.TYPE_STRING,
    )
    service = definition.service.add(name="TemporaryDirectoryInstaller")
    service.method.add(
        name="InstallTemporaryDirectory",
        input_type=".buildbarn.tmp_installer.InstallTemporaryDirectoryRequest",
        output_type=".google.protobuf.Empty",
    )
    service.method.add(
        name="CheckReadiness",
        input_type=".google.protobuf.Empty",
        output_type=".google.protobuf.Empty",
    )
    pool.Add(definition)
    request_type = message_factory.GetMessageClass(
        pool.FindMessageTypeByName("buildbarn.tmp_installer.InstallTemporaryDirectoryRequest")
    )
    return grpc, request_type, empty_pb2.Empty


def _abort_filesystem_error(grpc, context, operation: str, error: OSError):
    if error.errno in (errno.EACCES, errno.EPERM):
        code = grpc.StatusCode.PERMISSION_DENIED
    elif error.errno in (errno.ENOSPC, errno.EDQUOT):
        code = grpc.StatusCode.RESOURCE_EXHAUSTED
    elif error.errno in (errno.ENOENT, errno.ENOTDIR, errno.ELOOP, errno.EROFS):
        code = grpc.StatusCode.FAILED_PRECONDITION
    elif error.errno in (errno.EIO, errno.ESTALE, errno.ETIMEDOUT):
        code = grpc.StatusCode.UNAVAILABLE
    else:
        code = grpc.StatusCode.INTERNAL
    context.abort(code, f"{operation}: {error}")


def create_server(preparer: SandboxPreparer, listen: str, executor: ThreadPoolExecutor):
    """Return an unstarted server bound only to the supplied filesystem socket."""
    if not listen.startswith("unix:/"):
        raise ValueError("listen must use unix:/absolute/socket/path")
    _absolute_components(listen[len("unix:"):])
    grpc, request_type, empty_type = load_protocol()
    from google.protobuf.message import DecodeError

    def handler(message_type, operation, description):
        def invoke(payload, context):
            try:
                request = message_type.FromString(payload)
            except DecodeError as error:
                context.abort(grpc.StatusCode.INVALID_ARGUMENT, f"invalid protobuf request: {error}")
            try:
                operation(request)
            except ValueError as error:
                context.abort(grpc.StatusCode.INVALID_ARGUMENT, str(error))
            except OSError as error:
                _abort_filesystem_error(grpc, context, description, error)
            return empty_type()

        return grpc.unary_unary_rpc_method_handler(
            invoke, response_serializer=empty_type.SerializeToString
        )

    server = grpc.server(executor, maximum_concurrent_rpcs=MAX_WORKERS)
    server.add_generic_rpc_handlers((grpc.method_handlers_generic_handler(SERVICE, {
        "InstallTemporaryDirectory": handler(
            request_type, lambda request: preparer.prepare(request.temporary_directory), "sandbox preparation failed"
        ),
        "CheckReadiness": handler(
            empty_type, lambda request: preparer.check_readiness(), "build directory is not ready"
        ),
    }),))
    if not server.add_insecure_port(listen):
        raise OSError(f"failed to bind {listen}")
    return server


def check_server(listen: str) -> None:
    """Probe the real readiness RPC; connection or server failures are failures."""
    if not listen.startswith("unix:/"):
        raise ValueError("listen must use unix:/absolute/socket/path")
    _absolute_components(listen[len("unix:"):])
    grpc, _, empty_type = load_protocol()
    with grpc.insecure_channel(listen) as channel:
        check = channel.unary_unary(
            f"/{SERVICE}/CheckReadiness",
            request_serializer=empty_type.SerializeToString,
            response_deserializer=empty_type.FromString,
        )
        check(empty_type(), timeout=3, wait_for_ready=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-directory", default="/worker/build")
    parser.add_argument("--listen", required=True)
    parser.add_argument("--check", action="store_true", help="check readiness via gRPC and exit")
    args = parser.parse_args(argv)
    logging.basicConfig(level=logging.INFO, format="%(levelname)s: %(message)s")
    if args.check:
        try:
            check_server(args.listen)
        except Exception as error:
            logging.error("sandbox preparer readiness failed: %s", error)
            return 1
        return 0

    stopping = False

    def stop(signum, frame):
        # Do not call gRPC, acquire locks, or log from a signal handler.
        nonlocal stopping
        stopping = True

    previous = {signum: signal.signal(signum, stop) for signum in (signal.SIGTERM, signal.SIGINT)}
    try:
        preparer = SandboxPreparer(args.build_directory)
        preparer.check_readiness()
        with ThreadPoolExecutor(max_workers=MAX_WORKERS) as executor:
            server = create_server(preparer, args.listen, executor)
            try:
                server.start()
                logging.info("sandbox preparer listening on %s", args.listen)
                while not stopping:
                    time.sleep(0.2)
            finally:
                server.stop(10).wait()
    except Exception as error:
        logging.error("sandbox preparer failed: %s", error)
        return 1
    finally:
        for signum, original in previous.items():
            signal.signal(signum, original)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
