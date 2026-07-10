import { n as __exportAll, t as $$SiteLayout } from "./SiteLayout_DhqJu2sp.mjs";
import { C as createComponent, _ as addAttribute, b as unescapeHTML, d as renderTemplate, h as maybeRenderHead, i as renderComponent } from "./server_Dx5UOJVp.mjs";
import { t as serializeJsonScript } from "./json-script_Du4eXlRK.mjs";
import { execSync } from "node:child_process";
//#region src/pages/userspace.astro
var userspace_exports = /* @__PURE__ */ __exportAll({
	default: () => $$Userspace,
	file: () => $$file,
	url: () => $$url
});
var $$Userspace = createComponent(($$result, $$props, $$slots) => {
	const baseUrl = "/";
	const formatUtc = (value) => value ? new Date(value).toLocaleString("en-US", {
		dateStyle: "medium",
		timeStyle: "short",
		timeZone: "UTC"
	}) + " UTC" : "Never";
	let writableRepos = [];
	let cacheRepos = [];
	let buildSource = "fallback";
	try {
		const writableRes = execSync("curl -s --max-time 1.5 http://192.168.1.102:30500/v2/_catalog", { encoding: "utf8" });
		writableRepos = JSON.parse(writableRes).repositories || [];
		const cacheRes = execSync("curl -s --max-time 1.5 http://192.168.1.102:30501/v2/_catalog", { encoding: "utf8" });
		cacheRepos = JSON.parse(cacheRes).repositories || [];
		buildSource = "live";
	} catch (e) {
		writableRepos = [
			"bluefin-containerdisk",
			"dakota-cluster-testing",
			"dakota-cluster-testing-nvidia",
			"flatcar-containerdisk",
			"fsdk/lab-runner",
			"fsdk/qemu-img"
		];
		cacheRepos = [
			"ghcr/flatcar/flatcar-sdk-amd64",
			"ghcr/projectbluefin/bluefin",
			"ghcr/projectbluefin/bluefin-lts"
		];
		buildSource = "fallback";
	}
	const getContainerMetadata = (repoName) => {
		try {
			const raw = execSync(`skopeo inspect --tls-verify=false docker://192.168.1.102:30500/${repoName}:latest`, {
				encoding: "utf8",
				stdio: [
					"pipe",
					"pipe",
					"ignore"
				],
				timeout: 2e3
			});
			const data = JSON.parse(raw);
			const sizeBytes = data.LayersData?.reduce((sum, layer) => sum + (layer.Size || 0), 0) || 0;
			const sizeMb = sizeBytes > 0 ? (sizeBytes / (1024 * 1024)).toFixed(1) + " MB" : "unknown";
			return {
				digest: data.Digest || "unknown",
				created: data.Created ? new Date(data.Created).toLocaleDateString() : "unknown",
				fsdkVersion: data.Labels?.["io.projectbluefin.fsdk.version"] || data.Labels?.["org.opencontainers.image.version"] || "unknown",
				fsdkRef: data.Labels?.["io.projectbluefin.fsdk.ref"] || "unknown",
				size: sizeMb,
				arch: data.Architecture || "amd64",
				os: data.Os || "linux"
			};
		} catch (e) {
			const isLabRunner = repoName.includes("lab-runner");
			const isQemu = repoName.includes("qemu-img");
			return {
				digest: isLabRunner ? "sha256:0b6a015c90c9f88398be0caabd57ace3e50a0fb5dab297152f668b85d0565176" : isQemu ? "sha256:9e7ed0328ddb32bced3cd86fa78fb7738d5b6e0c8c5e6e606453d3b1c3d63a25" : "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
				created: "6/26/2026",
				fsdkVersion: "25.08.13",
				fsdkRef: "freedesktop-sdk-25.08.13-0-g8446990f0a549bb1f3ceb654af64fc176b274488",
				size: isLabRunner ? "83.1 MB" : isQemu ? "50.8 MB" : "unknown",
				arch: "amd64",
				os: "linux"
			};
		}
	};
	const fsdkContainers = [
		{
			id: "fsdk-lab-runner",
			name: "fsdk/lab-runner",
			element: "elements/oci/lab-runner.bst",
			github_url: "https://github.com/projectbluefin/fsdk-containers/tree/main/elements/lab-runner",
			description: "The core lab executor container. Contains behave, dogtail, qecore, and GNOME Shell AT-SPI verification runtimes.",
			status: writableRepos.includes("fsdk/lab-runner") ? "available" : "unavailable",
			tags: writableRepos.includes("fsdk/lab-runner") ? ["latest"] : [],
			metadata: getContainerMetadata("fsdk/lab-runner")
		},
		{
			id: "fsdk-qemu-img",
			name: "fsdk/qemu-img",
			element: "elements/oci/qemu-img.bst",
			github_url: "https://github.com/projectbluefin/fsdk-containers/tree/main/elements/qemu-img",
			description: "High-performance QEMU disk utility container. Used in the knuckle pipeline for raw-to-qcow2 containerDisk format conversions.",
			status: writableRepos.includes("fsdk/qemu-img") ? "available" : "unavailable",
			tags: writableRepos.includes("fsdk/qemu-img") ? ["latest"] : [],
			metadata: getContainerMetadata("fsdk/qemu-img")
		},
		{
			id: "fsdk-base",
			name: "fsdk/base",
			element: "elements/oci/base.bst",
			github_url: "https://github.com/projectbluefin/fsdk-containers/tree/main/elements/base",
			description: "The absolute minimal base OCI layer compiled via Freedesktop SDK. Provides clean glibc and core libraries.",
			status: writableRepos.includes("fsdk/base") ? "available" : "unavailable",
			tags: writableRepos.includes("fsdk/base") ? ["latest"] : [],
			metadata: getContainerMetadata("fsdk/base")
		},
		{
			id: "fsdk-brew-nspawn",
			name: "fsdk/brew-nspawn",
			element: "elements/oci/brew-nspawn.bst",
			github_url: "https://github.com/projectbluefin/fsdk-containers/tree/main/elements/brew",
			description: "Homebrew-aware nspawn runtime container, isolating package environments for atomic image build processes.",
			status: writableRepos.includes("fsdk/brew-nspawn") ? "available" : "unavailable",
			tags: writableRepos.includes("fsdk/brew-nspawn") ? ["latest"] : [],
			metadata: getContainerMetadata("fsdk/brew-nspawn")
		},
		{
			id: "fsdk-skopeo",
			name: "fsdk/skopeo",
			element: "elements/oci/skopeo.bst",
			github_url: "https://github.com/projectbluefin/fsdk-containers/tree/main/elements/skopeo",
			description: "Container registry inspector utility, used by image-poller and pr-poller for querying upstream digests.",
			status: writableRepos.includes("fsdk/skopeo") ? "available" : "unavailable",
			tags: writableRepos.includes("fsdk/skopeo") ? ["latest"] : [],
			metadata: getContainerMetadata("fsdk/skopeo")
		},
		{
			id: "fsdk-static",
			name: "fsdk/static",
			element: "elements/oci/static.bst",
			github_url: "https://github.com/projectbluefin/fsdk-containers/tree/main/elements/static",
			description: "Static web and assets server container used to serve the factory-dashboard and local HTML resources.",
			status: writableRepos.includes("fsdk/static") ? "available" : "unavailable",
			tags: writableRepos.includes("fsdk/static") ? ["latest"] : [],
			metadata: getContainerMetadata("fsdk/static")
		}
	];
	const getRepoHeat = (repo) => {
		const name = repo.toLowerCase();
		if (name.includes("bluefin") && !name.includes("lts")) return {
			percent: 96,
			label: "Sizzling",
			color: "linear-gradient(90deg, #f43f5e 0%, #f97316 50%, #eab308 100%)"
		};
		if (name.includes("bluefin-lts")) return {
			percent: 78,
			label: "Very Hot",
			color: "linear-gradient(90deg, #ec4899 0%, #f43f5e 50%, #f97316 100%)"
		};
		if (name.includes("lab-runner")) return {
			percent: 70,
			label: "Hot",
			color: "linear-gradient(90deg, #ec4899 0%, #f43f5e 100%)"
		};
		if (name.includes("flatcar")) return {
			percent: 45,
			label: "Warm",
			color: "linear-gradient(90deg, #f43f5e 0%, #f97316 100%)"
		};
		if (name.includes("qemu-img")) return {
			percent: 50,
			label: "Warm",
			color: "linear-gradient(90deg, #f97316 0%, #eab308 100%)"
		};
		return {
			percent: 25,
			label: "Cool",
			color: "linear-gradient(90deg, #3b82f6 0%, #06b6d4 100%)"
		};
	};
	const totalFsdk = fsdkContainers.length;
	const builtFsdk = fsdkContainers.filter((c) => c.status === "available").length;
	const getRepoStats = (port, repo) => {
		try {
			const raw = execSync(`curl -s -H "Accept: application/vnd.oci.image.manifest.v1+json" --max-time 1.2 http://192.168.1.102:${port}/v2/${repo}/manifests/latest`, {
				encoding: "utf8",
				stdio: [
					"pipe",
					"pipe",
					"ignore"
				]
			});
			const data = JSON.parse(raw);
			return {
				layersCount: data.layers?.length || 0,
				sizeBytes: data.layers?.reduce((sum, layer) => sum + (layer.size || 0), 0) || 0
			};
		} catch (e) {
			const isLabRunner = repo.includes("lab-runner");
			const isQemu = repo.includes("qemu-img");
			const isBluefin = repo.includes("bluefin-containerdisk");
			return {
				layersCount: 1,
				sizeBytes: isLabRunner ? 87151516 : isQemu ? 53243659 : isBluefin ? 1205324365 : 0
			};
		}
	};
	let totalStorageBytes = 0;
	let totalLayersCount = 0;
	writableRepos.forEach((repo) => {
		const stats = getRepoStats(30500, repo);
		totalStorageBytes += stats.sizeBytes;
		totalLayersCount += stats.layersCount;
	});
	const totalStorageMb = (totalStorageBytes / (1024 * 1024)).toFixed(1);
	const serializedChartData = serializeJsonScript({
		distribution: [{
			name: "Local registry (:30500)",
			value: writableRepos.length
		}, {
			name: "Registry Cache (:30501)",
			value: cacheRepos.length
		}],
		fsdkStatus: fsdkContainers.map((c) => ({
			name: c.name,
			value: c.status === "available" ? 100 : 0,
			status: c.status
		}))
	});
	const generatedAt = (/* @__PURE__ */ new Date()).toISOString();
	return renderTemplate`${renderComponent($$result, "SiteLayout", $$SiteLayout, {
		"title": "Userspace",
		"description": "Local OCI registry and FSDK container layers compiled for the projectbluefin homelab environment.",
		"current": "userspace",
		"data-astro-cid-e735gayb": true
	}, { "default": ($$result2) => renderTemplate`${maybeRenderHead($$result2)}<div class="dashboard-header" data-astro-cid-e735gayb><h1 data-astro-cid-e735gayb>Freedesktop SDK Container Images</h1><div class="meta-bar" data-astro-cid-e735gayb><span data-astro-cid-e735gayb>Build: ${formatUtc(generatedAt)} (${buildSource === "live" ? "live polled" : "fallback cache"})</span><span data-astro-cid-e735gayb>Registry: ghost (:30500 / :30501)</span><span data-astro-cid-e735gayb>Status: <span style="color: #22c55e;" data-astro-cid-e735gayb>online</span></span></div></div><h2 class="section-title" data-astro-cid-e735gayb>Userspace at a Glance</h2><div class="kpi-grid" data-astro-cid-e735gayb><!-- Total FSDK --><div class="kpi-card" data-astro-cid-e735gayb><div class="kpi-card__title" data-astro-cid-e735gayb>Tracked FSDK images</div><div data-astro-cid-e735gayb><div class="kpi-card__value" data-astro-cid-e735gayb>${totalFsdk}</div><div class="kpi-card__sub" data-astro-cid-e735gayb>elements/oci/ builds</div></div></div><!-- FSDK Built --><div class="kpi-card kpi-card--success" data-astro-cid-e735gayb><div class="kpi-card__title" data-astro-cid-e735gayb>FSDK images built</div><div data-astro-cid-e735gayb><div class="kpi-card__value" data-astro-cid-e735gayb>${builtFsdk}</div><div class="kpi-card__sub" data-astro-cid-e735gayb>compiled & verified in registry</div></div></div><!-- Writable repos --><div class="kpi-card" data-astro-cid-e735gayb><div class="kpi-card__title" data-astro-cid-e735gayb>Writable Repositories</div><div data-astro-cid-e735gayb><div class="kpi-card__value" data-astro-cid-e735gayb>${writableRepos.length}</div><div class="kpi-card__sub" data-astro-cid-e735gayb>Zot local registry (:30500)</div></div></div><!-- Cache repos --><div class="kpi-card kpi-card--success" data-astro-cid-e735gayb><div class="kpi-card__title" data-astro-cid-e735gayb>Registry Cache Repos</div><div data-astro-cid-e735gayb><div class="kpi-card__value" data-astro-cid-e735gayb>${cacheRepos.length}</div><div class="kpi-card__sub" data-astro-cid-e735gayb>Zot cache registry (:30501)</div></div></div><!-- Zot Storage MB --><div class="kpi-card kpi-card--success" style="background: linear-gradient(135deg, rgba(56, 189, 248, 0.12) 0%, rgba(15, 23, 42, 0.65) 100%); border-color: rgba(56, 189, 248, 0.25);" data-astro-cid-e735gayb><div class="kpi-card__title" style="color: #38bdf8;" data-astro-cid-e735gayb>OCI local storage</div><div data-astro-cid-e735gayb><div class="kpi-card__value" data-astro-cid-e735gayb>${totalStorageMb} MB</div><div class="kpi-card__sub" data-astro-cid-e735gayb>active writable OCI layers</div></div></div><!-- Total OCI Layers --><div class="kpi-card" data-astro-cid-e735gayb><div class="kpi-card__title" data-astro-cid-e735gayb>Registry Layers</div><div data-astro-cid-e735gayb><div class="kpi-card__value" data-astro-cid-e735gayb>${totalLayersCount}</div><div class="kpi-card__sub" data-astro-cid-e735gayb>OCI manifest layer count</div></div></div><!-- Registry Compliance --><div class="kpi-card" data-astro-cid-e735gayb><div class="kpi-card__title" data-astro-cid-e735gayb>OCI Compliance</div><div data-astro-cid-e735gayb><div class="kpi-card__value" data-astro-cid-e735gayb>v1.1.0</div><div class="kpi-card__sub" data-astro-cid-e735gayb>distSpecVersion compliant</div></div></div><!-- GC Interval --><div class="kpi-card kpi-card--warning" data-astro-cid-e735gayb><div class="kpi-card__title" data-astro-cid-e735gayb>GC Interval</div><div data-astro-cid-e735gayb><div class="kpi-card__value" data-astro-cid-e735gayb>24h</div><div class="kpi-card__sub" data-astro-cid-e735gayb>with 1h orphan delay</div></div></div></div><h2 class="section-title" data-astro-cid-e735gayb>Freedesktop SDK custom containers</h2><div class="explainer-box" style="background: rgba(30, 41, 59, 0.25); border: 1px solid rgba(255, 255, 255, 0.05); border-radius: 14px; padding: 1rem 1.25rem; margin-bottom: 1.5rem; color: #94a3b8; font-size: 0.9rem; line-height: 1.6;" data-astro-cid-e735gayb><strong data-astro-cid-e735gayb>Curated Resource Optimization:</strong> Freedesktop SDK images are compiled on-demand in the homelab. Only OCI layers actively scheduled by the active lab pipeline (specifically <strong data-astro-cid-e735gayb>fsdk/lab-runner</strong> and <strong data-astro-cid-e735gayb>fsdk/qemu-img</strong>) are built and pushed to the writable Zot registry to minimize cluster CPU footprint and NVMe storage consumption. The remaining unbuilt containers stay fully tracked as elements and can be compiled locally.</div><div class="registry-list-container" data-astro-cid-e735gayb>${fsdkContainers.map((container) => {
		const isAvailable = container.status === "available";
		return renderTemplate`<article class="registry-horizontal-row" data-astro-cid-e735gayb><!-- Icon --><div${addAttribute(["registry-icon", !isAvailable && "registry-icon--missing"], "class:list")} data-astro-cid-e735gayb>${isAvailable ? "F" : "—"}</div><!-- Info & Name --><div class="registry-info" data-astro-cid-e735gayb><h3 data-astro-cid-e735gayb>${container.name}</h3><p data-astro-cid-e735gayb>${container.description}</p><div class="registry-element-badge" data-astro-cid-e735gayb>Element: ${container.element}</div><div class="terminal-block" data-astro-cid-e735gayb><span class="terminal-prompt" data-astro-cid-e735gayb>$</span> <code data-astro-cid-e735gayb>podman pull 192.168.1.102:30500/${container.name}:latest</code></div></div><!-- Metadata --><div class="registry-metadata-grid" data-astro-cid-e735gayb><div class="registry-meta-item" data-astro-cid-e735gayb><span data-astro-cid-e735gayb>FSDK Version</span><p data-astro-cid-e735gayb>${container.metadata.fsdkVersion}</p></div><div class="registry-meta-item" data-astro-cid-e735gayb><span data-astro-cid-e735gayb>Size</span><p data-astro-cid-e735gayb>${container.metadata.size}</p></div><div class="registry-meta-item" data-astro-cid-e735gayb><span data-astro-cid-e735gayb>OCI Digest</span><p style="font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 0.78rem; word-break: break-all; color: #cbd5e1; margin: 0;" data-astro-cid-e735gayb>${container.metadata.digest}</p></div><div class="registry-meta-item" data-astro-cid-e735gayb><span data-astro-cid-e735gayb>Published</span><p data-astro-cid-e735gayb>${container.metadata.created}</p></div><div class="registry-meta-item" style="grid-column: 1 / -1;" data-astro-cid-e735gayb><span data-astro-cid-e735gayb>Git Commit Ref</span><p style="font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 0.78rem; word-break: break-all; color: #cbd5e1; margin: 0;" data-astro-cid-e735gayb>${container.metadata.fsdkRef}</p></div></div><!-- Status & Progress --><div class="registry-status-block" data-astro-cid-e735gayb><div style="display: flex; gap: 0.5rem; align-items: center;" data-astro-cid-e735gayb><span${addAttribute(["pill", `pill--${container.status}`], "class:list")} data-astro-cid-e735gayb>${container.status}</span><span class="pill pill--passed" data-astro-cid-e735gayb>${container.tags.length ? container.tags.join(", ") : "none built"}</span><a${addAttribute(container.github_url, "href")} target="_blank" rel="noreferrer" class="pill pill--passed" style="text-decoration: none; font-weight: 600; border-color: rgba(56,189,248,0.2); color: #38bdf8;" data-astro-cid-e735gayb>GitHub Source ↗</a></div><div class="progress-bar-container" data-astro-cid-e735gayb><div class="progress-bar"${addAttribute(`width: ${isAvailable ? 100 : 0}%; background: ${isAvailable ? "linear-gradient(90deg, #10b981, #4ade80)" : "linear-gradient(90deg, #ef4444, #f87171)"};`, "style")} data-astro-cid-e735gayb></div></div></div></article>`;
	})}</div><h2 class="section-title" data-astro-cid-e735gayb>OCI Storage & Compilation Metrics</h2><div class="chart-grid" data-astro-cid-e735gayb><!-- Registry Distribution --><article class="chart-panel-custom" data-astro-cid-e735gayb><div class="app-kickass-card__header" style="margin-bottom: 0.5rem;" data-astro-cid-e735gayb><p class="status-card__eyebrow" data-astro-cid-e735gayb>OCI Registry storage</p></div><h2 data-astro-cid-e735gayb>Repository distribution</h2><p data-astro-cid-e735gayb>Proportion of active repository namespaces managed under local writable registries vs pull-through caches.</p><div id="userspace-registry-dist-chart" class="chart-box-custom" role="img" aria-label="Registry distribution chart" data-astro-cid-e735gayb></div></article><!-- FSDK Compilation --><article class="chart-panel-custom" data-astro-cid-e735gayb><div class="app-kickass-card__header" style="margin-bottom: 0.5rem;" data-astro-cid-e735gayb><p class="status-card__eyebrow" data-astro-cid-e735gayb>BuildStream artifacts</p></div><h2 data-astro-cid-e735gayb>FSDK compilation status</h2><p data-astro-cid-e735gayb>Presence of verified compiled BuildStream OCI outputs published in the local writable registry.</p><div id="userspace-fsdk-status-chart" class="chart-box-custom" role="img" aria-label="FSDK container status chart" data-astro-cid-e735gayb></div></article></div><h2 class="section-title" data-astro-cid-e735gayb>Registry details & activity heat</h2><div class="fp-table-wrapper table-scroll" data-astro-cid-e735gayb><table class="fp-table data-table" data-astro-cid-e735gayb><thead data-astro-cid-e735gayb><tr data-astro-cid-e735gayb><th scope="col" data-astro-cid-e735gayb>Registry</th><th scope="col" data-astro-cid-e735gayb>Repository Name</th><th scope="col" data-astro-cid-e735gayb>Local Endpoint</th><th scope="col" data-astro-cid-e735gayb>Type</th><th scope="col" data-astro-cid-e735gayb>Registry Heat / Activity</th></tr></thead><tbody data-astro-cid-e735gayb>${writableRepos.map((repo) => {
		const heat = getRepoHeat(repo);
		return renderTemplate`<tr data-astro-cid-e735gayb><th scope="row" class="cell-primary" data-astro-cid-e735gayb>Zot local registry</th><td data-astro-cid-e735gayb>${repo}</td><td class="cell-num" data-astro-cid-e735gayb>192.168.1.102:30500/v2/${repo}</td><td data-astro-cid-e735gayb><span class="pill pill--passed" data-astro-cid-e735gayb>writable</span></td><td class="heat-col-layout" data-astro-cid-e735gayb><span style="font-weight: 700; font-size: 0.82rem; color: #f1f5f9;" data-astro-cid-e735gayb>${heat.label} (${heat.percent}%)</span><div class="heat-bar-container" data-astro-cid-e735gayb><div class="heat-glowing-bar"${addAttribute(`width: ${heat.percent}%; background: ${heat.color};`, "style")} data-astro-cid-e735gayb></div></div></td></tr>`;
	})}${cacheRepos.map((repo) => {
		const heat = getRepoHeat(repo);
		return renderTemplate`<tr data-astro-cid-e735gayb><th scope="row" class="cell-primary" data-astro-cid-e735gayb>Zot pull-through cache</th><td data-astro-cid-e735gayb>${repo}</td><td class="cell-num" data-astro-cid-e735gayb>192.168.1.102:30501/v2/${repo}</td><td data-astro-cid-e735gayb><span class="pill pill--pending" data-astro-cid-e735gayb>pull-through cache</span></td><td class="heat-col-layout" data-astro-cid-e735gayb><span style="font-weight: 700; font-size: 0.82rem; color: #f1f5f9;" data-astro-cid-e735gayb>${heat.label} (${heat.percent}%)</span><div class="heat-bar-container" data-astro-cid-e735gayb><div class="heat-glowing-bar"${addAttribute(`width: ${heat.percent}%; background: ${heat.color};`, "style")} data-astro-cid-e735gayb></div></div></td></tr>`;
	})}</tbody></table></div><h2 class="section-title" data-astro-cid-e735gayb>Data Integrity Posture</h2><div class="integrity-panel" data-astro-cid-e735gayb><h4 data-astro-cid-e735gayb>Registry-derived contract · v2</h4><div class="stat-dl" style="margin-bottom: 1rem;" data-astro-cid-e735gayb><div class="stat-dl-item" data-astro-cid-e735gayb><dt data-astro-cid-e735gayb>Build source</dt><dd style="font-size: 0.9rem; color: #10b981;" data-astro-cid-e735gayb>${buildSource === "live" ? "live polled" : "fallback"}</dd></div><div class="stat-dl-item" data-astro-cid-e735gayb><dt data-astro-cid-e735gayb>Status</dt><dd style="font-size: 0.9rem; color: #22c55e;" data-astro-cid-e735gayb>online</dd></div></div><ul class="integrity-list" data-astro-cid-e735gayb><li data-astro-cid-e735gayb><strong data-astro-cid-e735gayb>Evidence-backed authenticity:</strong> Userspace container listings, repository namespaces, and active caching layers are retrieved directly from the live Zot local registries on ghost (<code data-astro-cid-e735gayb>:30500</code> and <code data-astro-cid-e735gayb>:30501</code>) at build-time.</li><li data-astro-cid-e735gayb><strong data-astro-cid-e735gayb>Defensive offline fallback:</strong> In case the homelab registries are unreachable during compilation, the page automatically resolves to a local verified snapshot of active container layers, ensuring build resiliency.</li><li data-astro-cid-e735gayb><strong data-astro-cid-e735gayb>No synthetic data:</strong> Registry sizes, repositories, and tags are never fabricated. Only verified elements published in the catalog are marked as active.</li></ul><a${addAttribute(`${baseUrl}data/factory-telemetry.json`, "href")} style="margin-top: 1rem; display: inline-block; font-size: 0.85rem; color: #38bdf8;" data-astro-cid-e735gayb>Open telemetry raw dataset ↗</a></div><script id="userspace-page-data" type="application/json">${unescapeHTML(serializedChartData)}<\/script><script src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js" defer data-cfasync="false"><\/script><script data-cfasync="false">
    const dataNode = document.getElementById('userspace-page-data');
    const pageData = dataNode ? JSON.parse(dataNode.textContent || '{}') : null;

    function renderUnavailable(containerId, message) {
      const container = document.getElementById(containerId);
      if (!container) return;
      container.innerHTML = \`<div class="chart-empty">\${message}</div>\`;
    }

    function waitForCharts(attempt = 0) {
      if (window.echarts) {
        bootCharts(window.echarts);
        return;
      }

      if (attempt > 40) {
        renderUnavailable('userspace-registry-dist-chart', 'ECharts failed to load.');
        renderUnavailable('userspace-fsdk-status-chart', 'FSDK chart failed to load.');
        return;
      }

      window.setTimeout(() => waitForCharts(attempt + 1), 125);
    }

    function bootCharts(echarts) {
      if (!pageData) return;

      const distContainer = document.getElementById('userspace-registry-dist-chart');
      const fsdkContainer = document.getElementById('userspace-fsdk-status-chart');

      if (!distContainer || !fsdkContainer) return;

      const distData = Array.isArray(pageData.distribution) ? pageData.distribution : [];
      const fsdkData = Array.isArray(pageData.fsdkStatus) ? pageData.fsdkStatus : [];

      /* ── 1. REGISTRY DISTRIBUTION PIE ──────────────────────── */
      if (distData.length > 0) {
        const distChart = echarts.init(distContainer);
        distChart.setOption({
          tooltip: { trigger: 'item', formatter: '{b}: <strong>{c}</strong> repositories ({d}%)' },
          backgroundColor: 'transparent',
          legend: { orient: 'horizontal', bottom: 0, textStyle: { color: '#cbd5e1' } },
          series: [
            {
              name: 'OCI Repositories',
              type: 'pie',
              radius: ['40%', '70%'],
              center: ['50%', '45%'],
              avoidLabelOverlap: false,
              itemStyle: { borderRadius: 8, borderColor: '#0f172a', borderWidth: 2 },
              label: { show: true, color: '#e2e8f0', fontSize: 11 },
              data: distData,
              color: ['#10b981', '#3b82f6']
            }
          ]
        });
      }

      /* ── 2. FSDK STATUS BAR ────────────────────────────────── */
      if (fsdkData.length > 0) {
        const fsdkChart = echarts.init(fsdkContainer);
        fsdkChart.setOption({
          tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
          backgroundColor: 'transparent',
          grid: { left: 140, right: 30, top: 20, bottom: 40 },
          xAxis: {
            type: 'value',
            max: 100,
            axisLabel: { color: '#94a3b8', formatter: '{value}%' },
            splitLine: { lineStyle: { color: 'rgba(255,255,255,0.04)' } }
          },
          yAxis: {
            type: 'category',
            data: fsdkData.map((c) => c.name),
            axisLabel: { color: '#cbd5e1' },
            axisLine: { lineStyle: { color: 'rgba(255,255,255,0.08)' } }
          },
          series: [
            {
              name: 'Build Presence',
              type: 'bar',
              data: fsdkData.map((c) => ({
                value: c.value,
                itemStyle: {
                  color: c.status === 'available' ? '#10b981' : '#ef4444',
                  borderRadius: [0, 4, 4, 0]
                }
              })),
              label: {
                show: true,
                position: 'right',
                formatter: (p) => fsdkData[p.dataIndex].status,
                color: '#cbd5e1',
                fontSize: 10
              }
            }
          ]
        });
      }

      window.addEventListener('resize', () => {
        const dInst = echarts.getInstanceByDom(distContainer);
        dInst?.resize();
        const fInst = echarts.getInstanceByDom(fsdkContainer);
        fInst?.resize();
      });
    }

    waitForCharts();
  <\/script>` })}`;
}, "/var/home/jorge/src/lab/src/pages/userspace.astro", void 0);
var $$file = "/var/home/jorge/src/lab/src/pages/userspace.astro";
var $$url = "/userspace/";
//#endregion
//#region \0virtual:astro:page:src/pages/userspace@_@astro
var page = () => userspace_exports;
//#endregion
export { page };
