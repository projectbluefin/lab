import tomllib
from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "manifests/registry-mirror-config.yaml"


def _config_map():
    return next(
        document
        for document in yaml.safe_load_all(MANIFEST.read_text(encoding="utf-8"))
        if document["kind"] == "ConfigMap"
    )


def test_registry_mirrors_prefer_zot_and_fall_back_upstream():
    # Zot first; the upstream `server` is only tried when the mirror is down, so a
    # Zot outage at boot cannot block the pulls needed to recover Zot.
    hosts = _config_map()["data"]

    for registry, namespace, upstream in (
        ("ghcr.io", "ghcr", "https://ghcr.io"),
        ("docker.io", "docker", "https://registry-1.docker.io"),
        ("quay.io", "quay", "https://quay.io"),
        ("registry.fedoraproject.org", "fedora", "https://registry.fedoraproject.org"),
        ("registry.k8s.io", "k8s", "https://registry.k8s.io"),
        ("cgr.dev", "cgr", "https://cgr.dev"),
        ("public.ecr.aws", "ecr", "https://public.ecr.aws"),
        ("lscr.io", "lscr", "https://lscr.io"),
    ):
        config = tomllib.loads(hosts[f"{registry}.hosts.toml"])
        assert config["server"] == upstream
        mirror = config["host"][f"http://192.168.1.102:30501/v2/{namespace}"]
        assert mirror["capabilities"] == ["pull", "resolve"]
        assert mirror["override_path"] is True
