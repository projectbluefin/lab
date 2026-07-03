import * as echarts from 'echarts';

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

const renderUnavailable = (element, message) => {
  element.innerHTML = `<div class="sparkline-empty">${message}</div>`;
};

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
      formatter: (params) => {
        const [, duration, point] = durations[params[0].dataIndex];
        const when = point.started_at ? new Date(point.started_at).toLocaleString() : 'unknown time';
        return `${when}<br/>${duration} min · ${point.overall}`;
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
