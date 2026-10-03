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
    assert [host(s["url"]) for s in controller["storages"] if "url" in s] == [BUILDBARN_HOST]
    assert [host(c["url"]) for c in controller["caches"]] == [BUILDBARN_HOST]

    cas_front = yaml.load(manifest("buildgrid-cas.yaml")[0]["data"]["cas.yml"], Loader=_Tagged)
    assert [host(s["url"]) for s in cas_front["storages"] if "url" in s] == [BUILDBARN_HOST]

    worker = manifest("buildgrid-worker.yaml")[0]["spec"]["template"]["spec"]["containers"][0]
    env = {e["name"]: e.get("value") for e in worker["env"]}
    assert host(env["CAS_URL"]) == "cas.buildgrid.svc.cluster.local:50051"
    assert host(env["BUILDBARN_URL"]) == BUILDBARN_HOST
    assert host(env["BUILDGRID_URL"]) == BUILDGRID_HOST
    assert host(env["BOTS_URL"]) == "bots.buildgrid.svc.cluster.local:50051"


def test_scheduler_accepts_every_platform_key_buildstream_sends():
    # BuildGrid rejects actions carrying platform keys it does not know.
    # BuildStream's remote sandbox sends these; GNOME's recc adds chrootRootDigest.
    controller = yaml.load(manifest("buildgrid-controller.yaml")[0]["data"]["controller.yml"], Loader=_Tagged)
    props = controller["schedulers"][0]["property-set"]
    known = set(props["match-property-keys"]) | set(props["wildcard-property-keys"])
    assert {"OSFamily", "ISA", "unixUID", "unixGID", "network", "remoteApisSocketPath", "chrootRootDigest"} <= known



def test_controller_and_bots_share_one_scheduler():
    # Jobs queued through the controller are leased through the bots service;
    # if their scheduler configs drift, workers stop matching queued jobs.
    data = manifest("buildgrid-controller.yaml")[0]["data"]
    controller = yaml.load(data["controller.yml"], Loader=_Tagged)
    bots = yaml.load(data["bots.yml"], Loader=_Tagged)
    for key in ("storages", "caches"):
        assert controller[key] == bots[key]
    # Pool sizes differ per front end; everything else in the scheduler must not.
    strip = lambda schedulers: [{k: v for k, v in s.items() if k != "sql"} for s in schedulers]
    assert strip(controller["schedulers"]) == strip(bots["schedulers"])
    assert controller["connections"][0]["connection-string"] == bots["connections"][0]["connection-string"]


def test_element_builds_cannot_starve_their_own_compiles():
    # An element action holds a worker slot while its recc compiles queue as
    # separate actions. If BuildStream can keep as many element actions in
    # flight as one node has slots, a single-node grid deadlocks.
    data = manifest("buildstream-remote-cache-config.yaml")[0]["data"]
    builders = yaml.safe_load(data["dakota-buildstream.conf"])["scheduler"]["builders"]
    worker = manifest("buildgrid-worker.yaml")[0]["spec"]["template"]["spec"]["containers"][0]
    slots = int({e["name"]: e.get("value") for e in worker["env"]}["CONCURRENT_JOBS"])
    assert builders < slots
