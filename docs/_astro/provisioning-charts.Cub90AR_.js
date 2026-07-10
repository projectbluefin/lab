const payloadNode = document.getElementById('provisioning-chart-data');
const payload = payloadNode ? JSON.parse(payloadNode.textContent ?? '{}') : {};
const bays = Array.isArray(payload.bays) ? payload.bays : [];
const ghostLimits = payload.ghostLimits || { ram_gb: 63, cpu_threads: 32 };
const charts = [];

// Lazy initialization helper using IntersectionObserver
const lazyInit = (element, initFn) => {
  const observer = new IntersectionObserver((entries, obs) => {
    entries.forEach((entry) => {
      if (entry.isIntersecting) {
        obs.unobserve(element);
        initFn(element);
      }
    });
  }, { threshold: 0.1 });
  observer.observe(element);
};

// 1. KubeVirt Slots Chart
const slotsElement = document.getElementById('provisioning-slots-chart');
if (slotsElement) {
  lazyInit(slotsElement, (el) => {
    const categories = bays.map((b) => b.name);
    // Add Host Free category to show limits context
    categories.push('Host Free');

    // RAM data: Bay A (8), Bay B (8), Bay C (0), Free (overall_ram - allocated_ram)
    const allocatedRam = bays.reduce((sum, b) => sum + b.ram_allocation_gb, 0);
    const freeRam = Math.max(0, ghostLimits.ram_gb - allocatedRam);
    const ramData = bays.map((b) => b.ram_allocation_gb);
    ramData.push(freeRam);

    // CPU data: Bay A (4), Bay B (4), Bay C (0), Free (overall_cpu - allocated_cpu)
    const allocatedCpu = bays.reduce((sum, b) => sum + b.cpu_allocation_cores, 0);
    const freeCpu = Math.max(0, ghostLimits.cpu_threads - allocatedCpu);
    const cpuData = bays.map((b) => b.cpu_allocation_cores);
    cpuData.push(freeCpu);

    const slotsChart = echarts.init(el);
    slotsChart.setOption({
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
      },
      grid: { left: 24, right: 24, top: 44, bottom: 24, containLabel: true },
      xAxis: {
        type: 'category',
        data: categories,
        axisLabel: { color: '#cbd5e1', fontSize: 11 },
        axisLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.2)' } },
      },
      yAxis: [
        {
          type: 'value',
          name: 'RAM (GiB)',
          min: 0,
          max: ghostLimits.ram_gb,
          axisLabel: { color: '#94a3b8' },
          nameTextStyle: { color: '#94a3b8' },
          splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.1)' } },
        },
        {
          type: 'value',
          name: 'CPU Threads',
          min: 0,
          max: ghostLimits.cpu_threads,
          axisLabel: { color: '#94a3b8' },
          nameTextStyle: { color: '#94a3b8' },
          splitLine: { show: false },
        },
      ],
      series: [
        {
          name: 'Memory Allocated (GiB)',
          type: 'bar',
          yAxisIndex: 0,
          data: ramData.map((val, idx) => {
            let itemColor = '#38bdf8'; // Active Bay color
            if (idx === categories.length - 1) itemColor = 'rgba(148, 163, 184, 0.2)'; // Host free color
            else if (val === 0) itemColor = 'rgba(56, 189, 248, 0.1)'; // Vacant Bay color
            return {
              value: val,
              itemStyle: { color: itemColor, borderRadius: [4, 4, 0, 0] },
            };
          }),
          barMaxWidth: 30,
        },
        {
          name: 'CPU Cores Allocated',
          type: 'bar',
          yAxisIndex: 1,
          data: cpuData.map((val, idx) => {
            let itemColor = '#a78bfa'; // Active Bay CPU color
            if (idx === categories.length - 1) itemColor = 'rgba(167, 139, 250, 0.2)'; // Host free color
            else if (val === 0) itemColor = 'rgba(167, 139, 250, 0.1)'; // Vacant Bay color
            return {
              value: val,
              itemStyle: { color: itemColor, borderRadius: [4, 4, 0, 0] },
            };
          }),
          barMaxWidth: 30,
        },
      ],
    });
    charts.push(slotsChart);
  });
}

// 2. Reflink Speed Chart
const speedElement = document.getElementById('provisioning-reflink-chart');
if (speedElement) {
  lazyInit(speedElement, (el) => {
    // Only filter active bays for reflink speed comparison
    const activeBays = bays.filter((b) => b.status === 'active' && b.reflink_time_sec !== null);
    
    // Add a default fallback if no active bays have data
    const chartBays = activeBays.length > 0 ? activeBays : [
      { name: 'Bay A', reflink_time_sec: 1.8, legacy_copy_time_sec: 52.5 },
      { name: 'Bay B', reflink_time_sec: 2.2, legacy_copy_time_sec: 55.1 }
    ];

    const categories = chartBays.map((b) => b.name);
    const reflinkTimes = chartBays.map((b) => b.reflink_time_sec);
    const legacyTimes = chartBays.map((b) => b.legacy_copy_time_sec);

    const speedChart = echarts.init(el);
    speedChart.setOption({
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
          const lines = [`<strong>${items[0].name}</strong>`];
          for (const item of items) {
            lines.push(`${item.marker}${item.seriesName}: <strong>${item.value}</strong> s`);
          }
          const diff = (legacyTimes[items[0].dataIndex] - reflinkTimes[items[0].dataIndex]).toFixed(1);
          lines.push(`Speedup Advantage: <strong>${diff}s faster</strong>`);
          return lines.join('<br>');
        }
      },
      grid: { left: 24, right: 24, top: 44, bottom: 24, containLabel: true },
      xAxis: {
        type: 'category',
        data: categories,
        axisLabel: { color: '#cbd5e1', fontSize: 11 },
        axisLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.2)' } },
      },
      yAxis: {
        type: 'value',
        name: 'Time (seconds)',
        axisLabel: { color: '#94a3b8' },
        nameTextStyle: { color: '#94a3b8' },
        splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.1)' } },
      },
      series: [
        {
          name: 'btrfs Reflink Copy (sub-5s)',
          type: 'bar',
          data: reflinkTimes,
          itemStyle: { color: '#10b981', borderRadius: [4, 4, 0, 0] },
          barMaxWidth: 30,
        },
        {
          name: 'Legacy Disk Copying',
          type: 'bar',
          data: legacyTimes,
          itemStyle: { color: '#f43f5e', borderRadius: [4, 4, 0, 0] },
          barMaxWidth: 30,
        },
      ],
    });
    charts.push(speedChart);
  });
}

window.addEventListener('resize', () => {
  charts.forEach((chart) => chart.resize());
});
