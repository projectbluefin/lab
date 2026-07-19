from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def test_image_poller_templates_do_not_self_reference_containerdisk_tag_defaults():
    bluefin_pipeline = (ROOT / "argo/workflow-templates/bluefin-qa-pipeline.yaml").read_text(
        encoding="utf-8"
    )
    image_poller = (ROOT / "argo/workflow-templates/image-poller.yaml").read_text(
        encoding="utf-8"
    )

    assert (
        '- name: containerdisk-tag\n      value: "{{workflow.parameters.image-tag}}"'
        not in bluefin_pipeline
    )
    assert (
        '- name: containerdisk-tag\n        value: "{{workflow.parameters.image-tag}}"'
        not in image_poller
    )


def test_image_poller_cron_manifests_do_not_pass_containerdisk_tag():
    offenders = []

    for manifest in sorted((ROOT / "manifests").glob("image-poll-*.yaml")):
        content = manifest.read_text(encoding="utf-8")
        if "workflowTemplateRef:\n      name: image-poller" not in content:
            continue
        if "containerdisk-tag" in content:
            offenders.append(manifest.name)

    assert not offenders, f"obsolete containerdisk-tag in: {', '.join(offenders)}"


def test_dakota_requires_distributed_capacity_matched_execution():
    config = (ROOT / "manifests/buildstream-remote-cache-config.yaml").read_text(
        encoding="utf-8"
    )
    pipeline = (ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml").read_text(
        encoding="utf-8"
    )

    assert "fetchers: 4" in config
    assert "builders: 2" in config
    assert "pushers: 2" in config
    assert "max-jobs: 8" in config
    assert "nodeSelector:\n        kubernetes.io/hostname: ghost" not in pipeline
    assert "build-bluefin.Succeeded" in pipeline
    assert "Verified BuildStream remote execution configuration" in pipeline


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


def test_dakota_nvidia_build_waits_for_its_bluefin_parent_artifact():
    pipeline = (ROOT / "argo/workflow-templates/dakota-build-pipeline.yaml").read_text(
        encoding="utf-8"
    )

    nvidia_task = pipeline.split("          - name: build-bluefin-nvidia", 1)[1]
    assert "depends: build-bluefin.Succeeded" in nvidia_task
