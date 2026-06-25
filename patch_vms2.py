import os
import glob

for f in glob.glob("argo/workflow-templates/provision*.yaml") + ["argo/workflow-templates/flatcar-kernel-build.yaml"]:
    with open(f, "r") as file:
        lines = file.read().split('\n')
    
    out_lines = []
    patched = False
    for line in lines:
        out_lines.append(line)
        if line.endswith("devices:") and not patched:
            indent = len(line) - len(line.lstrip())
            out_lines.append(" " * (indent + 2) + "rng: {}")
            patched = True
            
    if patched:
        with open(f, "w") as file:
            file.write('\n'.join(out_lines))
        print(f"Patched {f}")
