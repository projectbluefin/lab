import os
import stat
import subprocess
import tarfile
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture(scope="module")
def contributor():
    with (ROOT / "manifests/contribute.yaml").open() as manifest:
        resources = {doc["kind"]: doc for doc in yaml.safe_load_all(manifest)}
    return resources["StatefulSet"], resources["ConfigMap"]["data"]


def test_contributor_placement_and_session_identity(contributor):
    stateful_set, _ = contributor
    spec = stateful_set["spec"]
    pod = spec["template"]
    labels = {"app.kubernetes.io/name": "contribute"}
    app = yaml.safe_load((ROOT / "argocd/infra-application.yaml").read_text())
    assert any(rule.get("kind") == "StatefulSet"
               and rule.get("name") == stateful_set["metadata"]["name"]
               and rule.get("namespace") == stateful_set["metadata"]["namespace"]
               and "/spec/replicas" in rule.get("jsonPointers", [])
               for rule in app["spec"]["ignoreDifferences"])
    assert spec["replicas"] == 0
    assert spec["selector"]["matchLabels"] == labels
    assert pod["metadata"]["labels"].items() >= labels.items()
    affinity = pod["spec"]["affinity"]
    required_nodes = affinity["nodeAffinity"]["requiredDuringSchedulingIgnoredDuringExecution"]
    (term,) = required_nodes["nodeSelectorTerms"]
    (node_selector,) = term["matchExpressions"]
    assert node_selector["key"] == "kubernetes.io/hostname"
    assert node_selector["operator"] == "In"
    assert set(node_selector["values"]) == {"exo-0", "ghost"}
    (anti_affinity,) = affinity["podAntiAffinity"]["requiredDuringSchedulingIgnoredDuringExecution"]
    assert anti_affinity["topologyKey"] == "kubernetes.io/hostname"
    assert anti_affinity["labelSelector"]["matchLabels"] == labels
    relay = next(container for container in pod["spec"]["containers"] if container["name"] == "relay")
    env = {item["name"]: item for item in relay["env"]}
    assert env["HIVE_SESSION"]["valueFrom"]["fieldRef"]["fieldPath"] == "metadata.name"


def test_review_evidence_uses_a_private_subdirectory_of_a_per_pod_claim(contributor):
    stateful_set, _ = contributor
    spec = stateful_set["spec"]
    (claim,) = spec["volumeClaimTemplates"]
    assert claim["spec"]["accessModes"] == ["ReadWriteOnce"]
    assert claim["spec"]["storageClassName"] == "local-path"
    assert claim["spec"]["resources"]["requests"]["storage"] == "1Gi"
    (mount,) = (
        mount for container in spec["template"]["spec"]["containers"] if container["name"] == "relay"
        for mount in container["volumeMounts"]
        if mount["mountPath"] == "/var/lib/hive-review"
    )
    assert mount["name"] == claim["metadata"]["name"]
    assert mount["subPath"] == "evidence"


def test_model_containers_cannot_mount_relay_credentials_or_each_others_control_sockets(contributor):
    stateful_set, _ = contributor
    pod = stateful_set["spec"]["template"]["spec"]
    assert pod["automountServiceAccountToken"] is False
    assert pod["shareProcessNamespace"] is False
    mounts = {container["name"]: {mount["name"] for mount in container["volumeMounts"]}
              for container in pod["containers"]}
    assert "registration" in mounts["relay"]
    assert not mounts["contribute"] & {"registration", "reviewer-socket", "relay-tmp", "review-evidence"}
    assert not mounts["reviewer"] & {"registration", "worker-socket", "relay-tmp", "review-evidence", "handoff"}
    init = next(container for container in pod["initContainers"] if container["name"] == "seed-home")
    assert not any(mount["name"] == "registration" for mount in init["volumeMounts"])
    projected = next(volume["secret"]["items"] for volume in pod["volumes"] if volume["name"] == "secret")
    assert {item["key"] for item in projected} == {"config.yml", "ghost-context.tgz"}


