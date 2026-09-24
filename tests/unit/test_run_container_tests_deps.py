from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[2]
CONTAINER_RUNNER = ROOT / "argo/workflow-templates/run-container-tests.yaml"


def _template(path, name):
    document = yaml.safe_load(path.read_text(encoding="utf-8"))
    return next(template for template in document["spec"]["templates"] if template["name"] == name)


def test_container_runner_preserves_nested_systemd_podman_requirement():
    runner = _template(CONTAINER_RUNNER, "run-container-tests")
    source = runner["script"]["source"]

    assert "command -v podman" in source
    assert "Podman is required for nested systemd QA" in source
    assert "--privileged" in source
    assert "--systemd=always" in source


def test_container_runner_uses_baked_runner_tools_without_dnf_bootstrap():
    runner = _template(CONTAINER_RUNNER, "run-container-tests")
    source = runner["script"]["source"]

    assert runner["script"]["image"] == "ghcr.io/projectbluefin/arc-runner:latest"
    assert "dnf install -y skopeo" not in source
    assert "dnf install -y git-core" not in source
    assert "--volume /opt/qa-wheels:/var/opt/qa-wheels:ro" in source
    assert "--cache-dir" in source
    assert "PIP_CACHE_DIR=/var/cache/bluefin-qa-pip" in source
    assert "--no-index --find-links /var/opt/qa-wheels" in source
    assert "--find-links /opt/qa-wheels" in source
    assert "falling back to the configured package index" in source
    container_text = CONTAINER_RUNNER.read_text(encoding="utf-8")
    assert "kubernetes.io/hostname: ghost" in container_text
    assert "path: /var/mnt/ghost-data/local-path" in container_text
    assert "type: Directory" in container_text


def test_target_python_dependencies_remain_target_installs_with_load_bearing_pin():
    source = _template(CONTAINER_RUNNER, "run-container-tests")["script"]["source"]

    # These packages exercise the shipped target image, so they must still be
    # installed inside that disposable image rather than imported from the
    # runner's Python environment. python-uinput provides the `uinput` C
    # extension qecore.utility.check_uinput_availability() needs for synthetic
    # input events; the PyPI distribution name is `python-uinput`.
    assert 'qa_dependencies=("setuptools<81" qecore dogtail behave python-uinput)' in source
    assert "--find-links" in source
    assert "--no-index" in source
    assert "falling back to the configured package index" in source
    assert "bluefin-qa-pip" in source
    assert "Sandbox._attach_version_status_to_report()" in source


def test_container_runner_wheel_mount_targets_ostree_var_opt_hierarchy():
    runner = _template(CONTAINER_RUNNER, "run-container-tests")
    source = runner["script"]["source"]

    # In ostree/bootc images (Bluefin), /opt is a symlink to var/opt or /var/opt,
    # but /var/opt does not exist in the rootfs before boot/mounts.
    # Mounting to destination /opt/qa-wheels causes crun openat2 to fail with ENOENT
    # on 'opt'. Mounting to /var/opt/qa-wheels allows crun to create /var/opt
    # under existing /var, which simultaneously satisfies the /opt symlink.
    assert "--volume /opt/qa-wheels:/var/opt/qa-wheels:ro" in source
    assert "--volume /opt/qa-wheels:/opt/qa-wheels:ro" not in source
    assert "/var/opt/qa-wheels" in source

