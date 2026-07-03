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
      textStyle: { color: '#cbd5e1' },
    },
    grid: { left: 160, right: 30, top: 40, bottom: 30 },
    tooltip: {
      trigger: 'item',
      backgroundColor: 'rgba(15, 23, 42, 0.95)',
      borderColor: 'rgba(255, 255, 255, 0.1)',
      textStyle: { color: '#cbd5e1' },
      formatter(params) {
        const value = params.data.value;
        const dateStr = new Date(value[0]).toLocaleDateString('en-US', {
          dateStyle: 'medium',
          timeZone: 'UTC'
        });
        const freshness = params.data.freshnessAgeDays;
        const freshnessLabel = typeof freshness === 'number' ? `${freshness} day${freshness === 1 ? '' : 's'} old` : 'Unavailable';
        return `<strong>${params.name}</strong><br/>` +
               `<span style="color:${params.color}">●</span> ${params.seriesName}<br/>` +
               `Released: ${dateStr}<br/>` +
               `Age: ${freshnessLabel}`;
      },
    },
    xAxis: {
      type: 'time',
      axisLabel: { color: '#94a3b8', fontSize: 10 },
      splitLine: { lineStyle: { color: 'rgba(255, 255, 255, 0.04)' } },
    },
    yAxis: {
      type: 'category',
      data: model.charts.timeline.categories,
      axisLabel: { color: '#cbd5e1', fontSize: 10 },
      splitLine: { show: true, lineStyle: { color: 'rgba(255, 255, 255, 0.03)' } }
    },
    series: model.charts.timeline.series.map((series) => ({
      ...series,
      type: 'effectScatter',
      showEffectOn: 'render',
      rippleEffect: {
        brushType: 'stroke',
        scale: 2.2,
        period: 4
      },
      symbolSize: 12,
    })),
  });
  return chart;
}

function initDistributionChart(model) {
  const element = document.getElementById('upstream-distribution-chart');
  if (!element || !model.charts.distribution) return null;

  const chart = echarts.init(element);
  chart.setOption({
    backgroundColor: 'transparent',
    tooltip: {
      trigger: 'item',
      formatter: '{b}: <strong>{c}</strong> ({d}%)',
      backgroundColor: 'rgba(15, 23, 42, 0.9)',
      borderColor: 'rgba(255, 255, 255, 0.1)',
      textStyle: { color: '#cbd5e1' }
    },
    legend: {
      top: '5%',
      left: 'center',
      textStyle: { color: '#cbd5e1', fontSize: 10 }
    },
    series: [
      {
        name: 'Streams by Family',
        type: 'pie',
        radius: ['35%', '65%'],
        center: ['50%', '60%'],
        avoidLabelOverlap: false,
        itemStyle: {
          borderRadius: 8,
          borderColor: '#0f172a',
          borderWidth: 2
        },
        label: { show: false },
        emphasis: {
          label: {
            show: true,
            fontSize: '12',
            fontWeight: 'bold',
            color: '#ffffff'
          }
        },
        data: model.charts.distribution.data,
        color: ['#7dd3fc', '#a78bfa', '#f59e0b', '#38bdf8']
      }
    ]
  });
  return chart;
}

function initBracketsChart(model) {
  const element = document.getElementById('upstream-brackets-chart');
  if (!element || !model.charts.freshnessBrackets) return null;

  const chart = echarts.init(element);
  chart.setOption({
    backgroundColor: 'transparent',
    tooltip: {
      trigger: 'item',
      formatter: '{b}: <strong>{c}</strong> ({d}%)',
      backgroundColor: 'rgba(15, 23, 42, 0.9)',
      borderColor: 'rgba(255, 255, 255, 0.1)',
      textStyle: { color: '#cbd5e1' }
    },
    legend: {
      top: '5%',
      left: 'center',
      textStyle: { color: '#cbd5e1', fontSize: 10 }
    },
    series: [
      {
        name: 'Freshness Brackets',
        type: 'pie',
        radius: ['35%', '65%'],
        center: ['50%', '60%'],
        avoidLabelOverlap: false,
        itemStyle: {
          borderRadius: 8,
          borderColor: '#0f172a',
          borderWidth: 2
        },
        label: { show: false },
        emphasis: {
          label: {
            show: true,
            fontSize: '12',
            fontWeight: 'bold',
            color: '#ffffff'
          }
        },
        data: model.charts.freshnessBrackets.data,
        color: ['#10b981', '#3b82f6', '#f59e0b', '#ef4444']
      }
    ]
  });
  return chart;
}

const model = readModel();
if (model) {
  const charts = [
    initAvailabilityChart(model),
    initFreshnessChart(model),
    initTimelineChart(model),
    initDistributionChart(model),
    initBracketsChart(model)
  ].filter(Boolean);
  if (charts.length) {
    window.addEventListener('resize', () => {
      charts.forEach((chart) => chart.resize());
    });
  }
}
