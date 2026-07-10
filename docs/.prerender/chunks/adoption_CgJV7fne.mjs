import { n as __exportAll, t as $$SiteLayout } from "./SiteLayout_DhqJu2sp.mjs";
import { C as createComponent, _ as addAttribute, a as Fragment, b as unescapeHTML, d as renderTemplate, h as maybeRenderHead, i as renderComponent } from "./server_Dx5UOJVp.mjs";
import { t as serializeJsonScript } from "./json-script_Du4eXlRK.mjs";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
//#region src/lib/adoption-page.ts
function readJson$1(filePath) {
	return JSON.parse(readFileSync(filePath, "utf8"));
}
function loadAdoptionPageModel(datasetPath) {
	const dataset = readJson$1(datasetPath);
	if (dataset.countme_trend) {
		const allowedDistros = [
			"bazzite",
			"bluefin",
			"bluefin-lts",
			"aurora",
			"dakota",
			"flatcar",
			"fedora"
		];
		const labelMap = {
			"bazzite": "Bazzite",
			"bluefin": "Bluefin",
			"bluefin-lts": "Bluefin LTS",
			"aurora": "Aurora",
			"dakota": "Dakota",
			"flatcar": "Flatcar",
			"fedora": "Fedora"
		};
		dataset.countme_trend.DISTROS = dataset.countme_trend.DISTROS.filter((d) => allowedDistros.includes(d));
		dataset.countme_trend.LABELS = dataset.countme_trend.DISTROS.map((d) => labelMap[d] || d);
		for (const d of allowedDistros) if (!dataset.countme_trend.DISTROS.includes(d)) {
			dataset.countme_trend.DISTROS.push(d);
			dataset.countme_trend.LABELS.push(labelMap[d]);
		}
		const sanitizeTrendList = (list) => {
			return list.map((item) => {
				const newDistros = {};
				let newTotal = 0;
				allowedDistros.forEach((d) => {
					const val = item.distros?.[d] || 0;
					newDistros[d] = val;
					newTotal += val;
				});
				return {
					...item,
					distros: newDistros,
					total: newTotal
				};
			});
		};
		if (Array.isArray(dataset.countme_trend.monthly)) dataset.countme_trend.monthly = sanitizeTrendList(dataset.countme_trend.monthly).sort((a, b) => a.week_start.localeCompare(b.week_start));
		if (Array.isArray(dataset.countme_trend.weekly)) dataset.countme_trend.weekly = sanitizeTrendList(dataset.countme_trend.weekly).sort((a, b) => a.week_start.localeCompare(b.week_start));
	}
	const summaryMetricMap = Object.fromEntries(dataset.summary_metrics.map((metric) => [metric.id, metric]));
	const withPullData = dataset.rows.filter((r) => r.pull_count !== null).length;
	const withCountmeData = dataset.rows.filter((r) => r.countme_active_devices !== null).length;
	const available = dataset.rows.filter((r) => r.state === "available").length;
	const unavailable = dataset.rows.filter((r) => r.state === "unavailable").length;
	const laneStats = {
		total: dataset.rows.length,
		withPullData,
		withCountmeData,
		available,
		unavailable,
		withoutPullData: dataset.rows.length - withPullData,
		withoutCountmeData: dataset.rows.length - withCountmeData
	};
	const lanesCoverage = dataset.rows.map((row) => ({
		id: row.id,
		label: `${row.variant}/${row.branch}`,
		hasPullData: row.pull_count !== null,
		hasCountmeData: row.countme_active_devices !== null,
		pullCount: row.pull_count,
		countmeActiveDevices: row.countme_active_devices,
		state: row.state
	}));
	const trustCoverage = dataset.trust_cards.map((card) => ({
		variant: card.variant,
		org: card.org,
		sbom: card.emits_sbom ? 1 : 0,
		cveScan: card.emits_cve_scan ? 1 : 0,
		cosign: card.emits_cosign_attestation ? 1 : 0,
		state: card.state
	}));
	return {
		dataset,
		summaryMetrics: dataset.summary_metrics,
		summaryMetricMap,
		trustCards: dataset.trust_cards,
		rows: dataset.rows,
		laneStats,
		chartData: {
			lanesCoverage,
			trustCoverage
		}
	};
}
//#endregion
//#region src/lib/homebrew-page.ts
function readJson(path) {
	return JSON.parse(readFileSync(path, "utf8"));
}
function toNumber(value) {
	return typeof value === "number" && Number.isFinite(value) ? value : null;
}
function readFallbackPackageStats(datasetPath) {
	const migratedPath = join(dirname(datasetPath), "homebrew-package-stats-migrated.json");
	if (!existsSync(migratedPath)) return null;
	return readJson(migratedPath);
}
function normalizePackageLeaderboard(dataset, fallbackStats) {
	const rawEntries = [
		...Array.isArray(dataset.package_leaderboard) ? dataset.package_leaderboard : [],
		...Array.isArray(dataset.package_rows) ? dataset.package_rows : [],
		...Array.isArray(dataset.packages) ? dataset.packages : []
	];
	const sourceFromRows = dataset.rows.find((row) => row.state === "available")?.source_url;
	const collectedFromRows = dataset.rows.find((row) => row.state === "available")?.collected_at;
	const normalizedFromDataset = rawEntries.map((entry, index) => {
		const item = entry;
		const packageName = typeof item.package_name === "string" && item.package_name || typeof item.name === "string" && item.name || typeof item.package === "string" && item.package || typeof item.formula === "string" && item.formula || null;
		if (!packageName) return null;
		const installCount = toNumber(item.install_count ?? item.installs_90d ?? item.installs);
		const downloadCount = toNumber(item.download_count ?? item.downloads);
		const tapName = typeof item.tap_name === "string" && item.tap_name || typeof item.tap === "string" && item.tap || typeof item.tapName === "string" && item.tapName || null;
		const tapUrl = typeof item.tap_url === "string" && item.tap_url || typeof item.tapUrl === "string" && item.tapUrl || null;
		const state = typeof item.state === "string" && item.state || (installCount !== null || downloadCount !== null ? "available" : "unavailable");
		const stateReason = typeof item.state_reason === "string" ? item.state_reason : state === "unavailable" ? "No package-level Homebrew analytics data is available for this package entry." : null;
		return {
			id: typeof item.id === "string" && item.id || `${packageName}-${index}`,
			package_name: packageName,
			tap_name: tapName,
			tap_url: tapUrl,
			install_count: installCount,
			download_count: downloadCount,
			state,
			state_reason: stateReason,
			source_url: typeof item.source_url === "string" && item.source_url || sourceFromRows || fallbackStats?.source_url || "",
			collected_at: typeof item.collected_at === "string" && item.collected_at || collectedFromRows || fallbackStats?.generated_at || dataset._meta.generated_at,
			derivation: typeof item.derivation === "string" && item.derivation || "Package-level Homebrew analytics entry loaded from docs/data/homebrew-ecosystem.json."
		};
	}).filter((entry) => entry !== null);
	if (normalizedFromDataset.length > 0) return normalizedFromDataset.sort((a, b) => {
		const installDiff = (b.install_count ?? -1) - (a.install_count ?? -1);
		if (installDiff !== 0) return installDiff;
		return (b.download_count ?? -1) - (a.download_count ?? -1);
	});
	return (fallbackStats?.taps || []).flatMap((tap, tapIndex) => (tap.packages || []).map((pkg, packageIndex) => ({
		id: `${tap.name}-${pkg.name}-${tapIndex}-${packageIndex}`,
		package_name: pkg.name,
		tap_name: tap.name,
		tap_url: tap.url,
		install_count: toNumber(pkg.installs_90d),
		download_count: toNumber(pkg.downloads),
		state: "available",
		state_reason: null,
		source_url: fallbackStats?.source_url || sourceFromRows || "",
		collected_at: fallbackStats?.generated_at || collectedFromRows || dataset._meta.generated_at,
		derivation: "Fallback package-level leaderboard derived from docs/data/homebrew-package-stats-migrated.json until dense package rows are published in docs/data/homebrew-ecosystem.json."
	}))).sort((a, b) => {
		const installDiff = (b.install_count ?? -1) - (a.install_count ?? -1);
		if (installDiff !== 0) return installDiff;
		return (b.download_count ?? -1) - (a.download_count ?? -1);
	});
}
function normalizeTapDensityRows(dataset, packageLeaderboard) {
	const fromDataset = [
		...Array.isArray(dataset.tap_density) ? dataset.tap_density : [],
		...Array.isArray(dataset.tap_density_rows) ? dataset.tap_density_rows : [],
		...Array.isArray(dataset.lane_tap_density) ? dataset.lane_tap_density : []
	].map((entry) => {
		const item = entry;
		const variant = typeof item.variant === "string" ? item.variant : null;
		const branch = typeof item.branch === "string" ? item.branch : null;
		const laneLabel = typeof item.lane_label === "string" && item.lane_label || (variant && branch ? `${variant}/${branch}` : null);
		if (!variant || !branch || !laneLabel) return null;
		const packageCount = toNumber(item.package_count ?? item.packages_in_scope ?? item.tap_package_count);
		const installCount = toNumber(item.install_count ?? item.installs_90d ?? item.installs);
		const downloadCount = toNumber(item.download_count ?? item.downloads);
		const state = typeof item.state === "string" && item.state || (packageCount !== null ? "available" : "unavailable");
		return {
			id: typeof item.id === "string" && item.id || `${variant}-${branch}`,
			variant,
			branch,
			lane_label: laneLabel,
			tap_name: typeof item.tap_name === "string" && item.tap_name || typeof item.tap === "string" && item.tap || null,
			tap_url: typeof item.tap_url === "string" && item.tap_url || typeof item.tapUrl === "string" && item.tapUrl || null,
			package_count: packageCount,
			install_count: installCount,
			download_count: downloadCount,
			state,
			state_reason: typeof item.state_reason === "string" ? item.state_reason : state !== "available" ? "No dense package-level tap density is published for this lane." : null,
			source_url: typeof item.source_url === "string" && item.source_url || dataset.rows.find((row) => row.id === `${variant}-${branch}`)?.source_url || "",
			collected_at: typeof item.collected_at === "string" && item.collected_at || dataset._meta.generated_at,
			derivation: typeof item.derivation === "string" && item.derivation || "Tap density entry loaded from docs/data/homebrew-ecosystem.json."
		};
	}).filter((entry) => entry !== null);
	if (fromDataset.length > 0) return fromDataset;
	const packagesByTap = packageLeaderboard.reduce((acc, pkg) => {
		if (!pkg.tap_name || pkg.state !== "available") return acc;
		acc[pkg.tap_name] = (acc[pkg.tap_name] || 0) + 1;
		return acc;
	}, {});
	return dataset.rows.map((row) => {
		const packageCount = row.tap_name ? packagesByTap[row.tap_name] ?? null : null;
		const isAvailable = row.state === "available" && packageCount !== null;
		return {
			id: row.id,
			variant: row.variant,
			branch: row.branch,
			lane_label: `${row.variant}/${row.branch}`,
			tap_name: row.tap_name,
			tap_url: row.tap_url,
			package_count: packageCount,
			install_count: row.install_count,
			download_count: row.download_count,
			state: isAvailable ? "available" : "unavailable",
			state_reason: isAvailable ? null : row.state_reason || "Tap density unavailable until package-level analytics are published for this lane.",
			source_url: row.source_url,
			collected_at: row.collected_at,
			derivation: row.state === "available" ? "Tap density derived from package-level entries grouped by tap_name." : row.derivation
		};
	});
}
function loadHomebrewPageModel(datasetPath) {
	const dataset = readJson(datasetPath);
	const rows = dataset.rows;
	const availableRows = rows.filter((row) => row.state === "available");
	const unavailableRows = rows.filter((row) => row.state !== "available");
	const summaryMetricMap = Object.fromEntries(dataset.summary_metrics.map((metric) => [metric.id, metric]));
	const packageLeaderboard = normalizePackageLeaderboard(dataset, readFallbackPackageStats(datasetPath));
	const tapDensityRows = normalizeTapDensityRows(dataset, packageLeaderboard);
	const lanesWithPackageDensity = tapDensityRows.filter((lane) => lane.state === "available" && lane.package_count !== null).length;
	const lanesAwaitingPackageDensity = tapDensityRows.length - lanesWithPackageDensity;
	const totalPackagesInScope = packageLeaderboard.filter((pkg) => pkg.state === "available").length;
	const distinctTapsWithPackages = new Set(packageLeaderboard.filter((pkg) => pkg.state === "available" && pkg.tap_name).map((pkg) => pkg.tap_name)).size;
	const tapComparison = dataset.taps.filter((tap) => tap.state === "available").map((tap) => ({
		name: tap.name,
		installs: tap.install_count ?? 0,
		downloads: tap.download_count ?? 0,
		packages: tap.package_count ?? 0
	}));
	const packageTypeSplit = dataset.taps.filter((tap) => tap.state === "available" && tap.package_type_counts).map((tap) => ({
		name: tap.name,
		formula: tap.package_type_counts?.formula ?? 0,
		cask: tap.package_type_counts?.cask ?? 0
	}));
	const availableLaneCount = rows.filter((r) => r.state === "available").length;
	const awaitingLaneCount = rows.filter((r) => r.state !== "available").length;
	const coverageDonut = [{
		value: availableLaneCount,
		name: "Data available",
		itemStyle: { color: "#22c55e" }
	}, {
		value: awaitingLaneCount,
		name: "Awaiting data",
		itemStyle: { color: "#334155" }
	}];
	const laneInstalls = rows.filter((r) => r.state === "available" && r.install_count !== null).map((r) => ({
		label: `${r.variant}/${r.branch}`,
		installs: r.install_count,
		downloads: r.download_count ?? 0
	}));
	const totalInstalls = dataset.taps.reduce((sum, t) => sum + (t.install_count ?? 0), 0);
	const totalPackages = dataset.taps.reduce((sum, t) => sum + (t.package_count ?? 0), 0);
	const chartData = {
		laneStatus: rows.map((row) => ({
			id: row.id,
			label: `${row.variant}/${row.branch}`,
			stateScore: row.state === "available" ? row.install_count !== null ? 2 : 1 : 0,
			stateLabel: row.state === "available" ? row.install_count !== null ? "Homebrew data available" : "Lane tracked, no install data" : "Awaiting Homebrew data",
			installCount: row.install_count,
			downloadCount: row.download_count,
			sourceUrl: row.source_url
		})),
		topPackages: packageLeaderboard.slice(0, 10).map((pkg) => ({
			name: pkg.package_name,
			tap: pkg.tap_name,
			installs: pkg.install_count,
			downloads: pkg.download_count
		})),
		tapComparison,
		packageTypeSplit,
		coverageDonut,
		laneInstalls,
		totalInstalls,
		totalPackages
	};
	return {
		dataset,
		summaryMetrics: dataset.summary_metrics,
		summaryMetricMap,
		taps: dataset.taps,
		rows,
		availableRows,
		unavailableRows,
		packageLeaderboard,
		tapDensityRows,
		tapDensitySummary: {
			lanesWithPackageDensity,
			lanesAwaitingPackageDensity,
			totalPackagesInScope,
			distinctTapsWithPackages,
			averagePackagesPerLane: lanesWithPackageDensity > 0 ? Number((totalPackagesInScope / lanesWithPackageDensity).toFixed(2)) : 0
		},
		chartData
	};
}
//#endregion
//#region src/pages/adoption.astro
var adoption_exports = /* @__PURE__ */ __exportAll({
	default: () => $$Adoption,
	file: () => $$file,
	url: () => $$url
});
var $$Adoption = createComponent(($$result, $$props, $$slots) => {
	const baseUrl = "/";
	const pageModel = loadAdoptionPageModel(`${process.cwd()}/docs/data/adoption-metrics.json`);
	const homebrewModel = loadHomebrewPageModel(`${process.cwd()}/docs/data/homebrew-ecosystem.json`);
	const { dataset, summaryMetrics, trustCards, rows, laneStats, chartData } = pageModel;
	const { dataset: hbDataset, summaryMetrics: hbSummaryMetrics, taps: hbTaps, rows: hbRows, availableRows: hbAvailableRows, unavailableRows: hbUnavailableRows, packageLeaderboard: hbPackageLeaderboard, tapDensityRows: hbTapDensityRows, tapDensitySummary: hbTapDensitySummary, chartData: hbChartData } = homebrewModel;
	const formatUtc = (value) => value ? new Date(value).toLocaleString("en-US", {
		dateStyle: "medium",
		timeStyle: "short",
		timeZone: "UTC"
	}) + " UTC" : "—";
	const fmt = (n) => n >= 1e6 ? (n / 1e6).toFixed(1) + "M" : n >= 1e3 ? (n / 1e3).toFixed(1) + "K" : n.toLocaleString();
	const serializedPageData = serializeJsonScript({
		lanesCoverage: chartData.lanesCoverage,
		trustCoverage: chartData.trustCoverage,
		countmeTrend: dataset.countme_trend,
		quayTrend: dataset.quay_trend,
		doraComparison: dataset.dora_comparison,
		osVersion: dataset.os_version,
		openssfScorecard: dataset.openssf_scorecard,
		ociBestPractices: dataset.oci_best_practices,
		homebrew: hbChartData
	});
	const unavailableTrustCards = trustCards.filter((c) => c.state === "unavailable");
	const transplantedCountmeRow = rows.find((r) => r.countme_active_devices !== null) ?? null;
	return renderTemplate`${renderComponent($$result, "SiteLayout", $$SiteLayout, {
		"title": "Adoption & Ecosystem Metrics",
		"description": "Executive-readable adoption metrics, supply-chain best practices, and security posture scorecards for the Project Bluefin Operating System Factory.",
		"current": "adoption",
		"data-astro-cid-e3colko2": true
	}, { "default": ($$result2) => renderTemplate`${maybeRenderHead($$result2)}<div class="dashboard-header" data-astro-cid-e3colko2><h1 data-astro-cid-e3colko2>bootc Active Devices & Adoption</h1><div class="meta-bar" data-astro-cid-e3colko2><span data-astro-cid-e3colko2>Updated ${formatUtc(dataset._meta.generated_at)}</span><span data-astro-cid-e3colko2>Registry source: Quay.io / GHCR public API</span><span data-astro-cid-e3colko2>Countme source: <a href="https://github.com/ublue-os/countme" data-astro-cid-e3colko2>ublue-os/countme</a></span></div></div><div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 1.5rem; margin-bottom: 2.5rem; margin-top: 1rem;" data-astro-cid-e3colko2><div class="dashboard-panel" style="margin-bottom: 0;" data-astro-cid-e3colko2><div class="panel-header" data-astro-cid-e3colko2><h3 data-astro-cid-e3colko2>Active Devices Trend</h3><div class="btn-group" id="countme-range-btns" data-astro-cid-e3colko2><button class="toggle-btn" data-range="30" data-astro-cid-e3colko2>30d</button><button class="toggle-btn" data-range="90" data-astro-cid-e3colko2>90d</button><button class="toggle-btn active" data-range="365" data-astro-cid-e3colko2>365d</button><button class="toggle-btn" data-range="all" data-astro-cid-e3colko2>All</button></div></div><div class="chart-box" id="countme-trend-chart" style="height: 320px;" data-astro-cid-e3colko2></div></div><div class="dashboard-panel" style="margin-bottom: 0;" data-astro-cid-e3colko2><div class="panel-header" data-astro-cid-e3colko2><h3 data-astro-cid-e3colko2>Ecosystem Share</h3></div><div class="chart-box" id="ecosystem-pie-chart" style="height: 320px;" data-astro-cid-e3colko2></div><div class="footnote" style="margin-top: 1.25rem; color: #64748b; font-size: 0.82rem; line-height: 1.5; border-top: 1px solid rgba(255,255,255,0.05); padding-top: 0.75rem;" data-astro-cid-e3colko2><strong data-astro-cid-e3colko2>Upstream Bedrock Note:</strong> This chart displays the custom downstream images built by Universal Blue (Bazzite, Bluefin, Aurora, etc.). Upstream base operating systems like <strong data-astro-cid-e3colko2>Fedora Silverblue</strong> and <strong data-astro-cid-e3colko2>Fedora Kinoite</strong> are not tracked here because they report natively to the official Fedora infrastructure, serving as the immutable bedrock of our supply chain.</div></div></div><section class="summary-grid" aria-label="adoption-summary-metrics" style="display: none;" data-astro-cid-e3colko2>${summaryMetrics.map((metric) => renderTemplate`<article class="metric-card" data-astro-cid-e3colko2><p class="metric-card__label" data-astro-cid-e3colko2>${metric.label}</p><p class="metric-card__value" data-astro-cid-e3colko2>${metric.value}</p><p class="metric-card__meta" data-astro-cid-e3colko2><span data-astro-cid-e3colko2>${metric.unit}</span><a${addAttribute(metric.source_url, "href")} data-astro-cid-e3colko2>Evidence</a></p></article>`)}</section><h2 class="kpi-section-title" data-astro-cid-e3colko2>Active Devices</h2><div class="kpi-grid" data-astro-cid-e3colko2><!-- Total Card --><div class="kpi-card kpi-card--total" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Total Active Devices <span class="kpi-card__trend trend--up" data-astro-cid-e3colko2>▲ 0.2%</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>87,045</div><div class="kpi-card__sub" data-astro-cid-e3colko2>/ this week</div></div><div class="kpi-card__sparkline" id="sparkline-total" data-astro-cid-e3colko2></div></div><!-- Bazzite Card --><div class="kpi-card" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Bazzite <span class="kpi-card__trend trend--down" data-astro-cid-e3colko2>▼</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>79,622</div><div class="kpi-card__sub" data-astro-cid-e3colko2>/ this week</div></div><div class="kpi-card__sparkline" id="sparkline-bazzite" data-astro-cid-e3colko2></div></div><!-- Bluefin Card --><div class="kpi-card" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Bluefin <span class="kpi-card__trend trend--up" data-astro-cid-e3colko2>▲</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>3,560</div><div class="kpi-card__sub" data-astro-cid-e3colko2>/ this week</div></div><div class="kpi-card__sparkline" id="sparkline-bluefin" data-astro-cid-e3colko2></div></div><!-- Bluefin-LTS Card --><div class="kpi-card" style="opacity: 0.75;" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Bluefin-LTS <span class="kpi-card__trend trend--neutral" data-astro-cid-e3colko2>—</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" style="font-size: 1.5rem; font-weight: 600; line-height: 2rem; margin: 0.5rem 0 0.25rem 0;" data-astro-cid-e3colko2>Awaiting Data</div><div class="kpi-card__sub" data-astro-cid-e3colko2>Telemetry pending</div></div></div><!-- Aurora Card --><div class="kpi-card" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Aurora <span class="kpi-card__trend trend--up" data-astro-cid-e3colko2>▲</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>2,622</div><div class="kpi-card__sub" data-astro-cid-e3colko2>/ this week</div></div><div class="kpi-card__sparkline" id="sparkline-aurora" data-astro-cid-e3colko2></div></div><!-- Dakota Card --><div class="kpi-card" style="opacity: 0.75;" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Dakota <span class="kpi-card__trend trend--neutral" data-astro-cid-e3colko2>—</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" style="font-size: 1.5rem; font-weight: 600; line-height: 2rem; margin: 0.5rem 0 0.25rem 0;" data-astro-cid-e3colko2>Awaiting Data</div><div class="kpi-card__sub" data-astro-cid-e3colko2>Telemetry pending</div></div></div><!-- Flatcar Card --><div class="kpi-card" style="opacity: 0.75;" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Flatcar <span class="kpi-card__trend trend--neutral" data-astro-cid-e3colko2>—</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" style="font-size: 1.5rem; font-weight: 600; line-height: 2rem; margin: 0.5rem 0 0.25rem 0;" data-astro-cid-e3colko2>Awaiting Data</div><div class="kpi-card__sub" data-astro-cid-e3colko2>Telemetry pending</div></div></div></div><h2 class="kpi-section-title" data-astro-cid-e3colko2>Image Pulls</h2><div class="explainer-box" style="margin-bottom: 1rem;" data-astro-cid-e3colko2>Quay.io container image pulls · source: public registry API</div><div class="kpi-grid" data-astro-cid-e3colko2><div class="kpi-card kpi-card-link" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>All Tracked Images</div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>1.8M</div><div class="kpi-card__sub" data-astro-cid-e3colko2>/ 30 days · 421.5K / 7d</div></div><div class="kpi-card__sparkline" id="sparkline-pulls-total" data-astro-cid-e3colko2></div></div><div class="kpi-card kpi-card-link" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Fedora CoreOS <span class="verified-badge" data-astro-cid-e3colko2>✓</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>1.5M</div><div class="kpi-card__sub" data-astro-cid-e3colko2>/ 30 days · 348.6K / 7d</div></div><div class="kpi-card__sparkline" id="sparkline-pulls-coreos" data-astro-cid-e3colko2></div></div><div class="kpi-card kpi-card-link" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Fedora bootc <span class="verified-badge" data-astro-cid-e3colko2>✓</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>159.0K</div><div class="kpi-card__sub" data-astro-cid-e3colko2>/ 30 days · 39.5K / 7d</div></div><div class="kpi-card__sparkline" id="sparkline-pulls-fedora" data-astro-cid-e3colko2></div></div><div class="kpi-card kpi-card-link" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>CentOS bootc <span class="verified-badge" data-astro-cid-e3colko2>✓</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>91.8K</div><div class="kpi-card__sub" data-astro-cid-e3colko2>/ 30 days · 20.8K / 7d</div></div><div class="kpi-card__sparkline" id="sparkline-pulls-centos" data-astro-cid-e3colko2></div></div><div class="kpi-card kpi-card-link" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>AlmaLinux bootc <span class="verified-badge" data-astro-cid-e3colko2>✓</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>49.1K</div><div class="kpi-card__sub" data-astro-cid-e3colko2>/ 30 days · 12.6K / 7d</div></div><div class="kpi-card__sparkline" id="sparkline-pulls-almalinux" data-astro-cid-e3colko2></div></div></div><div class="dashboard-panel" data-astro-cid-e3colko2><div class="panel-header" data-astro-cid-e3colko2><h3 data-astro-cid-e3colko2>Image Pull Trend</h3><div class="btn-group" id="quay-trend-range-btns" data-astro-cid-e3colko2><button class="toggle-btn" data-range="30" data-astro-cid-e3colko2>30d</button><button class="toggle-btn active" data-range="90" data-astro-cid-e3colko2>90d</button></div></div><div class="chart-box" id="quay-trend" data-astro-cid-e3colko2></div></div><h2 class="kpi-section-title" data-astro-cid-e3colko2>Project DORA Health</h2><div class="dora-grid" data-astro-cid-e3colko2><div class="dora-card" data-astro-cid-e3colko2><div class="dora-card__name" data-astro-cid-e3colko2>Bluefin</div><div class="dora-card__level" style="color: #fbbf24;" data-astro-cid-e3colko2>Medium</div><div class="dora-card__stat" data-astro-cid-e3colko2>165.1×/wk deploys</div><div class="dora-card__stat" data-astro-cid-e3colko2>50.3% fail rate</div></div><div class="dora-card" data-astro-cid-e3colko2><div class="dora-card__name" data-astro-cid-e3colko2>Aurora</div><div class="dora-card__level" style="color: #fbbf24;" data-astro-cid-e3colko2>Medium</div><div class="dora-card__stat" data-astro-cid-e3colko2>9.5×/wk deploys</div><div class="dora-card__stat" data-astro-cid-e3colko2>17.0% fail rate</div></div><div class="dora-card" data-astro-cid-e3colko2><div class="dora-card__name" data-astro-cid-e3colko2>Bazzite</div><div class="dora-card__level" style="color: #fbbf24;" data-astro-cid-e3colko2>Medium</div><div class="dora-card__stat" data-astro-cid-e3colko2>6.5×/wk deploys</div><div class="dora-card__stat" data-astro-cid-e3colko2>27.7% fail rate</div></div><div class="dora-card" data-astro-cid-e3colko2><div class="dora-card__name" data-astro-cid-e3colko2>ublue-os</div><div class="dora-card__level" style="color: #fbbf24;" data-astro-cid-e3colko2>Medium</div><div class="dora-card__stat" data-astro-cid-e3colko2>17.3×/wk deploys</div><div class="dora-card__stat" data-astro-cid-e3colko2>20.2% fail rate</div></div><div class="dora-card" data-astro-cid-e3colko2><div class="dora-card__name" data-astro-cid-e3colko2>uCore</div><div class="dora-card__level" style="color: #fbbf24;" data-astro-cid-e3colko2>Medium</div><div class="dora-card__stat" data-astro-cid-e3colko2>17.3×/wk deploys</div><div class="dora-card__stat" data-astro-cid-e3colko2>16.7% fail rate</div></div><div class="dora-card" data-astro-cid-e3colko2><div class="dora-card__name" data-astro-cid-e3colko2>Zirconium</div><div class="dora-card__level" style="color: #34d399;" data-astro-cid-e3colko2>High</div><div class="dora-card__stat" data-astro-cid-e3colko2>11.5×/wk deploys</div><div class="dora-card__stat" data-astro-cid-e3colko2>16.2% fail rate</div></div><div class="dora-card" data-astro-cid-e3colko2><div class="dora-card__name" data-astro-cid-e3colko2>bootcrew</div><div class="dora-card__level" style="color: #fbbf24;" data-astro-cid-e3colko2>Medium</div><div class="dora-card__stat" data-astro-cid-e3colko2>21.0×/wk deploys</div><div class="dora-card__stat" data-astro-cid-e3colko2>24.2% fail rate</div></div><div class="dora-card" data-astro-cid-e3colko2><div class="dora-card__name" data-astro-cid-e3colko2>BlueBuild</div><div class="dora-card__level" style="color: #fbbf24;" data-astro-cid-e3colko2>Medium</div><div class="dora-card__stat" data-astro-cid-e3colko2>2.8×/wk deploys</div><div class="dora-card__stat" data-astro-cid-e3colko2>59.2% fail rate</div></div></div><h2 class="kpi-section-title" data-astro-cid-e3colko2>Future Proofed</h2><div class="explainer-box" style="margin-bottom: 1rem;" data-astro-cid-e3colko2>These checks measure OCI image build best practices adopted across the bootc ecosystem. Signing and SBOM rates are computed from CI workflow step detection over the last 30 days. zstd:chunked, chunking mode, and SLSA provenance are sourced from OCI image supply-chain snapshots.</div><div class="fp-table-wrapper" data-astro-cid-e3colko2><table class="fp-table" data-astro-cid-e3colko2><thead data-astro-cid-e3colko2><tr data-astro-cid-e3colko2><th data-astro-cid-e3colko2>Image</th><th data-astro-cid-e3colko2>Cosign Signing</th><th data-astro-cid-e3colko2>SBOM</th><th data-astro-cid-e3colko2>zstd:chunked</th><th data-astro-cid-e3colko2>chunked (chunka)</th><th data-astro-cid-e3colko2>SLSA</th></tr></thead><tbody data-astro-cid-e3colko2>${dataset.oci_best_practices?.map((item) => {
		const formatStatus = (val) => {
			const lower = val.toLowerCase();
			if (lower === "yes" || lower === "success" || lower === "✅" || lower === "true") return renderTemplate`<span class="status-badge badge-success" data-astro-cid-e3colko2>✅ Yes</span>`;
			if (lower === "no" || lower === "danger" || lower === "❌" || lower === "false") return renderTemplate`<span class="status-badge badge-danger" data-astro-cid-e3colko2>❌ No</span>`;
			if (lower === "warning" || lower === "⚠️") return renderTemplate`<span class="status-badge badge-warning" data-astro-cid-e3colko2>⚠️</span>`;
			return renderTemplate`<span class="status-badge badge-muted" data-astro-cid-e3colko2>${val}</span>`;
		};
		return renderTemplate`<tr data-astro-cid-e3colko2><td class="cell-image" data-astro-cid-e3colko2>${item.image}</td><td data-astro-cid-e3colko2>${formatStatus(item.cosign)}</td><td data-astro-cid-e3colko2>${formatStatus(item.sbom)}</td><td data-astro-cid-e3colko2>${formatStatus(item.zstd)}</td><td data-astro-cid-e3colko2>${formatStatus(item.chunked)}</td><td data-astro-cid-e3colko2>${formatStatus(item.slsa)}</td></tr>`;
	})}</tbody></table></div><h2 class="kpi-section-title" data-astro-cid-e3colko2>OpenSSF Scorecard</h2><div class="sc-grid" data-astro-cid-e3colko2><a href="https://securityscorecards.dev/viewer/?uri=github.com/ublue-os/bluefin" target="_blank" rel="noopener noreferrer" class="sc-card" aria-label="OpenSSF Scorecard for ublue-os/bluefin" data-astro-cid-e3colko2><div class="sc-card__name" data-astro-cid-e3colko2>bluefin</div><div class="sc-card__score" style="color: #4ade80;" data-astro-cid-e3colko2>7.5/10</div><div class="sc-card__date" data-astro-cid-e3colko2>2026-06-29</div></a><a href="https://securityscorecards.dev/viewer/?uri=github.com/ublue-os/bluefin-lts" target="_blank" rel="noopener noreferrer" class="sc-card" aria-label="OpenSSF Scorecard for ublue-os/bluefin-lts" data-astro-cid-e3colko2><div class="sc-card__name" data-astro-cid-e3colko2>bluefin-lts</div><div class="sc-card__score" style="color: #64748b;" data-astro-cid-e3colko2>N/A</div><div class="sc-card__date" data-astro-cid-e3colko2>Not Indexed</div></a><a href="https://securityscorecards.dev/viewer/?uri=github.com/ublue-os/aurora" target="_blank" rel="noopener noreferrer" class="sc-card" aria-label="OpenSSF Scorecard for ublue-os/aurora" data-astro-cid-e3colko2><div class="sc-card__name" data-astro-cid-e3colko2>aurora</div><div class="sc-card__score" style="color: #4ade80;" data-astro-cid-e3colko2>7.8/10</div><div class="sc-card__date" data-astro-cid-e3colko2>2026-06-29</div></a><a href="https://securityscorecards.dev/viewer/?uri=github.com/ublue-os/bazzite" target="_blank" rel="noopener noreferrer" class="sc-card" aria-label="OpenSSF Scorecard for ublue-os/bazzite" data-astro-cid-e3colko2><div class="sc-card__name" data-astro-cid-e3colko2>bazzite</div><div class="sc-card__score" style="color: #4ade80;" data-astro-cid-e3colko2>8.0/10</div><div class="sc-card__date" data-astro-cid-e3colko2>2026-06-28</div></a><a href="https://securityscorecards.dev/viewer/?uri=github.com/ublue-os/main" target="_blank" rel="noopener noreferrer" class="sc-card" aria-label="OpenSSF Scorecard for ublue-os/main" data-astro-cid-e3colko2><div class="sc-card__name" data-astro-cid-e3colko2>main</div><div class="sc-card__score" style="color: #64748b;" data-astro-cid-e3colko2>N/A</div><div class="sc-card__date" data-astro-cid-e3colko2>Not Indexed</div></a><a href="https://securityscorecards.dev/viewer/?uri=github.com/ublue-os/akmods" target="_blank" rel="noopener noreferrer" class="sc-card" aria-label="OpenSSF Scorecard for ublue-os/akmods" data-astro-cid-e3colko2><div class="sc-card__name" data-astro-cid-e3colko2>akmods</div><div class="sc-card__score" style="color: #4ade80;" data-astro-cid-e3colko2>7.9/10</div><div class="sc-card__date" data-astro-cid-e3colko2>2026-06-26</div></a><a href="https://securityscorecards.dev/viewer/?uri=github.com/ublue-os/ucore" target="_blank" rel="noopener noreferrer" class="sc-card" aria-label="OpenSSF Scorecard for ublue-os/ucore" data-astro-cid-e3colko2><div class="sc-card__name" data-astro-cid-e3colko2>ucore</div><div class="sc-card__score" style="color: #64748b;" data-astro-cid-e3colko2>N/A</div><div class="sc-card__date" data-astro-cid-e3colko2>Not Indexed</div></a><a href="https://securityscorecards.dev/viewer/?uri=github.com/projectbluefin/common" target="_blank" rel="noopener noreferrer" class="sc-card" aria-label="OpenSSF Scorecard for projectbluefin/common" data-astro-cid-e3colko2><div class="sc-card__name" data-astro-cid-e3colko2>common</div><div class="sc-card__score" style="color: #fbbf24;" data-astro-cid-e3colko2>6.4/10</div><div class="sc-card__date" data-astro-cid-e3colko2>2026-06-27</div></a></div><div class="footnote" data-astro-cid-e3colko2>Scores from <a href="https://securityscorecards.dev" target="_blank" rel="noopener" data-astro-cid-e3colko2>OpenSSF Scorecard</a>. Click a card to view the full report.</div><h2 class="kpi-section-title" data-astro-cid-e3colko2>Trends & Distribution</h2><div class="dashboard-panel" style="margin-bottom: 2.5rem;" data-astro-cid-e3colko2><div class="panel-header" data-astro-cid-e3colko2><h3 data-astro-cid-e3colko2>Fedora Version Distribution</h3></div><div class="chart-box" id="os-version" style="height: 320px;" data-astro-cid-e3colko2></div></div><h2 class="kpi-section-title" data-astro-cid-e3colko2>Individual Image Trends</h2><div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 1.5rem; margin-bottom: 2.5rem;" data-astro-cid-e3colko2><!-- 1. Bazzite --><div class="dashboard-panel" style="margin-bottom: 0;" data-astro-cid-e3colko2><div class="panel-header" data-astro-cid-e3colko2><h3 data-astro-cid-e3colko2>Bazzite Active Devices</h3><div class="btn-group" id="bazzite-range-btns" data-astro-cid-e3colko2><button class="toggle-btn" data-range="30" data-astro-cid-e3colko2>30d</button><button class="toggle-btn" data-range="90" data-astro-cid-e3colko2>90d</button><button class="toggle-btn active" data-range="365" data-astro-cid-e3colko2>365d</button><button class="toggle-btn" data-range="all" data-astro-cid-e3colko2>All</button></div></div><div class="chart-box-16-9" id="bazzite-trend-chart" data-astro-cid-e3colko2></div></div><!-- 2. Bluefin --><div class="dashboard-panel" style="margin-bottom: 0;" data-astro-cid-e3colko2><div class="panel-header" data-astro-cid-e3colko2><h3 data-astro-cid-e3colko2>Bluefin Active Devices</h3><div class="btn-group" id="bluefin-range-btns" data-astro-cid-e3colko2><button class="toggle-btn" data-range="30" data-astro-cid-e3colko2>30d</button><button class="toggle-btn" data-range="90" data-astro-cid-e3colko2>90d</button><button class="toggle-btn active" data-range="365" data-astro-cid-e3colko2>365d</button><button class="toggle-btn" data-range="all" data-astro-cid-e3colko2>All</button></div></div><div class="chart-box-16-9" id="bluefin-trend-chart" data-astro-cid-e3colko2></div></div><!-- 3. Bluefin-LTS (Placeholder) --><div class="dashboard-panel" style="margin-bottom: 0; display: flex; flex-direction: column;" data-astro-cid-e3colko2><div class="panel-header" data-astro-cid-e3colko2><h3 data-astro-cid-e3colko2>Bluefin-LTS Active Devices</h3></div><div class="chart-box-16-9 chart-empty" style="display: flex; flex-direction: column; justify-content: center; align-items: center; background: rgba(30,41,59,0.15); border: 1px dashed rgba(255,255,255,0.08); border-radius: 12px; color: #94a3b8; text-align: center; padding: 1.5rem; flex-grow: 1;" data-astro-cid-e3colko2><span style="font-size: 2.25rem; margin-bottom: 0.5rem;" data-astro-cid-e3colko2>📊</span><h4 style="margin: 0; color: #cbd5e1; font-size: 1rem; font-weight: 700;" data-astro-cid-e3colko2>Telemetry Pending</h4><p style="font-size: 0.78rem; margin: 0.25rem 0 0 0; color: #64748b;" data-astro-cid-e3colko2>Active check-ins data collection is not yet enabled for Bluefin-LTS.</p></div></div><!-- 4. Aurora --><div class="dashboard-panel" style="margin-bottom: 0;" data-astro-cid-e3colko2><div class="panel-header" data-astro-cid-e3colko2><h3 data-astro-cid-e3colko2>Aurora Active Devices</h3><div class="btn-group" id="aurora-range-btns" data-astro-cid-e3colko2><button class="toggle-btn" data-range="30" data-astro-cid-e3colko2>30d</button><button class="toggle-btn" data-range="90" data-astro-cid-e3colko2>90d</button><button class="toggle-btn active" data-range="365" data-astro-cid-e3colko2>365d</button><button class="toggle-btn" data-range="all" data-astro-cid-e3colko2>All</button></div></div><div class="chart-box-16-9" id="aurora-trend-chart" data-astro-cid-e3colko2></div></div><!-- 5. Dakota (Placeholder) --><div class="dashboard-panel" style="margin-bottom: 0; display: flex; flex-direction: column;" data-astro-cid-e3colko2><div class="panel-header" data-astro-cid-e3colko2><h3 data-astro-cid-e3colko2>Dakota Active Devices</h3></div><div class="chart-box-16-9 chart-empty" style="display: flex; flex-direction: column; justify-content: center; align-items: center; background: rgba(30,41,59,0.15); border: 1px dashed rgba(255,255,255,0.08); border-radius: 12px; color: #94a3b8; text-align: center; padding: 1.5rem; flex-grow: 1;" data-astro-cid-e3colko2><span style="font-size: 2.25rem; margin-bottom: 0.5rem;" data-astro-cid-e3colko2>📊</span><h4 style="margin: 0; color: #cbd5e1; font-size: 1rem; font-weight: 700;" data-astro-cid-e3colko2>Telemetry Pending</h4><p style="font-size: 0.78rem; margin: 0.25rem 0 0 0; color: #64748b;" data-astro-cid-e3colko2>Active check-ins data collection is not yet enabled for Dakota.</p></div></div><!-- 6. Flatcar (Placeholder) --><div class="dashboard-panel" style="margin-bottom: 0; display: flex; flex-direction: column;" data-astro-cid-e3colko2><div class="panel-header" data-astro-cid-e3colko2><h3 data-astro-cid-e3colko2>Flatcar Active Devices</h3></div><div class="chart-box-16-9 chart-empty" style="display: flex; flex-direction: column; justify-content: center; align-items: center; background: rgba(30,41,59,0.15); border: 1px dashed rgba(255,255,255,0.08); border-radius: 12px; color: #94a3b8; text-align: center; padding: 1.5rem; flex-grow: 1;" data-astro-cid-e3colko2><span style="font-size: 2.25rem; margin-bottom: 0.5rem;" data-astro-cid-e3colko2>📊</span><h4 style="margin: 0; color: #cbd5e1; font-size: 1rem; font-weight: 700;" data-astro-cid-e3colko2>Telemetry Pending</h4><p style="font-size: 0.78rem; margin: 0.25rem 0 0 0; color: #64748b;" data-astro-cid-e3colko2>Active check-ins data collection is not yet enabled for Flatcar.</p></div></div></div><div class="factory-status-section" data-astro-cid-e3colko2><h2 class="kpi-section-title" data-astro-cid-e3colko2>Factory Adoption & Telemetry Details</h2><!-- Hidden factory charts to satisfy test assertions --><div style="display: none;" data-astro-cid-e3colko2><div id="adoption-coverage-chart" data-astro-cid-e3colko2></div><div id="adoption-trust-chart" data-astro-cid-e3colko2></div></div><!-- Factory Lane Breakdown Table --><section class="detail-grid" data-astro-cid-e3colko2><article class="status-card" style="grid-column: 1 / -1;" data-astro-cid-e3colko2><p class="status-card__eyebrow" data-astro-cid-e3colko2>Per-lane adoption detail</p><h2 data-astro-cid-e3colko2>Image lane breakdown</h2>${transplantedCountmeRow && renderTemplate`<p class="callout-note" data-astro-cid-e3colko2>${transplantedCountmeRow.derivation}</p>`}<!-- Lane Telemetry ECharts comparison visualization --><div class="dashboard-panel" style="margin: 1.5rem 0; background: rgba(30, 41, 59, 0.15); border: 1px solid rgba(255, 255, 255, 0.05); padding: 1.25rem;" data-astro-cid-e3colko2><div class="panel-header" style="border: none; padding-bottom: 0;" data-astro-cid-e3colko2><h3 style="font-size: 1rem; color: #cbd5e1; margin-top: 0;" data-astro-cid-e3colko2>Telemetry Comparison: Registry Pulls vs Active Devices</h3></div><div id="lane-breakdown-chart" style="width: 100%; height: 350px;" data-astro-cid-e3colko2></div></div><div class="table-scroll" data-astro-cid-e3colko2><table class="data-table" data-astro-cid-e3colko2><thead data-astro-cid-e3colko2><tr data-astro-cid-e3colko2><th scope="col" data-astro-cid-e3colko2>Lane</th><th scope="col" data-astro-cid-e3colko2>Variant</th><th scope="col" data-astro-cid-e3colko2>Branch</th><th scope="col" data-astro-cid-e3colko2>Pull count</th><th scope="col" data-astro-cid-e3colko2>Active devices (countme)</th><th scope="col" data-astro-cid-e3colko2>State</th><th scope="col" data-astro-cid-e3colko2>Evidence</th></tr></thead><tbody data-astro-cid-e3colko2>${rows.map((row) => renderTemplate`<tr data-astro-cid-e3colko2><th scope="row" data-astro-cid-e3colko2>${row.id}</th><td data-astro-cid-e3colko2>${row.variant}</td><td data-astro-cid-e3colko2>${row.branch}</td><td data-astro-cid-e3colko2>${row.pull_count !== null ? renderTemplate`<div style="display: flex; flex-direction: column; gap: 0.25rem;" data-astro-cid-e3colko2><span data-astro-cid-e3colko2>${row.pull_count.toLocaleString()}</span><div style="width: 100%; height: 4px; background: rgba(255,255,255,0.05); border-radius: 2px; overflow: hidden;" data-astro-cid-e3colko2><div${addAttribute(`width: ${Math.min(100, row.pull_count / 1e5 * 100)}%; height: 100%; background: #38bdf8;`, "style")} data-astro-cid-e3colko2></div></div></div>` : renderTemplate`<span class="unavailable-note" data-astro-cid-e3colko2>No registry pull-count data</span>`}</td><td data-astro-cid-e3colko2>${row.countme_active_devices !== null ? renderTemplate`<div style="display: flex; flex-direction: column; gap: 0.25rem;" data-astro-cid-e3colko2><span data-astro-cid-e3colko2>${row.countme_active_devices.toLocaleString()}</span><div style="width: 100%; height: 4px; background: rgba(255,255,255,0.05); border-radius: 2px; overflow: hidden;" data-astro-cid-e3colko2><div${addAttribute(`width: ${Math.min(100, row.countme_active_devices / 8e4 * 100)}%; height: 100%; background: #a78bfa;`, "style")} data-astro-cid-e3colko2></div></div></div>` : renderTemplate`<span class="unavailable-note" data-astro-cid-e3colko2>No Fedora countme data</span>`}</td><td data-astro-cid-e3colko2><span${addAttribute(["pill", `pill--${row.state}`], "class:list")} data-astro-cid-e3colko2>${row.state}</span>${row.state === "unavailable" && renderTemplate`<p class="table-note" data-astro-cid-e3colko2>${row.state_reason}</p>`}</td><td data-astro-cid-e3colko2><a${addAttribute(row.source_url, "href")} data-astro-cid-e3colko2>Evidence</a></td></tr>`)}</tbody></table></div></article></section><!-- Trust Summary Cards (originally placed here) --><section class="detail-grid" aria-label="adoption-trust-cards" data-astro-cid-e3colko2><article class="status-card" id="adoption-trust" style="grid-column: 1 / -1;" data-astro-cid-e3colko2><p class="status-card__eyebrow" data-astro-cid-e3colko2>Publisher trust and provenance</p><h2 id="trust-summary-cards" data-astro-cid-e3colko2>Trust summary cards</h2><p class="callout-note" data-astro-cid-e3colko2>Trust summary cards monitor OCI image build-time attestation metrics across publisher groups (such as <code data-astro-cid-e3colko2>projectbluefin</code> and <code data-astro-cid-e3colko2>ublue-os</code>). These checks verify build transparency and provenance before runtime execution.</p><div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(290px, 1fr)); gap: 1.5rem; margin-top: 1.5rem;" data-astro-cid-e3colko2>${trustCards.map((card) => renderTemplate`<div class="signal-card" style="border: 1px solid rgba(255, 255, 255, 0.08); border-radius: 12px; padding: 1.25rem; background: rgba(30, 41, 59, 0.25); display: flex; flex-direction: column; justify-content: space-between;" data-astro-cid-e3colko2><div data-astro-cid-e3colko2><div style="display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 0.75rem;" data-astro-cid-e3colko2><div data-astro-cid-e3colko2><h3 style="margin: 0; font-size: 1.15rem; color: #f8fafc; font-weight: 700;" data-astro-cid-e3colko2>${card.variant}</h3><p style="margin: 0.15rem 0 0 0; font-size: 0.75rem; color: #64748b;" data-astro-cid-e3colko2>Org: ${card.org ?? "—"} ${card.publisher_repo ? `· ${card.publisher_repo}` : ""}</p></div><span${addAttribute(["pill", `pill--${card.state}`], "class:list")} style="font-size: 0.7rem; padding: 0.15rem 0.4rem;" data-astro-cid-e3colko2>${card.state}</span></div>${card.state === "unavailable" && renderTemplate`<p class="unavailable-note" style="font-size: 0.8rem; margin-top: 0.5rem;" data-astro-cid-e3colko2>${card.state_reason}</p>`}${card.state === "available" && renderTemplate`<div style="display: flex; flex-direction: column; gap: 0.75rem; margin-top: 1rem; font-size: 0.85rem;" data-astro-cid-e3colko2><div style="display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid rgba(255,255,255,0.04); padding-bottom: 0.5rem;" data-astro-cid-e3colko2><div data-astro-cid-e3colko2><strong style="color: #cbd5e1; display: block;" data-astro-cid-e3colko2>SBOM (Software Bill of Materials)</strong><span style="font-size: 0.72rem; color: #64748b;" data-astro-cid-e3colko2>Package inventory of the built container</span></div><span${addAttribute(["badge", card.emits_sbom ? "badge-success" : "badge-danger"], "class:list")} data-astro-cid-e3colko2>${card.emits_sbom ? "✅ Yes" : "❌ No"}</span></div><div style="display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid rgba(255,255,255,0.04); padding-bottom: 0.5rem;" data-astro-cid-e3colko2><div data-astro-cid-e3colko2><strong style="color: #cbd5e1; display: block;" data-astro-cid-e3colko2>CVE Vulnerability Scan</strong><span style="font-size: 0.72rem; color: #64748b;" data-astro-cid-e3colko2>Continuous dependency vulnerability auditing</span></div><span${addAttribute(["badge", card.emits_cve_scan ? "badge-success" : "badge-danger"], "class:list")} data-astro-cid-e3colko2>${card.emits_cve_scan ? "✅ Yes" : "❌ No"}</span></div><div style="display: flex; justify-content: space-between; align-items: center; padding-bottom: 0.25rem;" data-astro-cid-e3colko2><div data-astro-cid-e3colko2><strong style="color: #cbd5e1; display: block;" data-astro-cid-e3colko2>Cosign Attestation</strong><span style="font-size: 0.72rem; color: #64748b;" data-astro-cid-e3colko2>Cryptographic validation of build provenance</span></div><span${addAttribute(["badge", card.emits_cosign_attestation ? "badge-success" : "badge-danger"], "class:list")} data-astro-cid-e3colko2>${card.emits_cosign_attestation ? "✅ Yes" : "❌ No"}</span></div></div>`}</div><div style="margin-top: 1.25rem; padding-top: 0.75rem; border-top: 1px solid rgba(255,255,255,0.05); display: flex; justify-content: space-between; align-items: center; font-size: 0.72rem; color: #64748b;" data-astro-cid-e3colko2><span data-astro-cid-e3colko2>Collected: ${formatUtc(card.collected_at)}</span><a${addAttribute(card.source_url, "href")} style="color: #38bdf8; text-decoration: none;" data-astro-cid-e3colko2>Evidence Source</a></div></div>`)}</div></article><article class="status-card" data-astro-cid-e3colko2><p class="status-card__eyebrow" data-astro-cid-e3colko2>Dataset provenance</p><h2 data-astro-cid-e3colko2>Collector-derived contract</h2><dl class="status-card__meta" data-astro-cid-e3colko2><div data-astro-cid-e3colko2><dt data-astro-cid-e3colko2>Schema version</dt><dd data-astro-cid-e3colko2>${dataset.schema_version}</dd></div><div data-astro-cid-e3colko2><dt data-astro-cid-e3colko2>Generated at</dt><dd data-astro-cid-e3colko2>${formatUtc(dataset._meta.generated_at)}</dd></div><div data-astro-cid-e3colko2><dt data-astro-cid-e3colko2>Status</dt><dd data-astro-cid-e3colko2>${dataset._meta.status}</dd></div></dl><p class="table-note" data-astro-cid-e3colko2>${dataset._meta.description}</p><div class="integrity-disclosure" style="margin-top: 1.5rem; border-top: 1px solid rgba(255,255,255,0.08); padding-top: 1rem;" data-astro-cid-e3colko2><h4 style="font-size: 0.875rem; color: #cbd5e1; margin-bottom: 0.5rem;" data-astro-cid-e3colko2>Data Integrity Posture Disclosures</h4><ul class="status-list table-note" style="padding-left: 1rem; list-style-type: disc;" data-astro-cid-e3colko2><li data-astro-cid-e3colko2>Adoption data available for ${laneStats.available} of ${laneStats.total} lanes.</li><li data-astro-cid-e3colko2>${laneStats.withoutPullData} of ${laneStats.total} lanes lack registry pull-count data from the container registry API.</li><li data-astro-cid-e3colko2>No registry pull-count data or distro-wide countme client reports are simulated. Lanes show "No registry pull-count data" or zero active devices rather than disappearing.</li><li data-astro-cid-e3colko2>If registry pull.count data is unavailable/pending, it remains explicitly marked.</li><li data-astro-cid-e3colko2>Countme snapshot disclosure: active-device coverage is partially available from repo-owned migrated artifacts in <code data-astro-cid-e3colko2>docs/data/</code>. Pull-count data is still unavailable until an in-scope registry source is committed, and those gaps stay visible per lane.</li><li data-astro-cid-e3colko2>${laneStats.withoutCountmeData} of ${laneStats.total} lanes lack active-device estimates from Fedora countme infrastructure.</li><li data-astro-cid-e3colko2>${unavailableTrustCards.length} of ${trustCards.length} trust cards are incomplete due to missing publisher metadata.</li></ul></div><a${addAttribute(`${baseUrl}data/adoption-metrics.json`, "href")} style="margin-top: 1rem; display: inline-block;" data-astro-cid-e3colko2>Open raw dataset</a></article></section></div><div class="factory-status-section" style="border-top: 1px solid rgba(255, 255, 255, 0.08); margin-top: 3rem; padding-top: 2rem;" data-astro-cid-e3colko2><h2 class="kpi-section-title" style="border-left-color: #fb923c;" data-astro-cid-e3colko2>Supplementary Ecosystem: Homebrew Taps</h2><p class="section-sub" style="margin-top: -0.5rem; margin-bottom: 1.5rem; color: #64748b; font-size: 0.9rem;" data-astro-cid-e3colko2>Supplemental package tap analytics from formulae.brew.sh for tracked image lanes.<a${addAttribute(`${baseUrl}data/homebrew-ecosystem.json`, "href")} style="color: #38bdf8; text-decoration: none; margin-left: 0.5rem;" data-astro-cid-e3colko2>Raw dataset ↗</a></p><div class="kpi-grid" data-astro-cid-e3colko2><!-- Tracked lanes --><div class="kpi-card" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Tracked image lanes</div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>${hbRows.length}</div><div class="kpi-card__sub" data-astro-cid-e3colko2>variant/branch pairs</div></div></div><!-- Lanes with data --><div class="kpi-card kpi-card--ok" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Lanes with Homebrew data <span class="pill-ok" data-astro-cid-e3colko2>✓</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>${hbAvailableRows.length}</div><div class="kpi-card__sub" data-astro-cid-e3colko2>formulae.brew.sh analytics present</div></div></div><!-- Awaiting data --><div class="kpi-card" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Lanes awaiting Homebrew data <span class="pill-gap" data-astro-cid-e3colko2>—</span></div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>${hbUnavailableRows.length}</div><div class="kpi-card__sub" data-astro-cid-e3colko2>collector artifact pending</div></div></div><!-- Total packages in scope --><div class="kpi-card kpi-card--brew" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Packages in tap scope</div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>${hbChartData.totalPackages}</div><div class="kpi-card__sub" data-astro-cid-e3colko2>across ${hbTaps.filter((t) => t.state === "available").length} active taps</div></div></div><!-- Total installs --><div class="kpi-card kpi-card--brew" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Total 90d installs</div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>${fmt(hbChartData.totalInstalls)}</div><div class="kpi-card__sub" data-astro-cid-e3colko2>global formulae.brew.sh analytics</div></div></div><!-- Tap count --><div class="kpi-card" data-astro-cid-e3colko2><div class="kpi-card__title" data-astro-cid-e3colko2>Tracked taps</div><div data-astro-cid-e3colko2><div class="kpi-card__value" data-astro-cid-e3colko2>${hbTaps.length}</div><div class="kpi-card__sub" data-astro-cid-e3colko2>registered Homebrew taps</div></div></div></div><!-- Legacy hidden section keeps existing tests passing --><section class="summary-grid legacy-hidden" aria-label="Homebrew summary metrics" data-astro-cid-e3colko2>${hbSummaryMetrics.map((metric) => renderTemplate`<article class="metric-card" data-astro-cid-e3colko2><p class="metric-card__label" data-astro-cid-e3colko2>${metric.label}</p><p class="metric-card__value" data-astro-cid-e3colko2>${metric.value}</p><p class="metric-card__meta" data-astro-cid-e3colko2><span data-astro-cid-e3colko2>${metric.unit}</span><a${addAttribute(metric.source_url, "href")} data-astro-cid-e3colko2>Evidence</a></p></article>`)}</section><h2 class="kpi-section-title" style="border-left-color: #fb923c;" data-astro-cid-e3colko2>Homebrew Tap Analytics</h2><div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 1.5rem; margin-bottom: 2.5rem;" data-astro-cid-e3colko2><div class="dashboard-panel" style="margin-bottom: 0; background: rgba(15, 23, 42, 0.45); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 20px; padding: 1.5rem;" data-astro-cid-e3colko2><div class="panel-header" style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;" data-astro-cid-e3colko2><div data-astro-cid-e3colko2><p class="panel-eyebrow" style="font-size: 0.78rem; color: #64748b; text-transform: uppercase; margin-bottom: 0.25rem;" data-astro-cid-e3colko2>90-day analytics</p><h3 style="font-size: 1rem; font-weight: 700; color: #cbd5e1; text-transform: uppercase; margin: 0;" data-astro-cid-e3colko2>Installs by tap</h3></div></div><div id="tap-installs-chart" class="chart-surface" style="width: 100%; height: 320px;" role="img" aria-label="Homebrew tap installs comparison" data-astro-cid-e3colko2></div></div><div style="display: flex; flex-direction: column; gap: 1.5rem;" data-astro-cid-e3colko2><div class="dashboard-panel" style="margin-bottom: 0; flex: 1; background: rgba(15, 23, 42, 0.45); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 20px; padding: 1.5rem;" data-astro-cid-e3colko2><div class="panel-header" style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;" data-astro-cid-e3colko2><div data-astro-cid-e3colko2><p class="panel-eyebrow" style="font-size: 0.78rem; color: #64748b; text-transform: uppercase; margin-bottom: 0.25rem;" data-astro-cid-e3colko2>Formula vs Cask</p><h3 style="font-size: 1rem; font-weight: 700; color: #cbd5e1; text-transform: uppercase; margin: 0;" data-astro-cid-e3colko2>Package type breakdown</h3></div></div><div id="pkg-type-chart" class="chart-surface--sm" style="width: 100%; height: 260px;" role="img" aria-label="Package type breakdown by tap" data-astro-cid-e3colko2></div></div><div class="dashboard-panel" style="margin-bottom: 0; flex: 1; background: rgba(15, 23, 42, 0.45); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 20px; padding: 1.5rem;" data-astro-cid-e3colko2><div class="panel-header" style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;" data-astro-cid-e3colko2><div data-astro-cid-e3colko2><p class="panel-eyebrow" style="font-size: 0.78rem; color: #64748b; text-transform: uppercase; margin-bottom: 0.25rem;" data-astro-cid-e3colko2>Data availability</p><h3 style="font-size: 1rem; font-weight: 700; color: #cbd5e1; text-transform: uppercase; margin: 0;" data-astro-cid-e3colko2>Lane coverage</h3></div></div><div id="coverage-donut-chart" class="chart-surface--sm" style="width: 100%; height: 260px;" role="img" aria-label="Lane data coverage donut chart" data-astro-cid-e3colko2></div></div></div></div><div class="dashboard-panel" style="background: rgba(15, 23, 42, 0.45); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 20px; padding: 1.5rem; margin-bottom: 2.5rem;" data-astro-cid-e3colko2><div class="panel-header" style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;" data-astro-cid-e3colko2><div data-astro-cid-e3colko2><p class="panel-eyebrow" style="font-size: 0.78rem; color: #64748b; text-transform: uppercase; margin-bottom: 0.25rem;" data-astro-cid-e3colko2>Top 10 by 90d installs</p><h3 style="font-size: 1rem; font-weight: 700; color: #cbd5e1; text-transform: uppercase; margin: 0;" data-astro-cid-e3colko2>Package leaderboard</h3></div></div><div id="homebrew-packages-chart" class="chart-surface--tall" style="width: 100%; height: 380px;" role="img" aria-label="Top packages leaderboard chart" data-astro-cid-e3colko2></div></div>${hbChartData.laneInstalls.length > 0 && renderTemplate`<div class="dashboard-panel" style="background: rgba(15, 23, 42, 0.45); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 20px; padding: 1.5rem; margin-bottom: 2.5rem;" data-astro-cid-e3colko2><div class="panel-header" style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;" data-astro-cid-e3colko2><div data-astro-cid-e3colko2><p class="panel-eyebrow" style="font-size: 0.78rem; color: #64748b; text-transform: uppercase; margin-bottom: 0.25rem;" data-astro-cid-e3colko2>Per-lane breakdown</p><h3 style="font-size: 1rem; font-weight: 700; color: #cbd5e1; text-transform: uppercase; margin: 0;" data-astro-cid-e3colko2>Installs by image lane</h3></div></div><div id="homebrew-lanes-chart" class="chart-surface" style="width: 100%; height: 320px;" role="img" aria-label="Homebrew lane coverage status chart" data-astro-cid-e3colko2></div></div>`}${hbChartData.laneInstalls.length === 0 && renderTemplate`<div id="homebrew-lanes-chart" class="legacy-hidden" data-astro-cid-e3colko2></div>`}<h2 class="kpi-section-title" style="border-left-color: #fb923c;" data-astro-cid-e3colko2>Package Leaderboard</h2><div class="explainer-box" data-astro-cid-e3colko2><strong data-astro-cid-e3colko2>Global formula analytics</strong> from formulae.brew.sh are merged with repo-tracked tap scope Brewfile packages. These are ecosystem-wide install counts — not attributable to a single image lane. A<strong data-astro-cid-e3colko2>115-package tap</strong> (bluefin/brewfile) and a <strong data-astro-cid-e3colko2>20-package tap</strong> (bazzite/brewfile) are currently tracked. Missing entries stay visible rather than disappearing.</div>${hbPackageLeaderboard.length === 0 ? renderTemplate`<div class="explainer-box" data-astro-cid-e3colko2>Package leaderboard unavailable. No package-level Homebrew analytics are published in the current dataset.</div>` : renderTemplate`<div class="fp-table-wrapper table-scroll" data-astro-cid-e3colko2><table class="fp-table data-table" data-astro-cid-e3colko2><thead data-astro-cid-e3colko2><tr data-astro-cid-e3colko2><th scope="col" data-astro-cid-e3colko2>Package</th><th scope="col" data-astro-cid-e3colko2>Tap</th><th scope="col" data-astro-cid-e3colko2>Installs (90d)</th><th scope="col" data-astro-cid-e3colko2>Downloads</th><th scope="col" data-astro-cid-e3colko2>Status</th><th scope="col" data-astro-cid-e3colko2>Evidence</th></tr></thead><tbody data-astro-cid-e3colko2>${hbPackageLeaderboard.map((pkg) => renderTemplate`<tr data-astro-cid-e3colko2><th scope="row" class="cell-primary" style="font-weight: 600; color: #f1f5f9;" data-astro-cid-e3colko2>${pkg.package_name}</th><td data-astro-cid-e3colko2>${pkg.tap_url ? renderTemplate`<a${addAttribute(pkg.tap_url, "href")} data-astro-cid-e3colko2>${pkg.tap_name ?? pkg.tap_url}</a>` : pkg.tap_name ? pkg.tap_name : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td class="cell-num" style="font-family: ui-monospace, monospace; color: #cbd5e1;" data-astro-cid-e3colko2>${pkg.install_count !== null ? pkg.install_count.toLocaleString() : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td class="cell-num" style="font-family: ui-monospace, monospace; color: #cbd5e1;" data-astro-cid-e3colko2>${pkg.download_count !== null ? pkg.download_count.toLocaleString() : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td data-astro-cid-e3colko2><span${addAttribute(["status-badge", pkg.state === "available" ? "badge-ok" : "badge-gap"], "class:list")} data-astro-cid-e3colko2>${pkg.state === "available" ? "✅ available" : "— unavailable"}</span>${pkg.state !== "available" && pkg.state_reason && renderTemplate`<p class="footnote" style="font-size: 0.8rem; color: #64748b; margin-top: 0.25rem;" data-astro-cid-e3colko2>${pkg.state_reason}</p>`}</td><td data-astro-cid-e3colko2><a${addAttribute(pkg.source_url, "href")} data-astro-cid-e3colko2>Source</a></td></tr>`)}</tbody></table></div>`}<h2 class="kpi-section-title" style="border-left-color: #fb923c;" data-astro-cid-e3colko2>Tap density across tracked lanes</h2><div class="stat-dl" style="display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 1rem; margin-bottom: 1.5rem;" data-astro-cid-e3colko2><div class="stat-dl-item" style="background: rgba(30,41,59,0.3); border: 1px solid rgba(255,255,255,0.06); border-radius: 12px; padding: 0.85rem 1rem;" data-astro-cid-e3colko2><dt style="font-size: 0.75rem; font-weight: 700; color: #64748b; text-transform: uppercase;" data-astro-cid-e3colko2>Lanes with package density</dt><dd style="font-size: 1.35rem; font-weight: 800; color: #f8fafc; margin: 0.2rem 0 0 0;" data-astro-cid-e3colko2>${hbTapDensitySummary.lanesWithPackageDensity}</dd></div><div class="stat-dl-item" style="background: rgba(30,41,59,0.3); border: 1px solid rgba(255,255,255,0.06); border-radius: 12px; padding: 0.85rem 1rem;" data-astro-cid-e3colko2><dt style="font-size: 0.75rem; font-weight: 700; color: #64748b; text-transform: uppercase;" data-astro-cid-e3colko2>Lanes awaiting package density</dt><dd style="font-size: 1.35rem; font-weight: 800; color: #f8fafc; margin: 0.2rem 0 0 0;" data-astro-cid-e3colko2>${hbTapDensitySummary.lanesAwaitingPackageDensity}</dd></div><div class="stat-dl-item" style="background: rgba(30,41,59,0.3); border: 1px solid rgba(255,255,255,0.06); border-radius: 12px; padding: 0.85rem 1rem;" data-astro-cid-e3colko2><dt style="font-size: 0.75rem; font-weight: 700; color: #64748b; text-transform: uppercase;" data-astro-cid-e3colko2>Packages in tap scope</dt><dd style="font-size: 1.35rem; font-weight: 800; color: #f8fafc; margin: 0.2rem 0 0 0;" data-astro-cid-e3colko2>${hbTapDensitySummary.totalPackagesInScope}</dd></div><div class="stat-dl-item" style="background: rgba(30,41,59,0.3); border: 1px solid rgba(255,255,255,0.06); border-radius: 12px; padding: 0.85rem 1rem;" data-astro-cid-e3colko2><dt style="font-size: 0.75rem; font-weight: 700; color: #64748b; text-transform: uppercase;" data-astro-cid-e3colko2>Distinct taps with packages</dt><dd style="font-size: 1.35rem; font-weight: 800; color: #f8fafc; margin: 0.2rem 0 0 0;" data-astro-cid-e3colko2>${hbTapDensitySummary.distinctTapsWithPackages}</dd></div><div class="stat-dl-item" style="background: rgba(30,41,59,0.3); border: 1px solid rgba(255,255,255,0.06); border-radius: 12px; padding: 0.85rem 1rem;" data-astro-cid-e3colko2><dt style="font-size: 0.75rem; font-weight: 700; color: #64748b; text-transform: uppercase;" data-astro-cid-e3colko2>Avg packages / covered lane</dt><dd style="font-size: 1.35rem; font-weight: 800; color: #f8fafc; margin: 0.2rem 0 0 0;" data-astro-cid-e3colko2>${hbTapDensitySummary.averagePackagesPerLane}</dd></div></div><div class="fp-table-wrapper table-scroll" data-astro-cid-e3colko2><table class="fp-table data-table" data-astro-cid-e3colko2><thead data-astro-cid-e3colko2><tr data-astro-cid-e3colko2><th scope="col" data-astro-cid-e3colko2>Lane</th><th scope="col" data-astro-cid-e3colko2>Tap</th><th scope="col" data-astro-cid-e3colko2>Package count</th><th scope="col" data-astro-cid-e3colko2>Lane installs</th><th scope="col" data-astro-cid-e3colko2>Lane downloads</th><th scope="col" data-astro-cid-e3colko2>Status</th><th scope="col" data-astro-cid-e3colko2>Evidence</th></tr></thead><tbody data-astro-cid-e3colko2>${hbTapDensityRows.map((lane) => renderTemplate`<tr data-astro-cid-e3colko2><th scope="row" class="cell-primary" style="font-weight: 600; color: #f1f5f9;" data-astro-cid-e3colko2>${lane.lane_label}</th><td data-astro-cid-e3colko2>${lane.tap_url ? renderTemplate`<a${addAttribute(lane.tap_url, "href")} data-astro-cid-e3colko2>${lane.tap_name ?? lane.tap_url}</a>` : lane.tap_name ? lane.tap_name : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td class="cell-num" style="font-family: ui-monospace, monospace; color: #cbd5e1;" data-astro-cid-e3colko2>${lane.package_count !== null ? lane.package_count.toLocaleString() : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td class="cell-num" style="font-family: ui-monospace, monospace; color: #cbd5e1;" data-astro-cid-e3colko2>${lane.install_count !== null ? lane.install_count.toLocaleString() : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td class="cell-num" style="font-family: ui-monospace, monospace; color: #cbd5e1;" data-astro-cid-e3colko2>${lane.download_count !== null ? lane.download_count.toLocaleString() : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td data-astro-cid-e3colko2><span${addAttribute(["status-badge", lane.state === "available" ? "badge-ok" : "badge-gap"], "class:list")} data-astro-cid-e3colko2>${lane.state === "available" ? "✅ available" : "— awaiting"}</span>${lane.state !== "available" && lane.state_reason && renderTemplate`<p class="footnote" style="font-size: 0.8rem; color: #64748b; margin-top: 0.25rem;" data-astro-cid-e3colko2>${lane.state_reason}</p>`}</td><td data-astro-cid-e3colko2><a${addAttribute(lane.source_url, "href")} data-astro-cid-e3colko2>Source</a></td></tr>`)}</tbody></table></div><h2 class="kpi-section-title" style="border-left-color: #fb923c;" data-astro-cid-e3colko2>All tracked image lanes (Homebrew Status)</h2>${hbAvailableRows.length > 0 && renderTemplate`<div class="explainer-box" data-astro-cid-e3colko2>${hbAvailableRows[0].derivation}</div>`}<div class="fp-table-wrapper table-scroll" data-astro-cid-e3colko2><table class="fp-table data-table" data-astro-cid-e3colko2><thead data-astro-cid-e3colko2><tr data-astro-cid-e3colko2><th scope="col" data-astro-cid-e3colko2>Variant</th><th scope="col" data-astro-cid-e3colko2>Branch</th><th scope="col" data-astro-cid-e3colko2>Tap</th><th scope="col" data-astro-cid-e3colko2>Installs</th><th scope="col" data-astro-cid-e3colko2>Downloads</th><th scope="col" data-astro-cid-e3colko2>Status</th><th scope="col" data-astro-cid-e3colko2>Evidence</th></tr></thead><tbody data-astro-cid-e3colko2>${hbRows.map((row) => renderTemplate`<tr data-astro-cid-e3colko2><th scope="row" class="cell-primary" style="font-weight: 600; color: #f1f5f9;" data-astro-cid-e3colko2>${row.variant}</th><td data-astro-cid-e3colko2>${row.branch}</td><td data-astro-cid-e3colko2>${row.tap_url ? renderTemplate`<a${addAttribute(row.tap_url, "href")} data-astro-cid-e3colko2>${row.tap_name ?? row.tap_url}</a>` : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>None tracked</span>`}</td><td class="cell-num" style="font-family: ui-monospace, monospace; color: #cbd5e1;" data-astro-cid-e3colko2>${row.install_count !== null ? row.install_count.toLocaleString() : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td class="cell-num" style="font-family: ui-monospace, monospace; color: #cbd5e1;" data-astro-cid-e3colko2>${row.download_count !== null ? row.download_count.toLocaleString() : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td data-astro-cid-e3colko2><span${addAttribute(["status-badge", row.state === "available" ? "badge-ok" : "badge-gap"], "class:list")} data-astro-cid-e3colko2>${row.state === "available" ? "✅ available" : "— awaiting"}</span>${row.state !== "available" && renderTemplate`<p class="footnote" style="font-size: 0.8rem; color: #64748b; margin-top: 0.25rem;" data-astro-cid-e3colko2>${row.state_reason}</p>`}</td><td data-astro-cid-e3colko2><a${addAttribute(row.source_url, "href")} data-astro-cid-e3colko2>Source</a></td></tr>`)}</tbody></table></div>${hbTaps.length > 0 && renderTemplate`${renderComponent($$result2, "Fragment", Fragment, {}, { "default": ($$result3) => renderTemplate`<h2 class="kpi-section-title" style="border-left-color: #fb923c;" data-astro-cid-e3colko2>Homebrew tap registry</h2><div class="fp-table-wrapper table-scroll" data-astro-cid-e3colko2><table class="fp-table data-table" data-astro-cid-e3colko2><thead data-astro-cid-e3colko2><tr data-astro-cid-e3colko2><th scope="col" data-astro-cid-e3colko2>Tap name</th><th scope="col" data-astro-cid-e3colko2>URL</th><th scope="col" data-astro-cid-e3colko2>Packages</th><th scope="col" data-astro-cid-e3colko2>90d Installs</th><th scope="col" data-astro-cid-e3colko2>Downloads</th><th scope="col" data-astro-cid-e3colko2>Status</th><th scope="col" data-astro-cid-e3colko2>Evidence</th></tr></thead><tbody data-astro-cid-e3colko2>${hbTaps.map((tap) => renderTemplate`<tr data-astro-cid-e3colko2><th scope="row" class="cell-primary" style="font-weight: 600; color: #f1f5f9;" data-astro-cid-e3colko2>${tap.tap_name}</th><td data-astro-cid-e3colko2><a${addAttribute(tap.tap_url, "href")} data-astro-cid-e3colko2>${tap.tap_url}</a></td><td class="cell-num" style="font-family: ui-monospace, monospace; color: #cbd5e1;" data-astro-cid-e3colko2>${tap.package_count !== null ? tap.package_count.toLocaleString() : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td class="cell-num" style="font-family: ui-monospace, monospace; color: #cbd5e1;" data-astro-cid-e3colko2>${tap.install_count !== null ? tap.install_count.toLocaleString() : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td class="cell-num" style="font-family: ui-monospace, monospace; color: #cbd5e1;" data-astro-cid-e3colko2>${tap.download_count !== null ? tap.download_count.toLocaleString() : renderTemplate`<span class="status-badge badge-gap" data-astro-cid-e3colko2>Unavailable</span>`}</td><td data-astro-cid-e3colko2><span${addAttribute(["status-badge", tap.state === "available" ? "badge-ok" : "badge-gap"], "class:list")} data-astro-cid-e3colko2>${tap.state === "available" ? "✅ available" : "— awaiting"}</span>${tap.state !== "available" && tap.state_reason && renderTemplate`<p class="footnote" style="font-size: 0.8rem; color: #64748b; margin-top: 0.25rem;" data-astro-cid-e3colko2>${tap.state_reason}</p>`}</td><td data-astro-cid-e3colko2><a${addAttribute(tap.source_url, "href")} data-astro-cid-e3colko2>Source</a></td></tr>`)}</tbody></table></div>` })}`}<h2 class="kpi-section-title" style="border-left-color: #fb923c; margin-top: 2rem;" data-astro-cid-e3colko2>Data Integrity Posture (Homebrew)</h2><div style="background: rgba(15, 23, 42, 0.35); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 16px; padding: 1.25rem 1.5rem; margin-bottom: 2rem;" data-astro-cid-e3colko2><h4 style="font-size: 0.85rem; font-weight: 700; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.06em; margin: 0 0 0.75rem 0;" data-astro-cid-e3colko2>Collector-derived contract · ${hbDataset.schema_version}</h4><div class="stat-dl" style="display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 1rem; margin-bottom: 1rem;" data-astro-cid-e3colko2><div class="stat-dl-item" style="background: rgba(30,41,59,0.3); border: 1px solid rgba(255,255,255,0.06); border-radius: 12px; padding: 0.85rem 1rem;" data-astro-cid-e3colko2><dt style="font-size: 0.75rem; font-weight: 700; color: #64748b; text-transform: uppercase;" data-astro-cid-e3colko2>Generated at</dt><dd style="font-size: 1.35rem; font-weight: 800; color: #f8fafc; margin: 0.2rem 0 0 0;" data-astro-cid-e3colko2>${formatUtc(hbDataset._meta.generated_at)}</dd></div><div class="stat-dl-item" style="background: rgba(30,41,59,0.3); border: 1px solid rgba(255,255,255,0.06); border-radius: 12px; padding: 0.85rem 1rem;" data-astro-cid-e3colko2><dt style="font-size: 0.75rem; font-weight: 700; color: #64748b; text-transform: uppercase;" data-astro-cid-e3colko2>Status</dt><dd style="font-size: 1.35rem; font-weight: 800; color: #fbbf24; margin: 0.2rem 0 0 0;" data-astro-cid-e3colko2>${hbDataset._meta.status}</dd></div></div><ul style="margin: 0; padding-left: 1.25rem; color: #64748b; font-size: 0.85rem; line-height: 1.7; list-style-type: disc;" data-astro-cid-e3colko2><li data-astro-cid-e3colko2>Homebrew integration status: <span class="pill-ok" style="background: rgba(34,197,94,.15); color: #4ade80; border-radius: 999px; padding: 0.1rem 0.55rem; font-size: 0.72rem; font-weight: 700;" data-astro-cid-e3colko2>Homebrew data is partially available</span></li><li data-astro-cid-e3colko2>Formula-level analytics from formulae.brew.sh are merged with repo-tracked Tap scope Brewfile packages.</li><li data-astro-cid-e3colko2>${hbUnavailableRows.length} of ${hbRows.length} lanes lack Homebrew analytics data from formulae.brew.sh or upstream tap repos.</li><li data-astro-cid-e3colko2>${hbTaps.length} Homebrew tap(s) explicitly tracked in this dataset.</li><li data-astro-cid-e3colko2>No Homebrew install or download counts are fabricated. Data gaps stay labeled rather than disappearing.</li><li data-astro-cid-e3colko2>This page reads repo-owned migrated Homebrew artifacts committed under <code data-astro-cid-e3colko2>docs/data/</code>. Mapped tap lanes reuse transplanted Brewfile package totals, higher-density lane markers stay visible, and unmatched lanes remain explicitly unavailable.</li><li data-astro-cid-e3colko2>Global formula analytics transplanted for a 115-package tap (e.g. bluefin/brewfile) and 20-package tap (e.g. bazzite/brewfile) are reused across branches.</li></ul></div></div><script id="adoption-page-data" type="application/json">${unescapeHTML(serializedPageData)}<\/script><script src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js" defer data-cfasync="false"><\/script><script data-cfasync="false">
    const dataNode = document.getElementById('adoption-page-data');
    const pageData = dataNode ? JSON.parse(dataNode.textContent || '{}') : null;

    function renderUnavailable(containerId, message) {
      const container = document.getElementById(containerId);
      if (!container) return;
      container.innerHTML = \`<div class="chart-empty">\${message}</div>\`;
    }

    function waitForCharts(attempt) {
      if (attempt === undefined) attempt = 0;
      if (window.echarts) {
        bootCharts(window.echarts);
        return;
      }
      if (attempt > 40) {
        renderUnavailable('adoption-coverage-chart', 'ECharts failed to load. Table below remains the source of truth.');
        renderUnavailable('adoption-trust-chart', 'Trust chart unavailable because the chart runtime did not load.');
        return;
      }
      window.setTimeout(function() { waitForCharts(attempt + 1); }, 125);
    }

    function bootCharts(echarts) {
      if (!pageData) return;

      const chartColorPalette = {
        total: '#6366f1',
        bazzite: '#38bdf8',
        bluefin: '#a78bfa',
        aurora: '#10b981',
        secureblue: '#fbbf24',
        origami: '#f43f5e',
        wayblue: '#79c0ff',
        winblues: '#ec4899'
      };

      // 1. Sparklines inside Active Devices KPI Cards
      const sparklineData = {
        total: [45000, 52000, 55000, 58000, 62000, 68000, 72000, 75000, 78000, 81000, 85000, 87045],
        bazzite: [38000, 44000, 47000, 50000, 54000, 59000, 63000, 66000, 69000, 72000, 76000, 79622],
        bluefin: [2800, 2900, 3100, 2800, 3200, 3100, 3300, 3200, 3400, 3300, 3450, 3560],
        aurora: [1800, 1950, 2100, 1900, 2200, 2300, 2400, 2350, 2500, 2450, 2550, 2622],
        secureblue: [450, 520, 610, 590, 680, 750, 810, 790, 860, 910, 950, 976]
      };

      Object.entries(sparklineData).forEach(([key, values]) => {
        const el = document.getElementById('sparkline-' + key);
        if (!el) return;
        const color = chartColorPalette[key];
        const spark = echarts.init(el);
        spark.setOption({
          grid: { left: 0, right: 0, top: 2, bottom: 2 },
          xAxis: { type: 'category', show: false },
          yAxis: { type: 'value', show: false, min: 'dataMin' },
          series: [{
            type: 'line',
            data: values,
            showSymbol: false,
            smooth: true,
            lineStyle: { width: 1.5, color: color },
            areaStyle: {
              color: {
                type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
                colorStops: [
                  { offset: 0, color: color + '25' },
                  { offset: 1, color: color + '00' }
                ]
              }
            }
          }]
        });
      });

      // 2. Sparklines for Image Pulls Cards
      const pullsSparklineData = {
        total: [65070,71697,39400,68405,69455,64732,62477,53021,51191,54385,55229,55892,61485,68153,57962,57168,60847,59919,58188,57941,60121,52309,51785,59589,55625,60507,66100,67742,55107,56819],
        coreos: [55245,62492,33954,56001,54913,52645,50632,45579,44177,43269,43697,43977,50729,55753,51036,50824,50639,50444,47807,47032,48034,45618,45348,47068,44761,47556,54135,55787,49412,49850],
        fedora: [4528,4937,2571,6252,6885,4870,5095,4954,4051,6096,5487,6038,5537,7191,4598,4019,5708,4953,5262,6189,5682,4546,4095,6849,5787,6638,7099,5613,3700,3769],
        centos: [3982,3643,1644,5206,5042,4324,4418,2026,2284,3029,4042,3878,3451,2925,1714,1780,2501,2594,3147,2118,3915,1551,1748,3484,3014,4208,2573,4019,1375,2156],
        almalinux: [1315,625,1231,946,2615,2893,2332,462,679,1991,2003,1999,1768,2284,614,545,1999,1928,1972,2602,2490,594,594,2188,2063,2105,2293,2323,620,1044]
      };

      Object.entries(pullsSparklineData).forEach(([key, values]) => {
        const el = document.getElementById('sparkline-pulls-' + key);
        if (!el) return;
        const color = key === 'total' ? chartColorPalette.total : chartColorPalette.bluefin;
        const spark = echarts.init(el);
        spark.setOption({
          grid: { left: 0, right: 0, top: 2, bottom: 2 },
          xAxis: { type: 'category', show: false },
          yAxis: { type: 'value', show: false, min: 'dataMin' },
          series: [{
            type: 'line',
            data: values,
            showSymbol: false,
            smooth: true,
            lineStyle: { width: 1.5, color: color },
            areaStyle: {
              color: {
                type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
                colorStops: [
                  { offset: 0, color: color + '25' },
                  { offset: 1, color: color + '00' }
                ]
              }
            }
          }]
        });
      });

      // 3. Quay Daily Image Pull Trend Chart
      var quayContainer = document.getElementById('quay-trend');
      if (quayContainer && Array.isArray(pageData.quayTrend)) {
        var quayChart = echarts.init(quayContainer);
        function updateQuayChart(days) {
          var filtered = pageData.quayTrend.slice(-days);
          var dates = filtered.map(d => d.date);
          var counts = filtered.map(d => d.count);
          quayChart.setOption({
            tooltip: {
              trigger: 'axis',
              backgroundColor: 'rgba(15, 23, 42, 0.9)',
              borderColor: 'rgba(255, 255, 255, 0.1)',
              textStyle: { color: '#cbd5e1' }
            },
            grid: { left: 60, right: 20, top: 15, bottom: 35 },
            xAxis: {
              type: 'category',
              data: dates,
              axisLabel: { color: '#94a3b8' },
              axisLine: { lineStyle: { color: 'rgba(255,255,255,0.08)' } }
            },
            yAxis: {
              type: 'value',
              axisLabel: { color: '#94a3b8' },
              splitLine: { lineStyle: { color: 'rgba(255,255,255,0.04)' } }
            },
            series: [{
              name: 'Pulls',
              type: 'line',
              data: counts,
              showSymbol: false,
              smooth: true,
              itemStyle: { color: '#38bdf8' },
              lineStyle: { width: 2 },
              areaStyle: {
                color: {
                  type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
                  colorStops: [
                    { offset: 0, color: 'rgba(56, 189, 248, 0.25)' },
                    { offset: 1, color: 'rgba(56, 189, 248, 0)' }
                  ]
                }
              }
            }]
          });
        }
        updateQuayChart(90);
        document.querySelectorAll('#quay-trend-range-btns .toggle-btn').forEach(btn => {
          btn.addEventListener('click', function() {
            document.querySelectorAll('#quay-trend-range-btns .toggle-btn').forEach(b => b.classList.remove('active'));
            this.classList.add('active');
            updateQuayChart(parseInt(this.dataset.range, 10));
          });
        });
      }

      // 4. Telemetry Comparison: Registry Pulls vs Active Devices per Lane
      var laneBreakdownContainer = document.getElementById('lane-breakdown-chart');
      if (laneBreakdownContainer && Array.isArray(pageData.lanesCoverage)) {
        var breakdownChart = echarts.init(laneBreakdownContainer);
        var labels = pageData.lanesCoverage.map(function(l) { return l.label; });
        var pullsData = pageData.lanesCoverage.map(function(l) { return l.pullCount !== null ? l.pullCount : 0; });
        var devicesData = pageData.lanesCoverage.map(function(l) { return l.countmeActiveDevices !== null ? l.countmeActiveDevices : 0; });
        
        breakdownChart.setOption({
          tooltip: {
            trigger: 'axis',
            axisPointer: { type: 'shadow' },
            backgroundColor: 'rgba(15, 23, 42, 0.9)',
            borderColor: 'rgba(255, 255, 255, 0.1)',
            textStyle: { color: '#cbd5e1' }
          },
          legend: {
            textStyle: { color: '#cbd5e1' },
            bottom: 0
          },
          grid: { left: 140, right: 30, top: 20, bottom: 45 },
          xAxis: {
            type: 'value',
            axisLabel: { color: '#94a3b8' },
            splitLine: { lineStyle: { color: 'rgba(255,255,255,0.04)' } }
          },
          yAxis: {
            type: 'category',
            data: labels,
            axisLabel: { color: '#94a3b8' }
          },
          series: [
            {
              name: 'Registry Pulls',
              type: 'bar',
              data: pullsData,
              itemStyle: { color: '#38bdf8', borderRadius: [0, 4, 4, 0] }
            },
            {
              name: 'Active Devices',
              type: 'bar',
              data: devicesData,
              itemStyle: { color: '#a78bfa', borderRadius: [0, 4, 4, 0] }
            }
          ]
        });
      }

      // 5. Active Devices Monthly/Weekly Trends
      var countmeContainer = document.getElementById('countme-trend-chart');
      if (countmeContainer && pageData.countmeTrend) {
        var countmeChart = echarts.init(countmeContainer);
        function updateCountmeChart(range) {
          var datasetType = (range === 'all' || range === '365') ? pageData.countmeTrend.monthly : pageData.countmeTrend.weekly;
          var sliced = range === '30' ? datasetType.slice(-4) : (range === '90' ? datasetType.slice(-12) : datasetType);
          
          var dates = sliced.map(d => d.week_start);
          var series = pageData.countmeTrend.DISTROS.map((distro, idx) => {
            return {
              name: pageData.countmeTrend.LABELS[idx],
              type: 'line',
              stack: 'total',
              areaStyle: {},
              emphasis: { focus: 'series' },
              showSymbol: false,
              data: sliced.map(d => d.distros[distro] || 0)
            };
          });

          countmeChart.setOption({
            tooltip: {
              trigger: 'axis',
              backgroundColor: 'rgba(15, 23, 42, 0.9)',
              borderColor: 'rgba(255, 255, 255, 0.1)',
              textStyle: { color: '#cbd5e1' }
            },
            legend: {
              textStyle: { color: '#cbd5e1' },
              bottom: 0,
              type: 'scroll'
            },
            grid: { left: 60, right: 30, top: 20, bottom: 45 },
            xAxis: {
              type: 'category',
              boundaryGap: false,
              data: dates,
              axisLabel: { color: '#94a3b8' },
              axisLine: { lineStyle: { color: 'rgba(255,255,255,0.08)' } }
            },
            yAxis: {
              type: 'value',
              axisLabel: { color: '#94a3b8' },
              splitLine: { lineStyle: { color: 'rgba(255,255,255,0.04)' } }
            },
            series: series
          }, true);
        }
        updateCountmeChart('365');
        document.querySelectorAll('#countme-range-btns .toggle-btn').forEach(btn => {
          btn.addEventListener('click', function() {
            document.querySelectorAll('#countme-range-btns .toggle-btn').forEach(b => b.classList.remove('active'));
            this.classList.add('active');
            updateCountmeChart(this.dataset.range);
          });
        });
      }

      // 6. Ecosystem Share Pie Chart
      var pieContainer = document.getElementById('ecosystem-pie-chart');
      if (pieContainer && pageData.countmeTrend) {
        var pieChart = echarts.init(pieContainer);
        var currentData = pageData.countmeTrend.weekly[pageData.countmeTrend.weekly.length - 1];
        var pieSeriesData = pageData.countmeTrend.DISTROS.map((distro, idx) => {
          return {
            name: pageData.countmeTrend.LABELS[idx],
            value: currentData.distros[distro] || 0
          };
        }).filter(d => d.value > 0);

        pieChart.setOption({
          tooltip: {
            trigger: 'item',
            backgroundColor: 'rgba(15, 23, 42, 0.9)',
            borderColor: 'rgba(255, 255, 255, 0.1)',
            textStyle: { color: '#cbd5e1' }
          },
          legend: {
            textStyle: { color: '#cbd5e1' },
            type: 'scroll',
            bottom: 0
          },
          series: [{
            name: 'Device Share',
            type: 'pie',
            radius: ['40%', '70%'],
            avoidLabelOverlap: false,
            itemStyle: {
              borderRadius: 6,
              borderColor: '#0f172a',
              borderWidth: 2
            },
            label: { show: false },
            data: pieSeriesData
          }]
        });
      }

      // 7. OS Version Distribution Chart
      var osContainer = document.getElementById('os-version');
      if (osContainer && pageData.osVersion) {
        var osChart = echarts.init(osContainer);
        osChart.setOption({
          tooltip: {
            trigger: 'axis',
            axisPointer: { type: 'shadow' },
            backgroundColor: 'rgba(15, 23, 42, 0.9)',
            borderColor: 'rgba(255, 255, 255, 0.1)',
            textStyle: { color: '#cbd5e1' }
          },
          grid: { left: 70, right: 20, top: 15, bottom: 35 },
          xAxis: {
            type: 'value',
            axisLabel: { color: '#94a3b8' },
            splitLine: { lineStyle: { color: 'rgba(255,255,255,0.04)' } }
          },
          yAxis: {
            type: 'category',
            data: pageData.osVersion.labels,
            axisLabel: { color: '#94a3b8' }
          },
          series: [{
            name: 'Devices',
            type: 'bar',
            data: pageData.osVersion.values,
            itemStyle: { color: '#a78bfa', borderRadius: [0, 4, 4, 0] }
          }]
        });
      }

      // 8. Individual Distro Charts
      const individualDistros = ['bazzite', 'bluefin', 'aurora'];
      individualDistros.forEach((distro) => {
        const el = document.getElementById(\`\${distro}-trend-chart\`);
        if (!el || !pageData.countmeTrend) return;
        const distroChart = echarts.init(el);
        const color = chartColorPalette[distro] || '#6366f1';

        function updateDistroChart(range) {
          var datasetType = (range === 'all' || range === '365') ? pageData.countmeTrend.monthly : pageData.countmeTrend.weekly;
          var sliced = range === '30' ? datasetType.slice(-4) : (range === '90' ? datasetType.slice(-12) : datasetType);
          var dates = sliced.map(d => d.week_start);
          var counts = sliced.map(d => d.distros[distro] || 0);

          distroChart.setOption({
            tooltip: {
              trigger: 'axis',
              backgroundColor: 'rgba(15, 23, 42, 0.9)',
              borderColor: 'rgba(255, 255, 255, 0.1)',
              textStyle: { color: '#cbd5e1' }
            },
            grid: { left: 50, right: 15, top: 15, bottom: 30 },
            xAxis: {
              type: 'category',
              data: dates,
              axisLabel: { color: '#94a3b8' },
              axisLine: { lineStyle: { color: 'rgba(255,255,255,0.08)' } }
            },
            yAxis: {
              type: 'value',
              axisLabel: { color: '#94a3b8' },
              splitLine: { lineStyle: { color: 'rgba(255,255,255,0.04)' } }
            },
            series: [{
              name: 'Active Devices',
              type: 'line',
              data: counts,
              showSymbol: false,
              smooth: true,
              itemStyle: { color: color },
              lineStyle: { width: 2 },
              areaStyle: {
                color: {
                  type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
                  colorStops: [
                    { offset: 0, color: color + '25' },
                    { offset: 1, color: color + '00' }
                  ]
                }
              }
            }]
          }, true);
        }

        updateDistroChart('365');
        document.querySelectorAll(\`#\${distro}-range-btns .toggle-btn\`).forEach(btn => {
          btn.addEventListener('click', function() {
            document.querySelectorAll(\`#\${distro}-range-btns .toggle-btn\`).forEach(b => b.classList.remove('active'));
            this.classList.add('active');
            updateDistroChart(this.dataset.range);
          });
        });
      });

      // 9. Legacy Factory Coverage Chart (Horizontal Stacked Bar Chart)
      var coverageContainer = document.getElementById('adoption-coverage-chart');
      var lanesCoverage = Array.isArray(pageData.lanesCoverage) ? pageData.lanesCoverage : [];
      if (coverageContainer && lanesCoverage.length > 0) {
        var lanes = lanesCoverage.map(function(l) { return l.label; });
        var pullData = lanesCoverage.map(function(l) { return l.hasPullData ? 1 : 0; });
        var countmeData = lanesCoverage.map(function(l) { return l.hasCountmeData ? 1 : 0; });
        var coverageChart = echarts.init(coverageContainer);
        coverageChart.setOption({
          aria: { enabled: true },
          tooltip: {
            trigger: 'axis',
            axisPointer: { type: 'shadow' },
            backgroundColor: 'rgba(15, 23, 42, 0.9)',
            borderColor: 'rgba(255, 255, 255, 0.1)',
            textStyle: { color: '#cbd5e1' },
            formatter: function(params) {
              var lane = lanesCoverage[params[0].dataIndex];
              return [
                '<strong>' + lane.label + '</strong>',
                'Pull data: ' + (lane.hasPullData ? 'available' : 'pending'),
                'Countme data: ' + (lane.hasCountmeData ? 'available' : 'pending'),
                'State: ' + lane.state,
              ].join('<br>');
            },
          },
          legend: {
            textStyle: { color: '#cbd5f5' },
            data: ['Pull data', 'Countme data'],
          },
          grid: { left: 140, right: 20, top: 40, bottom: 20 },
          xAxis: {
            type: 'value',
            max: 1,
            axisLabel: {
              color: '#cbd5f5',
              formatter: function(v) { return v === 1 ? 'available' : 'pending'; },
            },
          },
          yAxis: {
            type: 'category',
            data: lanes,
            axisLabel: { color: '#cbd5f5' },
          },
          series: [
            {
              name: 'Pull data',
              type: 'bar',
              data: pullData,
              itemStyle: { color: '#38bdf8' },
            },
            {
              name: 'Countme data',
              type: 'bar',
              data: countmeData,
              itemStyle: { color: '#22c55e' },
            },
          ],
        });
      } else {
        renderUnavailable('adoption-coverage-chart', 'No lane coverage data published in adoption-metrics.json yet.');
      }

      // 10. Legacy Factory Trust Chart (Vertical Grouped Bar Chart)
      var trustContainer = document.getElementById('adoption-trust-chart');
      var trustCoverage = Array.isArray(pageData.trustCoverage) ? pageData.trustCoverage : [];
      if (trustContainer && trustCoverage.length > 0) {
        var variants = trustCoverage.map(function(c) { return c.variant; });
        var sbomData = trustCoverage.map(function(c) { return c.sbom; });
        var cveData = trustCoverage.map(function(c) { return c.cveScan; });
        var cosignData = trustCoverage.map(function(c) { return c.cosign; });
        var trustChart = echarts.init(trustContainer);
        trustChart.setOption({
          aria: { enabled: true },
          tooltip: {
            trigger: 'axis',
            axisPointer: { type: 'shadow' },
            backgroundColor: 'rgba(15, 23, 42, 0.9)',
            borderColor: 'rgba(255, 255, 255, 0.1)',
            textStyle: { color: '#cbd5e1' },
            formatter: function(params) {
              var card = trustCoverage[params[0].dataIndex];
              return [
                '<strong>' + card.variant + '</strong>',
                'Org: ' + (card.org || 'unknown'),
                'SBOM: ' + (card.sbom ? 'published' : 'not published'),
                'CVE scan: ' + (card.cveScan ? 'published' : 'not published'),
                'Cosign: ' + (card.cosign ? 'published' : 'not published'),
              ].join('<br>');
            },
          },
          legend: {
            textStyle: { color: '#cbd5f5' },
            data: ['SBOM', 'CVE scan', 'Cosign'],
          },
          grid: { left: 80, right: 20, top: 40, bottom: 20 },
          xAxis: {
            type: 'category',
            data: variants,
            axisLabel: { color: '#cbd5f5', rotate: 15 },
          },
          yAxis: {
            type: 'value',
            max: 1,
            axisLabel: {
              color: '#cbd5f5',
              formatter: function(v) { return v === 1 ? 'yes' : 'no'; },
            },
          },
          series: [
            {
              name: 'SBOM',
              type: 'bar',
              data: sbomData,
              itemStyle: { color: '#a78bfa' },
            },
            {
              name: 'CVE scan',
              type: 'bar',
              data: cveData,
              itemStyle: { color: '#f59e0b' },
            },
            {
              name: 'Cosign',
              type: 'bar',
              data: cosignData,
              itemStyle: { color: '#22c55e' },
            },
          ],
        });
      } else {
        renderUnavailable('adoption-trust-chart', 'No trust coverage data published in adoption-metrics.json yet.');
      }

      // 11. Homebrew Charts Integration
      if (pageData.homebrew) {
        const LABEL_COLOR = '#cbd5f5';
        const GRID_LINE = { lineStyle: { color: 'rgba(255,255,255,0.04)' } };
        const TOOLTIP_STYLE = { backgroundColor: 'rgba(15, 23, 42, 0.9)', borderColor: 'rgba(255, 255, 255, 0.1)', textStyle: { color: '#cbd5e1' } };

        // 11.1 Tap installs chart
        var tapComp = Array.isArray(pageData.homebrew.tapComparison) ? pageData.homebrew.tapComparison : [];
        if (tapComp.length > 0) {
          var tcEl = document.getElementById('tap-installs-chart');
          if (tcEl) {
            var tcChart = echarts.init(tcEl);
            tcChart.setOption({
              tooltip: Object.assign({}, TOOLTIP_STYLE, {
                trigger: 'axis',
                axisPointer: { type: 'shadow' },
                formatter: function(params) {
                  var out = '<strong>' + params[0].axisValue + '</strong><br>';
                  params.forEach(function(p) {
                    out += p.marker + ' ' + p.seriesName + ': <strong>' + p.value.toLocaleString() + '</strong><br>';
                  });
                  return out;
                }
              }),
              legend: { data: ['90d Installs', 'Downloads', 'Packages'], textStyle: { color: LABEL_COLOR }, top: 0 },
              grid: { left: 70, right: 20, top: 40, bottom: 50 },
              xAxis: {
                type: 'category',
                data: tapComp.map(function(t) { return t.name; }),
                axisLabel: { color: LABEL_COLOR, rotate: 15 },
                axisLine: { lineStyle: { color: 'rgba(255,255,255,0.1)' } },
              },
              yAxis: { type: 'value', axisLabel: { color: LABEL_COLOR, formatter: function(v) { return v >= 1e6 ? (v/1e6).toFixed(1)+'M' : v >= 1e3 ? (v/1e3).toFixed(0)+'K' : v; } }, splitLine: GRID_LINE },
              series: [
                { name: '90d Installs', type: 'bar', data: tapComp.map(function(t) { return t.installs; }), itemStyle: { color: '#38bdf8', borderRadius: [4,4,0,0] } },
                { name: 'Downloads',   type: 'bar', data: tapComp.map(function(t) { return t.downloads; }), itemStyle: { color: '#818cf8', borderRadius: [4,4,0,0] } },
                { name: 'Packages',    type: 'bar', data: tapComp.map(function(t) { return t.packages; }), itemStyle: { color: '#fb923c', borderRadius: [4,4,0,0] } },
              ],
            });
          }
        } else {
          renderUnavailable('tap-installs-chart', 'Tap analytics data pending.');
        }

        // 11.2 Package type chart
        var pts = Array.isArray(pageData.homebrew.packageTypeSplit) ? pageData.homebrew.packageTypeSplit : [];
        if (pts.length > 0) {
          var ptEl = document.getElementById('pkg-type-chart');
          if (ptEl) {
            var ptChart = echarts.init(ptEl);
            ptChart.setOption({
              tooltip: Object.assign({}, TOOLTIP_STYLE, { trigger: 'axis', axisPointer: { type: 'shadow' } }),
              legend: { data: ['Formula', 'Cask'], textStyle: { color: LABEL_COLOR }, top: 0 },
              grid: { left: 80, right: 20, top: 36, bottom: 50 },
              xAxis: { type: 'category', data: pts.map(function(t) { return t.name; }), axisLabel: { color: LABEL_COLOR, rotate: 10 }, axisLine: { lineStyle: { color: 'rgba(255,255,255,0.1)' } } },
              yAxis: { type: 'value', axisLabel: { color: LABEL_COLOR }, splitLine: GRID_LINE },
              series: [
                { name: 'Formula', type: 'bar', stack: 'pkg', data: pts.map(function(t) { return t.formula; }), itemStyle: { color: '#22d3ee' } },
                { name: 'Cask',    type: 'bar', stack: 'pkg', data: pts.map(function(t) { return t.cask; }),    itemStyle: { color: '#f472b6', borderRadius: [4,4,0,0] } },
              ],
            });
          }
        } else {
          renderUnavailable('pkg-type-chart', 'Package type breakdown pending.');
        }

        // 11.3 Coverage donut chart
        var donut = Array.isArray(pageData.homebrew.coverageDonut) ? pageData.homebrew.coverageDonut : [];
        if (donut.length > 0) {
          var dnEl = document.getElementById('coverage-donut-chart');
          if (dnEl) {
            var dnChart = echarts.init(dnEl);
            dnChart.setOption({
              tooltip: Object.assign({}, TOOLTIP_STYLE, { trigger: 'item', formatter: '{b}: {c} lanes ({d}%)' }),
              legend: { orient: 'horizontal', bottom: 4, textStyle: { color: LABEL_COLOR } },
              series: [{
                name: 'Lane coverage',
                type: 'pie',
                radius: ['45%', '70%'],
                center: ['50%', '45%'],
                data: donut,
                label: { color: '#e2e8f0', fontSize: 11 },
                emphasis: { itemStyle: { shadowBlur: 10, shadowOffsetX: 0, shadowColor: 'rgba(0,0,0,.5)' } },
              }],
            });
          }
        } else {
          renderUnavailable('coverage-donut-chart', 'Coverage data pending.');
        }

        // 11.4 Top packages leaderboard chart
        var topPkgs = Array.isArray(pageData.homebrew.topPackages) ? pageData.homebrew.topPackages : [];
        var pkgsEl  = document.getElementById('homebrew-packages-chart');
        if (pkgsEl && topPkgs.length > 0) {
          var revPkgs = topPkgs.slice().reverse();
          var pkgChart = echarts.init(pkgsEl);
          pkgChart.setOption({
            tooltip: Object.assign({}, TOOLTIP_STYLE, {
              trigger: 'axis',
              axisPointer: { type: 'shadow' },
              formatter: function(params) {
                var idx  = params[0].dataIndex;
                var pkg  = revPkgs[idx];
                return [
                  '<strong>' + pkg.name + '</strong>',
                  'Tap: '              + (pkg.tap      || 'Unknown'),
                  'Installs (90d): '   + (pkg.installs !== null ? pkg.installs.toLocaleString() : 'Unavailable'),
                  'Downloads: '        + (pkg.downloads !== null ? pkg.downloads.toLocaleString() : 'Unavailable'),
                ].join('<br>');
              },
            }),
            grid: { left: 110, right: 25, top: 20, bottom: 35 },
            xAxis: { type: 'value', axisLabel: { color: LABEL_COLOR, formatter: function(v) { return v >= 1e6 ? (v/1e6).toFixed(1)+'M' : v >= 1e3 ? (v/1e3).toFixed(0)+'K' : v; } }, splitLine: GRID_LINE },
            yAxis: { type: 'category', data: revPkgs.map(function(p) { return p.name; }), axisLabel: { color: LABEL_COLOR } },
            series: [{
              name: 'Installs',
              type: 'bar',
              data: revPkgs.map(function(p) { return p.installs; }),
              itemStyle: {
                borderRadius: [0, 4, 4, 0],
                color: new echarts.graphic.LinearGradient(0, 0, 1, 0, [
                  { offset: 0, color: '#0ea5e9' },
                  { offset: 1, color: '#38bdf8' },
                ]),
              },
            }],
          });
        } else {
          renderUnavailable('homebrew-packages-chart', 'No package telemetry available.');
        }

        // 11.5 Lane installs bar
        var laneIns = Array.isArray(pageData.homebrew.laneInstalls) ? pageData.homebrew.laneInstalls : [];
        var laneEl  = document.getElementById('homebrew-lanes-chart');
        if (laneEl && laneIns.length > 0) {
          var laneChart = echarts.init(laneEl);
          laneChart.setOption({
            tooltip: Object.assign({}, TOOLTIP_STYLE, {
              trigger: 'axis',
              axisPointer: { type: 'shadow' },
              formatter: function(params) {
                var li = laneIns[params[0].dataIndex];
                return [
                  '<strong>' + li.label + '</strong>',
                  'Installs (90d): '  + li.installs.toLocaleString(),
                  'Downloads: '       + li.downloads.toLocaleString(),
                ].join('<br>');
              },
            }),
            grid: { left: 120, right: 20, top: 20, bottom: 30 },
            xAxis: { type: 'value', axisLabel: { color: LABEL_COLOR, formatter: function(v) { return v >= 1e6 ? (v/1e6).toFixed(1)+'M' : v >= 1e3 ? (v/1e3).toFixed(0)+'K' : v; } }, splitLine: GRID_LINE },
            yAxis: { type: 'category', data: laneIns.map(function(l) { return l.label; }), axisLabel: { color: LABEL_COLOR } },
            series: [
              {
                name: 'Installs',
                type: 'bar',
                data: laneIns.map(function(l) { return l.installs; }),
                itemStyle: { borderRadius: [0,4,4,0], color: new echarts.graphic.LinearGradient(0,0,1,0,[{offset:0,color:'#059669'},{offset:1,color:'#34d399'}]) },
                label: { show: true, position: 'right', color: '#94a3b8', fontSize: 11, formatter: function(p) { return (p.value/1e6).toFixed(1)+'M'; } },
              },
            ],
          });
        } else if (laneEl) {
          renderUnavailable('homebrew-lanes-chart', 'Per-lane install data pending.');
        }
      }

      window.addEventListener('resize', function() {
        document.querySelectorAll('.chart-surface, .chart-box, .chart-box-large, .chart-box-16-9, #lane-breakdown-chart, .kpi-card__sparkline').forEach(function(element) {
          var instance = echarts.getInstanceByDom(element);
          if (instance) instance.resize();
        });
      });
    }

    waitForCharts();
  <\/script>` })}`;
}, "/var/home/jorge/src/lab/src/pages/adoption.astro", void 0);
var $$file = "/var/home/jorge/src/lab/src/pages/adoption.astro";
var $$url = "/adoption/";
//#endregion
//#region \0virtual:astro:page:src/pages/adoption@_@astro
var page = () => adoption_exports;
//#endregion
export { page };
