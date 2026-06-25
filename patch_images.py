import os
import glob
import re

for f in glob.glob("argo/workflow-templates/*.yaml") + glob.glob("manifests/*.yaml"):
    with open(f, "r") as file:
        content = file.read()
    
    # Check if this file has the issue (kubectl:latest-dev but needing curl/jq)
    if "cgr.dev/chainguard/kubectl:latest-dev" in content and ("curl " in content or "jq " in content):
        # Only replace if it's the script image block
        content = re.sub(r'image:\s+cgr.dev/chainguard/kubectl:latest-dev', r'image: registry.fedoraproject.org/fedora-hummingbird:latest', content)
        # Re-add microdnf install to source blocks if missing
        if "microdnf" not in content and "dnf" not in content:
            content = re.sub(r'(source: \|\n\s+set -eu.*?\n)', r'\1          microdnf install -y curl jq >/dev/null 2>&1\n', content)
            
        with open(f, "w") as file:
            file.write(content)
        print(f"Patched {f}")
        
    elif "quay.io/skopeo" in content and "command: [bash]" in content:
        content = content.replace("command: [bash]", "command: [sh]")
        with open(f, "w") as file:
            file.write(content)
        print(f"Patched skopeo bash in {f}")
