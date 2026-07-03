use std assert
use tap.nu

tap begin "verify bootc status output formats"

assert equal (bootc switch blah:// e>| find "\u{1B}") []

# Verify soft-reboot is in help
bootc upgrade --help | grep -qF -e '--soft-reboot'

tap ok
