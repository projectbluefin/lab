from pathlib import Path
import yaml

ROOT = Path(__file__).resolve().parents[2]
TEMPLATE_PATH = ROOT / "argo/workflow-templates/dakota-vm-boot-test.yaml"
SEMAPHORES_PATH = ROOT / "manifests/workflow-semaphores.yaml"


def test_dakota_vm_boot_test_template_structure():
    doc = yaml.safe_load(TEMPLATE_PATH.read_text(encoding="utf-8"))
    assert doc["kind"] == "WorkflowTemplate"
    assert doc["metadata"]["name"] == "dakota-vm-boot-test"
    assert doc["metadata"]["namespace"] == "argo"

    spec = doc["spec"]
    assert spec["entrypoint"] == "boot-test"

    params = {p["name"]: p.get("value") for p in spec.get("arguments", {}).get("parameters", [])}
    assert "image" in params
    assert params["image"] == "192.168.1.102:30500/dakota:testing"
    assert "commands" in params

    templates = {t["name"]: t for t in spec.get("templates", [])}
    assert "boot-test" in templates

    boot_test = templates["boot-test"]
    assert boot_test.get("activeDeadlineSeconds") == 1800

    # Semaphore check on the leaf template
    sync = boot_test.get("synchronization", {})
    semaphores = sync.get("semaphores", [])
    assert len(semaphores) == 1
    ref = semaphores[0].get("configMapKeyRef", {})
    assert ref.get("name") == "workflow-semaphores"
    assert ref.get("key") == "dakota-vm-boot"

    # Semaphore exists in manifests/workflow-semaphores.yaml
    sem_doc = yaml.safe_load(SEMAPHORES_PATH.read_text(encoding="utf-8"))
    assert "dakota-vm-boot" in sem_doc["data"]
    assert int(sem_doc["data"]["dakota-vm-boot"]) >= 1

    # SecurityContext and resources
    script = boot_test.get("script", {})
    sec_ctx = script.get("securityContext", {})
    assert sec_ctx.get("privileged") is True
    assert sec_ctx.get("runAsUser") == 0

    res = script.get("resources", {})
    req = res.get("requests", {})
    lim = res.get("limits", {})
    assert "cpu" in req and "cpu" in lim
    assert "memory" in req and "memory" in lim
    assert "ephemeral-storage" in req and "ephemeral-storage" in lim

    # Volumes and mounts
    mounts = {m["name"]: m["mountPath"] for m in script.get("volumeMounts", [])}
    assert mounts.get("dev") == "/dev"
    assert mounts.get("work") == "/work"

    vols = {v["name"]: v for v in boot_test.get("volumes", [])}
    assert "hostPath" in vols["dev"]
    assert "emptyDir" in vols["work"]

    # Script assertions
    source = script.get("source", "")
    assert "skopeo copy" in source
    assert "mkfs.ext4 -q -L root -d" in source
    assert "qemu-system-x86_64" in source
    assert "evidence.log" in source
    assert "serial-tail.log" in source
    assert "losetup" not in source
