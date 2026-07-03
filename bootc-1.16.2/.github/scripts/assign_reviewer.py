#!/usr/bin/env python3
"""
Auto-reviewer assignment script for GitHub PRs.
Rotates through a list of reviewers based on a weekly cycle.
Automatically reads reviewers from MAINTAINERS.md
"""

import os
import sys
import yaml
import subprocess
import re
from datetime import datetime, timezone, timedelta
from typing import List, Optional, TypedDict


class RotationInfo(TypedDict):
    start_date: Optional[datetime]
    rotation_cycle_weeks: int
    weeks_since_start: float
    before_start: bool
    error: Optional[str]


class SprintInfo(TypedDict):
    sprint_number: int
    week_in_sprint: int
    total_weeks: int
    before_start: Optional[bool]


def load_config(config_path: str) -> dict:
    """Load the reviewer configuration from YAML file."""
    try:
        with open(config_path, 'r') as f:
            return yaml.safe_load(f)
    except FileNotFoundError:
        print(f"Error: Configuration file {config_path} not found", file=sys.stderr)
        sys.exit(1)
    except yaml.YAMLError as e:
        print(f"Error parsing YAML configuration: {e}", file=sys.stderr)
        sys.exit(1)


def extract_reviewers_from_maintainers() -> List[str]:
    """Extract GitHub usernames from MAINTAINERS.md file by parsing the table structure."""
    maintainers_path = os.environ.get("MAINTAINERS_PATH", 'MAINTAINERS.md')
    reviewers = []
    
    try:
        with open(maintainers_path, 'r') as f:
            content = f.read()
        
        lines = content.splitlines()
        github_id_column_index = None
        in_table = False
        
        for line in lines:
            line = line.strip()
            
            # Skip empty lines
            if not line:
                continue
            
            # Look for table header to find GitHub ID column
            if line.startswith('|') and 'GitHub ID' in line:
                in_table = True
                columns = [col.strip() for col in line.split('|')[1:-1]]  # Remove empty first/last elements
                try:
                    github_id_column_index = columns.index('GitHub ID')
                except ValueError:
                    print("Error: Could not find 'GitHub ID' column in MAINTAINERS.md table")
                    sys.exit(1)
                continue
            
            # Skip separator line (|---|---|...)
            if in_table and line.startswith('|') and '---' in line:
                continue
            
            # Process table data rows
            if line.startswith('|') and github_id_column_index is not None:
                columns = [col.strip() for col in line.split('|')[1:-1]]
                if len(columns) > github_id_column_index:
                    github_id_cell = columns[github_id_column_index]
                    match = re.search(r'\[([a-zA-Z0-9-]+)\]\(https://github\.com/[^)]+\)', github_id_cell)
                    if match:
                        username = match.group(1)
                        if username and username not in reviewers:
                            reviewers.append(username)
            
            # Stop parsing when we hit the end of the table
            if in_table and not line.startswith('|'):
                break

        if reviewers:
            print(f"Found {len(reviewers)} reviewers from MAINTAINERS.md: {', '.join(reviewers)}")
        else:
            print("Warning: No GitHub usernames found in MAINTAINERS.md")
            
    except FileNotFoundError:
        print(f"Error: MAINTAINERS.md file not found")
        sys.exit(1)
    except IOError as e:
        print(f"Error reading MAINTAINERS.md: {e}")
        sys.exit(1)
    
    return reviewers


def calculate_rotation_info(config: dict) -> RotationInfo:
    """Calculate rotation information from config (helper function)."""
    start_date_str = config.get('start_date')
    rotation_cycle_weeks = config.get('rotation_cycle_weeks', 3)
    
    if not start_date_str:
        return {
            "start_date": None,
            "rotation_cycle_weeks": rotation_cycle_weeks,
            "weeks_since_start": 0,
            "before_start": False,
            "error": "No start_date configured"
        }
    
    try:
        start_date = datetime.fromisoformat(start_date_str).replace(tzinfo=timezone.utc)
        weeks_since_start = (datetime.now(timezone.utc) - start_date) / timedelta(weeks=1)

        return {
            "start_date": start_date,
            "rotation_cycle_weeks": rotation_cycle_weeks,
            "weeks_since_start": weeks_since_start,
            "before_start": weeks_since_start < 0,
            "error": None
        }
    except ValueError:
        return {
            "start_date": None,
            "rotation_cycle_weeks": rotation_cycle_weeks,
            "weeks_since_start": 0,
            "before_start": False,
            "error": f"Invalid start_date format: {start_date_str}"
        }


