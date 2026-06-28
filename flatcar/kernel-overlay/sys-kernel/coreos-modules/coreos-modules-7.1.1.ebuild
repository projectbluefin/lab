# Copyright 2013-2014 CoreOS, Inc.
# Distributed under the terms of the GNU General Public License v2
#
# Builds and installs CoreOS (Flatcar) kernel modules.
# Ported from coreos-modules-6.12.ebuild.

EAPI=7

DESCRIPTION="Builds and installs CoreOS (Flatcar) kernel modules"
HOMEPAGE="https://www.kernel.org"
LICENSE="GPL-2"
SLOT="0"
S="${WORKDIR}"

DEPEND="=sys-kernel/coreos-kernel-${PV}
	=sys-kernel/coreos-sources-${PV}"
RDEPEND="${DEPEND}"

KEYWORDS="amd64 arm64"

src_install() {
	KV_FULL=$(ls "${SYSROOT}/usr/src/" | grep "${PV}" | head -1)
	KSRC="${SYSROOT}/usr/src/${KV_FULL}"
	[[ -d "${KSRC}" ]] || die "coreos-sources-${PV} not found in ${SYSROOT}/usr/src"
	cd "${KSRC}"
	emake -j$(nproc) ARCH=x86_64 \
		INSTALL_MOD_PATH="${D}" \
		modules_install

	local kv
	kv=$(make -s ARCH=x86_64 kernelrelease)
	rm -f "${D}/usr/lib/modules/${kv}/build" "${D}/usr/lib/modules/${kv}/source"
	dosym "/usr/src/${KV_FULL}" "/usr/lib/modules/${kv}/build"
	dosym "/usr/src/${KV_FULL}" "/usr/lib/modules/${kv}/source"
}
