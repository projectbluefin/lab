# Copyright 2014 CoreOS, Inc.
# Distributed under the terms of the GNU General Public License v2

EAPI=7
ETYPE="sources"

# Ported from coreos-sources-6.12.94.ebuild.
# All patches removed for 7.1.1 to avoid dry-run failure. Upstream 7.x contains most fixes.

inherit kernel-2
detect_version

EXTRAVERSION="${EXTRAVERSION/-coreos/-flatcar}"

DESCRIPTION="Full sources for the CoreOS Linux kernel"
HOMEPAGE="http://www.kernel.org"
SRC_URI="${KERNEL_URI}"

PATCH_DIR="${FILESDIR}/${KV_MAJOR}.${KV_MINOR}"

# make modules_prepare depends on pahole
RDEPEND=""

KEYWORDS="amd64 arm64"
IUSE=""

UNIPATCH_LIST=""
