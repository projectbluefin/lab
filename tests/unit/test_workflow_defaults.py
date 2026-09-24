from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
BUILDBARN = "grpc://frontend.buildbarn.svc.cluster.local:8980"


def templates(name):
    doc = yaml.safe_load((ROOT / "argo/workflow-templates" / name).read_text(encoding="utf-8"))
    return {t["name"]: t for t in doc["spec"]["templates"]}


def test_buildstream_executes_and_caches_through_buildbarn():
    data = yaml.safe_load((ROOT / "manifests/buildstream-remote-cache-config.yaml").read_text())["data"]
    remote = yaml.safe_load(data["remote-execution.conf"])["remote-execution"]
    dakota = yaml.safe_load(data["dakota-buildstream.conf"])

    for service in ("execution-service", "storage-service", "action-cache-service"):
        assert remote[service]["url"] == BUILDBARN
    assert dakota["artifacts"]["servers"][0] == {"url": BUILDBARN, "push": True}
    assert dakota["source-caches"]["override-project-caches"] is True
    assert dakota["source-caches"]["servers"][0] == {"url": "https://gbm.gnome.org:11003", "push": False}


def test_bluefin_server_build_pipeline_builds_k0s_sysext():
    tasks = {t["name"]: t for t in templates("bluefin-server-build-pipeline.yaml")["build-core"]["dag"]["tasks"]}
    assert {"build-ddi", "build-installer", "build-sysext"} <= tasks.keys()
    args = {p["name"]: p["value"] for p in tasks["build-sysext"]["arguments"]["parameters"]}
    assert (args["element"], args["tag"]) == ("oci/k0s-sysext.bst", "k0s-sysext")


def test_dakota_build_uses_ephemeral_cache_and_keeps_upstream_recc_off():
    build = templates("dakota-build-pipeline.yaml")["bst-build-re"]
    cache = {v["name"]: v for v in build["volumes"]}["bst-cache"]
    spec = cache["ephemeral"]["volumeClaimTemplate"]["spec"]
    assert (spec["storageClassName"], spec["resources"]["requests"]["storage"]) == ("local-path", "200Gi")
    assert "['recc'] = 'passthrough'" in build["script"]["source"]
