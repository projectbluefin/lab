#!/usr/bin/env python3
import json
import subprocess
import datetime
import os
import re
from pathlib import Path

# Config
ALLOWED_REGISTRIES = ["ghcr.io", "quay.io", "registry.fedoraproject.org", "registry.k8s.io", "cgr.dev", "192.168.1.102", "localhost"]
IGNORED_IMAGES = ["docker.io/rocm/k8s-device-plugin"]

def run_cmd(cmd):
    try:
        out = subprocess.check_output(cmd, shell=True, text=True, stderr=subprocess.DEVNULL, timeout=15)
        return out.strip()
    except Exception:
        return None

def extract_images_from_yaml(content):
    # Regex to find image: references
    images = []
    lines = content.split('\n')
    for line in lines:
        if "image-lint-ignore" in line or "registry-lint-ignore" in line:
            continue
        m = re.search(r'image:\s*["\']?([^"\'\s]+)["\']?', line)
        if m:
            images.append(m.group(1))
    return images

def is_registry_allowed(image):
    if image in IGNORED_IMAGES:
        return True
    
    # Extract registry part
    if "/" not in image:
        # Implicit docker.io/library/
        return False
        
    parts = image.split("/")
    first = parts[0]
    
    if "." not in first and "localhost" not in first:
        # Implicit docker.io
        return False
        
    return first in ALLOWED_REGISTRIES

