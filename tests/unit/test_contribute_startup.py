import json
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def test_missing_safe_tool_assets_do_not_rotate_registration(tmp_path):
    """An invalid startup bundle must not invalidate the existing contributor lease."""
    lines = (ROOT / "docs/skills/hive-contribute/contribute.just").read_text().splitlines()
    start = next(i for i, line in enumerate(lines) if line.startswith("contribute-on advisor="))
    body = []
    for line in lines[start + 1:]:
        if line and not line.startswith("    "):
            break
        body.append(line[4:] if line else "")
    script = "\n".join(body).replace("{{advisor}}", "local")

    home = tmp_path / "home"
    profile = home / ".omp/profiles/ghost/agent"
    for name in ("skills", "managed-skills"):
        (profile / name).mkdir(parents=True, exist_ok=True)
    (profile / "APPEND_SYSTEM.md").write_text("Existing owner instructions.\n")
    registration = tmp_path / "registration.env"
    original = "HIVE_HUB=wss://example.invalid/contribute\nHIVE_REGISTRATION_TOKEN=existing-token\n"
    registration.write_text(original)
    config = tmp_path / "config.json"
    config.write_text(json.dumps({"data": {"mcp.json": "{}", "tooling.md": "guidance\n"}}))

    binaries = tmp_path / "bin"
    binaries.mkdir()
    gh = binaries / "gh"
    gh.write_text("#!/bin/sh\nprintf '%s' fixture-token\n")
    kubectl = binaries / "kubectl"
    kubectl.write_text(
        f"#!/bin/sh\nprintf queried > '{tmp_path / 'config_queried'}'\n"
        f"cat '{config}'\n"
    )
    curl = binaries / "curl"
    curl.write_text(
        f"#!/bin/sh\nprintf rotated > '{tmp_path / 'registration_rotated'}'\n"
        "printf '%s' '{\"registration_token\":\"replacement-token\"}'\n"
    )
    for executable in (gh, kubectl, curl):
        executable.chmod(0o755)
    environment = {**os.environ, "HOME": str(home), "HIVE_CONTRIBUTE_REGISTRATION": str(registration), "PATH": f"{binaries}:{os.environ['PATH']}"}
    result = subprocess.run(["bash", "-c", script], env=environment, capture_output=True, text=True)
    assert result.returncode != 0
    assert (tmp_path / "config_queried").exists(), result.stderr
    assert not (tmp_path / "registration_rotated").exists()
    assert registration.read_text() == original
