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

function initPollerHeatmapChart(model) {
  const element = document.getElementById('upstream-poller-heatmap-chart');
  if (!element || !model.charts.pollerHeatmap) return null;

  const chart = echarts.init(element);
  chart.setOption({
    backgroundColor: 'transparent',
    tooltip: {
      position: 'top',
      backgroundColor: 'rgba(15, 23, 42, 0.95)',
      borderColor: 'rgba(125, 211, 252, 0.35)',
      textStyle: { color: '#e2e8f0' },
      formatter(params) {
        const value = params.value;
        const dateStr = value[0];
        const count = value[1];
        return `<strong>${dateStr}</strong><br/>Polls: ${count}`;
      }
    },
    visualMap: {
      min: 0,
      max: 24,
      type: 'continuous',
      orient: 'horizontal',
      left: 'center',
      bottom: 0,
      text: ['High Density', 'Low Density'],
      textStyle: { color: '#94a3b8' },
      inRange: {
        color: ['#1e293b', '#0369a1', '#0ea5e9', '#38bdf8']
      }
    },
    calendar: {
      top: 40,
      bottom: 40,
      left: 60,
      right: 30,
      cellSize: ['auto', 13],
      range: '2026',
      itemStyle: {
        color: '#1e293b',
        borderWidth: 1,
        borderColor: '#0f172a'
      },
      splitLine: {
        show: true,
        lineStyle: {
          color: '#0f172a',
          width: 2,
          type: 'solid'
        }
      },
      yearLabel: { show: false },
      dayLabel: {
        firstDay: 1,
        color: '#94a3b8',
        nameMap: ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']
      },
      monthLabel: {
        color: '#94a3b8',
        nameMap: 'en'
      }
    },
    series: {
      type: 'heatmap',
      coordinateSystem: 'calendar',
      data: model.charts.pollerHeatmap.data
    }
  });
  return chart;
}

const model = readModel();
const charts = [];

function lazyInit(id, initFn) {
  const element = document.getElementById(id);
  if (!element) return;

  const observer = new IntersectionObserver((entries, obs) => {
    entries.forEach((entry) => {
      if (entry.isIntersecting) {
        obs.unobserve(element);
        const chart = initFn();
        if (chart) {
          charts.push(chart);
        }
      }
    });
  }, { rootMargin: '100px' });
  observer.observe(element);
}

if (model) {
  lazyInit('upstream-availability-chart', () => initAvailabilityChart(model));
  lazyInit('upstream-freshness-chart', () => initFreshnessChart(model));
  lazyInit('upstream-timeline-chart', () => initTimelineChart(model));
  lazyInit('upstream-distribution-chart', () => initDistributionChart(model));
  lazyInit('upstream-brackets-chart', () => initBracketsChart(model));
  lazyInit('upstream-poller-heatmap-chart', () => initPollerHeatmapChart(model));

  window.addEventListener('resize', () => {
    charts.forEach((chart) => chart.resize());
  });
}
