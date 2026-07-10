import * as echarts from 'echarts';

const payloadNode = document.getElementById('tests-chart-data');
const payload = payloadNode ? JSON.parse(payloadNode.textContent ?? '{}') : {};
const rows = Array.isArray(payload.rows) ? payload.rows : [];
const availableRows = rows.filter((row) => row.state === 'available');
const charts = [];

const toPassRate = (scenarios, failed) => {
  if (!Number.isFinite(scenarios) || scenarios <= 0 || !Number.isFinite(failed)) {
    return null;
  }
  return Number((((scenarios - failed) / scenarios) * 100).toFixed(2));
};

const renderUnavailable = (element, message) => {
  element.innerHTML = `<div class="chart-panel__empty">${message}</div>`;
};

const renderChart = (id, option, emptyMessage) => {
  const element = document.getElementById(id);
  if (!element) {
    return;
  }

  if (!option) {
    renderUnavailable(element, emptyMessage);
    return;
  }

  const chart = echarts.init(element);
  chart.setOption(option);
  charts.push(chart);
};

const palette = [
  { color: '#38bdf8', rgbaStart: 'rgba(56, 189, 248, 0.4)', rgbaEnd: 'rgba(56, 189, 248, 0)', glow: 'rgba(56, 189, 248, 0.8)' },
  { color: '#4ade80', rgbaStart: 'rgba(74, 222, 128, 0.4)', rgbaEnd: 'rgba(74, 222, 128, 0)', glow: 'rgba(74, 222, 128, 0.8)' },
  { color: '#f59e0b', rgbaStart: 'rgba(245, 158, 11, 0.4)', rgbaEnd: 'rgba(245, 158, 11, 0)', glow: 'rgba(245, 158, 11, 0.8)' },
  { color: '#ec4899', rgbaStart: 'rgba(236, 72, 153, 0.4)', rgbaEnd: 'rgba(236, 72, 153, 0)', glow: 'rgba(236, 72, 153, 0.8)' },
  { color: '#8b5cf6', rgbaStart: 'rgba(139, 92, 246, 0.4)', rgbaEnd: 'rgba(139, 92, 246, 0)', glow: 'rgba(139, 92, 246, 0.8)' },
  { color: '#f43f5e', rgbaStart: 'rgba(244, 63, 94, 0.4)', rgbaEnd: 'rgba(244, 63, 94, 0)', glow: 'rgba(244, 63, 94, 0.8)' },
  { color: '#10b981', rgbaStart: 'rgba(16, 185, 129, 0.4)', rgbaEnd: 'rgba(16, 185, 129, 0)', glow: 'rgba(16, 185, 129, 0.8)' },
  { color: '#3b82f6', rgbaStart: 'rgba(59, 130, 246, 0.4)', rgbaEnd: 'rgba(59, 130, 246, 0)', glow: 'rgba(59, 130, 246, 0.8)' },
];

const trendSeries = availableRows
  .map((row, index) => {
    const history = Array.isArray(row.details?.history) ? [...row.details.history] : [];
    const points = history
      .sort((left, right) => new Date(left.run_date).getTime() - new Date(right.run_date).getTime())
      .map((entry) => {
        const rate = toPassRate(entry.scenarios, entry.failed);
        return rate === null ? null : [entry.run_date, rate, entry.workflow_name, entry.status, entry.failed, entry.scenarios];
      })
      .filter(Boolean);

    if (!points.length) {
      return null;
    }

    const theme = palette[index % palette.length];

    return {
      name: row.id,
      type: 'line',
      smooth: true,
      showSymbol: points.length <= 8,
      symbolSize: 8,
      emphasis: { focus: 'series' },
      lineStyle: {
        width: 3,
        shadowColor: theme.glow,
        shadowBlur: 10,
        shadowOffsetY: 4,
      },
      itemStyle: {
        color: theme.color,
      },
      areaStyle: {
        color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
          { offset: 0, color: theme.rgbaStart },
          { offset: 1, color: theme.rgbaEnd },
        ]),
      },
      data: points,
    };
  })
  .filter(Boolean);