@pytest.fixture
def seed_home(contributor, tmp_path):
    stateful_set, data = contributor
    pod = stateful_set["spec"]["template"]["spec"]
    (init,) = (container for container in pod["initContainers"] if container["name"] == "seed-home")
    (node_env,) = (
        item for item in init["env"]
        if item.get("valueFrom", {}).get("fieldRef", {}).get("fieldPath") == "spec.nodeName"
    )
    home = tmp_path / "home"
    home.mkdir()
    mounts = {mount["name"]: mount["mountPath"] for mount in init["volumeMounts"]}
    sources = {name: tmp_path / mount.lstrip("/") for name, mount in mounts.items() if name != "home"}
    for directory in sources.values():
        directory.mkdir(parents=True)
    (sources["omp"] / "models.yml").write_text(data["models.yml"])
    (sources["secret"] / "config.yml").write_text("advisor:\n  enabled: true\n")
    (sources["secret"] / "contributor.env").write_text("HIVE_REGISTRATION_TOKEN=never-seed-this")
    with tarfile.open(sources["secret"] / "ghost-context.tgz", "w:gz"):
        pass

    # Simulate the mounts without rewriting the actual seed logic.
    command = [arg.replace("/seed/", f"{tmp_path}/seed/") for arg in init["command"]]
    evidence = next(mount for container in pod["containers"] if container["name"] == "relay"
                    for mount in container["volumeMounts"] if mount["mountPath"] == "/var/lib/hive-review")

    def run(node):
        return subprocess.run(
            command,
            env={"HOME": str(home), "PATH": os.environ["PATH"], node_env["name"]: node},
            capture_output=True,
            text=True,
        )

    return run, home / ".omp/agent/models.yml", sources[evidence["name"]] / evidence["subPath"]


@pytest.mark.parametrize(("node", "endpoint"), [
    ("exo-0", "http://llm-d-exo-0.llm-d.svc.cluster.local:8000/v1"),
    ("ghost", "http://llm-d-ghost.llm-d.svc.cluster.local:8000/v1"),
])
def test_seed_writes_only_the_assigned_nodes_model(seed_home, node, endpoint):
    run, catalog, evidence = seed_home
    result = run(node)
    assert result.returncode == 0, result.stderr
    providers = yaml.safe_load(catalog.read_text())["providers"]
    assert set(providers) == {"lab-worker"}
    assert providers["lab-worker"]["baseUrl"] == endpoint
    assert [model["id"] for model in providers["lab-worker"]["models"]] == ["ornith"]
    assert evidence.is_dir()
    assert stat.S_IMODE(evidence.stat().st_mode) == 0o700
    assert not (catalog.parents[2] / ".config/hive/contributor.env").exists()


@pytest.mark.parametrize("node", ["other-node", ""])
def test_seed_rejects_an_unmapped_node_before_writing_a_catalog(seed_home, node):
    run, catalog, evidence = seed_home
    result = run(node)
    assert result.returncode != 0
    assert "Unsupported contributor node" in result.stderr
    assert not catalog.exists()
    assert not evidence.exists()


def test_last_worker_overlay_disables_the_per_turn_advisor(contributor):
    stateful_set, config_map = contributor
    container = stateful_set["spec"]["template"]["spec"]["containers"][0]
    env = {item["name"]: item.get("value") for item in container["env"]}
    overlay = env["PI_CONFIG_FILES"].split(":")[-1]
    (config_mount,) = (mount for mount in container["volumeMounts"] if mount["name"] == "omp")
    roles = yaml.safe_load(config_map[str(Path(overlay).relative_to(config_mount["mountPath"]))])
    assert roles["modelRoles"] == {"default": "lab-worker/ornith", "smol": "lab-worker/ornith"}
    assert roles["advisor"] == {"enabled": False}
    assert roles["autolearn"]["enabled"] is True
