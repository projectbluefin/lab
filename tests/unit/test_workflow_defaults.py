from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[2]


def test_dakota_requires_distributed_capacity_matched_execution():
    config = (ROOT / "manifests/buildstream-remote-cache-config.yaml").read_text(
        encoding="utf-8"
    )
    pipeline = (ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml").read_text(
        encoding="utf-8"
    )

    assert "fetchers: 8" in config
    assert "builders: 4" in config
    assert "pushers: 4" in config
    assert "max-jobs: 12" in config
    assert "nodeSelector:\n        kubernetes.io/hostname: ghost" not in pipeline
    assert "depends: detect-build-mode" in pipeline
    assert "Verified BuildStream remote execution configuration" in pipeline


def test_bst_pipelines_require_fresh_usb4_backed_remote_execution():
    for filename in (
        "dakota-build-pipeline.yaml",
        "bluefin-server-build-pipeline.yaml",
        "bst-qa-pipeline.yaml",
    ):
        pipeline = (ROOT / "argo/workflow-templates" / filename).read_text(
            encoding="utf-8"
        )

        assert "set -euo pipefail" in pipeline
        assert "for NODE in ghost exo-0" in pipeline
        assert "usb4-link" in pipeline
        assert "usb4-link-observed-at" in pipeline
        assert "kubectl get pods -n buildbarn -l app=worker" in pipeline
        assert "template: bst-build-local" not in pipeline
        assert "name: bst-build-local" not in pipeline


def test_dakota_production_lane_has_no_local_fallback():
    pipeline = (ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml").read_text(
        encoding="utf-8"
    )

    assert "template: run-bst-step" in pipeline
    assert "name: bst-build-re" in pipeline
    assert "name: bst-build-local" not in pipeline
    assert "template: bst-build-local" not in pipeline


def test_usb4_monitor_publishes_a_fresh_observation_on_every_probe():
    monitor = (ROOT / "manifests/usb4-link-monitor.yaml").read_text(
        encoding="utf-8"
    )

    assert "lab.projectbluefin.io/usb4-link-observed-at" in monitor
    assert "date -u +%s" in monitor
    assert "kubectl label node" in monitor
    assert "N % 20" not in monitor


def test_no_standalone_cache_warming_buildstream_workflow_remains():
    assert not (
        ROOT / "argo/workflow-templates/dakota-buildstream-warm-cache.yaml"
    ).exists()


def test_ghost_lab_status_reporter_authenticates_with_the_github_token():
    reporter = (ROOT / "argo/workflow-templates/github-status-reporter.yaml").read_text(
        encoding="utf-8"
    )

    assert "name: GITHUB_TOKEN" in reporter
    assert "name: github-token" in reporter
    assert '-H "Authorization: Bearer ${GITHUB_TOKEN}"' in reporter
    assert reporter.count("GITHUB_TOKEN") == 2
    assert "set -x" not in reporter
    assert "echo ${GITHUB_TOKEN}" not in reporter
    assert 'echo "${GITHUB_TOKEN}"' not in reporter


def test_dakota_runner_allows_native_chroot_input_root_execution():
    worker = (ROOT / "manifests/buildbarn-worker.yaml").read_text(encoding="utf-8")
    assert "name: runner" in worker
    assert "privileged: true" in worker
    assert "runAsUser: 0" in worker
    assert "type: spc_t" in worker
    assert "bb-runner-installer:20260722T162832Z-236bcd9" in worker
    assert "bb-worker:20260722T162832Z-236bcd9" in worker
    assert "add: [SYS_CHROOT]" not in worker


def test_buildbarn_runner_uses_stable_tmpdir_after_chroot():
    config = (ROOT / "manifests/buildbarn-config.yaml").read_text(encoding="utf-8")
    assert "setTmpdirEnvironmentVariable:" not in config
    assert "concurrency: 12" in config
    assert "runCommandsAs: { userId: 0, groupId: 0 }" in config
    # Production uses the native build directory: the virtual/FUSE experiment
    # failed startup with "operation not permitted" and is not a valid gate.
    assert "native:" in config
    assert "virtual:" not in config
    assert "buildDirectoryPath: '/worker/build'" in config
    assert "maximumCacheFileCount: 1000000" in config
    assert "maximumCacheSizeBytes: 96 * 1024 * 1024 * 1024" in config
    assert "filePool:" not in config


def test_bluefin_server_build_pipeline_builds_k0s_sysext():
    pipeline_path = ROOT / "argo/workflow-templates/bluefin-server-build-pipeline.yaml"
    assert pipeline_path.exists()
    pipeline = yaml.safe_load(pipeline_path.read_text(encoding="utf-8"))

    core = next(t for t in pipeline["spec"]["templates"] if t["name"] == "build-core")
    tasks = {task["name"]: task for task in core["dag"]["tasks"]}

    assert "build-ddi" in tasks
    assert "build-installer" in tasks
    assert "build-sysext" in tasks

    sysext_args = {p["name"]: p["value"] for p in tasks["build-sysext"]["arguments"]["parameters"]}
    assert sysext_args["element"] == "oci/k0s-sysext.bst"
    assert sysext_args["tag"] == "k0s-sysext"


def test_cache_only_diagnostic_disables_remote_execution_explicitly():
    config = (ROOT / "manifests/buildstream-remote-cache-config.yaml").read_text(encoding="utf-8")
    assert "remote-execution: {}" not in config


