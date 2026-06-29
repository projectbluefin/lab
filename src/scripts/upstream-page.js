import * as echarts from 'echarts';

function readModel() {
  const element = document.getElementById('upstream-page-data');
  if (!element?.textContent) return null;
  return JSON.parse(element.textContent);
}

function initAvailabilityChart(model) {
  const element = document.getElementById('upstream-availability-chart');
  if (!element) return null;

  const chart = echarts.init(element);
  chart.setOption({
    backgroundColor: 'transparent',
    grid: { left: 120, right: 24, top: 24, bottom: 24 },
    legend: {
      top: 0,
      textStyle: { color: '#e2e8f0' },
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
    },
    xAxis: {
      type: 'value',
      minInterval: 1,
      axisLabel: { color: '#94a3b8' },
      splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.14)' } },
    },
    yAxis: {
      type: 'category',
      data: model.charts.availability.categories,
      axisLabel: { color: '#e2e8f0' },
    },
    series: [
      {
        name: 'Available',
        type: 'bar',
        stack: 'availability',
        data: model.charts.availability.available,
        itemStyle: { color: '#4ade80', borderRadius: [0, 8, 8, 0] },
      },
      {
        name: 'Unavailable',
        type: 'bar',
        stack: 'availability',
        data: model.charts.availability.unavailable,
        itemStyle: { color: '#fb7185', borderRadius: [0, 8, 8, 0] },
      },
    ],
  });
  return chart;
}

function initFreshnessChart(model) {
  const element = document.getElementById('upstream-freshness-chart');
  if (!element) return null;

  const chart = echarts.init(element);
  chart.setOption({
    backgroundColor: 'transparent',
    grid: { left: 170, right: 28, top: 24, bottom: 28 },
    tooltip: {
      trigger: 'item',
      formatter(params) {
        if (params.seriesName === 'Unavailable') {
          return `<strong>${params.name}</strong><br>${params.data.stateReason}`;
        }
        return `<strong>${params.name}</strong><br>${params.value} day${params.value === 1 ? '' : 's'} old`;
      },
    },
    xAxis: {
      type: 'value',
      name: 'Days old',
      nameTextStyle: { color: '#94a3b8' },
      axisLabel: { color: '#94a3b8' },
      splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.14)' } },
    },
    yAxis: {
      type: 'category',
      data: model.charts.freshness.categories,
      axisLabel: { color: '#e2e8f0' },
    },
    series: [
      {
        name: 'Published freshness',
        type: 'bar',
        data: model.charts.freshness.available,
        itemStyle: { color: '#7dd3fc', borderRadius: [0, 8, 8, 0] },
      },
      {
        name: 'Unavailable',
        type: 'scatter',
        data: model.charts.freshness.unavailable,
        symbol: 'diamond',
        symbolSize: 14,
        label: {
          show: true,
          position: 'right',
          color: '#fb7185',
          formatter: 'Unavailable',
        },
        itemStyle: { color: '#fb7185' },
      },
    ],
  });
  return chart;
}

function initTimelineChart(model) {
  const element = document.getElementById('upstream-timeline-chart');
  if (!element) return null;

  const chart = echarts.init(element);
  chart.setOption({
    backgroundColor: 'transparent',
    legend: {
      top: 0,
      textStyle: { color: '#e2e8f0' },
    },
    grid: { left: 160, right: 24, top: 42, bottom: 32 },
    tooltip: {
      trigger: 'item',
      formatter(params) {
        const value = params.data.value;
        const freshness = params.data.freshnessAgeDays;
        const freshnessLabel = typeof freshness === 'number' ? `${freshness} day${freshness === 1 ? '' : 's'} old` : 'Unavailable';
        return `<strong>${params.name}</strong><br>${params.seriesName}<br>${value[0]}<br>${freshnessLabel}`;
      },
    },
    xAxis: {
      type: 'time',
      axisLabel: { color: '#94a3b8' },
      splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.14)' } },
    },
    yAxis: {
      type: 'category',
      data: model.charts.timeline.categories,
      axisLabel: { color: '#e2e8f0' },
    },
    series: model.charts.timeline.series.map((series) => ({
      ...series,
      type: 'scatter',
      symbolSize: 16,
    })),
  });
  return chart;
}

const model = readModel();
if (model) {
  const charts = [initAvailabilityChart(model), initFreshnessChart(model), initTimelineChart(model)].filter(Boolean);
  if (charts.length) {
    window.addEventListener('resize', () => {
      charts.forEach((chart) => chart.resize());
    });
  }
}
