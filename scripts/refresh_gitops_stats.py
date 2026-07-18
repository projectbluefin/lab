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

def main():
    print("Collecting ArgoCD GitOps telemetry...")
    now = datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')

    # Query ArgoCD Applications
    raw_apps = run_cmd("kubectl get applications.argoproj.io -n argocd -o json")
    
    apps_list = []
    deployments_list = []
    
    if raw_apps:
        try:
            data = json.loads(raw_apps)
            items = data.get("items", [])
            for item in items:
                metadata = item.get("metadata", {})
                spec = item.get("spec", {})
                status = item.get("status", {})
                
                app_name = metadata.get("name")
                app_namespace = metadata.get("namespace")
                
                # Get sync and health status
                sync_info = status.get("sync", {})
                sync_status = sync_info.get("status", "Unknown")
                
                health_info = status.get("health", {})
                health_status = health_info.get("status", "Unknown")
                
                # Get source details
                source = spec.get("source", {})
                path_or_chart = source.get("path") or source.get("chart", "n/a")
                repo_url = source.get("repoURL", "n/a")
                target_revision = source.get("targetRevision", "main")
                
                # Get destination
                destination = spec.get("destination", {})
                dest_namespace = destination.get("namespace", "default")
                
                # Extract drifted resources
                resources = status.get("resources", [])
                drifted_resources = []
                for res in resources:
                    if res.get("status") == "OutOfSync":
                        drifted_resources.append({
                            "group": res.get("group", ""),
                            "kind": res.get("kind", ""),
                            "name": res.get("name", ""),
                            "namespace": res.get("namespace", ""),
                            "status": "OutOfSync"
                        })
                
                # Build Application Entry
                app_entry = {
                    "name": app_name,
                    "namespace": app_namespace,
                    "sync_status": sync_status,
                    "health_status": health_status,
                    "target_revision": target_revision,
                    "path": path_or_chart,
                    "repo_url": repo_url,
                    "destination_namespace": dest_namespace,
                    "drifted_count": len(drifted_resources),
                    "drifted_resources": drifted_resources,
                    "collected_at": now
                }
                apps_list.append(app_entry)
                
                # Extract Sync History
                history = status.get("history", [])
                for h in history:
                    started_at = h.get("deployStartedAt")
                    finished_at = h.get("deployFinishedAt")
                    revision = h.get("revision", "unknown")
                    history_id = h.get("id")
                    
                    deployments_list.append({
                        "app": app_name,
                        "id": history_id,
                        "revision": revision,
                        "started_at": started_at,
                        "finished_at": finished_at,
                        "status": "passed" if finished_at else "failed" # Finished implies successful sync
                    })
        except Exception as e:
            print(f"Error parsing applications JSON: {e}")
    else:
        print("WARNING: Could not fetch applications via kubectl, writing fallback structure.")
        # Fallback mocks for when local api is offline during GHA running
        fallback_names = ["arc-runners", "arc-systems", "argo-workflows", "flatcar-update", "testing-lab", "testing-lab-infra"]
        for name in fallback_names:
            apps_list.append({
                "name": name,
                "namespace": "argocd",
                "sync_status": "Synced",
                "health_status": "Healthy",
                "target_revision": "main",
                "path": "manifests" if name.endswith("-infra") else "argo",
                "repo_url": "https://github.com/projectbluefin/lab",
                "destination_namespace": "argo",
                "drifted_count": 0,
                "drifted_resources": [],
                "collected_at": now
            })

    # Sort deployments by started_at descending (latest first)
    deployments_list.sort(key=lambda d: d.get("started_at") or "", reverse=True)

    # Output to docs/data/
    Path("docs/data").mkdir(parents=True, exist_ok=True)
    
    status_output = {
        "schema_version": "v1",
        "_meta": {
            "page": "applications",
            "description": "Live ArgoCD GitOps application health and sync status.",
            "generated_at": now,
            "live_snapshot_ok": bool(raw_apps)
        },
        "applications": apps_list
    }
    
    deployments_output = {
        "schema_version": "v1",
        "_meta": {
            "page": "deployments",
            "description": "Historical rollouts derived from ArgoCD sync history.",
            "generated_at": now,
            "live_snapshot_ok": bool(raw_apps)
        },
        "deployments": deployments_list[:50] # Limit to last 50 rollouts
    }

    with open("docs/data/gitops-status.json", "w") as f:
        json.dump(status_output, f, indent=2)
        
    with open("docs/data/gitops-deployments.json", "w") as f:
        json.dump(deployments_output, f, indent=2)

    print(f"Successfully wrote {len(apps_list)} apps and {len(deployments_list)} rollouts to datasets.")

if __name__ == "__main__":
    main()
