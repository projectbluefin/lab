import os

def patch_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    lines = content.split('\n')
    out_lines = []
    
    for line in lines:
        if "export LD_PRELOAD=/tmp/fsetxattr_wrapper.so" in line:
            # We don't export it globally, we'll apply it just to bootc
            out_lines.append(line.replace('export LD_PRELOAD=/tmp/fsetxattr_wrapper.so', 'LD_PRELOAD_VAR=/tmp/fsetxattr_wrapper.so'))
        elif "bootc install to-disk \\" in line:
            out_lines.append(line.replace('bootc install to-disk \\', 'LD_PRELOAD=${LD_PRELOAD_VAR:-} bootc install to-disk \\'))
        else:
            out_lines.append(line)
            
    if content != '\n'.join(out_lines):
        with open(filepath, 'w') as f:
            f.write('\n'.join(out_lines))
        print(f"Patched {filepath}")

for root, dirs, files in os.walk('.'):
    for f in files:
        if f.endswith('.yaml'):
            patch_file(os.path.join(root, f))