def test_dakota_persists_sources_in_buildbarn():
    config_map = yaml.safe_load(
        (ROOT / "manifests/buildstream-remote-cache-config.yaml").read_text(
            encoding="utf-8"
        )
    )
    config = yaml.safe_load(config_map["data"]["dakota-buildstream.conf"])
    source_servers = config["source-caches"]["servers"]
    pipeline = (ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml").read_text(
        encoding="utf-8"
    )

    assert config["source-caches"]["override-project-caches"] is True
    assert source_servers[:1] == [
        {"url": "https://gbm.gnome.org:11003", "push": False},
    ]
    assert "type: index" in pipeline
    assert "type: storage" in pipeline
    assert "grpc://bb-remote-asset.buildbarn.svc.cluster.local:8984" in pipeline
    assert "grpc://frontend.buildbarn.svc.cluster.local:8980" in pipeline
    assert "override-project-caches: true" in pipeline
    source_cache_block = pipeline.split("source-caches:", 1)[1]
    assert "cache.projectbluefin.io" not in source_cache_block
    # The deployed bb-remote-asset endpoint cannot FetchBlob BuildStream
    # source URNs, so no source-cache server list may point at it. Inspect
    # the lines directly following each source-caches key rather than the
    # remainder of the file, which legitimately mentions bb-remote-asset for
    # artifact indexing.
    for block in pipeline.split("source-caches:")[1:]:
        head = block.splitlines()[:12]
        url_lines = [line for line in head if "url:" in line]
        assert url_lines, "source-caches block missing servers"
        assert not any("bb-remote-asset" in line for line in url_lines)


def test_dakota_patch_sync_fetches_junction_commit_ids():
    pipeline = (ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml").read_text(
        encoding="utf-8"
    )

    assert 'GNOME_COMMIT="${GNOME_REF##*-g}"' in pipeline
    assert 'FDS_COMMIT="${FDS_REF##*-g}"' in pipeline
    assert 'git fetch --depth=1 origin "${GNOME_COMMIT}"' in pipeline
    assert 'git fetch --depth=1 origin "${FDS_COMMIT}"' in pipeline
    assert 'git fetch --depth=1 origin "${GNOME_REF}"' not in pipeline
    assert 'git fetch --depth=1 origin "${FDS_REF}"' not in pipeline


def test_dakota_build_pipeline_includes_non_blocking_nvidia_variant():
    dakota = (ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml").read_text(
        encoding="utf-8"
    )

    assert "name: build-bluefin" in dakota
    assert "oci/bluefin.bst" in dakota
    assert "name: build-bluefin-nvidia" in dakota
    assert "oci/bluefin-nvidia.bst" in dakota
    assert "tag\n                  value: \"dakota-nvidia\"" in dakota
    default_task = dakota.split("- name: build-bluefin", 1)[1].split(
        "- name: build-bluefin-nvidia", 1
    )[0]
    assert "template: run-bst-step-nonblocking" in dakota
    nonblocking = dakota.split("- name: run-bst-step-nonblocking", 1)[1].split(
        "- name: bst-build-re", 1
    )[0]
    assert "continueOn:" in nonblocking
    assert "continueOn:" not in default_task


def test_dakota_build_pipeline_uses_generic_ephemeral_cache_volume():
    pipeline = yaml.safe_load(
        (ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml").read_text(
            encoding="utf-8"
        )
    )
    templates = {item["name"]: item for item in pipeline["spec"]["templates"]}
    volumes = {item["name"]: item for item in templates["bst-build-re"]["volumes"]}

    assert "bst-cache" in volumes
    cache_volume = volumes["bst-cache"]
    assert "hostPath" not in cache_volume
    assert "ephemeral" in cache_volume
    spec = cache_volume["ephemeral"]["volumeClaimTemplate"]["spec"]
    assert spec["accessModes"] == ["ReadWriteOnce"]
    assert spec["storageClassName"] == "local-path"
    assert spec["resources"]["requests"]["storage"] == "200Gi"


def test_dakota_build_pipeline_sets_gbm_recc_passthrough_and_patches_glib_stage1():
    pipeline = yaml.safe_load(
        (ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml").read_text(
            encoding="utf-8"
        )
    )
    templates = {item["name"]: item for item in pipeline["spec"]["templates"]}
    source = templates["bst-build-re"]["script"]["source"]

    assert "doc.setdefault('config', {}).setdefault('options', {})['recc'] = 'passthrough'" in source
    assert "0001-conditional-remote-apis-socket.patch" in source
    assert "0002-glib-stage1-serialize-jobs.patch" in source
    assert "-Dtests=false" in source
    assert "-Dinstalled_tests=false" in source
    assert "0003-gobject-introspection-ld-library-path.patch" in source
    assert 'env LD_LIBRARY_PATH=\\"$(pwd)/_builddir/girepository:${LD_LIBRARY_PATH:-}\\"' in source
    assert "0004-networkmanager-create-exports-patch.patch" in source
    assert "0001-create-exports-avoid-procfs.patch" in source
    assert "-Ddocs=false" in source
    assert "-Dman=false" in source
    assert "0002-libmbim-meson-build.patch" in source
    assert "env LD_LIBRARY_PATH=\"$(pwd)/_builddir/src/libmbim-glib:${LD_LIBRARY_PATH:-}\"" in source
    assert "0003-libqmi-meson-build.patch" in source
    assert "env LD_LIBRARY_PATH=\"$(pwd)/_builddir/src/libqmi-glib:${LD_LIBRARY_PATH:-}\"" in source
    assert "0004-appstream-meson-build.patch" in source
    assert "ln -sf libappstream.so.1.1.6 _builddir/src/libappstream.so.5" in source
    assert "split-rules:" in source
    assert "0005-colord-meson-build.patch" in source
    assert 'env LD_PRELOAD=\\"$(pwd)/_builddir/lib/colord/libcolordprivate.so.2.0.5\\"' in source
    assert "-type f ! -name 'series'" in source