def main():
    print("Running GitOps and Cluster Policy Compliance scans...")
    now = datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')

    # Scandata
    git_manifests_scanned = 0
    git_image_checks = 0
    git_image_violations = []
    
    git_hostpath_checks = 0
    git_hostpath_violations = []
    
    git_nodepin_checks = 0
    git_nodepin_violations = []

    # 1. SCAN GIT MANIFESTS (OFFLINE SCAN)
    manifest_dirs = ["manifests", "argo/workflow-templates"]
    for m_dir in manifest_dirs:
        path = Path(m_dir)
        if not path.exists():
            continue
        for y_file in path.glob("**/*.yaml"):
            git_manifests_scanned += 1
            try:
                content = y_file.read_text()
                
                # Check Registry Images
                images = extract_images_from_yaml(content)
                for img in images:
                    git_image_checks += 1
                    if not is_registry_allowed(img):
                        git_image_violations.append({
                            "source": f"git:{y_file}",
                            "detail": f"Banned registry for image '{img}'"
                        })
                
                # Check hostPath
                hostpaths = re.findall(r'path:\s*["\']?([^"\'\s]+)["\']?', content)
                for hp in hostpaths:
                    # Filter for hostPath section in yaml context
                    if "hostPath" in content:
                        git_hostpath_checks += 1
                        # Approved hostPaths are: /var/tmp/knuckle-test, /var/mnt/ghost-data, local-path configs, or within scripts
                        if hp.startswith("/") and not hp.startswith(("/var/tmp/knuckle-test", "/var/mnt/ghost-data", "/var/log", "/run/containerd")):
                            # Check if it looks like a script comment or script line rather than hostPath
                            if "hostPath:" in content:
                                git_hostpath_violations.append({
                                    "source": f"git:{y_file}",
                                    "detail": f"Potentially unauthorized hostPath allocation on root disk: '{hp}'"
                                })
                                
                # Check Node Selector Pinning
                node_selectors = re.findall(r'nodeSelector:\s*([^\n]+)', content)
                for ns in node_selectors:
                    git_nodepin_checks += 1
                    # DaemonSets are allowed; check if file is named config or tuning
                    if "DaemonSet" not in content and "kubernetes.io/hostname" in ns:
                        git_nodepin_violations.append({
                            "source": f"git:{y_file}",
                            "detail": f"Hard node hostname selector pin found: '{ns}'"
                        })
                        
            except Exception as e:
                print(f"Error scanning yaml file {y_file}: {e}")

    # 2. SCAN LIVE CLUSTER PODS
    live_pods_scanned = 0
    live_image_checks = 0
    live_image_violations = []
    
    live_nodepin_checks = 0
    live_nodepin_violations = []

    raw_pods = run_cmd("kubectl get pods -A -o json")
    live_data_ok = False
    
    if raw_pods:
        try:
            pods_data = json.loads(raw_pods)
            for pod in pods_data.get("items", []):
                metadata = pod.get("metadata", {})
                spec = pod.get("spec", {})
                pod_name = metadata.get("name")
                pod_namespace = metadata.get("namespace")
                
                # Skip system pods if desired, but here we scan all for completeness
                live_pods_scanned += 1
                
                # Check container images
                containers = spec.get("containers", []) + spec.get("initContainers", [])
                for c in containers:
                    img = c.get("image", "")
                    if img:
                        live_image_checks += 1
                        if not is_registry_allowed(img):
                            live_image_violations.append({
                                "source": f"cluster:{pod_namespace}/{pod_name}",
                                "detail": f"Running image violates registry allowlist: '{img}'"
                            })
                            
                # Check node selector and hard pinning
                node_selector = spec.get("nodeSelector", {})
                if node_selector and "kubernetes.io/hostname" in node_selector:
                    live_nodepin_checks += 1
                    # Daemonsets or system-level pods (like local-path-provisioner, zot-cache) are approved
                    is_ds = False
                    owner_refs = metadata.get("ownerReferences", [])
                    for ref in owner_refs:
                        if ref.get("kind") in ("DaemonSet", "ReplicaSet") and pod_namespace in ("kube-system", "kubevirt", "argocd", "local-registry"):
                            is_ds = True
                    if not is_ds and pod_namespace not in ("kube-system", "kubevirt"):
                        live_nodepin_violations.append({
                            "source": f"cluster:{pod_namespace}/{pod_name}",
                            "detail": f"Pod hard-pinned to node: {node_selector}"
                        })
            live_data_ok = True
        except Exception as pe:
            print(f"Error parsing live pods JSON for policy check: {pe}")

    # Aggregating Scorecard Rules
    rules = [
        {
            "id": "registry_allowlist_git",
            "name": "Git Manifest Registry Allowlist",
            "description": "All container images in git-tracked manifests must reside on allowed registries.",
            "status": "failed" if git_image_violations else "passed",
            "total_checked": git_image_checks,
            "violations_count": len(git_image_violations),
            "violations": git_image_violations
        },
        {
            "id": "registry_allowlist_cluster",
            "name": "Live Cluster Registry Allowlist",
            "description": "All running container images in the cluster must reside on allowed registries.",
            "status": "failed" if live_image_violations else "passed",
            "total_checked": live_image_checks,
            "violations_count": len(live_image_violations),
            "violations": live_image_violations
        },
        {
            "id": "no_root_storage_git",
            "name": "No Root Disk Storage Allocations",
            "description": "No hard hostPath volume mounts on root disks (must use PVCs or approved storage).",
            "status": "failed" if git_hostpath_violations else "passed",
            "total_checked": git_hostpath_checks,
            "violations_count": len(git_hostpath_violations),
            "violations": git_hostpath_violations
        },
        {
            "id": "no_hard_node_pins_git",
            "name": "No Hard Node Selectors in Git",
            "description": "Workloads must not use hard nodeName or hostname nodeSelectors to bypass scheduler placement.",
            "status": "failed" if git_nodepin_violations else "passed",
            "total_checked": git_nodepin_checks,
            "violations_count": len(git_nodepin_violations),
            "violations": git_nodepin_violations
        },
        {
            "id": "no_hard_node_pins_cluster",
            "name": "No Hard Node Selectors in Cluster",
            "description": "No active pods with hard node selector pinning (excluding approved infrastructure controllers).",
            "status": "failed" if live_nodepin_violations else "passed",
            "total_checked": live_nodepin_checks,
            "violations_count": len(live_nodepin_violations),
            "violations": live_nodepin_violations
        }
    ]

    # Calculate overall compliance score
    total_checks = sum(r["total_checked"] for r in rules)
    total_violations = sum(r["violations_count"] for r in rules)
    compliance_score = 100.0
    if total_checks > 0:
        compliance_score = round(((total_checks - total_violations) / total_checks) * 100, 1)

    # Output to docs/data/policy-compliance.json
    Path("docs/data").mkdir(parents=True, exist_ok=True)
    output = {
        "schema_version": "v1",
        "_meta": {
            "page": "compliance",
            "description": "GitOps and running cluster compliance scorecard.",
            "generated_at": now,
            "live_snapshot_ok": live_data_ok,
            "git_manifests_scanned": git_manifests_scanned,
            "live_pods_scanned": live_pods_scanned
        },
        "score": compliance_score,
        "rules": rules
    }

    with open("docs/data/policy-compliance.json", "w") as f:
        json.dump(output, f, indent=2)

    print(f"Scan complete. Scanned {git_manifests_scanned} git manifests and {live_pods_scanned} live pods.")
    print(f"Compliance Score: {compliance_score}% ({total_violations} violations found out of {total_checks} checks).")

if __name__ == "__main__":
    main()
