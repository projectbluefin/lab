import os

def patch_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    if 'cgr.dev/chainguard/wolfi-base:latest' not in content:
        return

    lines = content.split('\n')
    out_lines = []
    expecting_source = False
    for line in lines:
        if 'command: [bash, -c]' in line:
            out_lines.append(line.replace('command: [bash, -c]', 'command: [sh, -c]'))
        elif 'command: [bash]' in line:
            out_lines.append(line.replace('command: [bash]', 'command: [sh]'))
        else:
            out_lines.append(line)
            
    with open(filepath, 'w') as f:
        f.write('\n'.join(out_lines))
    print(f"Patched {filepath}")

for root, dirs, files in os.walk('.'):
    for f in files:
        if f.endswith('.yaml'):
            patch_file(os.path.join(root, f))
