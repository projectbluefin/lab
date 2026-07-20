import * as echarts from 'echarts';

const payloadNode = document.getElementById('provisioning-chart-data');
const payload = payloadNode ? JSON.parse(payloadNode.textContent ?? '{}') : {};
const nodes = Array.isArray(payload.nodes) ? payload.nodes : [];
const containerdisks = Array.isArray(payload.containerdisks) ? payload.containerdisks : [];
const charts = [];

const renderUnavailable = (element, message) => {
  element.innerHTML = `<div style="display:flex;align-items:center;justify-content:center;height:100%;color:#94a3b8;font-size:0.9rem;">${message}</div>`;
};

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

// 1. Hypervisor capacity bar chart
const capacityElement = document.getElementById('provisioning-capacity-chart');
if (capacityElement) {
  lazyInit(capacityElement, (el) => {
    if (nodes.length === 0) {
      renderUnavailable(el, 'No hypervisor node data available.');
      return;
    }

    const categories = nodes.map((n) => n.name);
    const ramData = nodes.map((n) => n.ram_gb || 0);
    const cpuData = nodes.map((n) => n.cpu_threads || 0);

    const chart = echarts.init(el);
    chart.setOption({
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
          axisLabel: { color: '#94a3b8' },
          nameTextStyle: { color: '#94a3b8' },
          splitLine: { lineStyle: { color: 'rgba(148, 163, 184, 0.1)' } },
        },
        {
          type: 'value',
          name: 'CPU Threads',
          axisLabel: { color: '#94a3b8' },
          nameTextStyle: { color: '#94a3b8' },
          splitLine: { show: false },
        },
      ],
      series: [
        {
          name: 'RAM (GiB)',
          type: 'bar',
          yAxisIndex: 0,
          data: ramData.map((val) => ({
            value: val,
            itemStyle: { color: '#38bdf8', borderRadius: [4, 4, 0, 0] },
          })),
          barMaxWidth: 30,
        },
        {
          name: 'CPU Threads',
          type: 'bar',
          yAxisIndex: 1,
          data: cpuData.map((val) => ({
            value: val,
            itemStyle: { color: '#a78bfa', borderRadius: [4, 4, 0, 0] },
          })),
          barMaxWidth: 30,
        },
      ],
    });
    charts.push(chart);
  });
}

// 2. Guest filesystem distribution donut chart
const fsElement = document.getElementById('provisioning-filesystems-chart');
if (fsElement) {
  lazyInit(fsElement, (el) => {
    const counts = {};
    for (const cd of containerdisks) {
      if (cd.filesystem) {
        counts[cd.filesystem] = (counts[cd.filesystem] || 0) + 1;
      }
    }
    const data = Object.entries(counts).map(([name, value]) => ({ name: name.toUpperCase(), value }));

    if (data.length === 0) {
      renderUnavailable(el, 'No containerDisk filesystem data available.');
      return;
    }

    const colorMap = {
      BTRFS: '#34d399',
      XFS: '#fbbf24',
      EXT4: '#a78bfa',
    };

    const chart = echarts.init(el);
    chart.setOption({
      backgroundColor: 'transparent',
      tooltip: {
        trigger: 'item',
        backgroundColor: 'rgba(15, 23, 42, 0.95)',
        borderColor: 'rgba(125, 211, 252, 0.3)',
        textStyle: { color: '#e2e8f0' },
        formatter: '{b}: {c} ({d}%)',
      },
      legend: {
        top: 0,
        textStyle: { color: '#cbd5e1', fontSize: 11 },
      },
      series: [
        {
          name: 'Guest Filesystem',
          type: 'pie',
          radius: ['40%', '70%'],
          center: ['50%', '55%'],
          avoidLabelOverlap: true,
          itemStyle: {
            borderRadius: 6,
            borderColor: 'rgba(15, 23, 42, 0.8)',
            borderWidth: 2,
          },
          label: {
            color: '#cbd5e1',
            fontSize: 11,
          },
          labelLine: {
            lineStyle: { color: 'rgba(148, 163, 184, 0.4)' },
          },
          data: data.map((item) => ({
            ...item,
            itemStyle: { color: colorMap[item.name] || '#94a3b8' },
          })),
        },
      ],
    });
    charts.push(chart);
  });
}

window.addEventListener('resize', () => {
  charts.forEach((chart) => chart.resize());
});
