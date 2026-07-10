#!/usr/bin/env python3
import sys
import os
import json
import subprocess
import shutil
from datetime import datetime

def run_cmd(cmd, cwd=None, env=None, check=True):
    print(f"Running command: {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True)
    if check and result.returncode != 0:
        print(f"Command failed with exit code {result.returncode}")
        print(f"STDOUT:\n{result.stdout}")
        print(f"STDERR:\n{result.stderr}")
        sys.exit(result.returncode)
    return result

def main():
    if len(sys.argv) < 6:
        print("Usage: publish_test_results.py <results_json_path> <img_slug> <suite> <workflow_name> <github_token>")
        sys.exit(1)

    results_json_path = sys.argv[1]
    img_slug = sys.argv[2]
    suite = sys.argv[3]
    workflow_name = sys.argv[4]
    github_token = sys.argv[5]

    if not github_token:
        print("ERROR: github_token is empty. Skipping publication.")
        sys.exit(0)

    if not os.path.exists(results_json_path):
        print(f"WARNING: {results_json_path} not found. Skipping publication.")
        sys.exit(0)

    # 1. Parse behave results.json
    try:
        with open(results_json_path, 'r') as f:
            data = json.load(f)
    except Exception as e:
        print(f"ERROR: Failed to parse {results_json_path}: {e}")
        sys.exit(0)

    failed_scenarios = []
    scenarios_total = 0
    scenarios_failed = 0

    for feature in data:
        for element in feature.get('elements', []):
            if element.get('type') == 'scenario':
                scenarios_total += 1
                if element.get('status') == 'failed':
                    scenarios_failed += 1
                    failed_scenarios.append(element.get('name', 'Unnamed Scenario'))

    status = "passed" if scenarios_failed == 0 else "failed"
    current_utc = datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")

    # 2. Clone projectbluefin/lab to a temporary directory
    temp_dir = "/tmp/lab-repo-clone"
    if os.path.exists(temp_dir):
        shutil.rmtree(temp_dir)

    repo_url = f"https://x-access-token:{github_token}@github.com/projectbluefin/lab.git"
    run_cmd(["git", "clone", "--depth", "1", repo_url, temp_dir])

    # 3. Locate or create docs/results/<img_slug>-<suite>.json
    results_dir = os.path.join(temp_dir, "docs", "results")
    os.makedirs(results_dir, exist_ok=True)
    result_filename = f"{img_slug}-{suite}.json"
    result_filepath = os.path.join(results_dir, result_filename)

    history = []
    if os.path.exists(result_filepath):
        try:
            with open(result_filepath, 'r') as f:
                existing_data = json.load(f)
                history = existing_data.get("history", [])
        except Exception as e:
            print(f"WARNING: Failed to parse existing results file {result_filepath}: {e}")

    # Add the current run to history
    new_history_entry = {
        "run_date": current_utc,
        "workflow_name": workflow_name,
        "status": status,
        "scenarios": scenarios_total,
        "failed": scenarios_failed
    }
    history.insert(0, new_history_entry)
    # Keep history capped to last 15 runs
    history = history[:15]

    # Construct updated structure
    screenshot_url = f"https://projectbluefin.github.io/lab/screenshots/{img_slug}-{suite}-latest.png"
    updated_data = {
        "variant": f"{img_slug}",
        "suite": suite,
        "last_run": current_utc,
        "workflow_name": workflow_name,
        "status": status,
        "scenarios": scenarios_total,
        "failed": scenarios_failed,
        "failed_scenarios": failed_scenarios,
        "screenshot_url": screenshot_url,
        "history": history
    }

    with open(result_filepath, 'w') as f:
        json.dump(updated_data, f)

    # 4. Commit and push back to git
    # Set config
    run_cmd(["git", "config", "user.name", "github-actions[bot]"], cwd=temp_dir)
    run_cmd(["git", "config", "user.email", "github-actions[bot]@users.noreply.github.com"], cwd=temp_dir)

    # Git add and commit
    run_cmd(["git", "add", f"docs/results/{result_filename}"], cwd=temp_dir)

    # Check if there are changes before committing
    diff_check = run_cmd(["git", "diff", "--cached", "--quiet"], cwd=temp_dir, check=False)
    if diff_check.returncode == 0:
        print("No changes to test results. Skipping push.")
        # Clean up
        shutil.rmtree(temp_dir)
        sys.exit(0)

    commit_msg = f"chore: update test results for {img_slug}-{suite} ({workflow_name})\n\nCo-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
    run_cmd(["git", "commit", "-m", commit_msg], cwd=temp_dir)

    # Git push back to main
    run_cmd(["git", "push", "origin", "HEAD:main"], cwd=temp_dir)
    print(f"SUCCESS: Pushed updated test results for {img_slug}-{suite} back to repository!")

    # Clean up
    shutil.rmtree(temp_dir)

if __name__ == "__main__":
    main()
