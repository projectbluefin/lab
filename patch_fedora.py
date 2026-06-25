import os

def patch_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    if 'quay.io/fedora/fedora:latest' not in content:
        return

    lines = content.split('\n')
    out_lines = []
    in_script_source = False
    expecting_source = False
    indent = ""
    for i, line in enumerate(lines):
        if 'quay.io/fedora/fedora:latest' in line:
            out_lines.append(line.replace('quay.io/fedora/fedora:latest', 'cgr.dev/chainguard/wolfi-base:latest'))
            expecting_source = True
        elif expecting_source and ('command: [bash]' in line or 'command: [sh]' in line):
            out_lines.append(line.replace('command: [bash]', 'command: [sh]'))
        elif expecting_source and 'source: |' in line:
            out_lines.append(line)
            in_script_source = True
            expecting_source = False
            indent = line.split('source:')[0] + "  "
            out_lines.append(indent + "apk add --no-cache bash curl jq kubectl git >/dev/null 2>&1")
            out_lines.append(indent + "exec bash <<'SCRIPT_EOF'")
        elif in_script_source:
            if line.strip() == "" or line.startswith(indent):
                if "dnf install" in line:
                    continue
                if "microdnf install" in line:
                    continue
                out_lines.append(line)
            else:
                # the block has ended
                out_lines.append(indent + "SCRIPT_EOF")
                out_lines.append(line)
                in_script_source = False
        else:
            out_lines.append(line)
            
    if in_script_source:
        out_lines.append(indent + "SCRIPT_EOF")

    with open(filepath, 'w') as f:
        f.write('\n'.join(out_lines))
    print(f"Patched {filepath}")

for root, dirs, files in os.walk('.'):
    for f in files:
        if f.endswith('.yaml'):
            patch_file(os.path.join(root, f))
