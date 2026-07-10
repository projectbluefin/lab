import { n as __exportAll, t as $$SiteLayout } from "./SiteLayout_DhqJu2sp.mjs";
import { C as createComponent, _ as addAttribute, b as unescapeHTML, d as renderTemplate, h as maybeRenderHead, i as renderComponent } from "./server_Dx5UOJVp.mjs";
import { t as serializeJsonScript } from "./json-script_Du4eXlRK.mjs";
import { t as upstream_status_default } from "./upstream-status_C5j1NWzN.mjs";
//#region src/scripts/upstream-page.js?url
var upstream_page_default = "/_astro/upstream-page.CZcKyHFt.js";
//#endregion
//#region src/lib/upstream-page.js
var dateFormatter = new Intl.DateTimeFormat("en-US", {
	dateStyle: "medium",
	timeStyle: "short",
	timeZone: "UTC"
});
function formatDate(value) {
	if (!value) return "Unavailable";
	const date = new Date(value);
	return Number.isNaN(date.getTime()) ? "Unavailable" : `${dateFormatter.format(date)} UTC`;
}
function formatMetricValue(metric) {
	if (metric.value == null) return "Unavailable";
	if (metric.unit === "count") return String(metric.value);
	return `${metric.value} ${metric.unit}`;
}
function formatFreshness(value) {
	if (typeof value !== "number" || Number.isNaN(value)) return "Unavailable";
	if (value === 0) return "0 days";
	if (value === 1) return "1 day";
	return `${value} days`;
}
function titleizeLane(value) {
	if (!value) return "Unknown lane";
	return value.replace(/^[a-z]/, (char) => char.toUpperCase());
}
function stateTone(state) {
	if (state === "available") return "good";
	if (state === "partial") return "warn";
	return "bad";
}
function normalizeTerminology(value) {
	if (!value) return value;
	return value.replace(/\blanes\b/gi, "streams").replace(/\blane\b/gi, "stream");
}
function buildUpstreamPageModel(dataset, options = {}) {
	const includeGroups = new Set(options.includeGroups || []);
	const excludeGroups = new Set(options.excludeGroups || []);
	const hasIncludeFilter = includeGroups.size > 0;
	const groupOrder = options.groupOrder || [];
	const groupRank = new Map(groupOrder.map((id, index) => [id, index]));
	const shouldIncludeGroup = (groupId) => {
		if (excludeGroups.has(groupId)) return false;
		if (hasIncludeFilter && !includeGroups.has(groupId)) return false;
		return true;
	};
	const groupsById = new Map((dataset.groups || []).map((group) => [group.id, {
		...group,
		description: normalizeTerminology(group.description),
		lanes: []
	}]));
	const lanes = (dataset.rows || []).filter((row) => shouldIncludeGroup(row.group)).map((row) => {
		const lane = {
			...row,
			label: titleizeLane(row.display_name),
			groupLabel: groupsById.get(row.group)?.label || row.group,
			publisherLabel: row.publisher_repo || "Repo-owned placeholder",
			publishedLabel: formatDate(row.published_at),
			freshnessLabel: formatFreshness(row.freshness_age_days),
			stateTone: stateTone(row.state),
			evidenceUrl: row.source_url,
			hasEvidence: Boolean(row.source_url),
			state_reason: normalizeTerminology(row.state_reason)
		};
		groupsById.get(row.group)?.lanes.push(lane);
		return lane;
	});
	const groups = [...groupsById.values()].filter((group) => shouldIncludeGroup(group.id)).map((group) => {
		const availableCount = group.lanes.filter((lane) => lane.state === "available").length;
		const unavailableCount = group.lanes.length - availableCount;
		const freshestLane = [...group.lanes].filter((lane) => typeof lane.freshness_age_days === "number").sort((left, right) => left.freshness_age_days - right.freshness_age_days)[0] || null;
		return {
			...group,
			lanes: [...group.lanes].sort((left, right) => left.label.localeCompare(right.label)),
			availableCount,
			unavailableCount,
			totalCount: group.lanes.length,
			freshestLaneLabel: freshestLane ? `${freshestLane.label} · ${freshestLane.freshnessLabel}` : "No published release yet",
			stateTone: unavailableCount ? availableCount ? "warn" : "bad" : "good"
		};
	}).sort((left, right) => {
		const leftRank = groupRank.has(left.id) ? groupRank.get(left.id) : Number.MAX_SAFE_INTEGER;
		const rightRank = groupRank.has(right.id) ? groupRank.get(right.id) : Number.MAX_SAFE_INTEGER;
		if (leftRank !== rightRank) return leftRank - rightRank;
		return left.label.localeCompare(right.label);
	});
	const missingLanes = lanes.filter((lane) => lane.state !== "available");
	const publishedLanes = lanes.filter((lane) => lane.published_at);
	return {
		meta: {
			...dataset._meta,
			generatedLabel: formatDate(dataset._meta?.generated_at),
			stateTone: stateTone(dataset._meta?.status)
		},
		summaryMetrics: (dataset.summary_metrics || []).map((metric) => ({
			...metric,
			label: normalizeTerminology(metric.label),
			derivation: normalizeTerminology(metric.derivation),
			state_reason: normalizeTerminology(metric.state_reason),
			displayValue: formatMetricValue(metric),
			collectedLabel: formatDate(metric.collected_at),
			stateTone: stateTone(metric.state)
		})),
		groups,
		lanes,
		missingLanes,
		charts: {
			availability: {
				categories: groups.map((group) => group.label),
				available: groups.map((group) => group.availableCount),
				unavailable: groups.map((group) => group.unavailableCount)
			},
			freshness: {
				categories: lanes.map((lane) => lane.label),
				available: lanes.map((lane) => lane.state === "available" ? lane.freshness_age_days : null),
				unavailable: missingLanes.map((lane) => ({
					name: lane.label,
					value: [0, lane.label],
					stateReason: lane.state_reason
				}))
			},
			timeline: {
				categories: lanes.map((lane) => lane.label),
				series: groups.map((group) => ({
					name: group.label,
					data: publishedLanes.filter((lane) => lane.group === group.id).map((lane) => ({
						name: lane.label,
						value: [lane.published_at, lane.label],
						freshnessAgeDays: lane.freshness_age_days,
						branch: lane.branch
					}))
				}))
			},
			distribution: {
				categories: groups.map((group) => group.label),
				data: groups.map((group) => ({
					name: group.label,
					value: group.lanes.length
				}))
			},
			freshnessBrackets: { data: [
				{
					name: "Fresh (< 3d)",
					value: lanes.filter((l) => l.state === "available" && typeof l.freshness_age_days === "number" && l.freshness_age_days < 3).length
				},
				{
					name: "Recent (3-14d)",
					value: lanes.filter((l) => l.state === "available" && typeof l.freshness_age_days === "number" && l.freshness_age_days >= 3 && l.freshness_age_days <= 14).length
				},
				{
					name: "Stale (> 14d)",
					value: lanes.filter((l) => l.state === "available" && typeof l.freshness_age_days === "number" && l.freshness_age_days > 14).length
				},
				{
					name: "Awaiting",
					value: missingLanes.length
				}
			] }
		}
	};
}
//#endregion
//#region src/pages/images.astro
var images_exports = /* @__PURE__ */ __exportAll({
	default: () => $$Images,
	file: () => $$file,
	url: () => $$url
});
var $$Images = createComponent(($$result, $$props, $$slots) => {
	const baseUrl = "/";
	const model = buildUpstreamPageModel(upstream_status_default, { groupOrder: [
		"projectbluefin",
		"gnome-os",
		"fedora-bootc",
		"ublue"
	] });
	const serializedModel = serializeJsonScript(model);
	const formatUtc = (value) => value ? new Date(value).toLocaleString("en-US", {
		dateStyle: "medium",
		timeStyle: "short",
		timeZone: "UTC"
	}) + " UTC" : "Never";
	const activeFamilies = model.summaryMetrics.find((m) => m.id === "active_upstream_families")?.value || model.groups.length;
	const activeStreams = model.summaryMetrics.find((m) => m.id === "active_upstream_streams")?.value || model.groups.reduce((acc, g) => acc + g.availableCount, 0);
	const unavailableStreams = model.summaryMetrics.find((m) => m.id === "unavailable_upstream_streams")?.value || model.missingLanes.length;
	return renderTemplate`${renderComponent($$result, "SiteLayout", $$SiteLayout, {
		"title": "Images",
		"description": "Factory image status across GNOME OS, Fedora bootc, and Bluefin family image streams.",
		"current": "images"
	}, { "default": ($$result2) => renderTemplate`${maybeRenderHead($$result2)}<header class="dashboard-header"><h1>Image status</h1><div class="meta-bar"><span>Updated ${formatUtc(upstream_status_default._meta.generated_at)}</span><span>Source: external base images & upstream families</span><a${addAttribute(`${baseUrl}data/upstream-status.json`, "href")}>Raw dataset ↗</a></div></header><section class="upstream-chart-grid"><article class="status-card upstream-chart-card upstream-chart-card--wide"><div class="upstream-card__header"><p class="status-card__eyebrow">Timeline</p><span class="upstream-pill muted">Evidence links below</span></div><h2>Release timeline</h2><p>Published streams plot against their release timestamp. Streams without published evidence remain listed underneath.</p><div id="upstream-timeline-chart" class="upstream-chart upstream-chart--timeline" role="img" aria-label="Release timeline for upstream streams"></div></article><article class="status-card upstream-chart-card"><div class="upstream-card__header"><p class="status-card__eyebrow">Availability</p><span class="upstream-pill good">Real data</span></div><h2>Stream availability by family</h2><p>Stacked counts show where release timestamps exist today and where collectors still owe evidence.</p><div id="upstream-availability-chart" class="upstream-chart" role="img" aria-label="Availability counts by upstream family"></div></article><article class="status-card upstream-chart-card"><div class="upstream-card__header"><p class="status-card__eyebrow">Freshness</p><span class="upstream-pill warn">Unavailable streams kept explicit</span></div><h2>Release freshness by stream</h2><p>Bars show published age in days. Missing streams stay on the axis with explicit unavailable markers.</p><div id="upstream-freshness-chart" class="upstream-chart" role="img" aria-label="Release freshness by upstream stream"></div></article><article class="status-card upstream-chart-card"><div class="upstream-card__header"><p class="status-card__eyebrow">Family Distribution</p><span class="upstream-pill good">Ecosystem share</span></div><h2>Streams by upstream family</h2><p>Proportion of streams mapped under each upstream family and publisher group.</p><div id="upstream-distribution-chart" class="upstream-chart" role="img" aria-label="Family distribution chart"></div></article><article class="status-card upstream-chart-card"><div class="upstream-card__header"><p class="status-card__eyebrow">Freshness Health</p><span class="upstream-pill good">Freshness brackets</span></div><h2>Release freshness brackets</h2><p>Image streams grouped by age ranges (Fresh, Recent, Stale, and Awaiting evidence).</p><div id="upstream-brackets-chart" class="upstream-chart" role="img" aria-label="Freshness brackets chart"></div></article></section><section class="upstream-lanes-section"><div class="upstream-section-heading"><div><p class="page-intro__eyebrow">Deep view</p><h2>Grouped upstream streams</h2></div><p>Every stream keeps its branch, freshness, availability, derivation, and evidence link visible.</p></div><section class="upstream-group-block" aria-label="Reference upstream parent operating systems"><div class="upstream-group-heading"><div><h3>Reference upstream parent OSes</h3><p>These Fedora-based desktop parent distributions are the upstream references that many image families build from.</p></div></div><div class="upstream-lane-grid"><article class="status-card upstream-lane-card"><div class="upstream-card__header"><div><p class="status-card__eyebrow">Fedora desktop parent</p><h4>Fedora Silverblue</h4></div><span class="upstream-pill good">Parent OS</span></div><p>Reference parent OS for immutable desktop experiences and the Silverblue family.</p><div class="upstream-evidence-row"><a href="https://fedoraproject.org/silverblue/">Fedora Silverblue homepage</a></div></article><article class="status-card upstream-lane-card"><div class="upstream-card__header"><div><p class="status-card__eyebrow">Fedora desktop parent</p><h4>Fedora Kinoite</h4></div><span class="upstream-pill good">Parent OS</span></div><p>Reference parent OS for KDE desktop experiences and the Kinoite family.</p><div class="upstream-evidence-row"><a href="https://fedoraproject.org/kinoite/">Fedora Kinoite homepage</a></div></article></div></section>${model.groups.map((group) => renderTemplate`<section${addAttribute(group.id, "id")} class="upstream-group-block"><div class="upstream-group-heading"><div><h3>${group.label}</h3><p>${group.description}</p></div><a${addAttribute(group.source_url, "href")}>Group evidence</a></div><div class="upstream-lane-grid">${group.lanes.map((lane) => renderTemplate`<article class="status-card upstream-lane-card"><div class="upstream-card__header"><div><p class="status-card__eyebrow">${lane.branch}</p><h4>${lane.label}</h4></div><span${addAttribute(["upstream-pill", lane.stateTone], "class:list")}>${lane.state}</span></div><dl class="upstream-lane-meta"><div><dt>Published</dt><dd>${lane.publishedLabel}</dd></div><div><dt>Freshness</dt><dd>${lane.freshnessLabel}</dd></div><div><dt>Publisher</dt><dd>${lane.publisherLabel}</dd></div></dl>${lane.state_reason && renderTemplate`<p class="upstream-state-reason">${lane.state_reason}</p>`}<details class="upstream-details"><summary>Derivation</summary><p>${lane.derivation}</p></details><div class="upstream-evidence-row"><a${addAttribute(lane.source_url, "href")}>Evidence link</a><span>Collected ${lane.collected_at}</span></div></article>`)}</div></section>`)}</section><section class="upstream-family-grid" aria-label="Upstream family summaries" style="margin-top: 2rem;"><article class="status-card upstream-family-card"><div class="upstream-card__header"><p class="status-card__eyebrow">Coverage</p><span class="upstream-pill good">${activeFamilies} families</span></div><h2>${activeStreams} streams currently publishing evidence</h2><p>Upstream families are grouped by their originating ecosystem so release freshness and availability stay easy to compare.</p><dl class="status-card__meta"><div><dt>Active streams</dt><dd>${activeStreams}</dd></div><div><dt>Unavailable streams</dt><dd>${unavailableStreams}</dd></div></dl></article>${model.groups.map((group) => renderTemplate`<article class="status-card upstream-family-card"><div class="upstream-card__header"><p class="status-card__eyebrow">${group.label}</p><span${addAttribute(["upstream-pill", group.stateTone], "class:list")}>${group.availableCount}/${group.totalCount} available</span></div><h2>${group.availableCount ? "Published evidence present" : "Collector gap visible"}</h2><p>${group.description}</p><dl class="status-card__meta"><div><dt>Freshest stream</dt><dd>${group.freshestLaneLabel}</dd></div><div><dt>Unavailable streams</dt><dd>${group.unavailableCount}</dd></div><div><dt>Evidence</dt><dd><a${addAttribute(group.source_url, "href")}>Family source</a></dd></div></dl></article>`)}</section><section class="upstream-missing-section" style="margin-top: 2rem; padding-top: 1.5rem; border-top: 1px solid rgba(255,255,255,0.06);"><div class="upstream-section-heading" style="margin-bottom: 1rem;"><div><p class="page-intro__eyebrow">Awaiting evidence</p><h2 style="letter-spacing: -0.03em; margin: 0;">Streams Awaiting Published Evidence</h2></div><p>These image streams are configured but currently lack verified release timestamps.</p></div><div class="upstream-missing-list" style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 1.25rem;">${model.missingLanes.map((lane) => renderTemplate`<article class="upstream-missing-item" style="margin: 0; background: rgba(15,23,42,0.3); border: 1px solid rgba(239,68,68,0.15); border-radius: 18px; padding: 14px 16px;"><strong style="color: #f1f5f9; display: block; margin-bottom: 0.25rem;">${lane.label}</strong><span style="font-size: 0.8rem; color: #94a3b8; line-height: 1.55;">${lane.state_reason}</span></article>`)}</div></section><script type="application/json" id="upstream-page-data">${unescapeHTML(serializedModel)}<\/script><script src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js" defer data-cfasync="false"><\/script><script${addAttribute(upstream_page_default, "src")} defer data-cfasync="false"><\/script>` })}`;
}, "/var/home/jorge/src/lab/src/pages/images.astro", void 0);
var $$file = "/var/home/jorge/src/lab/src/pages/images.astro";
var $$url = "/images/";
//#endregion
//#region \0virtual:astro:page:src/pages/images@_@astro
var page = () => images_exports;
//#endregion
export { page };
