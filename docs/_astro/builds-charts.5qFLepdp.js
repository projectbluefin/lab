const payloadNode = document.getElementById('builds-chart-data');
const payload = payloadNode ? JSON.parse(payloadNode.textContent ?? '{}') : {};
const runs = Array.isArray(payload.runs) ? payload.runs : [];
const lanes = payload.lanes || { publish: [], lab: [] };
const laneMeta = payload.laneMeta || {};
const charts = [];

const STATUS_COLOR = {
  passed: '#4ade80',
  failed: '#f87171',
  running: '#38bdf8',
};

const TREND_COLOR = '#38bdf8';
const P50_COLOR = '#94a3b8';
const BAND_COLOR = 'rgba(56, 189, 248, 0.18)';

const renderUnavailable = (element, message) => {
  element.innerHTML = `<div class="chart-empty">${message}</div>`;
};

const parseTime = (value) => {
  if (!value) return null;
  const ms = new Date(value).getTime();
  return Number.isFinite(ms) ? ms : null;
};

const groupBy = (items, key) => {
  const map = {};
  for (const item of items) {
    const value = item[key];
    if (!map[value]) map[value] = [];
    map[value].push(item);
  }
  return map;
};

const percentile = (values, p) => {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  if (sorted.length === 1) return sorted[0];
  const idx = (sorted.length - 1) * p;
  const lower = Math.floor(idx);
  const upper = Math.ceil(idx);
  if (lower === upper) return sorted[lower];
  return sorted[lower] * (upper - idx) + sorted[upper] * (idx - lower);
};

const commonChartOptions = (extra = {}) => ({
  backgroundColor: 'transparent',
  textStyle: { fontFamily: 'Inter, sans-serif' },
  tooltip: {
    trigger: 'axis',
    backgroundColor: 'rgba(15, 23, 42, 0.95)',
    borderColor: 'rgba(125, 211, 252, 0.35)',
    textStyle: { color: '#e2e8f0' },
  },
  grid: { left: 56, right: 24, top: 24, bottom: 56, containLabel: false },
  xAxis: {
    type: 'time',
    axisLabel: { color: '#94a3b8', fontSize: 11 },
    axisLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.2)' } },
    splitLine: { show: false },
  },
  yAxis: {
    type: 'value',
    name: 'min',
    nameTextStyle: { color: '#64748b', padding: [0, 0, 0, -32] },
    axisLabel: { color: '#94a3b8', fontSize: 11 },
    splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.1)' } },
  },
  ...extra,
});

const renderLaneChart = (lane, plane) => {
  const element = document.getElementById(`builds-chart-${plane}-lane-${lane}`);
  if (!element) return;

  const laneRuns = (groupBy(runs, 'lane')[lane] || [])
    .filter((r) => r.plane === plane)
    .sort((a, b) => parseTime(a.started_at) - parseTime(b.started_at));

  if (laneRuns.length === 0) {
    renderUnavailable(element, 'No terminal runs recorded for this lane.');
    return;
  }

  const times = laneRuns.map((r) => parseTime(r.started_at));
  const durations = laneRuns.map((r) => r.duration_min);

  const p50 = [];
  const p95 = [];
  const bandFloor = [];
  const bandSpread = [];

  for (let i = 0; i < laneRuns.length; i++) {
    const windowStart = Math.max(0, i - 4);
    const window = laneRuns.slice(windowStart, i + 1).map((r) => r.duration_min);
    const median = percentile(window, 0.5);
    const upper = percentile(window, 0.95);
    const t = times[i];
    p50.push([t, median]);
    p95.push([t, upper]);
    bandFloor.push([t, median]);
    bandSpread.push([t, upper !== null && median !== null ? upper - median : null]);
  }

  const showBand = laneRuns.length >= 5;
  const meta = laneMeta[lane] || {};

  const series = [
    {
      name: 'Duration',
      type: 'line',
      showSymbol: laneRuns.length <= 30,
      symbolSize: 6,
      smooth: false,
      lineStyle: { color: TREND_COLOR, width: 2 },
      itemStyle: { color: TREND_COLOR },
      data: laneRuns.map((r) => [parseTime(r.started_at), r.duration_min, r]),
    },
  ];

  if (showBand) {
    series.push(
      {
        name: 'p50',
        type: 'line',
        showSymbol: false,
        smooth: true,
        lineStyle: { color: P50_COLOR, width: 2, type: 'dashed' },
        itemStyle: { color: P50_COLOR },
        data: p50,
      },
      {
        name: 'p50 floor',
        type: 'line',
        stack: 'band',
        showSymbol: false,
        smooth: true,
        lineStyle: { opacity: 0 },
        data: bandFloor,
      },
      {
        name: 'p50–p95 band',
        type: 'line',
        stack: 'band',
        showSymbol: false,
        smooth: true,
        lineStyle: { opacity: 0 },
        areaStyle: { color: BAND_COLOR },
        data: bandSpread,
      },
    );
  }

  const chart = echarts.init(element);
  chart.setOption(
    commonChartOptions({
      tooltip: {
        trigger: 'axis',
        backgroundColor: 'rgba(15, 23, 42, 0.95)',
        borderColor: 'rgba(125, 211, 252, 0.35)',
        textStyle: { color: '#e2e8f0' },
        formatter: (items) => {
          const lines = [];
          const date = items[0]?.axisValueLabel ?? '';
          lines.push(`<strong>${meta.display_name || lane}</strong><br/>${date}`);
          for (const item of items) {
            if (item.seriesName === 'p50 floor' || item.seriesName === 'p50–p95 band') continue;
            const val = item.value?.[1] ?? item.value;
            if (val == null) continue;
            const unit = item.seriesName === 'Duration' && item.data?.[2]?.status
              ? ` · ${item.data[2].status}`
              : '';
            lines.push(`${item.marker}${item.seriesName}: <strong>${val} min</strong>${unit}`);
          }
          return lines.join('<br/>');
        },
      },
      legend: {
        top: 0,
        right: 0,
        textStyle: { color: '#cbd5e1', fontSize: 11 },
        data: showBand ? ['Duration', 'p50', 'p50–p95 band'] : ['Duration'],
      },
      series,
    }),
  );
  charts.push(chart);

  if (!showBand) {
    const note = document.createElement('div');
    note.className = 'chart-panel__note';
    note.textContent = `${laneRuns.length} run${laneRuns.length === 1 ? '' : 's'} — percentile band needs 5+`;
    element.parentElement.appendChild(note);
  }
};

