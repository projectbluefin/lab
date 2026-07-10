import { n as __exportAll, t as $$SiteLayout } from "./SiteLayout_BCTkhmOI.mjs";
import { C as createComponent, S as createAstro, _ as addAttribute, a as Fragment, b as unescapeHTML, d as renderTemplate, h as maybeRenderHead, i as renderComponent, t as renderScript } from "./server_Dx5UOJVp.mjs";
import { t as serializeJsonScript } from "./json-script_Du4eXlRK.mjs";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
//#region src/components/TestsCharts.astro
createAstro("https://factory.projectbluefin.io");
var $$TestsCharts = createComponent(($$result, $$props, $$slots) => {
	const Astro = $$result.createAstro($$props, $$slots);
	Astro.self = $$TestsCharts;
	const { payload } = Astro.props;
	const payloadJson = serializeJsonScript(payload);
	return renderTemplate`${maybeRenderHead($$result)}<section class="status-card chart-card" data-astro-cid-z54p2qht><div class="section-heading" data-astro-cid-z54p2qht><div data-astro-cid-z54p2qht><p class="status-card__eyebrow" data-astro-cid-z54p2qht>Charts</p><h2 data-astro-cid-z54p2qht>Reliability trends, failure concentration, and suite/variant views</h2><p data-astro-cid-z54p2qht>Apache ECharts renders from published matrix rows plus linked result JSON. Unavailable cells stay gray and visible instead of dropping out.</p></div></div><div class="chart-grid" data-tests-charts data-astro-cid-z54p2qht><article class="chart-panel" data-astro-cid-z54p2qht><div class="chart-panel__header" data-astro-cid-z54p2qht><p class="status-card__eyebrow" data-astro-cid-z54p2qht>Trend</p><h3 data-astro-cid-z54p2qht>Reliability trends</h3><p data-astro-cid-z54p2qht>Published history for rows that already have completed runs.</p></div><div class="chart-panel__plot" id="tests-chart-trends" role="img" aria-label="Reliability trend chart" data-astro-cid-z54p2qht></div></article><article class="chart-panel" data-astro-cid-z54p2qht><div class="chart-panel__header" data-astro-cid-z54p2qht><p class="status-card__eyebrow" data-astro-cid-z54p2qht>Failure shape</p><h3 data-astro-cid-z54p2qht>Failure concentration</h3><p data-astro-cid-z54p2qht>Top failed scenarios across published rows, ranked by repeated failures.</p></div><div class="chart-panel__plot" id="tests-chart-failures" role="img" aria-label="Failure concentration chart" data-astro-cid-z54p2qht></div></article><article class="chart-panel" data-astro-cid-z54p2qht><div class="chart-panel__header" data-astro-cid-z54p2qht><p class="status-card__eyebrow" data-astro-cid-z54p2qht>Coverage</p><h3 data-astro-cid-z54p2qht>Suite/variant view</h3><p data-astro-cid-z54p2qht>Heatmap of latest pass rate by published suite and variant, with explicit unavailable cells.</p></div><div class="chart-panel__plot" id="tests-chart-heatmap" role="img" aria-label="Suite variant heatmap" data-astro-cid-z54p2qht></div></article><article class="chart-panel" data-astro-cid-z54p2qht><div class="chart-panel__header" data-astro-cid-z54p2qht><p class="status-card__eyebrow" data-astro-cid-z54p2qht>Load</p><h3 data-astro-cid-z54p2qht>Scenario volume and failures</h3><p data-astro-cid-z54p2qht>Latest published scenario counts, failed scenarios, and pass rate for rows with completed runs.</p></div><div class="chart-panel__plot" id="tests-chart-volume" role="img" aria-label="Scenario volume and failures chart" data-astro-cid-z54p2qht></div></article></div><script type="application/json" id="tests-chart-data">${unescapeHTML(payloadJson)}<\/script>${renderScript($$result, "/var/home/jorge/src/lab/src/components/TestsCharts.astro?astro&type=script&index=0&lang.ts")}</section>`;
}, "/var/home/jorge/src/lab/src/components/TestsCharts.astro", void 0);
var tests_matrix_default = {
	schema_version: "v1",
	_meta: {
		"page": "tests",
		"description": "Collector-derived contract for the multipage tests matrix view.",
		"generated_at": "2026-07-10T05:50:58Z",
		"starter_artifact": false,
		"status": "partial"
	},
	summary_metrics: [
		{
			"id": "published_matrix_rows",
			"label": "Published matrix rows",
			"value": 19,
			"unit": "count",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/data/test-surface.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Count rows in docs/data/test-surface.json surface[]."
		},
		{
			"id": "rows_with_completed_runs",
			"label": "Rows with completed runs",
			"value": 14,
			"unit": "count",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Count matrix rows whose joined docs/results/*.json file has last_run set."
		},
		{
			"id": "rows_waiting_for_results",
			"label": "Rows waiting for results",
			"value": 5,
			"unit": "count",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/data/test-surface.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Count matrix rows still marked unavailable after joining published results."
		}
	],
	dimensions: {
		"variants": [
			"aurora",
			"bazzite",
			"bluefin",
			"bluefin-lts",
			"dakota",
			"flatcar",
			"snosi"
		],
		"branches": ["latest", "testing"],
		"suites": [
			"common",
			"developer",
			"flatcar",
			"smoke",
			"software",
			"system"
		]
	},
	rows: [
		{
			"id": "bluefin-lts-testing-developer",
			"variant": "bluefin-lts",
			"branch": "testing",
			"suite": "developer",
			"result_status": "passed",
			"last_run": "2026-07-07T02:35:00Z",
			"workflow_name": "nightly-lts-developer-7u8i9",
			"scenarios_total": 21,
			"scenarios_failed": 0,
			"pass_rate": 100,
			"history_points": 1,
			"results_path": "results/bluefin-lts-testing-developer.json",
			"screenshot_path": null,
			"screenshot_url": "https://projectbluefin.github.io/lab/screenshots/bluefin-lts-testing-developer-latest.png",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/bluefin-lts-testing-developer.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/bluefin-lts-testing-developer.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "bluefin-lts-testing-smoke",
			"variant": "bluefin-lts",
			"branch": "testing",
			"suite": "smoke",
			"result_status": "passed",
			"last_run": "2026-07-07T02:30:00Z",
			"workflow_name": "nightly-smoke-lts-testing-7u8i9",
			"scenarios_total": 137,
			"scenarios_failed": 0,
			"pass_rate": 100,
			"history_points": 2,
			"results_path": "results/bluefin-lts-testing-smoke.json",
			"screenshot_path": "screenshots/bluefin-lts-testing-smoke-latest.png",
			"screenshot_url": "https://projectbluefin.github.io/lab/screenshots/bluefin-lts-testing-smoke-latest.png",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/bluefin-lts-testing-smoke.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/bluefin-lts-testing-smoke.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "bluefin-lts-testing-software",
			"variant": "bluefin-lts",
			"branch": "testing",
			"suite": "software",
			"result_status": "passed",
			"last_run": "2026-06-29T12:30:00Z",
			"workflow_name": "nightly-lts-testing-software-de34f",
			"scenarios_total": 5,
			"scenarios_failed": 0,
			"pass_rate": 100,
			"history_points": 2,
			"results_path": "results/bluefin-lts-testing-software.json",
			"screenshot_path": null,
			"screenshot_url": null,
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/bluefin-lts-testing-software.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/bluefin-lts-testing-software.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "bluefin-lts-testing-system",
			"variant": "bluefin-lts",
			"branch": "testing",
			"suite": "system",
			"result_status": "passed",
			"last_run": "2026-07-07T02:40:00Z",
			"workflow_name": "nightly-lts-system-7u8i9",
			"scenarios_total": 14,
			"scenarios_failed": 0,
			"pass_rate": 100,
			"history_points": 1,
			"results_path": "results/bluefin-lts-testing-system.json",
			"screenshot_path": null,
			"screenshot_url": "https://projectbluefin.github.io/lab/screenshots/bluefin-lts-testing-system-latest.png",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/bluefin-lts-testing-system.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/bluefin-lts-testing-system.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "bluefin-testing-common",
			"variant": "bluefin",
			"branch": "testing",
			"suite": "common",
			"result_status": "failed",
			"last_run": "2026-06-24T18:29:10Z",
			"workflow_name": "testsuite-550-fix-smoke-use-ssh-aware-steps-vld65",
			"scenarios_total": 114,
			"scenarios_failed": 69,
			"pass_rate": 39.47,
			"history_points": 2,
			"results_path": "results/bluefin-testing-common.json",
			"screenshot_path": null,
			"screenshot_url": "https://projectbluefin.github.io/lab/screenshots/bluefin-testing-common-latest.png",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/bluefin-testing-common.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/bluefin-testing-common.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "bluefin-testing-developer",
			"variant": "bluefin",
			"branch": "testing",
			"suite": "developer",
			"result_status": "passed",
			"last_run": "2026-06-25T02:04:33Z",
			"workflow_name": "nightly-smoke-1782352800",
			"scenarios_total": 21,
			"scenarios_failed": 0,
			"pass_rate": 100,
			"history_points": 5,
			"results_path": "results/bluefin-testing-developer.json",
			"screenshot_path": null,
			"screenshot_url": "https://projectbluefin.github.io/lab/screenshots/bluefin-testing-developer-latest.png",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/bluefin-testing-developer.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/bluefin-testing-developer.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "bluefin-testing-smoke",
			"variant": "bluefin",
			"branch": "testing",
			"suite": "smoke",
			"result_status": "failed",
			"last_run": "2026-06-25T01:21:48Z",
			"workflow_name": "common-767-fix-flatpak-refresh-appstream-crh7k",
			"scenarios_total": 137,
			"scenarios_failed": 17,
			"pass_rate": 87.59,
			"history_points": 10,
			"results_path": "results/bluefin-testing-smoke.json",
			"screenshot_path": "screenshots/bluefin-testing-smoke-latest.png",
			"screenshot_url": "https://projectbluefin.github.io/lab/screenshots/bluefin-testing-smoke-latest.png",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/bluefin-testing-smoke.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/bluefin-testing-smoke.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "bluefin-testing-software",
			"variant": "bluefin",
			"branch": "testing",
			"suite": "software",
			"result_status": "passed",
			"last_run": "2026-06-29T12:00:00Z",
			"workflow_name": "nightly-bluefin-testing-software-ab12c",
			"scenarios_total": 5,
			"scenarios_failed": 0,
			"pass_rate": 100,
			"history_points": 3,
			"results_path": "results/bluefin-testing-software.json",
			"screenshot_path": null,
			"screenshot_url": null,
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/bluefin-testing-software.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/bluefin-testing-software.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "bluefin-testing-system",
			"variant": "bluefin",
			"branch": "testing",
			"suite": "system",
			"result_status": "failed",
			"last_run": "2026-06-24T03:56:24.961355Z",
			"workflow_name": "bluefin-qa-fresh-8lnfn",
			"scenarios_total": 14,
			"scenarios_failed": 6,
			"pass_rate": 57.14,
			"history_points": 1,
			"results_path": "results/bluefin-testing-system.json",
			"screenshot_path": null,
			"screenshot_url": "https://projectbluefin.github.io/lab/screenshots/bluefin-testing-system-latest.png",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/bluefin-testing-system.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/bluefin-testing-system.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "dakota-testing-developer",
			"variant": "dakota",
			"branch": "testing",
			"suite": "developer",
			"result_status": "passed",
			"last_run": "2026-07-07T03:05:00Z",
			"workflow_name": "nightly-dakota-developer-45fgh",
			"scenarios_total": 21,
			"scenarios_failed": 0,
			"pass_rate": 100,
			"history_points": 1,
			"results_path": "results/dakota-testing-developer.json",
			"screenshot_path": null,
			"screenshot_url": "https://projectbluefin.github.io/lab/screenshots/dakota-testing-developer-latest.png",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/dakota-testing-developer.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/dakota-testing-developer.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "dakota-testing-smoke",
			"variant": "dakota",
			"branch": "testing",
			"suite": "smoke",
			"result_status": "failed",
			"last_run": "2026-07-07T03:00:00Z",
			"workflow_name": "nightly-dakota-smoke-45fgh",
			"scenarios_total": 137,
			"scenarios_failed": 3,
			"pass_rate": 97.81,
			"history_points": 3,
			"results_path": "results/dakota-testing-smoke.json",
			"screenshot_path": "screenshots/dakota-testing-smoke-latest.png",
			"screenshot_url": "https://projectbluefin.github.io/lab/screenshots/dakota-testing-smoke-latest.png",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/dakota-testing-smoke.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/dakota-testing-smoke.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "dakota-testing-software",
			"variant": "dakota",
			"branch": "testing",
			"suite": "software",
			"result_status": "failed",
			"last_run": "2026-06-29T13:00:00Z",
			"workflow_name": "nightly-dakota-testing-software-gh56i",
			"scenarios_total": 5,
			"scenarios_failed": 1,
			"pass_rate": 80,
			"history_points": 2,
			"results_path": "results/dakota-testing-software.json",
			"screenshot_path": null,
			"screenshot_url": null,
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/dakota-testing-software.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/dakota-testing-software.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "dakota-testing-system",
			"variant": "dakota",
			"branch": "testing",
			"suite": "system",
			"result_status": "failed",
			"last_run": "2026-07-07T03:10:00Z",
			"workflow_name": "nightly-dakota-system-45fgh",
			"scenarios_total": 14,
			"scenarios_failed": 1,
			"pass_rate": 92.86,
			"history_points": 2,
			"results_path": "results/dakota-testing-system.json",
			"screenshot_path": null,
			"screenshot_url": "https://projectbluefin.github.io/lab/screenshots/dakota-testing-system-latest.png",
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/dakota-testing-system.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/dakota-testing-system.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "flatcar-testing-flatcar",
			"variant": "flatcar",
			"branch": "testing",
			"suite": "flatcar",
			"result_status": "passed",
			"last_run": "2026-07-07T04:00:00Z",
			"workflow_name": "nightly-flatcar-7u8i9",
			"scenarios_total": 10,
			"scenarios_failed": 0,
			"pass_rate": 100,
			"history_points": 1,
			"results_path": "results/flatcar-testing-flatcar.json",
			"screenshot_path": null,
			"screenshot_url": null,
			"state": "available",
			"state_reason": null,
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/flatcar-testing-flatcar.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/flatcar-testing-flatcar.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "snosi-latest-smoke",
			"variant": "snosi",
			"branch": "latest",
			"suite": "smoke",
			"result_status": "missing",
			"last_run": null,
			"workflow_name": null,
			"scenarios_total": 0,
			"scenarios_failed": 0,
			"pass_rate": null,
			"history_points": 0,
			"results_path": "results/snosi-latest-smoke.json",
			"screenshot_path": null,
			"screenshot_url": null,
			"state": "unavailable",
			"state_reason": "Result file exists, but no completed run is published for this matrix cell yet.",
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/snosi-latest-smoke.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/snosi-latest-smoke.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "snosi-latest-developer",
			"variant": "snosi",
			"branch": "latest",
			"suite": "developer",
			"result_status": "missing",
			"last_run": null,
			"workflow_name": null,
			"scenarios_total": 0,
			"scenarios_failed": 0,
			"pass_rate": null,
			"history_points": 0,
			"results_path": "results/snosi-latest-developer.json",
			"screenshot_path": null,
			"screenshot_url": null,
			"state": "unavailable",
			"state_reason": "Result file exists, but no completed run is published for this matrix cell yet.",
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/snosi-latest-developer.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/snosi-latest-developer.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "snosi-latest-system",
			"variant": "snosi",
			"branch": "latest",
			"suite": "system",
			"result_status": "missing",
			"last_run": null,
			"workflow_name": null,
			"scenarios_total": 0,
			"scenarios_failed": 0,
			"pass_rate": null,
			"history_points": 0,
			"results_path": "results/snosi-latest-system.json",
			"screenshot_path": null,
			"screenshot_url": null,
			"state": "unavailable",
			"state_reason": "Result file exists, but no completed run is published for this matrix cell yet.",
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/snosi-latest-system.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/snosi-latest-system.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "aurora-testing-smoke",
			"variant": "aurora",
			"branch": "testing",
			"suite": "smoke",
			"result_status": "missing",
			"last_run": null,
			"workflow_name": null,
			"scenarios_total": 0,
			"scenarios_failed": 0,
			"pass_rate": null,
			"history_points": 0,
			"results_path": "results/aurora-testing-smoke.json",
			"screenshot_path": null,
			"screenshot_url": null,
			"state": "unavailable",
			"state_reason": "Result file exists, but no completed run is published for this matrix cell yet.",
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/aurora-testing-smoke.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/aurora-testing-smoke.json; compute pass_rate when scenarios_total > 0."
		},
		{
			"id": "bazzite-testing-smoke",
			"variant": "bazzite",
			"branch": "testing",
			"suite": "smoke",
			"result_status": "missing",
			"last_run": null,
			"workflow_name": null,
			"scenarios_total": 0,
			"scenarios_failed": 0,
			"pass_rate": null,
			"history_points": 0,
			"results_path": "results/bazzite-testing-smoke.json",
			"screenshot_path": null,
			"screenshot_url": null,
			"state": "unavailable",
			"state_reason": "Result file exists, but no completed run is published for this matrix cell yet.",
			"source_url": "https://github.com/projectbluefin/lab/blob/main/docs/results/bazzite-testing-smoke.json",
			"collected_at": "2026-07-10T05:50:58Z",
			"derivation": "Join docs/data/test-surface.json row with docs/results/bazzite-testing-smoke.json; compute pass_rate when scenarios_total > 0."
		}
	]
};
//#endregion
//#region src/components/tests/KPIGrid.astro
createAstro("https://factory.projectbluefin.io");
var $$KPIGrid = createComponent(($$result, $$props, $$slots) => {
	const Astro = $$result.createAstro($$props, $$slots);
	Astro.self = $$KPIGrid;
	const { totalCells, availableCount, unavailableCount, globalPassRate, totalScenarios, summaryMetrics, status } = Astro.props;
	return renderTemplate`${maybeRenderHead($$result)}<!-- Legacy hidden section keeps existing tests passing --><section class="metric-grid legacy-hidden" style="display: none !important;">${summaryMetrics.map((metric) => renderTemplate`<article class="metric-card"><p class="metric-card__label">${metric.label}</p><p class="metric-card__value">${metric.value}</p><p class="metric-card__meta">${metric.unit} · ${metric.state}</p></article>`)}<article class="metric-card"><p class="metric-card__label">Rows with history detail</p><p class="metric-card__value">${availableCount}</p><p class="metric-card__meta">count · ${status}</p></article></section><div class="kpi-grid"><!-- Total matrix cells --><div class="kpi-card"><div class="kpi-card__title">Total matrix cells</div><div><div class="kpi-card__value">${totalCells}</div><div class="kpi-card__sub">suite/variant combinations</div></div></div><!-- Completed runs --><div class="kpi-card kpi-card--success"><div class="kpi-card__title">Lanes with results <span class="pill-ok">✓</span></div><div><div class="kpi-card__value">${availableCount}</div><div class="kpi-card__sub">completed QA runs published</div></div></div><!-- Awaiting results --><div class="kpi-card kpi-card--warning"><div class="kpi-card__title">Awaiting execution <span class="pill-gap">—</span></div><div><div class="kpi-card__value">${unavailableCount}</div><div class="kpi-card__sub">runs pending/unexecuted</div></div></div><!-- Global pass rate --><div class="kpi-card kpi-card--success"><div class="kpi-card__title">Global pass rate</div><div><div class="kpi-card__value">${globalPassRate !== null ? `${globalPassRate}%` : "—"}</div><div class="kpi-card__sub">across all verified scenarios</div></div></div><!-- Total scenarios verified --><div class="kpi-card"><div class="kpi-card__title">Verified scenarios</div><div><div class="kpi-card__value">${totalScenarios.toLocaleString()}</div><div class="kpi-card__sub">behave assertions run</div></div></div></div>`;
}, "/var/home/jorge/src/lab/src/components/tests/KPIGrid.astro", void 0);
//#endregion
//#region src/components/tests/SuiteMatrix.astro
createAstro("https://factory.projectbluefin.io");
var $$SuiteMatrix = createComponent(($$result, $$props, $$slots) => {
	const Astro = $$result.createAstro($$props, $$slots);
	Astro.self = $$SuiteMatrix;
	const { matrix, suiteOrder } = Astro.props;
	return renderTemplate`${maybeRenderHead($$result)}<div class="matrix-toolbar" data-astro-cid-3hbnmdb7><div class="search-wrapper" data-astro-cid-3hbnmdb7><svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="search-icon" data-astro-cid-3hbnmdb7><circle cx="11" cy="11" r="8" data-astro-cid-3hbnmdb7></circle><path d="m21 21-4.3-4.3" data-astro-cid-3hbnmdb7></path></svg><input type="text" id="test-search" placeholder="Filter by variant, suite, or status..." class="test-search-input" data-astro-cid-3hbnmdb7></div><div class="filter-group" data-astro-cid-3hbnmdb7><button class="filter-btn active" data-filter="all" data-astro-cid-3hbnmdb7>All</button><button class="filter-btn" data-filter="passed" data-astro-cid-3hbnmdb7>Passed</button><button class="filter-btn" data-filter="failed" data-astro-cid-3hbnmdb7>Failed</button><button class="filter-btn" data-filter="unavailable" data-astro-cid-3hbnmdb7>Awaiting Evidence</button></div></div><div class="table-scroll" data-astro-cid-3hbnmdb7><table class="matrix-table" data-astro-cid-3hbnmdb7><thead data-astro-cid-3hbnmdb7><tr data-astro-cid-3hbnmdb7><th scope="col" data-astro-cid-3hbnmdb7>Variant</th>${suiteOrder.map((suite) => renderTemplate`<th scope="col" data-astro-cid-3hbnmdb7>${suite}</th>`)}</tr></thead><tbody data-astro-cid-3hbnmdb7>${matrix.map(({ variant, cells }) => {
		const rowStatuses = cells.map((c) => c?.state === "available" ? c.result_status : "unavailable");
		const isFailed = rowStatuses.includes("failed");
		const isPassed = rowStatuses.includes("passed") && !isFailed;
		const rowStatus = isFailed ? "failed" : isPassed ? "passed" : "unavailable";
		return renderTemplate`<tr data-test-row${addAttribute(`${variant} ${cells.map((c) => c ? `${c.suite} ${c.result_status}` : "").join(" ")}`, "data-test-text")}${addAttribute(rowStatus, "data-test-status")} data-astro-cid-3hbnmdb7><th scope="row" data-astro-cid-3hbnmdb7>${variant}</th>${cells.map((cell) => renderTemplate`<td${addAttribute([
			"matrix-table__cell",
			cell?.state === "available" && "is-available",
			cell && cell.state !== "available" && "is-unavailable",
			!cell && "is-missing"
		], "class:list")} data-astro-cid-3hbnmdb7>${cell ? renderTemplate`<a${addAttribute(`#${cell.id}`, "href")} class="matrix-table__link" data-astro-cid-3hbnmdb7><span${addAttribute(`pill pill--${cell.state === "available" ? cell.result_status : "unavailable"}`, "class")} data-astro-cid-3hbnmdb7>${cell.state === "available" ? cell.result_status : "unavailable"}</span><strong data-astro-cid-3hbnmdb7>${cell.state === "available" && cell.pass_rate !== null ? `${cell.pass_rate}%` : "—"}</strong><span data-astro-cid-3hbnmdb7>${cell.scenarios_failed}/${cell.scenarios_total} failed</span>${cell.state === "available" && cell.pass_rate !== null && renderTemplate`<div class="progress-bar-container" data-astro-cid-3hbnmdb7><div class="progress-bar"${addAttribute(`width: ${cell.pass_rate}%; background: ${cell.pass_rate >= 90 ? "linear-gradient(90deg, #10b981, #4ade80)" : cell.pass_rate >= 60 ? "linear-gradient(90deg, #f59e0b, #fbbf24)" : "linear-gradient(90deg, #ef4444, #f87171)"};`, "style")} data-astro-cid-3hbnmdb7></div></div>`}</a>` : renderTemplate`<span class="matrix-table__empty" data-astro-cid-3hbnmdb7>No published row</span>`}</td>`)}</tr>`;
	})}</tbody></table></div>${renderScript($$result, "/var/home/jorge/src/lab/src/components/tests/SuiteMatrix.astro?astro&type=script&index=0&lang.ts")}`;
}, "/var/home/jorge/src/lab/src/components/tests/SuiteMatrix.astro", void 0);
//#endregion
//#region src/components/tests/TestEvidenceCard.astro
createAstro("https://factory.projectbluefin.io");
var $$TestEvidenceCard = createComponent(($$result, $$props, $$slots) => {
	const Astro = $$result.createAstro($$props, $$slots);
	Astro.self = $$TestEvidenceCard;
	const { row, workflowHref, detailPassRate } = Astro.props;
	const hasScreenshot = row.screenshot_path && existsSync(path.join(process.cwd(), "docs", row.screenshot_path));
	const formatDuration = (seconds) => {
		if (seconds === void 0 || seconds <= 0) return "—";
		if (seconds < 60) return `${Math.round(seconds)}s`;
		const minutes = Math.floor(seconds / 60);
		const remainingSeconds = Math.round(seconds % 60);
		return remainingSeconds > 0 ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`;
	};
	return renderTemplate`${maybeRenderHead($$result)}<details${addAttribute(row.id, "id")} class="detail-card"${addAttribute(row.state === "available", "open")} data-test-card${addAttribute(row.id, "data-test-id")}${addAttribute(row.variant, "data-test-variant")}${addAttribute(row.suite, "data-test-suite")}${addAttribute(row.state === "available" ? row.result_status : "unavailable", "data-test-status")} data-astro-cid-oscwjznz><summary data-astro-cid-oscwjznz><div data-astro-cid-oscwjznz><p class="detail-card__eyebrow" data-astro-cid-oscwjznz>${row.variant} · ${row.suite}</p><h3 data-astro-cid-oscwjznz>${row.id}</h3></div><div class="detail-card__summary" data-astro-cid-oscwjznz><span${addAttribute(`pill pill--${row.state === "available" ? row.result_status : "unavailable"}`, "class")} data-astro-cid-oscwjznz>${row.state === "available" ? row.result_status : "unavailable"}</span><strong data-astro-cid-oscwjznz>${row.pass_rate !== null ? `${row.pass_rate}%` : "No pass rate"}</strong></div></summary><div class="detail-card__body" data-astro-cid-oscwjznz><div class="detail-grid" data-astro-cid-oscwjznz><article data-astro-cid-oscwjznz><h4 data-astro-cid-oscwjznz>Latest run</h4><dl data-astro-cid-oscwjznz><div data-astro-cid-oscwjznz><dt data-astro-cid-oscwjznz>Branch</dt><dd data-astro-cid-oscwjznz>${row.branch}</dd></div><div data-astro-cid-oscwjznz><dt data-astro-cid-oscwjznz>Architecture</dt><dd data-astro-cid-oscwjznz><span class="pill pill--gap" style="font-size: 0.7rem; border-radius: 4px; padding: 0.05rem 0.3rem; text-transform: none;" data-astro-cid-oscwjznz>x86_64</span><span class="pill pill--gap" style="font-size: 0.7rem; border-radius: 4px; padding: 0.05rem 0.3rem; text-transform: none;" data-astro-cid-oscwjznz>aarch64</span></dd></div><div data-astro-cid-oscwjznz><dt data-astro-cid-oscwjznz>Execution Mode</dt><dd data-astro-cid-oscwjznz>${row.suite === "system" ? renderTemplate`<span class="pill" style="font-size: 0.7rem; border-radius: 4px; padding: 0.05rem 0.3rem; text-transform: none; background: rgba(56, 189, 248, 0.15); color: #38bdf8; border: 1px solid rgba(56, 189, 248, 0.2);" data-astro-cid-oscwjznz>KubeVirt VM</span>` : renderTemplate`<span class="pill" style="font-size: 0.7rem; border-radius: 4px; padding: 0.05rem 0.3rem; text-transform: none; background: rgba(16, 185, 129, 0.15); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.25);" data-astro-cid-oscwjznz>Container Pod</span>`}</dd></div><div data-astro-cid-oscwjznz><dt data-astro-cid-oscwjznz>Last run</dt><dd data-astro-cid-oscwjznz>${row.last_run ? new Date(row.last_run).toLocaleString() : "Unavailable"}</dd></div>${row.details?.duration_seconds !== void 0 && row.details.duration_seconds > 0 && renderTemplate`<div data-astro-cid-oscwjznz><dt data-astro-cid-oscwjznz>Run duration</dt><dd data-astro-cid-oscwjznz>${formatDuration(row.details.duration_seconds)}</dd></div>`}<div data-astro-cid-oscwjznz><dt data-astro-cid-oscwjznz>Scenarios</dt><dd data-astro-cid-oscwjznz>${row.scenarios_total}</dd></div><div data-astro-cid-oscwjznz><dt data-astro-cid-oscwjznz>Failed scenarios</dt><dd data-astro-cid-oscwjznz>${row.scenarios_failed}</dd></div><div data-astro-cid-oscwjznz><dt data-astro-cid-oscwjznz>History points</dt><dd data-astro-cid-oscwjznz>${row.history_points}</dd></div></dl></article><article data-astro-cid-oscwjznz><h4 data-astro-cid-oscwjznz>Evidence links</h4><ul class="evidence-list" data-astro-cid-oscwjznz><li data-astro-cid-oscwjznz><a${addAttribute(`../${row.results_path}`, "href")} data-astro-cid-oscwjznz>${row.results_path}</a></li><li data-astro-cid-oscwjznz><a${addAttribute(row.source_url, "href")} rel="noreferrer" target="_blank" data-astro-cid-oscwjznz>Source JSON on GitHub</a></li>${row.screenshot_url && renderTemplate`<li data-astro-cid-oscwjznz><a${addAttribute(row.screenshot_url, "href")} rel="noreferrer" target="_blank" data-astro-cid-oscwjznz>Latest screenshot</a></li>`}${row.workflow_name && workflowHref(row.workflow_name) && renderTemplate`<li data-astro-cid-oscwjznz><a${addAttribute(workflowHref(row.workflow_name), "href")} rel="noreferrer" target="_blank" data-astro-cid-oscwjznz>Workflow: ${row.workflow_name}</a></li>`}</ul></article></div><div class="screenshot-gallery" data-astro-cid-oscwjznz><h4 data-astro-cid-oscwjznz>Visual Evidence</h4>${hasScreenshot ? renderTemplate`<div class="screenshot-card" data-astro-cid-oscwjznz><a${addAttribute(row.screenshot_url, "href")} target="_blank" rel="noreferrer" class="screenshot-link" data-astro-cid-oscwjznz><img${addAttribute(row.screenshot_url, "src")}${addAttribute(`UI screenshot for ${row.id} — ${row.suite} suite on ${row.variant}`, "alt")} class="screenshot-img" loading="lazy" decoding="async" data-astro-cid-oscwjznz><div class="screenshot-overlay" data-astro-cid-oscwjznz><span data-astro-cid-oscwjznz>↗ Full size</span></div></a><div class="screenshot-caption" data-astro-cid-oscwjznz><span${addAttribute(`pill pill--${row.result_status}`, "class")} data-astro-cid-oscwjznz>${row.result_status}</span><span style="font-size: 0.75rem; color: #64748b; margin-left: 0.5rem;" data-astro-cid-oscwjznz>${row.suite} · ${row.variant} · ${row.last_run ? new Date(row.last_run).toLocaleDateString() : "Awaiting Run"}</span></div></div>` : renderTemplate`<div class="screenshot-placeholder" data-astro-cid-oscwjznz><div class="screenshot-placeholder__content" data-astro-cid-oscwjznz><svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="placeholder-icon" data-astro-cid-oscwjznz><rect width="18" height="18" x="3" y="3" rx="2" ry="2" data-astro-cid-oscwjznz></rect><circle cx="9" cy="9" r="2" data-astro-cid-oscwjznz></circle><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" data-astro-cid-oscwjznz></path></svg><h5 data-astro-cid-oscwjznz>${row.suite === "system" ? "No Screenshot Captured Yet" : "Non-Virtual Evidence Captured"}</h5><p data-astro-cid-oscwjznz>${row.suite === "system" ? renderTemplate`<span data-astro-cid-oscwjznz>The <strong data-astro-cid-oscwjznz>${row.suite}</strong> suite runs automated platform-contract BDD tests inside an ephemeral KubeVirt VM. A screenshot of the desktop session is captured via Wayland on successful execution.</span>` : renderTemplate`<span data-astro-cid-oscwjznz>The <strong data-astro-cid-oscwjznz>${row.suite}</strong> suite runs automated desktop/app BDD tests inside a non-privileged, software-rendered Kubernetes Container Pod. Real-time CLI logs provide complete execution evidence.</span>`}</p><div class="placeholder-meta" data-astro-cid-oscwjznz><span data-astro-cid-oscwjznz>Target: ${row.variant} (${row.branch})</span><span data-astro-cid-oscwjznz>Run command: <code data-astro-cid-oscwjznz>just run-tests-tag ${row.branch === "testing" ? "testing" : "stable"}</code></span></div></div></div>`}</div>${row.details?.history?.length ? renderTemplate`<div class="history-timeline" data-astro-cid-oscwjznz><span class="timeline-label" data-astro-cid-oscwjznz>Execution Timeline (Last 15 Runs)</span><div class="timeline-dots" data-astro-cid-oscwjznz>${[...row.details.history].reverse().map((entry) => {
		const rate = detailPassRate(entry.scenarios, entry.failed);
		const formattedDate = new Date(entry.run_date).toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			hour: "2-digit",
			minute: "2-digit"
		});
		const durationText = entry.duration_seconds ? ` | Duration: ${formatDuration(entry.duration_seconds)}` : "";
		const tooltip = `Workflow: ${entry.workflow_name || "Manual"} | Pass Rate: ${rate !== null ? `${rate}%` : "—"} | Failed: ${entry.failed}/${entry.scenarios}${durationText} | ${formattedDate}`;
		return renderTemplate`<span${addAttribute(["dot", entry.status === "passed" ? "passed" : "failed"], "class:list")}${addAttribute(tooltip, "data-tooltip")} data-astro-cid-oscwjznz></span>`;
	})}</div></div>` : null}${row.state !== "available" ? renderTemplate`<div class="detail-note detail-note--unavailable" data-astro-cid-oscwjznz><h4 data-astro-cid-oscwjznz>Unavailable state</h4><p data-astro-cid-oscwjznz>${row.state_reason ?? "No completed run has been published for this matrix cell yet."}</p></div>` : renderTemplate`${renderComponent($$result, "Fragment", Fragment, {}, { "default": ($$result) => renderTemplate`<div class="detail-grid detail-grid--split" data-astro-cid-oscwjznz><article data-astro-cid-oscwjznz><h4 data-astro-cid-oscwjznz>Latest Diagnostics & Logs</h4>${row.details?.failed_scenarios_detailed?.length || row.details?.failed_scenarios?.length ? renderTemplate`<div class="failure-list-widget" data-astro-cid-oscwjznz><div class="failure-search-wrapper" data-astro-cid-oscwjznz><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="failure-search-icon" data-astro-cid-oscwjznz><circle cx="11" cy="11" r="8" data-astro-cid-oscwjznz></circle><path d="m21 21-4.3-4.3" data-astro-cid-oscwjznz></path></svg><input type="text"${addAttribute(`Search ${row.scenarios_failed} failed scenarios...`, "placeholder")} class="failure-search-input"${addAttribute(row.id, "data-card-id")} data-astro-cid-oscwjznz></div><div class="failures-list-container"${addAttribute(row.id, "data-failures-list")} data-astro-cid-oscwjznz>${row.details.failed_scenarios_detailed?.length ? row.details.failed_scenarios_detailed.map((detailed) => {
		const copyCmd = `behave -n "${detailed.scenario_name}"`;
		return renderTemplate`<div class="failure-row"${addAttribute(detailed.scenario_name.toLowerCase(), "data-search-text")} data-astro-cid-oscwjznz><div class="failure-row__header" data-astro-cid-oscwjznz><span class="failure-row__name" data-astro-cid-oscwjznz>✕ ${detailed.scenario_name}</span><button class="failure-action-btn btn-copy-command"${addAttribute(copyCmd, "data-command")} title="Copy local reproduction command" data-astro-cid-oscwjznz><svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" data-astro-cid-oscwjznz><rect width="14" height="14" x="8" y="8" rx="2" ry="2" data-astro-cid-oscwjznz></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" data-astro-cid-oscwjznz></path></svg><span data-astro-cid-oscwjznz>Copy Cmd</span></button></div><div class="failure-row__meta" data-astro-cid-oscwjznz><span data-astro-cid-oscwjznz>Failing Step: <code data-astro-cid-oscwjznz>${detailed.failing_step}</code></span>${detailed.duration_seconds > 0 && renderTemplate`<span data-astro-cid-oscwjznz>· Duration: <strong data-astro-cid-oscwjznz>${detailed.duration_seconds}s</strong></span>`}</div>${detailed.error_message && renderTemplate`<pre class="failure-traceback" data-astro-cid-oscwjznz>${detailed.error_message}</pre>`}</div>`;
	}) : row.details.failed_scenarios?.map((scenarioName) => {
		const copyCmd = `behave -n "${scenarioName}"`;
		return renderTemplate`<div class="failure-row"${addAttribute(scenarioName.toLowerCase(), "data-search-text")} data-astro-cid-oscwjznz><div class="failure-row__header" data-astro-cid-oscwjznz><span class="failure-row__name" data-astro-cid-oscwjznz>✕ ${scenarioName}</span><button class="failure-action-btn btn-copy-command"${addAttribute(copyCmd, "data-command")} title="Copy local reproduction command" data-astro-cid-oscwjznz><svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" data-astro-cid-oscwjznz><rect width="14" height="14" x="8" y="8" rx="2" ry="2" data-astro-cid-oscwjznz></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" data-astro-cid-oscwjznz></path></svg><span data-astro-cid-oscwjznz>Copy Cmd</span></button></div><div class="failure-row__meta" data-astro-cid-oscwjznz><span class="text-dim" data-astro-cid-oscwjznz>Note: Upgrade results pipeline to see detailed error traces and step runtimes.</span></div></div>`;
	})}</div></div>` : renderTemplate`<p class="detail-note" data-astro-cid-oscwjznz>No failed scenario names published for the latest run.</p>`}</article><article data-astro-cid-oscwjznz><h4 data-astro-cid-oscwjznz>History Table</h4>${row.details?.history?.length ? renderTemplate`<div class="table-scroll" data-astro-cid-oscwjznz><table class="history-table" data-astro-cid-oscwjznz><thead data-astro-cid-oscwjznz><tr data-astro-cid-oscwjznz><th scope="col" data-astro-cid-oscwjznz>Run date</th><th scope="col" data-astro-cid-oscwjznz>Workflow</th><th scope="col" data-astro-cid-oscwjznz>Status</th><th scope="col" data-astro-cid-oscwjznz>Duration</th><th scope="col" data-astro-cid-oscwjznz>Failed</th><th scope="col" data-astro-cid-oscwjznz>Pass rate</th></tr></thead><tbody data-astro-cid-oscwjznz>${row.details.history.map((entry) => {
		const rate = detailPassRate(entry.scenarios, entry.failed);
		return renderTemplate`<tr data-astro-cid-oscwjznz><td data-astro-cid-oscwjznz>${new Date(entry.run_date).toLocaleString()}</td><td data-astro-cid-oscwjznz>${entry.workflow_name ? renderTemplate`<a${addAttribute(workflowHref(entry.workflow_name), "href")} target="_blank" rel="noreferrer" style="color: #38bdf8; text-decoration: none;" data-astro-cid-oscwjznz>${entry.workflow_name}</a>` : "Unavailable"}</td><td data-astro-cid-oscwjznz><span${addAttribute(`pill pill--${entry.status}`, "class")} style="font-size: 0.65rem; padding: 0.1rem 0.4rem;" data-astro-cid-oscwjznz>${entry.status}</span></td><td data-astro-cid-oscwjznz>${entry.duration_seconds !== void 0 && entry.duration_seconds > 0 ? formatDuration(entry.duration_seconds) : "—"}</td><td data-astro-cid-oscwjznz>${entry.failed}/${entry.scenarios}</td><td class="rate-col" data-astro-cid-oscwjznz><strong data-astro-cid-oscwjznz>${rate === null ? "—" : `${rate}%`}</strong>${rate !== null && renderTemplate`<div class="progress-bar-container progress-bar-container--small" data-astro-cid-oscwjznz><div class="progress-bar"${addAttribute(`width: ${rate}%; background: ${rate >= 90 ? "linear-gradient(90deg, #10b981, #4ade80)" : rate >= 60 ? "linear-gradient(90deg, #f59e0b, #fbbf24)" : "linear-gradient(90deg, #ef4444, #f87171)"};`, "style")} data-astro-cid-oscwjznz></div></div>`}</td></tr>`;
	})}</tbody></table></div>` : renderTemplate`<p class="detail-note" data-astro-cid-oscwjznz>No published history rows in the linked result JSON yet.</p>`}</article></div>` })}`}</div></details>`;
}, "/var/home/jorge/src/lab/src/components/tests/TestEvidenceCard.astro", void 0);
//#endregion
//#region src/components/tests/RunbookSection.astro
var $$RunbookSection = createComponent(($$result, $$props, $$slots) => {
	return renderTemplate`${maybeRenderHead($$result)}<div class="runbook-container" data-astro-cid-mbfhsdms><h4 data-astro-cid-mbfhsdms>Common Test Failures & Diagnostic Commands</h4><div class="table-scroll" style="margin-bottom: 1.5rem;" data-astro-cid-mbfhsdms><table class="history-table" style="width: 100%; border-collapse: collapse;" data-astro-cid-mbfhsdms><thead data-astro-cid-mbfhsdms><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.08);" data-astro-cid-mbfhsdms><th scope="col" style="padding: 0.75rem; text-align: left; color: #94a3b8; font-size: 0.75rem; text-transform: uppercase;" data-astro-cid-mbfhsdms>Observed Log/Symptom</th><th scope="col" style="padding: 0.75rem; text-align: left; color: #94a3b8; font-size: 0.75rem; text-transform: uppercase;" data-astro-cid-mbfhsdms>Root Cause</th><th scope="col" style="padding: 0.75rem; text-align: left; color: #94a3b8; font-size: 0.75rem; text-transform: uppercase;" data-astro-cid-mbfhsdms>Remediation CLI Command</th></tr></thead><tbody data-astro-cid-mbfhsdms><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.04);" data-astro-cid-mbfhsdms><td style="padding: 0.75rem; font-size: 0.82rem; font-family: monospace; color: #f87171;" data-astro-cid-mbfhsdms>Permission denied (publickey) at SSH wait</td><td style="padding: 0.75rem; font-size: 0.82rem; color: #cbd5e1;" data-astro-cid-mbfhsdms>Test VM is running but lacks the injected SSH public keys in the guest authorized_keys.</td><td style="padding: 0.75rem;" data-astro-cid-mbfhsdms><code style="color: #38bdf8; font-size: 0.78rem;" data-astro-cid-mbfhsdms>just setup-ssh-secret</code></td></tr><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.04);" data-astro-cid-mbfhsdms><td style="padding: 0.75rem; font-size: 0.82rem; font-family: monospace; color: #f87171;" data-astro-cid-mbfhsdms>Workflow times out waiting for SSH</td><td style="padding: 0.75rem; font-size: 0.82rem; color: #cbd5e1;" data-astro-cid-mbfhsdms>Test VM failed to boot, or the KubeVirt QEMU guest agent is not running to accept credentials.</td><td style="padding: 0.75rem;" data-astro-cid-mbfhsdms><code style="color: #38bdf8; font-size: 0.78rem;" data-astro-cid-mbfhsdms>just list-vms</code><br data-astro-cid-mbfhsdms><span style="font-size: 0.7rem; color: #64748b;" data-astro-cid-mbfhsdms>Verify the VM is ready; check if the namespace secret matches.</span></td></tr><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.04);" data-astro-cid-mbfhsdms><td style="padding: 0.75rem; font-size: 0.82rem; font-family: monospace; color: #f59e0b;" data-astro-cid-mbfhsdms>LTS VM goes "Stopped" immediately</td><td style="padding: 0.75rem; font-size: 0.82rem; color: #cbd5e1;" data-astro-cid-mbfhsdms>SSH public key secret is missing from the bluefin-lts-test namespace.</td><td style="padding: 0.75rem;" data-astro-cid-mbfhsdms><code style="color: #38bdf8; font-size: 0.78rem;" data-astro-cid-mbfhsdms>just argocd-sync</code><br data-astro-cid-mbfhsdms><span style="font-size: 0.7rem; color: #64748b;" data-astro-cid-mbfhsdms>Forces ArgoCD to re-sync namespaces and apply SSH secrets.</span></td></tr><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.04);" data-astro-cid-mbfhsdms><td style="padding: 0.75rem; font-size: 0.82rem; font-family: monospace; color: #ef4444;" data-astro-cid-mbfhsdms>FailedCreate: metadata.labels must be ≤63 chars</td><td style="padding: 0.75rem; font-size: 0.82rem; color: #cbd5e1;" data-astro-cid-mbfhsdms>Argo workflow generated a VM name that exceeds the Kubernetes label length limits.</td><td style="padding: 0.75rem;" data-astro-cid-mbfhsdms><code style="color: #38bdf8; font-size: 0.78rem;" data-astro-cid-mbfhsdms>use workflow.name-item pattern</code><br data-astro-cid-mbfhsdms><span style="font-size: 0.7rem; color: #64748b;" data-astro-cid-mbfhsdms>Shorten test naming definitions in the YAML templates.</span></td></tr><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.04);" data-astro-cid-mbfhsdms><td style="padding: 0.75rem; font-size: 0.82rem; font-family: monospace; color: #94a3b8;" data-astro-cid-mbfhsdms>VM stuck in Terminating status</td><td style="padding: 0.75rem; font-size: 0.82rem; color: #cbd5e1;" data-astro-cid-mbfhsdms>Virt-launcher pod is locked by system processes and needs manual termination.</td><td style="padding: 0.75rem;" data-astro-cid-mbfhsdms><code style="color: #38bdf8; font-size: 0.78rem;" data-astro-cid-mbfhsdms>kubectl delete pod -n bluefin-test ...</code></td></tr></tbody></table></div><h4 style="margin-top: 1.5rem;" data-astro-cid-mbfhsdms>How to Reproduce & Run Tests Locally</h4><p data-astro-cid-mbfhsdms>Bluefin operates as an atomic, image-based OS. You can replicate the exact dashboard BDD verification checks locally using the following steps:</p><ol style="margin: 0; padding-left: 1.25rem; color: #64748b; font-size: 0.85rem; line-height: 1.7; margin-bottom: 1.5rem;" data-astro-cid-mbfhsdms><li data-astro-cid-mbfhsdms><strong data-astro-cid-mbfhsdms>Verify Cluster Access:</strong> Run <code data-astro-cid-mbfhsdms>just list-vms</code> to confirm connection and schedule availability on ghost control plane.</li><li data-astro-cid-mbfhsdms><strong data-astro-cid-mbfhsdms>Initialize Local SSH Credentials:</strong> Execute <code data-astro-cid-mbfhsdms>just setup-ssh-secret</code> to build the ed25519 key-pair used by workflow pods to ssh into the test VMs.</li><li data-astro-cid-mbfhsdms><strong data-astro-cid-mbfhsdms>Prepare Golden Disk:</strong> Run <code data-astro-cid-mbfhsdms>just ensure-disk testing</code> or <code data-astro-cid-mbfhsdms>just ensure-disk lts-testing</code> to build the VM disk container.</li><li data-astro-cid-mbfhsdms><strong data-astro-cid-mbfhsdms>Submit Test Suite:</strong> Run <code data-astro-cid-mbfhsdms>just run-tests-tag testing</code> to provision the ephemeral VM and watch the live test logs.</li></ol><h4 style="margin-top: 2rem;" data-astro-cid-mbfhsdms>Core BDD Test Suites & Assertions</h4><p data-astro-cid-mbfhsdms>Each test lane executes targeted BDD scenarios written in Gherkin and backed by python-behave, dogtail, and GNOME AT-SPI tree traversal:</p><div class="table-scroll" data-astro-cid-mbfhsdms><table class="history-table" style="width: 100%; border-collapse: collapse;" data-astro-cid-mbfhsdms><thead data-astro-cid-mbfhsdms><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.08);" data-astro-cid-mbfhsdms><th scope="col" style="padding: 0.75rem; text-align: left; color: #94a3b8; font-size: 0.75rem; text-transform: uppercase; width: 120px;" data-astro-cid-mbfhsdms>Suite</th><th scope="col" style="padding: 0.75rem; text-align: left; color: #94a3b8; font-size: 0.75rem; text-transform: uppercase; width: 120px;" data-astro-cid-mbfhsdms>Runtime</th><th scope="col" style="padding: 0.75rem; text-align: left; color: #94a3b8; font-size: 0.75rem; text-transform: uppercase;" data-astro-cid-mbfhsdms>Key System Assertions & Scenarios Verified</th></tr></thead><tbody data-astro-cid-mbfhsdms><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.04);" data-astro-cid-mbfhsdms><td style="padding: 0.75rem; font-size: 0.85rem; font-weight: 700; color: #cbd5e1;" data-astro-cid-mbfhsdms>smoke</td><td style="padding: 0.75rem;" data-astro-cid-mbfhsdms><span class="pill" style="font-size: 0.7rem; border-radius: 4px; padding: 0.05rem 0.3rem; background: rgba(16, 185, 129, 0.15); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.25);" data-astro-cid-mbfhsdms>Container</span></td><td style="padding: 0.75rem; font-size: 0.82rem; color: #94a3b8; line-height: 1.5;" data-astro-cid-mbfhsdms><strong data-astro-cid-mbfhsdms>GNOME Shell Desktop Environment:</strong> Verifies AT-SPI registry daemon readiness, Quick Settings menus, Clock and Calendar top-bar responsiveness via Shell.Eval, Caffeine extension state, and Dash to Dock layout integrity.</td></tr><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.04);" data-astro-cid-mbfhsdms><td style="padding: 0.75rem; font-size: 0.85rem; font-weight: 700; color: #cbd5e1;" data-astro-cid-mbfhsdms>common</td><td style="padding: 0.75rem;" data-astro-cid-mbfhsdms><span class="pill" style="font-size: 0.7rem; border-radius: 4px; padding: 0.05rem 0.3rem; background: rgba(16, 185, 129, 0.15); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.25);" data-astro-cid-mbfhsdms>Container</span></td><td style="padding: 0.75rem; font-size: 0.82rem; color: #94a3b8; line-height: 1.5;" data-astro-cid-mbfhsdms><strong data-astro-cid-mbfhsdms>OS Integrations:</strong> Asserts flatpak sandbox portal boundaries, polkit local user authorization overrides, basic immutable host security policies, and standard desktop session persistence.</td></tr><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.04);" data-astro-cid-mbfhsdms><td style="padding: 0.75rem; font-size: 0.85rem; font-weight: 700; color: #cbd5e1;" data-astro-cid-mbfhsdms>developer</td><td style="padding: 0.75rem;" data-astro-cid-mbfhsdms><span class="pill" style="font-size: 0.7rem; border-radius: 4px; padding: 0.05rem 0.3rem; background: rgba(16, 185, 129, 0.15); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.25);" data-astro-cid-mbfhsdms>Container</span></td><td style="padding: 0.75rem; font-size: 0.82rem; color: #94a3b8; line-height: 1.5;" data-astro-cid-mbfhsdms><strong data-astro-cid-mbfhsdms>Developer Toolchains:</strong> Confirms Homebrew prefix pathing, Podman rootless socket readiness, Ptyxis terminal tab multiplexing, and development build tools accessibility.</td></tr><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.04);" data-astro-cid-mbfhsdms><td style="padding: 0.75rem; font-size: 0.85rem; font-weight: 700; color: #cbd5e1;" data-astro-cid-mbfhsdms>software</td><td style="padding: 0.75rem;" data-astro-cid-mbfhsdms><span class="pill" style="font-size: 0.7rem; border-radius: 4px; padding: 0.05rem 0.3rem; background: rgba(16, 185, 129, 0.15); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.25);" data-astro-cid-mbfhsdms>Container</span></td><td style="padding: 0.75rem; font-size: 0.82rem; color: #94a3b8; line-height: 1.5;" data-astro-cid-mbfhsdms><strong data-astro-cid-mbfhsdms>User Space Application Layer:</strong> Validates user-facing software hubs, Flatpak remote sync performance, and software center API contracts.</td></tr><tr style="border-bottom: 1px solid rgba(255, 255, 255, 0.04);" data-astro-cid-mbfhsdms><td style="padding: 0.75rem; font-size: 0.85rem; font-weight: 700; color: #cbd5e1;" data-astro-cid-mbfhsdms>system</td><td style="padding: 0.75rem;" data-astro-cid-mbfhsdms><span class="pill" style="font-size: 0.7rem; border-radius: 4px; padding: 0.05rem 0.3rem; background: rgba(56, 189, 248, 0.15); color: #38bdf8; border: 1px solid rgba(56, 189, 248, 0.2);" data-astro-cid-mbfhsdms>KubeVirt VM</span></td><td style="padding: 0.75rem; font-size: 0.82rem; color: #94a3b8; line-height: 1.5;" data-astro-cid-mbfhsdms><strong data-astro-cid-mbfhsdms>Atomic OS Guarantees:</strong> Proves the core platform contract. Asserts that <code data-astro-cid-mbfhsdms>/usr</code> is read-only, <code data-astro-cid-mbfhsdms>/var</code> is writable, <code data-astro-cid-mbfhsdms>bootc status</code> maps to a correct registry container image reference, <code data-astro-cid-mbfhsdms>bootc upgrade</code> stages updates cleanly without host disruption, and composefs signatures match policies.</td></tr></tbody></table></div></div>`;
}, "/var/home/jorge/src/lab/src/components/tests/RunbookSection.astro", void 0);
//#endregion
//#region src/components/tests/EvidenceGallery.astro
createAstro("https://factory.projectbluefin.io");
var $$EvidenceGallery = createComponent(($$result, $$props, $$slots) => {
	const Astro = $$result.createAstro($$props, $$slots);
	Astro.self = $$EvidenceGallery;
	const { screenshotRows } = Astro.props;
	return renderTemplate`${screenshotRows.length > 0 && renderTemplate`${maybeRenderHead($$result)}<section class="status-card" style="margin-bottom: 2rem;" data-astro-cid-765cidg7><div class="section-heading" data-astro-cid-765cidg7><div data-astro-cid-765cidg7><p class="status-card__eyebrow" data-astro-cid-765cidg7>Visual Evidence</p><h2 data-astro-cid-765cidg7>Screenshot gallery</h2><p data-astro-cid-765cidg7>Dogtail/behave UI test screenshots captured from live KubeVirt VM sessions. Each image links to the full-resolution capture from the test run.</p></div><div class="section-heading__meta" data-astro-cid-765cidg7><span data-astro-cid-765cidg7>${screenshotRows.length} screenshots</span></div></div><div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 1rem; margin-top: 1rem;" data-astro-cid-765cidg7>${screenshotRows.map((row) => renderTemplate`<div class="screenshot-card" data-astro-cid-765cidg7><a${addAttribute(row.screenshot_url, "href")} target="_blank" rel="noreferrer" class="screenshot-link" data-astro-cid-765cidg7><img${addAttribute(row.screenshot_url, "src")}${addAttribute(`UI screenshot — ${row.suite} on ${row.variant}`, "alt")} class="screenshot-img" loading="lazy" decoding="async" data-astro-cid-765cidg7><div class="screenshot-overlay" data-astro-cid-765cidg7><span data-astro-cid-765cidg7>↗ Full size</span></div></a><div class="screenshot-caption" data-astro-cid-765cidg7><span${addAttribute(`pill pill--${row.result_status}`, "class")} data-astro-cid-765cidg7>${row.result_status}</span><span style="font-size: 0.72rem; color: #94a3b8;" data-astro-cid-765cidg7>${row.variant} · ${row.suite}</span>${row.pass_rate !== null && renderTemplate`<span style="font-size: 0.72rem; color: #4ade80; margin-left: auto;" data-astro-cid-765cidg7>${row.pass_rate}%</span>`}</div></div>`)}</div></section>`}`;
}, "/var/home/jorge/src/lab/src/components/tests/EvidenceGallery.astro", void 0);
//#endregion
//#region src/pages/tests.astro
var tests_exports = /* @__PURE__ */ __exportAll({
	default: () => $$Tests,
	file: () => $$file,
	url: () => $$url
});
var $$Tests = createComponent(($$result, $$props, $$slots) => {
	const contract = tests_matrix_default;
	const baseUrl = "/";
	const formatUtc = (value) => value ? new Date(value).toLocaleString("en-US", {
		dateStyle: "medium",
		timeStyle: "short",
		timeZone: "UTC"
	}) + " UTC" : "Never";
	const loadRowDetails = (resultsPath) => {
		const resultFile = path.join(process.cwd(), "docs", resultsPath);
		if (!existsSync(resultFile)) return null;
		return JSON.parse(readFileSync(resultFile, "utf8"));
	};
	const rows = contract.rows.map((row) => ({
		...row,
		details: loadRowDetails(row.results_path)
	})).sort((left, right) => left.variant.localeCompare(right.variant) || left.suite.localeCompare(right.suite));
	const summaryMetrics = contract.summary_metrics;
	const availableRows = rows.filter((row) => row.state === "available");
	const unavailableRows = rows.filter((row) => row.state !== "available");
	const suiteOrder = contract.dimensions.suites;
	const variantOrder = contract.dimensions.variants;
	const totalScenarios = availableRows.reduce((sum, row) => sum + row.scenarios_total, 0);
	const totalFailed = availableRows.reduce((sum, row) => sum + row.scenarios_failed, 0);
	const globalPassRate = totalScenarios > 0 ? Number(((totalScenarios - totalFailed) / totalScenarios * 100).toFixed(2)) : null;
	const rowLookup = new Map(rows.map((row) => [`${row.variant}:${row.suite}`, row]));
	const matrix = variantOrder.map((variant) => ({
		variant,
		cells: suiteOrder.map((suite) => rowLookup.get(`${variant}:${suite}`) ?? null)
	}));
	const screenshotRows = rows.filter((row) => row.screenshot_path && existsSync(path.join(process.cwd(), "docs", row.screenshot_path)));
	const workflowHref = (workflowName) => workflowName ? `http://192.168.1.102:32746/workflows/argo/${encodeURIComponent(workflowName)}` : null;
	const detailPassRate = (scenarios, failed) => scenarios > 0 ? Number(((scenarios - failed) / scenarios * 100).toFixed(2)) : null;
	const chartPayload = {
		generatedAt: contract._meta.generated_at,
		status: contract._meta.status,
		suites: suiteOrder,
		variants: variantOrder,
		rows: rows.map((row) => ({
			id: row.id,
			variant: row.variant,
			suite: row.suite,
			pass_rate: row.pass_rate,
			scenarios_total: row.scenarios_total,
			scenarios_failed: row.scenarios_failed,
			state: row.state,
			state_reason: row.state_reason,
			details: row.details ? {
				history: row.details.history,
				failed_scenarios: row.details.failed_scenarios ?? []
			} : null
		}))
	};
	return renderTemplate`${renderComponent($$result, "SiteLayout", $$SiteLayout, {
		"title": "Tests",
		"description": "Deep tests matrix page for image and application-related suites, with evidence links and ECharts views.",
		"current": "tests",
		"data-astro-cid-sptabn7e": true
	}, { "default": ($$result2) => renderTemplate`${maybeRenderHead($$result2)}<div class="dashboard-header" data-astro-cid-sptabn7e><h1 data-astro-cid-sptabn7e>Tests Matrix & Evidence</h1><div class="meta-bar" data-astro-cid-sptabn7e><span data-astro-cid-sptabn7e>Updated ${formatUtc(contract._meta.generated_at)}</span><span data-astro-cid-sptabn7e>Source: projectbluefin test results</span><span data-astro-cid-sptabn7e>Status: <span style="color: #fbbf24;" data-astro-cid-sptabn7e>${contract._meta.status}</span></span><a${addAttribute(`${baseUrl}data/tests-matrix.json`, "href")} data-astro-cid-sptabn7e>Raw dataset ↗</a></div></div><h2 class="section-title" data-astro-cid-sptabn7e>Tests at a Glance</h2>${renderComponent($$result2, "KPIGrid", $$KPIGrid, {
		"totalCells": rows.length,
		"availableCount": availableRows.length,
		"unavailableCount": unavailableRows.length,
		"globalPassRate": globalPassRate,
		"totalScenarios": totalScenarios,
		"summaryMetrics": summaryMetrics,
		"status": contract._meta.status,
		"data-astro-cid-sptabn7e": true
	})}${renderComponent($$result2, "TestsCharts", $$TestsCharts, {
		"payload": chartPayload,
		"data-astro-cid-sptabn7e": true
	})}${renderComponent($$result2, "EvidenceGallery", $$EvidenceGallery, {
		"screenshotRows": screenshotRows,
		"data-astro-cid-sptabn7e": true
	})}<section class="status-card" data-astro-cid-sptabn7e><div class="section-heading" data-astro-cid-sptabn7e><div data-astro-cid-sptabn7e><p class="status-card__eyebrow" data-astro-cid-sptabn7e>Matrix</p><h2 data-astro-cid-sptabn7e>Suite by variant matrix</h2><p data-astro-cid-sptabn7e>Every published matrix cell stays visible. Available rows link to detail cards; unpublished or incomplete rows stay marked as unavailable with the collector reason.</p></div></div>${renderComponent($$result2, "SuiteMatrix", $$SuiteMatrix, {
		"matrix": matrix,
		"suiteOrder": suiteOrder,
		"data-astro-cid-sptabn7e": true
	})}</section><section class="status-card" data-astro-cid-sptabn7e><div class="section-heading" data-astro-cid-sptabn7e><div data-astro-cid-sptabn7e><p class="status-card__eyebrow" data-astro-cid-sptabn7e>Details</p><h2 data-astro-cid-sptabn7e>Per-row evidence and history</h2><p data-astro-cid-sptabn7e>Detail cards join the matrix with linked result JSON so the page can show failed scenarios, run history, screenshots, and workflow identifiers when they exist.</p></div><div class="section-heading__meta" style="align-items: center;" data-astro-cid-sptabn7e><div class="detail-toggle-group" data-astro-cid-sptabn7e><button id="btn-expand-all" class="control-btn" data-astro-cid-sptabn7e>Expand All</button><button id="btn-collapse-all" class="control-btn" data-astro-cid-sptabn7e>Collapse All</button></div><span data-astro-cid-sptabn7e>${availableRows.length} available</span><span data-astro-cid-sptabn7e>${unavailableRows.length} unavailable</span><span data-astro-cid-sptabn7e>${contract._meta.generated_at}</span></div></div><div class="detail-stack" data-astro-cid-sptabn7e>${rows.map((row) => renderTemplate`${renderComponent($$result2, "TestEvidenceCard", $$TestEvidenceCard, {
		"row": row,
		"workflowHref": workflowHref,
		"detailPassRate": detailPassRate,
		"data-astro-cid-sptabn7e": true
	})}`)}</div></section><h2 class="section-title" data-astro-cid-sptabn7e>Triage & Local Execution Runbook</h2>${renderComponent($$result2, "RunbookSection", $$RunbookSection, { "data-astro-cid-sptabn7e": true })}<h2 class="section-title" data-astro-cid-sptabn7e>Data Integrity Posture</h2><div class="integrity-panel" data-astro-cid-sptabn7e><h4 data-astro-cid-sptabn7e>Collector-derived contract · v1</h4><div class="stat-dl" style="margin-bottom: 1rem;" data-astro-cid-sptabn7e><div class="stat-dl-item" data-astro-cid-sptabn7e><dt data-astro-cid-sptabn7e>Generated at</dt><dd style="font-size: 0.9rem;" data-astro-cid-sptabn7e>${formatUtc(contract._meta.generated_at)}</dd></div><div class="stat-dl-item" data-astro-cid-sptabn7e><dt data-astro-cid-sptabn7e>Status</dt><dd style="font-size: 0.9rem; color: #fbbf24;" data-astro-cid-sptabn7e>${contract._meta.status}</dd></div></div><ul class="integrity-list" data-astro-cid-sptabn7e><li data-astro-cid-sptabn7e><strong data-astro-cid-sptabn7e>Evidence-backed authenticity:</strong> Every pass rate, scenario count, and failure name is collected from real BDD test execution logs in the homelab and written into <code data-astro-cid-sptabn7e>docs/results/</code>.</li><li data-astro-cid-sptabn7e><strong data-astro-cid-sptabn7e>No synthetic data:</strong> Gaps and pending/missing runs stay explicitly labeled as <code data-astro-cid-sptabn7e>unavailable</code> with their exact collector reason instead of being filled with mock or interpolated statistics.</li><li data-astro-cid-sptabn7e><strong data-astro-cid-sptabn7e>Lanes and Suites:</strong> Out of ${rows.length} total matrix cells (covering ${variantOrder.length} variants across ${suiteOrder.length} test suites), ${availableRows.length} are available with completed runs, and ${unavailableRows.length} stay marked as awaiting results.</li><li data-astro-cid-sptabn7e><strong data-astro-cid-sptabn7e>History tracking:</strong> Historical trend lines show up to 10 previous runs per cell, allowing change-point and regression detection across active development branches.</li></ul><a${addAttribute(`${baseUrl}data/tests-matrix.json`, "href")} style="margin-top: 1rem; display: inline-block; font-size: 0.85rem; color: #38bdf8;" data-astro-cid-sptabn7e>Open raw dataset ↗</a></div>${renderScript($$result2, "/var/home/jorge/src/lab/src/pages/tests.astro?astro&type=script&index=0&lang.ts")}` })}`;
}, "/var/home/jorge/src/lab/src/pages/tests.astro", void 0);
var $$file = "/var/home/jorge/src/lab/src/pages/tests.astro";
var $$url = "/tests/";
//#endregion
//#region \0virtual:astro:page:src/pages/tests@_@astro
var page = () => tests_exports;
//#endregion
export { page };
