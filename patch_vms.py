import os
import glob
import re

for f in glob.glob("argo/workflow-templates/provision*.yaml") + ["argo/workflow-templates/flatcar-kernel-build.yaml"]:
    with open(f, "r") as file:
        content = file.read()
    
    if "rng: {}" not in content and "devices:" in content:
        # Add rng: {} under devices:
        content = re.sub(r'(\s+devices:\n)', r'\1\g<1>rng: {}\n'.replace('\n\s+devices:', '\n'), content)
        content = content.replace("devices:\n        rng: {}", "devices:\n                    rng: {}") # naive fix for indentation, will do better
        
        # better approach: find "devices:" and add rng: {} with same indentation + 2 spaces
        lines = content.split('\n')
        for i, line in enumerate(lines):
            if line.endswith("devices:"):
                indent = len(line) - len(line.lstrip())
                lines.insert(i + 1, " " * (indent + 2) + "rng: {}")
                break
        
        with open(f, "w") as file:
            file.write('\n'.join(lines))
        print(f"Patched {f}")
