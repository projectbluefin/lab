#!/usr/bin/env python3
import json
import subprocess
import datetime
from pathlib import Path

def run_cmd(cmd):
    try:
        out = subprocess.check_output(cmd, shell=True, text=True, stderr=subprocess.DEVNULL, timeout=15)
        return out.strip()
    except Exception:
        return None

def parse_cpu_cores(cpu_str):
    if not cpu_str:
        return 0.0
    s = str(cpu_str).strip()
    try:
        if s.endswith('n'):
            return float(s[:-1]) / 1_000_000_000.0
        if s.endswith('u'):
            return float(s[:-1]) / 1_000_000.0
        if s.endswith('m'):
            return float(s[:-1]) / 1000.0
        return float(s)
    except Exception:
        return 0.0

def parse_mem_mib(mem_str):
    if not mem_str:
        return 0.0
    s = str(mem_str).strip()
    try:
        if s.endswith('Ki'):
            return float(s[:-2]) / 1024.0
        if s.endswith('Mi'):
            return float(s[:-2])
        if s.endswith('Gi'):
            return float(s[:-2]) * 1024.0
        if s.endswith('Ti'):
            return float(s[:-2]) * 1024.0 * 1024.0
        return float(s) / (1024.0 * 1024.0) # Assume bytes if raw number
    except Exception:
        return 0.0

def get_app_for_pod(pod_name, pod_namespace, labels):
    # Check workflow pods specifically
    if "workflows.argoproj.io/workflow" in labels:
        return "testing-lab"
        
    if pod_namespace == "arc-runners":
        return "arc-runners"
    if pod_namespace == "arc-systems":
        return "arc-systems"
        
    if pod_namespace == "argo":
        if pod_name.startswith("argo-server") or pod_name.startswith("argo-workflows-workflow-controller"):
            return "argo-workflows"
        # Other pods in argo namespace (like homelab-access, k8sgpt, prometheus) belong to infra
        return "testing-lab-infra"
        
    if pod_namespace in ("flatcar-update", "system-upgrade"):
        return "flatcar-update"
        
    if pod_namespace in ("buildbarn", "local-registry", "cdi", "kubevirt"):
        return "testing-lab-infra"
        
    # Standard label fallbacks
    app_name = labels.get("argocd.argoproj.io/instance") or labels.get("app.kubernetes.io/instance")
    if app_name in ("arc-runners", "arc-systems", "argo-workflows", "flatcar-update", "testing-lab", "testing-lab-infra"):
        return app_name
        
    return None

