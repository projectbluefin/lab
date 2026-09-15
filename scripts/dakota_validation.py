#!/usr/bin/env python3
"""On-demand SHA-pinned Dakota lab validation.

Provides a bounded request, deduplication, submission, lifecycle tracking,
and reporting contract for projectbluefin/dakota pull requests under
validation class dakota-bst-qa.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from collections.abc import Callable
from typing import Any

SUPPORTED_REPOSITORY = "projectbluefin/dakota"
SUPPORTED_VALIDATION_CLASS = "dakota-bst-qa"
STATUS_CONTEXT = "testing-lab/dakota-bst-qa"
ARGO_NAMESPACE = "argo"
DEFAULT_ARGO_URL_BASE = "http://192.168.1.102:2746/workflows/argo"

VALID_STATUSES = {
    "unavailable",
    "rejected",
    "queued",
    "running",
    "success",
    "failure",
    "cancelled",
    "timeout",
}
TERMINAL_STATUSES = {"success", "failure", "cancelled", "timeout", "rejected"}
ACTIVE_STATUSES = {"queued", "running"}

SHA_RE = re.compile(r"^[0-9a-fA-F]{40}$")


class ValidationError(ValueError):
    """Raised when request validation fails."""


def validate_request_parameters(
    repository: str,
    validation_class: str,
    pr_number: int | str,
    sha: str,
) -> tuple[bool, str]:
    """Validate request parameters before contacting external APIs."""
    if repository != SUPPORTED_REPOSITORY:
        return False, f"unsupported repository: {repository!r} (expected {SUPPORTED_REPOSITORY!r})"
    if validation_class != SUPPORTED_VALIDATION_CLASS:
        return (
            False,
            f"unsupported validation class: {validation_class!r} (expected {SUPPORTED_VALIDATION_CLASS!r})",
        )
    if not isinstance(sha, str) or not SHA_RE.match(sha):
        return False, f"malformed commit SHA: {sha!r} (expected 40-character hexadecimal SHA)"
    try:
        pr_num = int(pr_number)
        if pr_num <= 0:
            return False, f"invalid PR number: {pr_number} (must be positive integer)"
    except (ValueError, TypeError):
        return False, f"invalid PR number: {pr_number} (must be positive integer)"
    return True, ""


def check_live_pr_head(
    repository: str,
    pr_number: int,
    sha: str,
    token: str | None = None,
    api_base: str = "https://api.github.com",
    fetcher: Callable[[str, dict[str, str]], tuple[int, dict[str, Any]]] | None = None,
) -> tuple[bool, str, dict[str, Any]]:
    """Validate that the PR exists on GitHub, is open, and head SHA matches requested SHA."""
    pr_url = f"{api_base}/repos/{repository}/pulls/{pr_number}"
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "bluefin-lab-dakota-validation",
    }
    if token:
        headers["Authorization"] = f"Bearer {token}"

    if fetcher is not None:
        status_code, data = fetcher(pr_url, headers)
    else:
        req = urllib.request.Request(pr_url, headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                status_code = resp.status
                data = json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
            status_code = exc.code
            try:
                data = json.loads(exc.read().decode("utf-8"))
            except (json.JSONDecodeError, UnicodeDecodeError):
                data = {}
        except (urllib.error.URLError, TimeoutError, OSError) as exc:
            return False, f"network error fetching PR #{pr_number}: {exc}", {}

    if status_code == 404:
        return False, f"PR #{pr_number} not found in {repository}", data
    if status_code != 200:
        return (
            False,
            f"GitHub API error fetching PR #{pr_number}: HTTP {status_code} ({data.get('message', '')})",
            data,
        )

    state = data.get("state", "").lower()
    if state != "open":
        return False, f"closed PR: PR #{pr_number} is {state}", data

    live_sha = data.get("head", {}).get("sha", "")
    if live_sha.lower() != sha.lower():
        return (
            False,
            f"head mismatch: requested SHA {sha.lower()} does not match current PR head SHA {live_sha.lower()}",
            data,
        )

    return True, "", data


def parse_workflow_status(wf: dict[str, Any]) -> str:
    """Map an Argo workflow dictionary to one of the 8 canonical statuses."""
    status_obj = wf.get("status", {})
    phase = status_obj.get("phase", "")
    shutdown = wf.get("spec", {}).get("shutdown", "")

    if shutdown == "Stop":
        return "cancelled"

    if phase == "Succeeded":
        return "success"
    if phase in ("Failed", "Error"):
        message = status_obj.get("message", "").lower()
        for node in status_obj.get("nodes", {}).values():
            if "deadline exceeded" in node.get("message", "").lower():
                return "timeout"
        if "deadline exceeded" in message or "timeout" in message:
            return "timeout"
        if "stopped" in message or "shutdown" in message:
            return "cancelled"
        return "failure"
    if phase == "Running":
        return "running"
    if phase in ("Pending", ""):
        return "queued"
    return "running"


def get_existing_workflows(
    pr_number: int,
    sha: str,
    repository: str = SUPPORTED_REPOSITORY,
    runner: Callable[[list[str]], str] | None = None,
) -> list[dict[str, Any]]:
    """Query Argo workflows matching this repository, PR number, and exact SHA."""
    product = repository.split("/")[-1]
    sha12 = sha[:12].lower()

    if runner is not None:
        cmd = [
            "kubectl",
            "get",
            "workflows",
            "-n",
            ARGO_NAMESPACE,
            "-l",
            f"bluefin.io/repository={product},bluefin.io/pr-number={pr_number},bluefin.io/pr-sha={sha12}",
            "-o",
            "json",
        ]
        raw_output = runner(cmd)
    else:
        cmd = [
            "kubectl",
            "get",
            "workflows",
            "-n",
            ARGO_NAMESPACE,
            "-l",
            f"bluefin.io/repository={product},bluefin.io/pr-number={pr_number},bluefin.io/pr-sha={sha12}",
            "-o",
            "json",
        ]
        try:
            res = subprocess.run(cmd, capture_output=True, text=True, check=True, timeout=30)
            raw_output = res.stdout
        except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
            return []

    try:
        data = json.loads(raw_output)
    except json.JSONDecodeError:
        return []

    matching = []
    for item in data.get("items", []):
        # Strict validation: commit-sha parameter MUST match exact full SHA
        item_sha = ""
        params = (
            item.get("spec", {}).get("arguments", {}).get("parameters", [])
        )
        for param in params:
            if param.get("name") == "commit-sha":
                item_sha = param.get("value", "")
                break
        if not item_sha:
            item_sha = item.get("metadata", {}).get("labels", {}).get("bluefin.io/pr-sha", "")

        # Only match if commit-sha matches exact 40-character SHA
        # (or prefix if full sha not stored in legacy wfs)
        if item_sha.lower() == sha.lower():
            matching.append(item)
    return matching


def render_workflow_manifest(
    repository: str,
    pr_number: int,
    sha: str,
    validation_class: str = SUPPORTED_VALIDATION_CLASS,
) -> dict[str, Any]:
    """Render the Argo Workflow manifest for on-demand Dakota validation."""
    product = repository.split("/")[-1]
    sha12 = sha[:12].lower()
    return {
        "apiVersion": "argoproj.io/v1alpha1",
        "kind": "Workflow",
        "metadata": {
            "generateName": f"dakota-val-{pr_number}-{sha12}-",
            "namespace": ARGO_NAMESPACE,
            "labels": {
                "bluefin.io/trigger": "on-demand",
                "bluefin.io/validation-class": validation_class,
                "bluefin.io/repository": product,
                "bluefin.io/pr-number": str(pr_number),
                "bluefin.io/pr-sha": sha12,
                "bluefin.io/bst-workload": "true",
            },
        },
        "spec": {
            "serviceAccountName": "argo",
            "activeDeadlineSeconds": 90000,
            "workflowTemplateRef": {"name": validation_class},
            "arguments": {
                "parameters": [
                    {"name": "repository", "value": repository},
                    {"name": "pr-number", "value": str(pr_number)},
                    {"name": "commit-sha", "value": sha},
                    {"name": "validation-class", "value": validation_class},
                ]
            },
        },
    }


def publish_github_evidence(
    repository: str,
    sha: str,
    pr_number: int,
    status: str,
    token: str | None = None,
    workflow_name: str = "",
    workflow_url: str = "",
    message: str = "",
    api_base: str = "https://api.github.com",
    poster: Callable[[str, str, dict[str, Any]], tuple[int, str]] | None = None,
) -> tuple[bool, str]:
    """Publish commit status and optional repository dispatch evidence against the exact SHA."""
    if not token and poster is None:
        return False, "no GitHub token available to publish evidence"

    state_map = {
        "queued": "pending",
        "running": "pending",
        "success": "success",
        "failure": "failure",
        "rejected": "failure",
        "cancelled": "error",
        "timeout": "error",
    }
    gh_state = state_map.get(status, "error")

    if not message:
        message_map = {
            "queued": f"Dakota lab validation queued for PR #{pr_number}",
            "running": f"Dakota lab validation running in {workflow_name or 'Argo'}",
            "success": f"Dakota lab validation succeeded for PR #{pr_number}",
            "failure": f"Dakota lab validation failed for PR #{pr_number}",
            "rejected": f"Dakota lab validation rejected for PR #{pr_number}",
            "cancelled": f"Dakota lab validation cancelled for PR #{pr_number}",
            "timeout": f"Dakota lab validation timed out for PR #{pr_number}",
        }
        message = message_map.get(status, f"Dakota lab validation: {status}")

    # Clip description to 140 characters per GitHub Commit Status limit
    description = message[:140]

    status_payload = {
        "state": gh_state,
        "context": STATUS_CONTEXT,
        "description": description,
    }
    if workflow_url and workflow_url.startswith(("http://", "https://")):
        status_payload["target_url"] = workflow_url

    status_url = f"{api_base}/repos/{repository}/statuses/{sha}"

    if poster is not None:
        http_code, resp_body = poster("POST", status_url, status_payload)
    else:
        req = urllib.request.Request(
            status_url,
            data=json.dumps(status_payload).encode("utf-8"),
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {token}",
                "X-GitHub-Api-Version": "2022-11-28",
                "Content-Type": "application/json",
                "User-Agent": "bluefin-lab-dakota-validation",
            },
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                http_code = resp.status
                resp_body = resp.read().decode("utf-8")
        except urllib.error.HTTPError as exc:
            http_code = exc.code
            resp_body = exc.read().decode("utf-8")
        except (urllib.error.URLError, TimeoutError, OSError) as exc:
            return False, f"network error publishing status: {exc}"

    if http_code not in (200, 201):
        return False, f"GitHub commit status returned HTTP {http_code}: {resp_body}"

    # Also attempt dispatching MergeRaptor lab-check event if receiver is active
    dispatch_payload = {
        "event_type": "lab-check",
        "client_payload": {
            "repository": repository,
            "sha": sha,
            "check": {
                "pr_number": str(pr_number),
                "state": "in_progress" if status == "running" else ("completed" if status in TERMINAL_STATUSES else "queued"),
                "conclusion": "success" if status == "success" else ("failure" if status in ("failure", "rejected") else ("timed_out" if status == "timeout" else ("cancelled" if status == "cancelled" else ""))),
                "external_id": workflow_name,
                "details_url": workflow_url,
                "title": f"Dakota lab validation: {status}",
                "summary": description,
                "text": "",
                "started_at": "",
                "completed_at": "",
            },
        },
    }
    dispatch_url = f"{api_base}/repos/{repository}/dispatches"
    if poster is not None:
        poster("POST", dispatch_url, dispatch_payload)
    else:
        try:
            d_req = urllib.request.Request(
                dispatch_url,
                data=json.dumps(dispatch_payload).encode("utf-8"),
                headers={
                    "Accept": "application/vnd.github+json",
                    "Authorization": f"Bearer {token}",
                    "X-GitHub-Api-Version": "2022-11-28",
                    "Content-Type": "application/json",
                    "User-Agent": "bluefin-lab-dakota-validation",
                },
                method="POST",
            )
            with urllib.request.urlopen(d_req, timeout=30):
                pass
        except (urllib.error.URLError, TimeoutError, OSError):
            # Advisory dispatch: receiver may be disabled, status write succeeded
            pass

    return True, "published"


def submit_validation_request(
    repository: str,
    validation_class: str,
    pr_number: int | str,
    sha: str,
    rerun: bool = False,
    token: str | None = None,
    runner: Callable[[list[str]], str] | None = None,
    creator: Callable[[dict[str, Any]], str] | None = None,
    fetcher: Callable[[str, dict[str, str]], tuple[int, dict[str, Any]]] | None = None,
    poster: Callable[[str, str, dict[str, Any]], tuple[int, str]] | None = None,
    api_base: str = "https://api.github.com",
    publish: bool = True,
) -> dict[str, Any]:
    """Execute the full preflight, deduplication, admission, submission, and reporting contract."""
    # 1. Parameter syntax and scope check
    valid, err = validate_request_parameters(repository, validation_class, pr_number, sha)
    if not valid:
        result = {
            "status": "rejected",
            "repository": repository,
            "validation_class": validation_class,
            "pr_number": pr_number,
            "sha": sha,
            "message": f"Request rejected: {err}",
            "reason": err,
        }
        if publish and token:
            publish_github_evidence(
                repository, sha, int(pr_number) if str(pr_number).isdigit() else 0,
                "rejected", token, message=err, api_base=api_base, poster=poster
            )
        return result

    pr_num = int(pr_number)

    # 2. Live PR head validation before submission
    valid_head, head_err, _pr_data = check_live_pr_head(
        repository, pr_num, sha, token=token, api_base=api_base, fetcher=fetcher
    )
    if not valid_head:
        result = {
            "status": "rejected",
            "repository": repository,
            "validation_class": validation_class,
            "pr_number": pr_num,
            "sha": sha,
            "message": f"Request rejected: {head_err}",
            "reason": head_err,
        }
        if publish and token:
            publish_github_evidence(
                repository, sha, pr_num, "rejected", token, message=head_err,
                api_base=api_base, poster=poster
            )
        return result

    # 3. Deduplication and terminal rerun inspection
    existing = get_existing_workflows(pr_num, sha, repository=repository, runner=runner)

    active_wf = None
    terminal_wf = None
    for wf in existing:
        wf_status = parse_workflow_status(wf)
        if wf_status in ACTIVE_STATUSES:
            active_wf = (wf, wf_status)
            break
        if wf_status in TERMINAL_STATUSES:
            terminal_wf = (wf, wf_status)

    # Dedupe identical active requests
    if active_wf is not None:
        wf, wf_status = active_wf
        wf_name = wf.get("metadata", {}).get("name", "")
        wf_url = f"{DEFAULT_ARGO_URL_BASE}/{wf_name}"
        return {
            "status": wf_status,
            "repository": repository,
            "validation_class": validation_class,
            "pr_number": pr_num,
            "sha": sha,
            "workflow_name": wf_name,
            "workflow_url": wf_url,
            "deduped": True,
            "rerun": False,
            "message": f"Active validation workflow {wf_name} already running for PR #{pr_num} at {sha}",
        }

    # Allow explicit terminal rerun
    if terminal_wf is not None and not rerun:
        wf, wf_status = terminal_wf
        wf_name = wf.get("metadata", {}).get("name", "")
        wf_url = f"{DEFAULT_ARGO_URL_BASE}/{wf_name}"
        return {
            "status": wf_status,
            "repository": repository,
            "validation_class": validation_class,
            "pr_number": pr_num,
            "sha": sha,
            "workflow_name": wf_name,
            "workflow_url": wf_url,
            "deduped": True,
            "rerun": False,
            "message": f"Terminal validation workflow {wf_name} exists for PR #{pr_num} at {sha} ({wf_status}). Pass rerun=true to rerun.",
        }

    # 4. Submit new workflow for this exact SHA
    manifest = render_workflow_manifest(repository, pr_num, sha, validation_class)
    if creator is not None:
        wf_name = creator(manifest)
    else:
        cmd = ["kubectl", "create", "-f", "-", "-o", "jsonpath={.metadata.name}"]
        try:
            res = subprocess.run(
                cmd,
                input=json.dumps(manifest),
                capture_output=True,
                text=True,
                check=True,
                timeout=30,
            )
            wf_name = res.stdout.strip()
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired, OSError) as exc:
            return {
                "status": "failure",
                "repository": repository,
                "validation_class": validation_class,
                "pr_number": pr_num,
                "sha": sha,
                "message": f"Failed to submit workflow to Argo: {exc}",
            }

    wf_url = f"{DEFAULT_ARGO_URL_BASE}/{wf_name}"

    # 5. Publish queued evidence against exact SHA
    if publish and token:
        publish_github_evidence(
            repository, sha, pr_num, "queued", token,
            workflow_name=wf_name, workflow_url=wf_url,
            message=f"Dakota lab validation queued as {wf_name}",
            api_base=api_base, poster=poster,
        )

    return {
        "status": "queued",
        "repository": repository,
        "validation_class": validation_class,
        "pr_number": pr_num,
        "sha": sha,
        "workflow_name": wf_name,
        "workflow_url": wf_url,
        "deduped": False,
        "rerun": rerun,
        "message": f"Dispatched Dakota lab validation workflow {wf_name} for PR #{pr_num} at {sha}",
    }


def query_validation_status(
    repository: str,
    validation_class: str,
    pr_number: int | str,
    sha: str,
    runner: Callable[[list[str]], str] | None = None,
    token: str | None = None,
    api_base: str = "https://api.github.com",
    fetcher: Callable[[str, dict[str, str]], tuple[int, dict[str, Any]]] | None = None,
) -> dict[str, Any]:
    """Query validation status for a PR and SHA. Returns one of the 8 canonical statuses."""
    valid, err = validate_request_parameters(repository, validation_class, pr_number, sha)
    if not valid:
        return {
            "status": "rejected",
            "repository": repository,
            "validation_class": validation_class,
            "pr_number": pr_number,
            "sha": sha,
            "message": err,
        }

    pr_num = int(pr_number)

    # 1. Search cluster workflows for exact PR and exact SHA
    existing = get_existing_workflows(pr_num, sha, repository=repository, runner=runner)
    if existing:
        # Latest workflow for this SHA wins
        latest = existing[-1]
        st = parse_workflow_status(latest)
        wf_name = latest.get("metadata", {}).get("name", "")
        wf_url = f"{DEFAULT_ARGO_URL_BASE}/{wf_name}"
        return {
            "status": st,
            "repository": repository,
            "validation_class": validation_class,
            "pr_number": pr_num,
            "sha": sha,
            "workflow_name": wf_name,
            "workflow_url": wf_url,
            "message": f"Workflow {wf_name} is {st}",
        }

    # 2. If not found in cluster, check GitHub commit statuses for this exact SHA
    if token or fetcher is not None:
        status_url = f"{api_base}/repos/{repository}/commits/{sha}/statuses"
        headers = {
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "bluefin-lab-dakota-validation",
        }
        if token:
            headers["Authorization"] = f"Bearer {token}"
        if fetcher is not None:
            code, statuses = fetcher(status_url, headers)
        else:
            try:
                req = urllib.request.Request(status_url, headers=headers)
                with urllib.request.urlopen(req, timeout=30) as resp:
                    code = resp.status
                    statuses = json.loads(resp.read().decode("utf-8"))
            except (urllib.error.URLError, TimeoutError, json.JSONDecodeError, OSError):
                code, statuses = 500, []

        if code == 200 and isinstance(statuses, list):
            for item in statuses:
                if item.get("context") == STATUS_CONTEXT:
                    gh_state = item.get("state")
                    desc = item.get("description", "").lower()
                    target_url = item.get("target_url", "")
                    if gh_state == "pending":
                        st = "queued" if "queued" in desc else "running"
                    elif gh_state == "success":
                        st = "success"
                    elif gh_state == "failure":
                        st = "rejected" if "rejected" in desc else "failure"
                    elif gh_state == "error":
                        if "cancelled" in desc:
                            st = "cancelled"
                        elif "timed out" in desc or "timeout" in desc:
                            st = "timeout"
                        else:
                            st = "failure"
                    else:
                        st = "unavailable"
                    return {
                        "status": st,
                        "repository": repository,
                        "validation_class": validation_class,
                        "pr_number": pr_num,
                        "sha": sha,
                        "workflow_name": target_url.rsplit("/", 1)[-1] if target_url else "",
                        "workflow_url": target_url,
                        "message": item.get("description", ""),
                    }

    # Evidence from SHA A never applies to SHA B. If no evidence exists for this SHA:
    return {
        "status": "unavailable",
        "repository": repository,
        "validation_class": validation_class,
        "pr_number": pr_num,
        "sha": sha,
        "message": f"No validation evidence found for SHA {sha}",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="On-demand SHA-pinned Dakota lab validation tool."
    )
    subparsers = parser.add_subparsers(dest="subcommand", required=True)

    # Subcommand: request
    req_parser = subparsers.add_parser("request", help="Submit or dedupe a validation request.")
    req_parser.add_argument("--repo", default=SUPPORTED_REPOSITORY, help="Repository")
    req_parser.add_argument(
        "--validation-class", default=SUPPORTED_VALIDATION_CLASS, help="Validation class"
    )
    req_parser.add_argument("--pr-number", required=True, help="PR number")
    req_parser.add_argument("--sha", required=True, help="Exact 40-character commit SHA")
    req_parser.add_argument(
        "--rerun", action="store_true", default=False, help="Allow explicit rerun of terminal runs"
    )
    req_parser.add_argument(
        "--token",
        default=os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN"),
        help="GitHub token",
    )
    req_parser.add_argument(
        "--no-publish", dest="publish", action="store_false", default=True, help="Disable GitHub evidence publishing"
    )

    # Subcommand: status
    stat_parser = subparsers.add_parser("status", help="Query validation status for a PR and SHA.")
    stat_parser.add_argument("--repo", default=SUPPORTED_REPOSITORY, help="Repository")
    stat_parser.add_argument(
        "--validation-class", default=SUPPORTED_VALIDATION_CLASS, help="Validation class"
    )
    stat_parser.add_argument("--pr-number", required=True, help="PR number")
    stat_parser.add_argument("--sha", required=True, help="Exact 40-character commit SHA")
    stat_parser.add_argument(
        "--token",
        default=os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN"),
        help="GitHub token",
    )

    # Subcommand: publish
    pub_parser = subparsers.add_parser("publish", help="Publish validation evidence for a SHA.")
    pub_parser.add_argument("--repo", default=SUPPORTED_REPOSITORY, help="Repository")
    pub_parser.add_argument("--pr-number", required=True, help="PR number")
    pub_parser.add_argument("--sha", required=True, help="Commit SHA")
    pub_parser.add_argument(
        "--status", required=True, choices=sorted(VALID_STATUSES), help="Validation status"
    )
    pub_parser.add_argument("--workflow-name", default="", help="Argo workflow name")
    pub_parser.add_argument("--workflow-url", default="", help="Argo workflow URL")
    pub_parser.add_argument("--message", default="", help="Status description")
    pub_parser.add_argument(
        "--token",
        default=os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN"),
        help="GitHub token",
    )

    args = parser.parse_args(argv)

    if args.subcommand == "request":
        res = submit_validation_request(
            repository=args.repo,
            validation_class=args.validation_class,
            pr_number=args.pr_number,
            sha=args.sha,
            rerun=args.rerun,
            token=args.token,
            publish=args.publish,
        )
        print(json.dumps(res, indent=2))
        return 1 if res.get("status") == "rejected" else 0

    if args.subcommand == "status":
        res = query_validation_status(
            repository=args.repo,
            validation_class=args.validation_class,
            pr_number=args.pr_number,
            sha=args.sha,
            token=args.token,
        )
        print(json.dumps(res, indent=2))
        return 0

    if args.subcommand == "publish":
        ok, msg = publish_github_evidence(
            repository=args.repo,
            sha=args.sha,
            pr_number=int(args.pr_number),
            status=args.status,
            token=args.token,
            workflow_name=args.workflow_name,
            workflow_url=args.workflow_url,
            message=args.message,
        )
        print(json.dumps({"success": ok, "message": msg}, indent=2))
        return 0 if ok else 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
