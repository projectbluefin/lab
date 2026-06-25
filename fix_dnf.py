import os
import re

def fix_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    lines = content.split('\n')
    out_lines = []
    
    for line in lines:
        if "dnf install" in line or "microdnf install" in line:
            continue
        out_lines.append(line)

    if content != '\n'.join(out_lines):
        with open(filepath, 'w') as f:
            f.write('\n'.join(out_lines))
        print(f"Fixed {filepath}")

for root, dirs, files in os.walk('.'):
    for f in files:
        if f.endswith('.yaml'):
            fix_file(os.path.join(root, f))
