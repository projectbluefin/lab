# Copyright 2014 CoreOS, Inc.
# Distributed under the terms of the GNU General Public License v2

EAPI=7
ETYPE="sources"

# Ported from coreos-sources-6.12.94.ebuild.
# Patches in files/7.1/ should be verified against 7.1.1 before building.
# Patches z0001 (kbuild) and z0002 (pahole) are Flatcar-specific and likely
# need minor context updates. z0004-z0007 (EFI lockdown) and z0009 (partition
# UUID) may already be upstream in 7.x — verify before removing from UNIPATCH_LIST.

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

UNIPATCH_LIST="
	${PATCH_DIR}/z0001-kbuild-derive-relative-path-for-srctree-from-CURDIR.patch
	${PATCH_DIR}/z0002-pahole-support-reproducible-builds.patch
	${PATCH_DIR}/z0003-Revert-x86-boot-Remove-the-bugger-off-message.patch
	${PATCH_DIR}/z0004-efi-add-an-efi_secure_boot-flag-to-indicate-secure-b.patch
	${PATCH_DIR}/z0005-efi-lock-down-the-kernel-if-booted-in-secure-boot-mo.patch
	${PATCH_DIR}/z0006-mtd-disable-slram-and-phram-when-locked-down.patch
	${PATCH_DIR}/z0007-arm64-add-kernel-config-option-to-lock-down-when.patch
	${PATCH_DIR}/z0009-block-add-partition-uuid-into-uevent.patch
"