def get_current_sprint_info(rotation_info: RotationInfo) -> SprintInfo:
    """Calculate current sprint information."""
    
    if rotation_info["before_start"]:
        return {
            "sprint_number": 1,
            "week_in_sprint": 1,
            "total_weeks": 0
        }
    
    weeks_since_start = rotation_info["weeks_since_start"]
    rotation_cycle_weeks = rotation_info["rotation_cycle_weeks"]
    
    sprint_number = int(weeks_since_start // rotation_cycle_weeks) + 1
    week_in_sprint = int(weeks_since_start % rotation_cycle_weeks) + 1
    
    return {
        "sprint_number": sprint_number,
        "week_in_sprint": week_in_sprint,
        "total_weeks": int(weeks_since_start)
    }


def get_pr_author(pr_number: str) -> str:
    """Get the author of the PR."""
    repo = os.environ.get('GITHUB_REPOSITORY', 'bootc-dev/bootc')
    result = run_gh_command(
        ['api', f'repos/{repo}/pulls/{pr_number}', '--jq', '.user.login'],
        f"Could not fetch PR author for PR {pr_number}"
    )
    return result.stdout.strip()


def calculate_current_reviewer(reviewers: List[str], rotation_info: RotationInfo, exclude_user: Optional[str] = None) -> Optional[str]:
    """Calculate the current reviewer based on the rotation schedule, excluding specified user."""
    if not reviewers:
        print("Error: No reviewers found")
        return None
    
    if rotation_info["before_start"]:
        print(f"Warning: Current date is before start date. Using first reviewer.")
        # Find first reviewer that's not the excluded user
        for reviewer in reviewers:
            if reviewer != exclude_user:
                return reviewer
        return reviewers[0] if reviewers else None
    
    # Calculate total weeks since start and map to reviewer
    # Each reviewer gets rotation_cycle_weeks weeks, then we cycle to the next
    total_weeks = int(rotation_info["weeks_since_start"])
    rotation_cycle_weeks = rotation_info["rotation_cycle_weeks"]
    reviewer_index = (total_weeks // rotation_cycle_weeks) % len(reviewers)
    
    # If the calculated reviewer is the excluded user, find the next one
    if exclude_user and reviewers[reviewer_index] == exclude_user:
        # Try next reviewer in the list
        next_reviewer_index = (reviewer_index + 1) % len(reviewers)
        attempts = 0
        while reviewers[next_reviewer_index] == exclude_user and attempts < len(reviewers):
            next_reviewer_index = (next_reviewer_index + 1) % len(reviewers)
            attempts += 1
        
        # If all reviewers are excluded, return None
        if reviewers[next_reviewer_index] == exclude_user:
            print(f"Warning: All reviewers are excluded ({exclude_user}), skipping assignment")
            return None
        
        return reviewers[next_reviewer_index]
    
    return reviewers[reviewer_index]


def run_gh_command(args: List[str], error_message: str) -> subprocess.CompletedProcess:
    """Run a GitHub CLI command with consistent error handling."""
    try:
        return subprocess.run(
            ['gh'] + args,
            capture_output=True, text=True, check=True
        )
    except FileNotFoundError:
        print("Error: 'gh' command not found. Is the GitHub CLI installed and in the PATH?")
        sys.exit(1)
    except subprocess.CalledProcessError as e:
        print(f"{error_message}: {e.stderr}", file=sys.stderr)
        sys.exit(1)


def get_existing_reviewers(pr_number: str) -> List[str]:
    """Get list of reviewers already assigned to the PR."""
    repo = os.environ.get('GITHUB_REPOSITORY', 'bootc-dev/bootc')
    result = run_gh_command(
        ['api', f'repos/{repo}/pulls/{pr_number}', '--jq', '.requested_reviewers[].login'],
        f"Could not fetch existing reviewers for PR {pr_number}"
    )
    return result.stdout.strip().split('\n') if result.stdout.strip() else []


def assign_reviewer(pr_number: str, reviewer: str) -> None:
    """Assign a reviewer to the PR using GitHub CLI."""
    print(f"Attempting to assign reviewer {reviewer} to PR {pr_number}")
    
    run_gh_command(
        ['pr', 'edit', pr_number, '--add-reviewer', reviewer],
        f"Error assigning reviewer {reviewer} to PR {pr_number}"
    )
    print(f"Successfully assigned reviewer {reviewer} to PR {pr_number}")


def main():
    """Main function to handle reviewer assignment."""
    # Get PR number from environment variable
    pr_number = os.environ.get('PR_NUMBER')
    if not pr_number:
        print("Error: PR_NUMBER environment variable not set")
        sys.exit(1)
    
    # Load configuration (for start_date and rotation_cycle_weeks)
    config_path = os.environ.get('AUTO_REVIEW_CONFIG_PATH', '.github/auto-review-config.yml')
    config = load_config(config_path)
    
    # Extract reviewers from MAINTAINERS.md
    reviewers = extract_reviewers_from_maintainers()
    if not reviewers:
        print("Error: No reviewers found in MAINTAINERS.md")
        sys.exit(1)
    
    # Calculate rotation information once
    rotation_info = calculate_rotation_info(config)
    if rotation_info['error']:
        print(f"Error in configuration: {rotation_info['error']}", file=sys.stderr)
        sys.exit(1)
    
    # Get sprint information
    sprint_info = get_current_sprint_info(rotation_info)
    print(f"Current sprint: {sprint_info['sprint_number']}, week: {sprint_info['week_in_sprint']}")
    
    # Get PR author to exclude them from being assigned
    pr_author = get_pr_author(pr_number)
    print(f"PR author: {pr_author}")
    
    # Calculate current reviewer, excluding the PR author
    current_reviewer = calculate_current_reviewer(reviewers, rotation_info, exclude_user=pr_author)
    if not current_reviewer:
        print("Error: Could not calculate current reviewer")
        sys.exit(1)
    
    print(f"Assigned reviewer for this week: {current_reviewer}")
    
    # Get existing reviewers
    existing_reviewers = get_existing_reviewers(pr_number)
    
    # Check if current reviewer is already assigned
    if current_reviewer in existing_reviewers:
        print(f"Reviewer {current_reviewer} is already assigned to PR {pr_number}")
        return
    
    # Assign the current reviewer
    assign_reviewer(pr_number, current_reviewer)

if __name__ == "__main__":
    main()