renderChart(
  'tests-chart-trends',
  trendSeries.length
    ? {
        backgroundColor: 'transparent',
        grid: { left: 56, right: 28, top: 48, bottom: 48 },
        legend: { type: 'scroll', top: 0, textStyle: { color: '#cbd5e1' } },
        tooltip: {
          trigger: 'axis',
          backgroundColor: 'rgba(15, 23, 42, 0.95)',
          borderColor: 'rgba(125, 211, 252, 0.35)',
          textStyle: { color: '#e2e8f0' },
          formatter: (items) =>
            items
              .map((item) => {
                const [runDate, rate, workflowName, status, failed, scenarios] = item.data;
                return `${item.marker}<strong>${item.seriesName}</strong><br>${new Date(runDate).toLocaleString()}<br>Pass rate: ${rate}%<br>Status: ${status}<br>Failed: ${failed}/${scenarios}<br>Workflow: ${workflowName ?? 'Unavailable'}`;
              })
              .join('<hr style="border-color: rgba(148, 163, 184, 0.2)">'),
        },
        xAxis: {
          type: 'time',
          axisLabel: { color: '#94a3b8' },
          axisLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.2)' } },
        },
        yAxis: {
          type: 'value',
          min: 0,
          max: 100,
          axisLabel: { color: '#94a3b8', formatter: '{value}%' },
          splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.12)' } },
        },
        series: trendSeries,
      }
    : null,
  'No published history yet for reliability trend lines.',
);

const scenarioFailures = new Map();
for (const row of availableRows) {
  const failedScenarios = Array.isArray(row.details?.failed_scenarios) ? row.details.failed_scenarios : [];
  for (const scenario of failedScenarios) {
    const name = String(scenario || '').trim();
    if (!name) {
      continue;
    }
    scenarioFailures.set(name, (scenarioFailures.get(name) ?? 0) + 1);
  }
}
const topScenarioFailures = [...scenarioFailures.entries()]
  .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
  .slice(0, 12);

renderChart(
  'tests-chart-failures',
  topScenarioFailures.length
    ? {
        backgroundColor: 'transparent',
        grid: { left: 240, right: 32, top: 24, bottom: 32 },
        tooltip: {
          backgroundColor: 'rgba(15, 23, 42, 0.95)',
          borderColor: 'rgba(125, 211, 252, 0.35)',
          textStyle: { color: '#e2e8f0' },
          trigger: 'axis',
          axisPointer: { type: 'shadow' },
          formatter: (items) => {
            const item = items[0];
            return `<strong>${item.axisValueLabel}</strong><br>Seen in ${item.value} latest failed-scenario entries`;
          },
        },
        xAxis: {
          type: 'value',
          axisLabel: { color: '#94a3b8' },
          splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.12)' } },
        },
        yAxis: {
          type: 'category',
          inverse: true,
          axisLabel: {
            color: '#cbd5e1',
            formatter: (value) => (value.length > 56 ? `${value.slice(0, 53)}...` : value),
          },
          data: topScenarioFailures.map(([scenario]) => scenario),
          axisLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.2)' } },
        },
        series: [
          {
            type: 'bar',
            data: topScenarioFailures.map(([, count]) => count),
            itemStyle: { color: '#f97316' },
            label: { show: true, position: 'right', color: '#cbd5e1' },
          },
        ],
      }
    : null,
  'No failed scenario names published yet for failure concentration.',
);

const suites = Array.isArray(payload.suites) ? payload.suites : [];
const variants = Array.isArray(payload.variants) ? payload.variants : [];

// Separate available and unavailable heatmap points for distinct rendering
const heatmapAvailable = [];
const heatmapUnavailable = [];
rows.forEach((row) => {
  const xIdx = suites.indexOf(row.suite);
  const yIdx = variants.indexOf(row.variant);
  const point = {
    value: [xIdx, yIdx, row.state === 'available' && typeof row.pass_rate === 'number' ? row.pass_rate : -1],
    rowId: row.id,
    state: row.state,
    stateReason: row.state_reason,
    scenariosTotal: row.scenarios_total,
    scenariosFailed: row.scenarios_failed,
  };
  if (row.state === 'available') {
    heatmapAvailable.push(point);
  } else {
    heatmapUnavailable.push(point);
  }
});
const heatmapData = [...heatmapAvailable, ...heatmapUnavailable];

