from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
BUILDBARN_HOST = "frontend.buildbarn.svc.cluster.local:8980"
BUILDGRID_HOST = "controller.buildgrid.svc.cluster.local:50051"


def host(url):
    return url.split("://", 1)[1]


def manifest(name):
    return list(yaml.safe_load_all((ROOT / "manifests" / name).read_text(encoding="utf-8")))


class _Tagged(yaml.SafeLoader):
    """Load BuildGrid's !tagged config into plain dicts."""


_Tagged.add_multi_constructor("!", lambda loader, _tag, node: loader.construct_mapping(node, deep=True))


def test_execution_and_storage_share_one_cas():
    # BuildStream uploads input roots to its storage-service and the BuildGrid
    # controller and workers read them from their own CAS. If these diverge,
    # every remote action fails with missing blobs.
    data = manifest("buildstream-remote-cache-config.yaml")[0]["data"]
    remote = yaml.safe_load(data["remote-execution.conf"])["remote-execution"]
    assert host(remote["execution-service"]["url"]) == BUILDGRID_HOST
    assert host(remote["storage-service"]["url"]) == BUILDBARN_HOST
    assert host(remote["action-cache-service"]["url"]) == BUILDBARN_HOST

    controller = yaml.load(manifest("buildgrid-controller.yaml")[0]["data"]["controller.yml"], Loader=_Tagged)
    assert [host(s["url"]) for s in controller["storages"]] == [BUILDBARN_HOST]
    assert [host(c["url"]) for c in controller["caches"]] == [BUILDBARN_HOST]

    cas_front = yaml.load(manifest("buildgrid-cas.yaml")[0]["data"]["cas.yml"], Loader=_Tagged)
    assert [host(s["url"]) for s in cas_front["storages"] if "url" in s] == [BUILDBARN_HOST]

    worker = manifest("buildgrid-worker.yaml")[0]["spec"]["template"]["spec"]["containers"][0]
    env = {e["name"]: e.get("value") for e in worker["env"]}
    assert host(env["CAS_URL"]) == "cas.buildgrid.svc.cluster.local:50051"
    assert host(env["BUILDBARN_URL"]) == BUILDBARN_HOST
    assert host(env["BUILDGRID_URL"]) == BUILDGRID_HOST


def test_scheduler_accepts_every_platform_key_buildstream_sends():
    # BuildGrid rejects actions carrying platform keys it does not know.
    # BuildStream's remote sandbox sends these; GNOME's recc adds chrootRootDigest.
    controller = yaml.load(manifest("buildgrid-controller.yaml")[0]["data"]["controller.yml"], Loader=_Tagged)
    props = controller["schedulers"][0]["property-set"]
    known = set(props["match-property-keys"]) | set(props["wildcard-property-keys"])
    assert {"OSFamily", "ISA", "unixUID", "unixGID", "network", "remoteApisSocketPath", "chrootRootDigest"} <= known

