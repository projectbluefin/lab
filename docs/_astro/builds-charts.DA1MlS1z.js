const payloadNode = document.getElementById('builds-chart-data');
const payload = payloadNode ? JSON.parse(payloadNode.textContent ?? '{}') : {};
const rows = Array.isArray(payload.rows) ? payload.rows : [];
const charts = [];

const STATUS_COLOR = {
  passed: '#4ade80',
  fail: '#f87171',
  running: '#38bdf8',
  pending: '#94a3b8',
};

const SOURCE_BADGE = {
  'github-actions': { label: 'GitHub Actions', color: '#7dd3fc' },
  argo: { label: 'Argo Workflows', color: '#a78bfa' },
};

const renderUnavailable = (element, message) => {
  element.innerHTML = `<div class="sparkline-empty">${message}</div>`;
};

// ── Reliability overview chart (success rate + avg duration per pipeline) ──
const overviewElement = document.getElementById('builds-chart-overview');
if (overviewElement) {
  const availableRows = rows.filter((row) => row.success_rate !== null && row.success_rate !== undefined);
  if (availableRows.length >= 1) {
    const labels = availableRows.map((row) => row.display_name || row.id);
    const successRates = availableRows.map((row) => row.success_rate ?? 0);
    const avgDurations = availableRows.map((row) => row.avg_duration_min ?? null);

    const overviewChart = echarts.init(overviewElement);
    overviewChart.setOption({
      backgroundColor: 'transparent',
      legend: {
        top: 0,
        textStyle: { color: '#cbd5e1', fontSize: 11 },
      },
      tooltip: {
        trigger: 'axis',
        backgroundColor: 'rgba(15, 23, 42, 0.95)',
        borderColor: 'rgba(125, 211, 252, 0.3)',
        textStyle: { color: '#e2e8f0' },
        axisPointer: { type: 'shadow' },
        formatter: (items) => {
          const idx = items[0].dataIndex;
          const row = availableRows[idx];
          const sourceInfo = SOURCE_BADGE[row.source] || SOURCE_BADGE['github-actions'];
          const lines = [`<strong>${row.display_name || row.id}</strong>`,
            `<span style="color:${sourceInfo.color}">● ${sourceInfo.label}</span>`];
          for (const item of items) {
            lines.push(`${item.marker}${item.seriesName}: <strong>${item.value ?? '—'}</strong>${item.seriesIndex === 0 ? '%' : ' min'}`);
          }
          if (row.runs_tracked) lines.push(`Runs tracked: ${row.runs_tracked}`);
          return lines.join('<br>');
        },
      },
      grid: { left: 24, right: 64, top: 44, bottom: 100, containLabel: true },
      xAxis: {
        type: 'category',
        data: labels,
        axisLabel: {
          color: '#cbd5e1',
          interval: 0,
          rotate: 28,
          fontSize: 10,
          formatter: (val) => (val.length > 20 ? `${val.slice(0, 17)}…` : val),
        },
        axisLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.2)' } },
      },
      yAxis: [
        {
          type: 'value',
          name: 'Success %',
          min: 0,
          max: 100,
          axisLabel: { color: '#94a3b8', formatter: '{value}%' },
          splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.1)' } },
        },
        {
          type: 'value',
          name: 'Avg min',
          axisLabel: { color: '#94a3b8' },
          splitLine: { show: false },
        },
      ],
      series: [
        {
          name: 'Success rate',
          type: 'bar',
          yAxisIndex: 0,
          data: successRates.map((rate) => ({
            value: rate,
            itemStyle: {
              color: rate >= 90 ? '#4ade80' : rate >= 60 ? '#fbbf24' : '#f87171',
              borderRadius: [4, 4, 0, 0],
            },
          })),
          barMaxWidth: 36,
        },
        {
          name: 'Avg duration',
          type: 'line',
          yAxisIndex: 1,
          data: avgDurations,
          smooth: true,
          showSymbol: true,
          symbolSize: 6,
          lineStyle: { color: '#38bdf8', width: 2 },
          itemStyle: { color: '#38bdf8' },
          connectNulls: false,
        },
      ],
    });
    charts.push(overviewChart);
  } else {
    renderUnavailable(overviewElement, 'No pipelines with published run history yet — reliability overview unavailable.');
  }
}

// ── Per-pipeline sparklines ──────────────────────────────────────────────────
// Each build row gets its own tiny ECharts instance: a duration-per-run
// sparkline with hidden axes, colored points per run outcome. Only rendered
// for rows with at least 2 published history points; single/zero-point rows
// get an explicit unavailable state instead of a fabricated flat line.
rows.forEach((row) => {
  const element = document.getElementById(`sparkline-${row.id}`);
  if (!element) {
    return;
  }

  const points = Array.isArray(row.history_points) ? row.history_points : [];
  const durations = points
    .map((point, index) => [index, point.duration_min, point])
    .filter(([, duration]) => Number.isFinite(duration));

  if (durations.length < 2) {
    renderUnavailable(
      element,
      durations.length === 0
        ? 'No published runs yet'
        : 'Only one published run — need 2+ for a trend',
    );
    return;
  }

  const chart = echarts.init(element);
  chart.setOption({
    grid: { left: 4, right: 4, top: 6, bottom: 6 },
    xAxis: {
      type: 'category',
      show: false,
      data: durations.map(([index]) => index),
    },
    yAxis: { type: 'value', show: false },
    tooltip: {
      trigger: 'axis',
      backgroundColor: 'rgba(15, 23, 42, 0.9)',
      borderColor: 'rgba(125, 211, 252, 0.25)',
      textStyle: { color: '#e2e8f0', fontSize: 11 },
      formatter: (params) => {
        const [, duration, point] = durations[params[0].dataIndex];
        const when = point.started_at ? new Date(point.started_at).toLocaleString() : 'unknown time';
        const statusColor = STATUS_COLOR[point.overall] ?? '#94a3b8';
        return `${when}<br/><span style="color:${statusColor}">●</span> ${duration} min · ${point.overall}`;
      },
    },
    series: [
      {
        type: 'line',
        showSymbol: true,
        symbolSize: 5,
        smooth: true,
        lineStyle: { width: 2, color: '#38bdf8' },
        itemStyle: {
          color: (item) => {
            const point = durations[item.dataIndex]?.[2];
            return STATUS_COLOR[point?.overall] ?? '#94a3b8';
          },
        },
        data: durations.map(([, duration]) => duration),
      },
    ],
  });
  charts.push(chart);
});

window.addEventListener('resize', () => {
  charts.forEach((chart) => chart.resize());
});