lanes.publish.forEach((lane) => renderLaneChart(lane, 'publish'));
lanes.lab.forEach((lane) => renderLaneChart(lane, 'lab'));

// ── Daily build outcomes (stacked area: passed/failed per day) ──
const dailyElement = document.getElementById('builds-chart-daily-outcomes');
if (dailyElement) {
  const byDay = {};
  for (const run of runs) {
    const day = run.started_at ? run.started_at.slice(0, 10) : null;
    if (!day) continue;
    const bucket = byDay[day] || { date: day, passed: 0, failed: 0 };
    if (run.status === 'passed') bucket.passed += 1;
    if (run.status === 'failed') bucket.failed += 1;
    byDay[day] = bucket;
  }

  const days = Object.values(byDay).sort((a, b) => a.date.localeCompare(b.date));

  if (days.length === 0) {
    renderUnavailable(dailyElement, 'No terminal runs recorded for daily outcome chart.');
  } else {
    const chart = echarts.init(dailyElement);
    chart.setOption({
      backgroundColor: 'transparent',
      textStyle: { fontFamily: 'Inter, sans-serif' },
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'cross' },
        backgroundColor: 'rgba(15, 23, 42, 0.95)',
        borderColor: 'rgba(125, 211, 252, 0.35)',
        textStyle: { color: '#e2e8f0' },
        formatter: (items) => {
          const date = items[0]?.axisValueLabel ?? '';
          const lines = [`<strong>${date}</strong>`];
          for (const item of items) {
            lines.push(`${item.marker}${item.seriesName}: <strong>${item.value}</strong>`);
          }
          return lines.join('<br/>');
        },
      },
      legend: {
        top: 0,
        textStyle: { color: '#cbd5e1', fontSize: 12 },
      },
      grid: { left: 48, right: 24, top: 40, bottom: 40, containLabel: false },
      xAxis: {
        type: 'category',
        data: days.map((d) => d.date),
        axisLabel: { color: '#94a3b8', fontSize: 11 },
        axisLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.2)' } },
      },
      yAxis: {
        type: 'value',
        name: 'runs',
        nameTextStyle: { color: '#64748b' },
        axisLabel: { color: '#94a3b8', fontSize: 11 },
        splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.1)' } },
      },
      series: [
        {
          name: 'Passed',
          type: 'bar',
          stack: 'total',
          data: days.map((d) => d.passed),
          itemStyle: { color: STATUS_COLOR.passed, borderRadius: [0, 0, 0, 0] },
          areaStyle: { color: STATUS_COLOR.passed },
        },
        {
          name: 'Failed',
          type: 'bar',
          stack: 'total',
          data: days.map((d) => d.failed),
          itemStyle: { color: STATUS_COLOR.failed, borderRadius: [4, 4, 0, 0] },
          areaStyle: { color: STATUS_COLOR.failed },
        },
      ],
    });
    charts.push(chart);
  }
}

// Single resize owner for all charts on this page.
if (charts.length) {
  window.addEventListener('resize', () => {
    charts.forEach((chart) => chart.resize());
  });
}
