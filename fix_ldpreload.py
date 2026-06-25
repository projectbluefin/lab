import os
import re

def fix_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    lines = content.split('\n')
    out_lines = []
    
    for line in lines:
        if "export LD_PRELOAD=/tmp/fsetxattr_wrapper.so" in line:
            # Modify to only preload during bootc command, not globally inside the script
            pass
        elif "bootc install to-disk" in line:
            # Find the index of this line in the original list
            idx = out_lines.index(line) if line in out_lines else len(out_lines)
            if idx == len(out_lines):
                # The script failed if LD_PRELOAD is exported globally because 'echo' and other bash builtins 
                # might try to load it. 
                # Instead of export, let's prepend it to the bootc invocation.
                pass
        
    # simpler approach: just don't export it globally
    
    with open(filepath, 'w') as f:
        f.write(content.replace('export LD_PRELOAD=/tmp/fsetxattr_wrapper.so', 'export LD_PRELOAD=/tmp/fsetxattr_wrapper.so\n                echo "[INNER] WARNING LD_PRELOAD exported"'))

# Actually we just want to remove the global export and apply it directly to bootc