renderChart(
  'tests-chart-heatmap',
  suites.length && variants.length
    ? {
        backgroundColor: 'transparent',
        grid: { left: 84, right: 28, top: 24, bottom: 88 },
        tooltip: {
          backgroundColor: 'rgba(15, 23, 42, 0.95)',
          borderColor: 'rgba(125, 211, 252, 0.35)',
          textStyle: { color: '#e2e8f0' },
          formatter: (params) => {
            const item = params.data;
            const suite = suites[item.value[0]];
            const variant = variants[item.value[1]];
            if (item.value[2] < 0) {
              return `<strong>${variant} / ${suite}</strong><br><span style="color:#fb7185">◆ Awaiting Evidence</span><br><em style="color:#94a3b8;font-size:0.85em">${item.stateReason ?? 'No completed run published yet.'}</em>`;
            }
            const rate = item.value[2];
            const color = rate >= 90 ? '#4ade80' : rate >= 60 ? '#fbbf24' : '#f87171';
            return `<strong>${variant} / ${suite}</strong><br>Pass rate: <span style="color:${color}">${rate}%</span><br>Failed: ${item.scenariosFailed}/${item.scenariosTotal}`;
          },
        },
        xAxis: {
          type: 'category',
          data: suites,
          axisLabel: { color: '#cbd5e1', interval: 0, rotate: 25 },
          axisLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.2)' } },
        },
        yAxis: {
          type: 'category',
          data: variants,
          axisLabel: { color: '#cbd5e1' },
          axisLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.2)' } },
        },
        visualMap: {
          orient: 'horizontal',
          left: 'center',
          bottom: 24,
          textStyle: { color: '#cbd5e1' },
          pieces: [
            { value: -1, label: 'Awaiting Evidence', color: '#1e2d4a' },
            { gt: -1, lte: 60, label: '0–60%', color: '#7f1d1d' },
            { gt: 60, lte: 90, label: '60–90%', color: '#c2410c' },
            { gt: 90, lte: 100, label: '90–100%', color: '#15803d' },
          ],
        },
        series: [
          {
            name: 'Pass rate',
            type: 'heatmap',
            label: {
              show: true,
              color: '#f8fafc',
              formatter: (params) => (params.data.value[2] < 0 ? '◆' : `${params.data.value[2]}%`),
            },
            data: heatmapAvailable,
            itemStyle: { borderColor: 'rgba(255,255,255,0.04)', borderWidth: 1, borderRadius: 4 },
            emphasis: { itemStyle: { borderColor: '#38bdf8', borderWidth: 2 } },
          },
          {
            name: 'Awaiting Evidence',
            type: 'scatter',
            coordinateSystem: 'cartesian2d',
            symbol: 'rect',
            symbolSize: [
              // fill the full cell — ECharts scatter on category axes needs explicit pixel size
              44,
              26,
            ],
            label: {
              show: true,
              color: '#fb7185',
              fontSize: 14,
              formatter: () => '◆',
            },
            data: heatmapUnavailable.map((d) => ({
              ...d,
              value: [d.value[0], d.value[1]],
            })),
            itemStyle: { color: '#1e2d4a', borderColor: 'rgba(251,113,133,0.25)', borderWidth: 1 },
            emphasis: { itemStyle: { borderColor: '#fb7185', borderWidth: 2 } },
            z: 2,
          },
        ],
      }
    : null,
  'No suite/variant rows published yet for the heatmap.',
);

const volumeRows = availableRows.filter((row) => Number.isFinite(row.scenarios_total));
renderChart(
  'tests-chart-volume',
  volumeRows.length
    ? {
        backgroundColor: 'transparent',
        legend: {
          top: 0,
          textStyle: { color: '#cbd5e1' },
          data: ['Scenarios', 'Failed', 'Pass rate'],
        },
        tooltip: {
          trigger: 'axis',
          axisPointer: { type: 'shadow' },
          backgroundColor: 'rgba(15, 23, 42, 0.95)',
          borderColor: 'rgba(125, 211, 252, 0.35)',
          textStyle: { color: '#e2e8f0' },
        },
        grid: { left: 64, right: 54, top: 48, bottom: 72 },
        xAxis: {
          type: 'category',
          data: volumeRows.map((row) => row.id),
          axisLabel: { color: '#cbd5e1', interval: 0, rotate: 25 },
          axisLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.2)' } },
        },
        yAxis: [
          {
            type: 'value',
            name: 'Scenarios',
            axisLabel: { color: '#94a3b8' },
            splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.12)' } },
          },
          {
            type: 'value',
            name: 'Pass rate',
            min: 0,
            max: 100,
            axisLabel: { color: '#94a3b8', formatter: '{value}%' },
          },
        ],
        series: [
          {
            name: 'Scenarios',
            type: 'bar',
            data: volumeRows.map((row) => row.scenarios_total),
            itemStyle: { color: '#38bdf8' },
          },
          {
            name: 'Failed',
            type: 'bar',
            data: volumeRows.map((row) => row.scenarios_failed),
            itemStyle: { color: '#f97316' },
          },
          {
            name: 'Pass rate',
            type: 'line',
            yAxisIndex: 1,
            data: volumeRows.map((row) => row.pass_rate),
            smooth: true,
            itemStyle: { color: '#4ade80' },
            lineStyle: { color: '#4ade80' },
          },
        ],
      }
    : null,
  'No completed rows published yet for scenario volume and failure charts.',
);

if (charts.length) {
  window.addEventListener('resize', () => {
    charts.forEach((chart) => chart.resize());
  });
}