def main():
    print("Collecting resource usage statistics per ArgoCD application...")
    now = datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')

    # Get pod specs (requests, limits, labels)
    raw_pods = run_cmd("kubectl get pods -A -o json")
    
    # Get pod metrics (actual CPU/RAM usage)
    raw_metrics = run_cmd("kubectl get --raw '/apis/metrics.k8s.io/v1beta1/pods'")

    app_resources = {}
    
    # Initialize entries for known applications
    known_apps = ["arc-runners", "arc-systems", "argo-workflows", "flatcar-update", "testing-lab", "testing-lab-infra"]
    for name in known_apps:
        app_resources[name] = {
            "name": name,
            "cpu_usage_cores": 0.0,
            "cpu_request_cores": 0.0,
            "cpu_limit_cores": 0.0,
            "mem_usage_mib": 0.0,
            "mem_request_mib": 0.0,
            "mem_limit_mib": 0.0,
            "pods_count": 0
        }

    live_data_ok = False

    if raw_pods:
        try:
            pods_data = json.loads(raw_pods)
            metrics_data = {}
            
            if raw_metrics:
                try:
                    m_json = json.loads(raw_metrics)
                    for item in m_json.get("items", []):
                        ns = item.get("metadata", {}).get("namespace")
                        name = item.get("metadata", {}).get("name")
                        containers = item.get("containers", [])
                        
                        cpu_sum = 0.0
                        mem_sum = 0.0
                        for c in containers:
                            usage = c.get("usage", {})
                            cpu_sum += parse_cpu_cores(usage.get("cpu"))
                            mem_sum += parse_mem_mib(usage.get("memory"))
                            
                        metrics_data[f"{ns}/{name}"] = {
                            "cpu": cpu_sum,
                            "mem": mem_sum
                        }
                    live_data_ok = True
                except Exception as me:
                    print(f"Warning parsing metrics JSON: {me}")

            # Parse pod specs
            for pod in pods_data.get("items", []):
                metadata = pod.get("metadata", {})
                spec = pod.get("spec", {})
                status = pod.get("status", {})
                
                # We only count running or active pods
                phase = status.get("phase")
                if phase not in ("Running", "Pending"):
                    continue
                
                pod_name = metadata.get("name")
                pod_namespace = metadata.get("namespace")
                labels = metadata.get("labels", {})
                
                # Match pod to an ArgoCD application using our robust routing rules
                app_name = get_app_for_pod(pod_name, pod_namespace, labels)
                if not app_name:
                    continue
                
                # Ensure entry exists
                if app_name not in app_resources:
                    app_resources[app_name] = {
                        "name": app_name,
                        "cpu_usage_cores": 0.0,
                        "cpu_request_cores": 0.0,
                        "cpu_limit_cores": 0.0,
                        "mem_usage_mib": 0.0,
                        "mem_request_mib": 0.0,
                        "mem_limit_mib": 0.0,
                        "pods_count": 0
                    }
                
                entry = app_resources[app_name]
                entry["pods_count"] += 1
                
                # Add actual usage from metrics
                pod_key = f"{pod_namespace}/{pod_name}"
                if pod_key in metrics_data:
                    entry["cpu_usage_cores"] += metrics_data[pod_key]["cpu"]
                    entry["mem_usage_mib"] += metrics_data[pod_key]["mem"]
                
                # Sum requests & limits from containers and initContainers
                for c in spec.get("containers", []) + spec.get("initContainers", []):
                    resources = c.get("resources", {})
                    requests = resources.get("requests", {})
                    limits = resources.get("limits", {})
                    
                    entry["cpu_request_cores"] += parse_cpu_cores(requests.get("cpu"))
                    entry["cpu_limit_cores"] += parse_cpu_cores(limits.get("cpu"))
                    
                    entry["mem_request_mib"] += parse_mem_mib(requests.get("memory"))
                    entry["mem_limit_mib"] += parse_mem_mib(limits.get("memory"))
                    
        except Exception as e:
            print(f"Error parsing pods JSON: {e}")

    if not live_data_ok:
        print("WARNING: Live resource metrics not collected, falling back to typical usage estimates.")
        # Provide realistic mock values for our Astro compilation when run offline
        mock_stats = {
            "arc-runners": {"cpu": 0.01, "mem": 45.0, "pods": 0}, # ARC scale sets scale down to 0
            "arc-systems": {"cpu": 0.15, "mem": 128.0, "pods": 2},
            "argo-workflows": {"cpu": 0.22, "mem": 350.0, "pods": 3},
            "flatcar-update": {"cpu": 0.0, "mem": 0.0, "pods": 0}, # manifest-only
            "testing-lab": {"cpu": 0.0, "mem": 0.0, "pods": 0}, # manifest-only
            "testing-lab-infra": {"cpu": 0.45, "mem": 2400.0, "pods": 4} # includes active buildbarn/Zot pods
        }
        for name, stats in mock_stats.items():
            entry = app_resources[name]
            # Only fallback if we don't have real pods counted from raw_pods
            if entry["pods_count"] == 0 and stats["pods"] > 0:
                entry["pods_count"] = stats["pods"]
                entry["cpu_request_cores"] = stats["cpu"] * 1.2
                entry["cpu_limit_cores"] = stats["cpu"] * 2.0
                entry["mem_request_mib"] = stats["mem"] * 1.1
                entry["mem_limit_mib"] = stats["mem"] * 1.5

            # Populate mock actual usages if metrics failed
            if entry["mem_usage_mib"] == 0.0 and stats["mem"] > 0.0:
                entry["cpu_usage_cores"] = stats["cpu"]
                entry["mem_usage_mib"] = stats["mem"]

    # Format values for readability
    apps_list = []
    for app_name, entry in app_resources.items():
        apps_list.append({
            "name": entry["name"],
            "pods_count": entry["pods_count"],
            "cpu": {
                "usage": round(entry["cpu_usage_cores"], 3),
                "request": round(entry["cpu_request_cores"], 3),
                "limit": round(entry["cpu_limit_cores"], 3)
            },
            "memory": {
                "usage": round(entry["mem_usage_mib"], 1),
                "request": round(entry["mem_request_mib"], 1),
                "limit": round(entry["mem_limit_mib"], 1)
            }
        })

    # Output to docs/data/app-resource-usage.json
    Path("docs/data").mkdir(parents=True, exist_ok=True)
    output = {
        "schema_version": "v1",
        "_meta": {
            "page": "applications",
            "description": "ArgoCD application pod resource (CPU and Memory) usage metrics.",
            "generated_at": now,
            "live_snapshot_ok": live_data_ok
        },
        "applications": apps_list
    }

    with open("docs/data/app-resource-usage.json", "w") as f:
        json.dump(output, f, indent=2)

    print(f"Successfully collected resource usage metrics for {len(apps_list)} applications.")

if __name__ == "__main__":
    main()
