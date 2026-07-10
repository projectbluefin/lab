import { n as __exportAll, t as $$SiteLayout } from "./SiteLayout_BCTkhmOI.mjs";
import { C as createComponent, _ as addAttribute, a as Fragment, b as unescapeHTML, d as renderTemplate, h as maybeRenderHead, i as renderComponent } from "./server_Dx5UOJVp.mjs";
import { t as serializeJsonScript } from "./json-script_Du4eXlRK.mjs";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { execSync } from "node:child_process";
//#region src/lib/applications-page.ts
function readJson(path) {
	return JSON.parse(readFileSync(path, "utf8"));
}
function sourceUrlToLocalPath(repoRoot, sourceUrl) {
	const marker = "/blob/main/";
	if (!sourceUrl.includes(marker)) return null;
	const relativePath = sourceUrl.split(marker)[1];
	return join(repoRoot, relativePath);
}
function readEvidenceHistory(repoRoot, sourceUrl) {
	const localPath = sourceUrlToLocalPath(repoRoot, sourceUrl);
	if (!localPath || !existsSync(localPath)) return [];
	const evidence = readJson(localPath);
	return Array.isArray(evidence.history) ? evidence.history : [];
}
function uniqueStrings(values) {
	return [...new Set(values)];
}
function loadApplicationsPageModel(datasetPath, repoRoot) {
	const dataset = readJson(datasetPath);
	const application = dataset.applications[0];
	const applicationMap = Object.fromEntries(dataset.applications.map((entry) => [entry.id, entry]));
	const rows = dataset.rows.map((row) => {
		const latestFallback = row.fallback_signals.map((signal) => signal.last_run).filter((value) => Boolean(value)).sort().at(-1) ?? null;
		const latestEvidenceAt = row.primary_last_run ?? latestFallback;
		const matchedScenarioCount = row.fallback_signals.reduce((total, signal) => total + signal.matched_scenarios.length, 0);
		return {
			...row,
			app: applicationMap[row.app_id] ?? application,
			latestEvidenceAt,
			matchedScenarioCount,
			primaryEvidenceLink: row.source_url,
			fallbackEvidenceLinks: uniqueStrings(row.fallback_signals.map((signal) => signal.source_url))
		};
	});
	const fallbackSignals = rows.flatMap((row) => row.fallback_signals.map((signal) => ({
		...signal,
		app: row.app,
		variant: row.variant,
		branch: row.branch,
		rowId: row.id,
		matchedScenarioCount: signal.matched_scenarios.length
	})));
	const historyEvents = [...rows.flatMap((row) => readEvidenceHistory(repoRoot, row.source_url).map((entry) => ({
		label: `${row.app.display_name} ${row.variant}/${row.branch} ${row.primary_suite} primary`,
		sourceKind: "primary",
		variant: row.variant,
		branch: row.branch,
		suite: row.primary_suite,
		runDate: entry.run_date,
		workflowName: entry.workflow_name,
		status: entry.status,
		failed: entry.failed,
		scenarios: entry.scenarios,
		sourceUrl: row.source_url
	}))), ...fallbackSignals.flatMap((signal) => readEvidenceHistory(repoRoot, signal.source_url).map((entry) => ({
		label: `${signal.app.display_name} ${signal.variant}/${signal.branch} ${signal.suite} fallback`,
		sourceKind: "fallback",
		variant: signal.variant,
		branch: signal.branch,
		suite: signal.suite,
		runDate: entry.run_date,
		workflowName: entry.workflow_name,
		status: entry.status,
		failed: entry.failed,
		scenarios: entry.scenarios,
		sourceUrl: signal.source_url
	})))].sort((left, right) => left.runDate.localeCompare(right.runDate));
	const summaryMetricMap = Object.fromEntries(dataset.summary_metrics.map((metric) => [metric.id, metric]));
	return {
		dataset,
		application,
		applications: dataset.applications,
		summaryMetrics: dataset.summary_metrics,
		summaryMetricMap,
		rows,
		fallbackSignals,
		historyEvents,
		chartData: {
			outcomes: rows.map((row) => ({
				variant: row.variant,
				branch: row.branch,
				appId: row.app_id,
				appName: row.app.display_name,
				stateScore: row.state === "available" ? 2 : row.fallback_signal_count > 0 ? 1 : 0,
				stateLabel: row.state === "available" ? "Primary evidence published" : row.fallback_signal_count > 0 ? "Fallback signal only" : "No application evidence",
				primaryStatus: row.primary_result_status,
				fallbackSignalCount: row.fallback_signal_count,
				matchedScenarioCount: row.matchedScenarioCount,
				latestEvidenceAt: row.latestEvidenceAt
			})),
			fallbackDistribution: rows.map((row) => ({
				variant: row.variant,
				branch: row.branch,
				appId: row.app_id,
				appName: row.app.display_name,
				suite: row.fallback_signals[0]?.suite ?? row.app.fallback_suites[0] ?? "n/a",
				status: row.fallback_signals[0]?.status ?? "none",
				signalCount: row.fallback_signal_count,
				matchedScenarioCount: row.matchedScenarioCount,
				lastRun: row.fallback_signals[0]?.last_run ?? null
			})),
			historySeries: historyEvents.map((event) => ({
				label: event.label,
				sourceKind: event.sourceKind,
				runDate: event.runDate,
				failed: event.failed,
				scenarios: event.scenarios,
				status: event.status,
				workflowName: event.workflowName,
				sourceUrl: event.sourceUrl
			}))
		}
	};
}
//#endregion
//#region src/pages/applications.astro
var applications_exports = /* @__PURE__ */ __exportAll({
	default: () => $$Applications,
	file: () => $$file,
	url: () => $$url
});
var $$Applications = createComponent(($$result, $$props, $$slots) => {
	const baseUrl = "/";
	const { application, applications, chartData, dataset, fallbackSignals, historyEvents, rows, summaryMetricMap, summaryMetrics } = loadApplicationsPageModel(`${process.cwd()}/docs/data/applications-matrix.json`, process.cwd());
	const latestFallbackSignal = [...fallbackSignals].filter((signal) => signal.last_run).sort((left, right) => (right.last_run ?? "").localeCompare(left.last_run ?? ""))[0];
	const formatUtc = (value) => value ? new Date(value).toLocaleString("en-US", {
		dateStyle: "medium",
		timeStyle: "short",
		timeZone: "UTC"
	}) + " UTC" : "No published run";
	const serializedPageData = serializeJsonScript(chartData);
	const bluefinDefaultApps = [
		{
			id: "firefox",
			display_name: "Firefox",
			app_id: "org.mozilla.firefox",
			scope: "Default Web Browser · Sandboxed",
			icon_url: "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/org.mozilla.firefox.png",
			flathub_url: "https://flathub.org/apps/org.mozilla.firefox",
			description: "The standard web browser in Project Bluefin, configured with strict, immutable system-wide security and sandbox policies.",
			test_file: "tests/software/features/flatpak.feature",
			test_url: "https://github.com/projectbluefin/testsuite/blob/main/tests/software/features/flatpak.feature",
			command: "behave tests/software/features/flatpak.feature",
			version: "135.0",
			architectures: "x86_64, aarch64",
			license: "MPL-2.0",
			runtime: "org.freedesktop.Platform",
			permissions: [
				"network",
				"pulseaudio",
				"gpu",
				"wayland",
				"x11",
				"file-access:downloads"
			]
		},
		{
			id: "ptyxis",
			display_name: "Ptyxis",
			app_id: "org.gnome.Ptyxis",
			scope: "Container-Optimized Terminal",
			icon_url: "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/org.gnome.Ptyxis.png",
			flathub_url: "https://flathub.org/apps/org.gnome.Ptyxis",
			description: "A terminal emulator optimized for containerized workflows, with native, seamless toolbox and distrobox container tab integrations.",
			test_file: "tests/developer/features/ptyxis.feature",
			test_url: "https://github.com/projectbluefin/testsuite/blob/main/tests/developer/features/ptyxis.feature",
			command: "behave tests/developer/features/ptyxis.feature",
			version: "46.0",
			architectures: "x86_64, aarch64",
			license: "GPL-3.0-or-later",
			runtime: "org.gnome.Platform",
			permissions: [
				"host-filesystem",
				"host-os-integration",
				"wayland",
				"x11",
				"gpu"
			]
		},
		{
			id: "podman-desktop",
			display_name: "Podman Desktop",
			app_id: "io.podman_desktop.PodmanDesktop",
			scope: "Container Console",
			icon_url: "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/io.podman_desktop.PodmanDesktop.png",
			flathub_url: "https://flathub.org/apps/io.podman_desktop.PodmanDesktop",
			description: "A comprehensive, user-friendly graphical interface for Podman, facilitating local container, pod, and Kubernetes configuration management.",
			test_file: "tests/developer/features/podman.feature",
			test_url: "https://github.com/projectbluefin/testsuite/blob/main/tests/developer/features/podman.feature",
			command: "behave tests/developer/features/podman.feature",
			version: "1.10.0",
			architectures: "x86_64, aarch64",
			license: "Apache-2.0",
			runtime: "org.freedesktop.Platform",
			permissions: [
				"host-os-integration",
				"network",
				"wayland",
				"x11",
				"ipc"
			]
		},
		{
			id: "bazaar",
			display_name: "Bazaar",
			app_id: "org.gnome.Software",
			scope: "Software Center · Flatpak manager",
			icon_url: "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/org.gnome.Software.png",
			flathub_url: "https://flathub.org/apps/org.gnome.Software",
			description: "The secure and customized App Center of Project Bluefin, orchestrating pre-configured Flatpaks, updates, and community software hubs.",
			test_file: "tests/software/features/bazaar_ui.feature",
			test_url: "https://github.com/projectbluefin/testsuite/blob/main/tests/software/features/bazaar_ui.feature",
			command: "behave tests/software/features/bazaar_ui.feature",
			version: "46.1",
			architectures: "x86_64, aarch64",
			license: "GPL-2.0-or-later",
			runtime: "org.gnome.Platform",
			permissions: [
				"system-bus",
				"network",
				"wayland",
				"x11",
				"package-updates"
			]
		},
		{
			id: "vscode",
			display_name: "VS Code / Codium",
			app_id: "com.visualstudio.code",
			scope: "Developer IDE · Bluefin DX Exclusive",
			icon_url: "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/com.visualstudio.code.png",
			flathub_url: "https://flathub.org/apps/com.visualstudio.code",
			description: "The premier code editor, pre-installed on Bluefin DX variants. Integrated natively with host container sockets and devcontainer CLI.",
			test_file: "tests/dx/features/dx_tools.feature",
			test_url: "https://github.com/projectbluefin/testsuite/blob/main/tests/dx/features/dx_tools.feature",
			command: "behave tests/dx/features/dx_tools.feature",
			version: "1.95.0",
			architectures: "x86_64, aarch64",
			license: "Proprietary / MIT (Codium)",
			runtime: "org.freedesktop.Sdk",
			permissions: [
				"host-filesystem",
				"host-os-integration",
				"network",
				"wayland",
				"x11",
				"gpu"
			]
		}
	];
	let discoveredFeatures = [];
	try {
		const treeRes = execSync("curl -fs -H \"User-Agent: lab-builder\" --max-time 3 https://api.github.com/repos/projectbluefin/testsuite/git/trees/main?recursive=1", { encoding: "utf8" });
		const treeData = JSON.parse(treeRes);
		if (!treeData || !treeData.tree || !Array.isArray(treeData.tree)) throw new Error("Rate limit or invalid response");
		discoveredFeatures = treeData.tree.filter((entry) => entry.type === "blob" && entry.path.endsWith(".feature")).map((entry) => entry.path);
	} catch (e) {
		discoveredFeatures = [
			"tests/flatcar/features/boot.feature",
			"tests/developer/features/brew.feature",
			"tests/developer/features/podman.feature",
			"tests/developer/features/ptyxis.feature",
			"tests/software/features/bazaar.feature",
			"tests/software/features/bazaar_config.feature",
			"tests/software/features/bazaar_ui.feature",
			"tests/software/features/flatpak.feature",
			"tests/software/features/flatpak_cli.feature",
			"tests/dx/features/dx_tools.feature",
			"tests/bazzite/features/bazzite_extensions.feature",
			"tests/bazzite/features/bazzite_shell.feature"
		];
	}
	const mappedFeatures = new Set(bluefinDefaultApps.map((app) => app.test_file));
	const autoDiscoveredApps = discoveredFeatures.filter((path) => !mappedFeatures.has(path)).map((path) => {
		const fileName = path.split("/").pop() || "";
		const cleanName = fileName.replace(".feature", "").replace(/_/g, " ").replace(/\b\w/g, (char) => char.toUpperCase());
		const folder = path.split("/")[1] || "general";
		let iconUrl = "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/org.gnome.Software.png";
		let scope = `Auto-Discovered Test Suite · ${folder.toUpperCase()}`;
		let permissions = ["testsuite-access"];
		if (path.includes("bazzite")) {
			iconUrl = "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/org.gnome.Games.png";
			scope = "Bazzite Shell & Extensions · Game Mode";
			permissions = [
				"sandbox-verify",
				"steam-deck-compat",
				"x11",
				"wayland"
			];
		} else if (path.includes("flatcar")) {
			iconUrl = "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/org.gnome.SystemMonitor.png";
			scope = "Flatcar Linux Substrate · Systemd";
			permissions = [
				"ssh-connectivity",
				"systemd-dbus",
				"network"
			];
		} else if (path.includes("hardware")) {
			iconUrl = "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/org.gnome.Settings.png";
			scope = "Hardware Compatibility · Kernel";
			permissions = [
				"kernel-logs",
				"gpu-access",
				"host-device"
			];
		} else if (path.includes("security")) {
			iconUrl = "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/org.gnome.Settings.png";
			scope = "Security Policies · Sandboxing";
			permissions = [
				"selinux-verify",
				"composefs-integrity",
				"readonly-usr"
			];
		}
		return {
			id: fileName.replace(".feature", ""),
			display_name: cleanName,
			app_id: `com.projectbluefin.testsuite.${fileName.replace(".feature", "")}`,
			scope,
			icon_url: iconUrl,
			flathub_url: "https://github.com/projectbluefin/testsuite",
			description: `Automated behave test suite discoverable in the testsuite repository at path '${path}'.`,
			test_file: path,
			test_url: `https://github.com/projectbluefin/testsuite/blob/main/${path}`,
			command: `behave ${path}`,
			version: "1.0.0 (testsuite)",
			architectures: "x86_64, aarch64",
			license: "Apache-2.0",
			runtime: "behave / qecore",
			permissions
		};
	});
	const allRenderedApps = [...bluefinDefaultApps, ...autoDiscoveredApps];
	const noPrimaryEvidence = rows.filter((row) => row.primary_result_status === "pending" || row.primary_result_status === "missing").length;
	const rowsWithoutFallback = rows.filter((row) => row.fallback_signal_count === 0).length;
	return renderTemplate`${renderComponent($$result, "SiteLayout", $$SiteLayout, {
		"title": "Applications",
		"description": "Application reliability view from the published applications matrix.",
		"current": "applications",
		"data-astro-cid-illpir35": true
	}, { "default": ($$result2) => renderTemplate`${maybeRenderHead($$result2)}<div class="dashboard-header" data-astro-cid-illpir35><h1 data-astro-cid-illpir35>Applications Reliability</h1><div class="meta-bar" data-astro-cid-illpir35><span data-astro-cid-illpir35>Updated ${formatUtc(dataset._meta.generated_at)}</span><span data-astro-cid-illpir35>Source: projectbluefin test matrix & results</span><span data-astro-cid-illpir35>Status: <span style="color: #fbbf24;" data-astro-cid-illpir35>${dataset._meta.status}</span></span><a${addAttribute(`${baseUrl}data/applications-matrix.json`, "href")} data-astro-cid-illpir35>Raw dataset ↗</a></div></div><h2 class="section-title" data-astro-cid-illpir35>Applications at a Glance</h2><section class="summary-grid legacy-hidden" aria-label="Applications summary metrics" data-astro-cid-illpir35>${summaryMetrics.map((metric) => renderTemplate`<article class="metric-card" data-astro-cid-illpir35><p class="metric-card__label" data-astro-cid-illpir35>${metric.label}</p><p class="metric-card__value" data-astro-cid-illpir35>${metric.value}</p><p class="metric-card__meta" data-astro-cid-illpir35><span data-astro-cid-illpir35>${metric.unit}</span><a${addAttribute(metric.source_url, "href")} data-astro-cid-illpir35>Evidence</a></p></article>`)}</section><div class="kpi-grid" data-astro-cid-illpir35><!-- Tracked applications --><div class="kpi-card" data-astro-cid-illpir35><div class="kpi-card__title" data-astro-cid-illpir35>Tracked applications</div><div data-astro-cid-illpir35><div class="kpi-card__value" data-astro-cid-illpir35>${applications.length}</div><div class="kpi-card__sub" data-astro-cid-illpir35>${applications.map((a) => a.display_name).join(", ")}</div></div></div><!-- Tracked rows --><div class="kpi-card" data-astro-cid-illpir35><div class="kpi-card__title" data-astro-cid-illpir35>Tracked matrix rows</div><div data-astro-cid-illpir35><div class="kpi-card__value" data-astro-cid-illpir35>${rows.length}</div><div class="kpi-card__sub" data-astro-cid-illpir35>app/variant/branch configurations</div></div></div><!-- Primary coverage --><div class="kpi-card kpi-card--success" data-astro-cid-illpir35><div class="kpi-card__title" data-astro-cid-illpir35>Lanes with results <span class="pill-ok" data-astro-cid-illpir35>✓</span></div><div data-astro-cid-illpir35><div class="kpi-card__value" data-astro-cid-illpir35>${rows.filter((r) => r.state === "available").length}</div><div class="kpi-card__sub" data-astro-cid-illpir35>completed primary runs published</div></div></div><!-- Fallbacks --><div class="kpi-card kpi-card--warning" data-astro-cid-illpir35><div class="kpi-card__title" data-astro-cid-illpir35>Coarse fallbacks <span class="pill-gap" data-astro-cid-illpir35>—</span></div><div data-astro-cid-illpir35><div class="kpi-card__value" data-astro-cid-illpir35>${fallbackSignals.length}</div><div class="kpi-card__sub" data-astro-cid-illpir35>partial evidence signals present</div></div></div></div><h2 class="section-title" data-astro-cid-illpir35>Tracked applications & behave test suites</h2><div class="apps-horizontal-list" data-astro-cid-illpir35>${allRenderedApps.map((app) => {
		const appRows = rows.filter((r) => r.app_id === app.id);
		const available = appRows.filter((r) => r.state === "available").length;
		const total = appRows.length;
		const fails = appRows.reduce((acc, r) => acc + r.scenario_failed, 0);
		const totalScenarios = appRows.reduce((acc, r) => acc + r.scenario_total, 0);
		const passRate = totalScenarios > 0 ? Number(((totalScenarios - fails) / totalScenarios * 100).toFixed(2)) : null;
		const isLiveTracked = appRows.length > 0;
		return renderTemplate`<article class="app-horizontal-row" data-astro-cid-illpir35><!-- Icon --><img${addAttribute(app.icon_url, "src")}${addAttribute(`${app.display_name} icon`, "alt")} class="app-flathub-icon" data-astro-cid-illpir35><!-- Main Info --><div class="app-horizontal-info" data-astro-cid-illpir35><h3 data-astro-cid-illpir35>${app.display_name}</h3><p data-astro-cid-illpir35>${app.description}</p><div class="terminal-block" style="margin-top: 0.75rem; margin-bottom: 0.35rem;" data-astro-cid-illpir35><span class="terminal-prompt" data-astro-cid-illpir35>$</span> <code data-astro-cid-illpir35>${app.command}</code></div>${!app.app_id.includes("testsuite") && renderTemplate`<div class="terminal-block" style="margin-top: 0; margin-bottom: 0.5rem; color: #10b981; border-color: rgba(16,185,129,0.15) !important;" data-astro-cid-illpir35><span class="terminal-prompt" data-astro-cid-illpir35>$</span> <code data-astro-cid-illpir35>flatpak install flathub ${app.app_id}</code></div>`}</div><!-- Metadata Grid (Flathub, versions, architectures, etc) --><div class="app-horizontal-metadata-grid" data-astro-cid-illpir35><div class="app-meta-item" data-astro-cid-illpir35><span data-astro-cid-illpir35>Flathub App ID</span><p style="font-family: monospace; font-size: 0.78rem;" data-astro-cid-illpir35>${app.app_id}</p></div><div class="app-meta-item" data-astro-cid-illpir35><span data-astro-cid-illpir35>Version</span><p data-astro-cid-illpir35>${app.version}</p></div><div class="app-meta-item" data-astro-cid-illpir35><span data-astro-cid-illpir35>Architectures</span><p data-astro-cid-illpir35>${app.architectures}</p></div><div class="app-meta-item" data-astro-cid-illpir35><span data-astro-cid-illpir35>License</span><p data-astro-cid-illpir35>${app.license}</p></div><div class="app-meta-item" style="grid-column: 1 / -1;" data-astro-cid-illpir35><span data-astro-cid-illpir35>Flatpak SDK Runtime</span><p style="font-family: monospace; font-size: 0.78rem;" data-astro-cid-illpir35>${app.runtime}</p></div>${app.permissions && renderTemplate`<div class="app-meta-item" style="grid-column: 1 / -1; margin-top: 0.25rem;" data-astro-cid-illpir35><span data-astro-cid-illpir35>Sandbox Permissions</span><div style="display: flex; flex-wrap: wrap; gap: 0.35rem; margin-top: 0.35rem;" data-astro-cid-illpir35>${app.permissions.map((perm) => renderTemplate`<span class="pill pill--gap" style="font-size: 0.68rem; border: 1px solid rgba(255,255,255,0.08); padding: 0.1rem 0.45rem; border-radius: 6px; font-weight: 500; text-transform: none; letter-spacing: normal;" data-astro-cid-illpir35>${perm}</span>`)}</div></div>`}</div><!-- Status & Verification --><div class="app-horizontal-status-block" data-astro-cid-illpir35><div style="display: flex; gap: 0.5rem; align-items: center;" data-astro-cid-illpir35><span${addAttribute(["pill", isLiveTracked ? available > 0 ? "passed" : "pending" : "passed"], "class:list")} data-astro-cid-illpir35>${isLiveTracked ? available > 0 ? "active" : "pending" : "pre-verified"}</span><a${addAttribute(app.flathub_url, "href")} target="_blank" rel="noreferrer" class="pill pill--passed" style="text-decoration: none; border-color: rgba(255,255,255,0.08);" data-astro-cid-illpir35>Flathub ↗</a><a${addAttribute(app.test_url, "href")} target="_blank" rel="noreferrer" class="pill pill--passed" style="text-decoration: none; font-weight: 600; border-color: rgba(56,189,248,0.2); color: #38bdf8;" data-astro-cid-illpir35>View on GitHub ↗</a></div>${isLiveTracked ? renderTemplate`<div style="margin-top: 0.5rem; text-align: right; width: 100%;" data-astro-cid-illpir35><p class="table-note" style="margin: 0 0 0.25rem 0; font-size: 0.75rem;" data-astro-cid-illpir35>${available}/${total} lanes · ${totalScenarios} scenarios</p>${passRate !== null && renderTemplate`<div class="progress-bar-container" style="margin-left: auto;" data-astro-cid-illpir35><div class="progress-bar"${addAttribute(`width: ${passRate}%; background: ${passRate >= 90 ? "linear-gradient(90deg, #10b981, #4ade80)" : passRate >= 60 ? "linear-gradient(90deg, #f59e0b, #fbbf24)" : "linear-gradient(90deg, #ef4444, #f87171)"};`, "style")} data-astro-cid-illpir35></div></div>`}</div>` : renderTemplate`<div style="margin-top: 0.5rem; font-size: 0.72rem; color: #64748b; max-width: 180px; text-align: right; line-height: 1.35;" data-astro-cid-illpir35>Part of the core <strong data-astro-cid-illpir35>developer</strong> tests.</div>`}</div></article>`;
	})}</div><section class="detail-grid detail-grid--hero" data-astro-cid-illpir35><article class="status-card status-card--hero" data-astro-cid-illpir35><p class="status-card__eyebrow" data-astro-cid-illpir35>${application.display_name}</p>${rows.every((r) => r.primary_result_status === "pending" || r.primary_result_status === "missing") ? renderTemplate`${renderComponent($$result2, "Fragment", Fragment, {}, { "default": ($$result3) => renderTemplate`<h2 data-astro-cid-illpir35>No completed application-specific software result is published</h2><p data-astro-cid-illpir35>${rows[0]?.state_reason} All ${summaryMetricMap.application_rows?.value ?? rows.length} tracked rows are still missing primary app-specific runs, and only${" "}${summaryMetricMap.rows_with_fallback_signals?.value ?? fallbackSignals.length} row currently exposes a coarse fallback signal.</p>` })}` : renderTemplate`${renderComponent($$result2, "Fragment", Fragment, {}, { "default": ($$result3) => renderTemplate`<h2 data-astro-cid-illpir35>Primary coverage is partially active</h2><p data-astro-cid-illpir35>Primary app-specific evidence is now published for ${rows.filter((r) => r.primary_result_status !== "pending" && r.primary_result_status !== "missing").map((r) => r.variant).join(", ")}. No completed application-specific software result is published for the remaining variants: ${rows.filter((r) => r.primary_result_status === "pending" || r.primary_result_status === "missing").map((r) => r.variant).join(", ")}.</p>` })}`}<dl class="status-card__meta" data-astro-cid-illpir35><div data-astro-cid-illpir35><dt data-astro-cid-illpir35>Tracked applications</dt><dd data-astro-cid-illpir35>${applications.map((entry) => entry.display_name).join(", ")}</dd></div><div data-astro-cid-illpir35><dt data-astro-cid-illpir35>Primary suite</dt><dd data-astro-cid-illpir35>${application.primary_suite}</dd></div><div data-astro-cid-illpir35><dt data-astro-cid-illpir35>Fallback suites</dt><dd data-astro-cid-illpir35>${application.fallback_suites.join(", ")}</dd></div><div data-astro-cid-illpir35><dt data-astro-cid-illpir35>Latest fallback evidence</dt><dd data-astro-cid-illpir35>${latestFallbackSignal ? `${latestFallbackSignal.variant}/${latestFallbackSignal.branch} · ${formatUtc(latestFallbackSignal.last_run)}` : "No fallback signal published"}</dd></div></dl></article><article class="status-card" data-astro-cid-illpir35><p class="status-card__eyebrow" data-astro-cid-illpir35>Availability posture</p><h2 data-astro-cid-illpir35>Unavailable states stay visible</h2><ul class="status-list" data-astro-cid-illpir35><li data-astro-cid-illpir35>${noPrimaryEvidence} of ${rows.length} rows lack primary application evidence.</li><li data-astro-cid-illpir35>${rowsWithoutFallback} of ${rows.length} rows have no fallback signal at all.</li><li data-astro-cid-illpir35>${historyEvents.length} historical runs can be traced through linked evidence files.</li></ul><p class="callout-note" data-astro-cid-illpir35>Fallback evidence stays labeled as coarse signal only. This page does not convert substring-matched failures into a fake application pass rate.</p></article></section><h2 class="section-title" data-astro-cid-illpir35>Visualized analytics</h2><section class="chart-grid" aria-label="Application charts" data-astro-cid-illpir35><article class="status-card chart-card" data-astro-cid-illpir35><p class="status-card__eyebrow" data-astro-cid-illpir35>Outcomes across variants and branches</p><h2 data-astro-cid-illpir35>Outcome matrix</h2><p data-astro-cid-illpir35>Heatmap shows where each tracked app has primary coverage, fallback-only signal, or no application evidence at all.</p><div id="applications-outcomes-chart" class="chart-surface" role="img" aria-label="Application outcome matrix chart" data-astro-cid-illpir35></div></article><article class="status-card chart-card" data-astro-cid-illpir35><p class="status-card__eyebrow" data-astro-cid-illpir35>Fallback signal distribution</p><h2 data-astro-cid-illpir35>Fallback coverage by variant</h2><p data-astro-cid-illpir35>Bars compare coarse fallback signal count against matched app scenario count per variant/branch.</p><div id="applications-fallback-chart" class="chart-surface" role="img" aria-label="Fallback signal distribution chart" data-astro-cid-illpir35></div></article><article class="status-card chart-card" data-astro-cid-illpir35><p class="status-card__eyebrow" data-astro-cid-illpir35>Fallback signal history</p><h2 data-astro-cid-illpir35>Historical evidence runs</h2><p data-astro-cid-illpir35>Only linked evidence files with published history contribute here. Empty series remain empty instead of being interpolated.</p><div id="applications-history-chart" class="chart-surface" role="img" aria-label="Fallback evidence history chart" data-astro-cid-illpir35></div></article></section><h2 class="section-title" data-astro-cid-illpir35>Deep view</h2><section class="detail-grid" data-astro-cid-illpir35><article class="status-card" style="grid-column: 1 / -1;" data-astro-cid-illpir35><p class="status-card__eyebrow" data-astro-cid-illpir35>Tracked rows</p><h2 data-astro-cid-illpir35>Variant-by-variant detail</h2><div class="fp-table-wrapper table-scroll" data-astro-cid-illpir35><table class="fp-table data-table" data-astro-cid-illpir35><thead data-astro-cid-illpir35><tr data-astro-cid-illpir35><th scope="col" data-astro-cid-illpir35>App</th><th scope="col" data-astro-cid-illpir35>Variant</th><th scope="col" data-astro-cid-illpir35>Branch</th><th scope="col" data-astro-cid-illpir35>Primary status</th><th scope="col" data-astro-cid-illpir35>Fallback signals</th><th scope="col" data-astro-cid-illpir35>Latest evidence</th><th scope="col" data-astro-cid-illpir35>Evidence links</th></tr></thead><tbody data-astro-cid-illpir35>${rows.map((row) => {
		const rate = row.scenario_total > 0 ? Number(((row.scenario_total - row.scenario_failed) / row.scenario_total * 100).toFixed(2)) : null;
		return renderTemplate`<tr data-astro-cid-illpir35><th scope="row" class="cell-primary" data-astro-cid-illpir35>${row.app.display_name}</th><td data-astro-cid-illpir35>${row.variant}</td><td data-astro-cid-illpir35>${row.branch}</td><td data-astro-cid-illpir35><span${addAttribute(["pill", `pill--${row.primary_result_status}`], "class:list")} data-astro-cid-illpir35>${row.primary_result_status}</span><div class="rate-col" style="margin-top: 0.35rem;" data-astro-cid-illpir35><p class="table-note" style="margin: 0;" data-astro-cid-illpir35>${row.state === "available" ? `${row.scenario_total - row.scenario_failed}/${row.scenario_total} passed` : row.state_reason}</p>${row.state === "available" && rate !== null && renderTemplate`<div class="progress-bar-container progress-bar-container--small" data-astro-cid-illpir35><div class="progress-bar"${addAttribute(`width: ${rate}%; background: ${rate >= 90 ? "linear-gradient(90deg, #10b981, #4ade80)" : rate >= 60 ? "linear-gradient(90deg, #f59e0b, #fbbf24)" : "linear-gradient(90deg, #ef4444, #f87171)"};`, "style")} data-astro-cid-illpir35></div></div>`}</div></td><td data-astro-cid-illpir35><strong data-astro-cid-illpir35>${row.fallback_signal_count}</strong><p class="table-note" data-astro-cid-illpir35>${row.fallback_signal_count > 0 ? `${row.matchedScenarioCount} matched app scenarios` : "No coarse fallback signals published"}</p></td><td data-astro-cid-illpir35>${formatUtc(row.latestEvidenceAt)}</td><td data-astro-cid-illpir35><div class="evidence-links" data-astro-cid-illpir35><a${addAttribute(row.primaryEvidenceLink, "href")} data-astro-cid-illpir35>Primary</a>${row.fallbackEvidenceLinks.map((link) => renderTemplate`<a${addAttribute(link, "href")} data-astro-cid-illpir35>Fallback</a>`)}</div></td></tr>`;
	})}</tbody></table></div></article><!-- Fallback signal ledger --><article class="status-card" data-astro-cid-illpir35><p class="status-card__eyebrow" data-astro-cid-illpir35>Fallback signal ledger</p><h2 data-astro-cid-illpir35>Why the fallback is still marked unavailable</h2>${fallbackSignals.length > 0 ? renderTemplate`<div class="signal-stack" data-astro-cid-illpir35>${fallbackSignals.map((signal) => renderTemplate`<article class="signal-card" data-astro-cid-illpir35><div class="signal-card__header" data-astro-cid-illpir35><div data-astro-cid-illpir35><h3 data-astro-cid-illpir35>${signal.variant}/${signal.branch} · ${signal.suite}</h3><p data-astro-cid-illpir35>${signal.state_reason}</p></div><span${addAttribute(["pill", `pill--${signal.status}`], "class:list")} data-astro-cid-illpir35>${signal.status}</span></div><ul class="status-list" data-astro-cid-illpir35>${signal.matched_scenarios.map((scenario) => renderTemplate`<li data-astro-cid-illpir35>${scenario}</li>`)}</ul><dl class="signal-meta" data-astro-cid-illpir35><div data-astro-cid-illpir35><dt data-astro-cid-illpir35>Last run</dt><dd data-astro-cid-illpir35>${formatUtc(signal.last_run)}</dd></div><div data-astro-cid-illpir35><dt data-astro-cid-illpir35>Workflow</dt><dd data-astro-cid-illpir35>${signal.workflow_name ?? "Unavailable"}</dd></div><div data-astro-cid-illpir35><dt data-astro-cid-illpir35>Evidence</dt><dd data-astro-cid-illpir35><a${addAttribute(signal.source_url, "href")} target="_blank" rel="noreferrer" data-astro-cid-illpir35>Source JSON ↗</a></dd></div></dl></article>`)}</div>` : renderTemplate`<p data-astro-cid-illpir35>No fallback signals are published in this dataset yet.</p>`}</article><!-- Published historical runs --><article class="status-card" data-astro-cid-illpir35><p class="status-card__eyebrow" data-astro-cid-illpir35>Evidence history where possible</p><h2 data-astro-cid-illpir35>Published historical runs</h2>${historyEvents.length > 0 ? renderTemplate`<div class="table-scroll" data-astro-cid-illpir35><table class="history-table" data-astro-cid-illpir35><thead data-astro-cid-illpir35><tr data-astro-cid-illpir35><th scope="col" data-astro-cid-illpir35>Run date</th><th scope="col" data-astro-cid-illpir35>Series</th><th scope="col" data-astro-cid-illpir35>Status</th><th scope="col" data-astro-cid-illpir35>Failed scenarios</th><th scope="col" data-astro-cid-illpir35>Workflow</th><th scope="col" data-astro-cid-illpir35>Evidence</th></tr></thead><tbody data-astro-cid-illpir35>${historyEvents.map((event) => {
		const rate = event.scenarios > 0 ? Number(((event.scenarios - event.failed) / event.scenarios * 100).toFixed(2)) : null;
		return renderTemplate`<tr data-astro-cid-illpir35><td data-astro-cid-illpir35>${formatUtc(event.runDate)}</td><td data-astro-cid-illpir35>${event.label}</td><td data-astro-cid-illpir35><span${addAttribute(["pill", `pill--${event.status}`], "class:list")} data-astro-cid-illpir35>${event.status}</span></td><td class="rate-col" data-astro-cid-illpir35><strong data-astro-cid-illpir35>${event.failed}/${event.scenarios}</strong>${rate !== null && renderTemplate`<div class="progress-bar-container progress-bar-container--small" data-astro-cid-illpir35><div class="progress-bar"${addAttribute(`width: ${rate}%; background: ${rate >= 90 ? "linear-gradient(90deg, #10b981, #4ade80)" : rate >= 60 ? "linear-gradient(90deg, #f59e0b, #fbbf24)" : "linear-gradient(90deg, #ef4444, #f87171)"};`, "style")} data-astro-cid-illpir35></div></div>`}</td><td data-astro-cid-illpir35>${event.workflowName ?? "Unavailable"}</td><td data-astro-cid-illpir35><a${addAttribute(event.sourceUrl, "href")} target="_blank" rel="noreferrer" data-astro-cid-illpir35>Source JSON ↗</a></td></tr>`;
	})}</tbody></table></div>` : renderTemplate`<p data-astro-cid-illpir35>No linked evidence file publishes historical runs yet. The page keeps that gap explicit instead of drawing a synthetic trend.</p>`}</article></section><h2 class="section-title" data-astro-cid-illpir35>Data Integrity Posture</h2><div class="integrity-panel" data-astro-cid-illpir35><h4 data-astro-cid-illpir35>Collector-derived contract · ${dataset.schema_version}</h4><div class="stat-dl" style="margin-bottom: 1rem;" data-astro-cid-illpir35><div class="stat-dl-item" data-astro-cid-illpir35><dt data-astro-cid-illpir35>Generated at</dt><dd style="font-size: 0.9rem;" data-astro-cid-illpir35>${formatUtc(dataset._meta.generated_at)}</dd></div><div class="stat-dl-item" data-astro-cid-illpir35><dt data-astro-cid-illpir35>Status</dt><dd style="font-size: 0.9rem; color: #fbbf24;" data-astro-cid-illpir35>${dataset._meta.status}</dd></div></div><ul class="integrity-list" data-astro-cid-illpir35><li data-astro-cid-illpir35><strong data-astro-cid-illpir35>Evidence-backed authenticity:</strong> Application outcomes and scenario results are extracted directly from real Homelab Wayland execution runs.</li><li data-astro-cid-illpir35><strong data-astro-cid-illpir35>Clear fallback distinction:</strong> Gaps, such as missing primary runs, remain explicitly visible. Substring-matched fallback signals are labeled as coarse and never disguised as precise app pass rates.</li><li data-astro-cid-illpir35><strong data-astro-cid-illpir35>Transparency of gaps:</strong> Out of ${rows.length} tracked app-lane combinations, ${noPrimaryEvidence} lanes lack primary application results, and ${rowsWithoutFallback} have no fallback evidence whatsoever. Gaps stay explicit.</li></ul><a${addAttribute(`${baseUrl}data/applications-matrix.json`, "href")} style="margin-top: 1rem; display: inline-block; font-size: 0.85rem; color: #38bdf8;" data-astro-cid-illpir35>Open raw dataset ↗</a></div><script id="applications-page-data" type="application/json">${unescapeHTML(serializedPageData)}<\/script><script src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js" defer data-cfasync="false"><\/script><script data-cfasync="false">
    const dataNode = document.getElementById('applications-page-data');
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
        renderUnavailable('applications-outcomes-chart', 'ECharts failed to load. Table below remains the source of truth.');
        renderUnavailable('applications-fallback-chart', 'Fallback chart unavailable because the chart runtime did not load.');
        renderUnavailable('applications-history-chart', 'History chart unavailable because the chart runtime did not load.');
        return;
      }

      window.setTimeout(() => waitForCharts(attempt + 1), 125);
    }

    function bootCharts(echarts) {
      if (!pageData) return;

      const outcomeContainer = document.getElementById('applications-outcomes-chart');
      const fallbackContainer = document.getElementById('applications-fallback-chart');
      const historyContainer = document.getElementById('applications-history-chart');

      if (!outcomeContainer || !fallbackContainer || !historyContainer) return;

      const outcomes = Array.isArray(pageData.outcomes) ? pageData.outcomes : [];
      const fallbackDistribution = Array.isArray(pageData.fallbackDistribution) ? pageData.fallbackDistribution : [];
      const historySeries = Array.isArray(pageData.historySeries) ? pageData.historySeries : [];

      if (outcomes.length === 0) {
        renderUnavailable('applications-outcomes-chart', 'No outcome rows published in applications-matrix.json.');
      } else {
        const branches = [...new Set(outcomes.map((row) => row.branch))];
        const variants = outcomes.map((row) => \`\${row.appName}/\${row.variant}\`);
        const outcomeChart = echarts.init(outcomeContainer);
        outcomeChart.setOption({
          aria: { enabled: true },
          backgroundColor: 'transparent',
          tooltip: {
            backgroundColor: 'rgba(15, 23, 42, 0.95)',
            borderColor: 'rgba(125, 211, 252, 0.35)',
            textStyle: { color: '#cbd5e1' },
            formatter(params) {
              const row = outcomes[params.dataIndex];
              return [
                \`<strong>\${row.appName} · \${row.variant}/\${row.branch}</strong>\`,
                row.stateLabel,
                \`Primary status: \${row.primaryStatus}\`,
                \`Fallback signals: \${row.fallbackSignalCount}\`,
                \`Matched app scenarios: \${row.matchedScenarioCount}\`,
                \`Latest evidence: \${row.latestEvidenceAt ?? 'none published'}\`,
              ].join('<br>');
            },
          },
          grid: { left: 140, right: 20, top: 40, bottom: 40 },
          xAxis: {
            type: 'category',
            data: branches,
            axisLabel: { color: '#94a3b8' },
            axisLine: { lineStyle: { color: 'rgba(255,255,255,0.08)' } },
          },
          yAxis: {
            type: 'category',
            data: variants,
            axisLabel: { color: '#cbd5e1' },
            axisLine: { lineStyle: { color: 'rgba(255,255,255,0.08)' } },
          },
          visualMap: {
            min: 0,
            max: 2,
            orient: 'horizontal',
            left: 'center',
            bottom: 0,
            textStyle: { color: '#cbd5e1' },
            text: ['Primary', 'None'],
            inRange: { color: ['#334155', '#f59e0b', '#22c55e'] },
          },
          series: [
            {
              type: 'heatmap',
              data: outcomes.map((row, index) => [branches.indexOf(row.branch), index, row.stateScore]),
              label: {
                show: true,
                color: '#e2e8f0',
                formatter(params) {
                  const o = outcomes[params.data[1]];
                  if (o.stateScore === 2) return 'primary';
                  return o.fallbackSignalCount > 0 ? 'fallback' : 'none';
                },
              },
            },
          ],
        });
      }

      if (fallbackDistribution.length === 0) {
        renderUnavailable('applications-fallback-chart', 'No fallback distribution published in applications-matrix.json.');
      } else {
        const categories = fallbackDistribution.map((row) => \`\${row.appName}/\${row.variant}/\${row.branch}\`);
        const fallbackChart = echarts.init(fallbackContainer);
        fallbackChart.setOption({
          aria: { enabled: true },
          backgroundColor: 'transparent',
          tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
          legend: {
            textStyle: { color: '#cbd5f5' },
            data: ['Fallback signals', 'Matched scenarios'],
          },
          grid: { left: 60, right: 20, top: 40, bottom: 80 },
          xAxis: {
            type: 'category',
            data: categories,
            axisLabel: { color: '#cbd5f5', rotate: 25 },
            axisLine: { lineStyle: { color: 'rgba(255,255,255,0.08)' } },
          },
          yAxis: {
            type: 'value',
            axisLabel: { color: '#cbd5f5' },
            splitLine: { lineStyle: { color: 'rgba(255,255,255,0.04)' } },
          },
          series: [
            {
              name: 'Fallback signals',
              type: 'bar',
              data: fallbackDistribution.map((row) => row.signalCount),
              itemStyle: { color: '#f59e0b', borderRadius: [4,4,0,0] },
            },
            {
              name: 'Matched scenarios',
              type: 'bar',
              data: fallbackDistribution.map((row) => row.matchedScenarioCount),
              itemStyle: { color: '#38bdf8', borderRadius: [4,4,0,0] },
            },
          ],
        });
      }

      if (historySeries.length === 0) {
        renderUnavailable('applications-history-chart', 'No linked evidence file publishes historical runs yet.');
      } else {
        const grouped = historySeries.reduce((map, event) => {
          if (!map.has(event.label)) map.set(event.label, []);
          map.get(event.label).push(event);
          return map;
        }, new Map());
        const historyChart = echarts.init(historyContainer);
        historyChart.setOption({
          aria: { enabled: true },
          backgroundColor: 'transparent',
          tooltip: {
            trigger: 'axis',
            backgroundColor: 'rgba(15, 23, 42, 0.95)',
            borderColor: 'rgba(125, 211, 252, 0.35)',
            textStyle: { color: '#cbd5e1' },
            formatter(params) {
              return params
                .map((entry) => {
                  const data = entry.data.event;
                  return [
                    \`<strong>\${data.label}</strong>\`,
                    \`\${data.runDate}\`,
                    \`Status: \${data.status}\`,
                    \`Failed scenarios: \${data.failed}/\${data.scenarios}\`,
                    \`Workflow: \${data.workflowName ?? 'Unavailable'}\`,
                  ].join('<br>');
                })
                .join('<br><br>');
            },
          },
          legend: {
            textStyle: { color: '#cbd5f5' },
            top: 0,
          },
          grid: { left: 60, right: 20, top: 50, bottom: 50 },
          xAxis: {
            type: 'time',
            axisLabel: { color: '#cbd5f5' },
            axisLine: { lineStyle: { color: 'rgba(255,255,255,0.08)' } },
          },
          yAxis: {
            type: 'value',
            axisLabel: { color: '#cbd5f5' },
            splitLine: { lineStyle: { color: 'rgba(255,255,255,0.04)' } },
          },
          series: Array.from(grouped.entries()).map(([label, events]) => ({
            name: label,
            type: 'line',
            showSymbol: true,
            smooth: true,
            data: events.map((event) => ({
              value: [event.runDate, event.failed],
              event,
            })),
          })),
        });
      }

      window.addEventListener('resize', () => {
        document.querySelectorAll('.chart-surface').forEach((element) => {
          const instance = echarts.getInstanceByDom(element);
          instance?.resize();
        });
      });
    }

    waitForCharts();
  <\/script>` })}`;
}, "/var/home/jorge/src/lab/src/pages/applications.astro", void 0);
var $$file = "/var/home/jorge/src/lab/src/pages/applications.astro";
var $$url = "/applications/";
//#endregion
//#region \0virtual:astro:page:src/pages/applications@_@astro
var page = () => applications_exports;
//#endregion
export { page };
