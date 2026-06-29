const dateFormatter = new Intl.DateTimeFormat('en-US', {
  dateStyle: 'medium',
  timeStyle: 'short',
  timeZone: 'UTC',
});

function formatDate(value) {
  if (!value) return 'Unavailable';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? 'Unavailable' : `${dateFormatter.format(date)} UTC`;
}

function formatMetricValue(metric) {
  if (metric.value == null) return 'Unavailable';
  if (metric.unit === 'count') return String(metric.value);
  return `${metric.value} ${metric.unit}`;
}

function formatFreshness(value) {
  if (typeof value !== 'number' || Number.isNaN(value)) return 'Unavailable';
  if (value === 0) return '0 days';
  if (value === 1) return '1 day';
  return `${value} days`;
}

function titleizeLane(value) {
  if (!value) return 'Unknown lane';
  return value.replace(/^[a-z]/, (char) => char.toUpperCase());
}

function stateTone(state) {
  if (state === 'available') return 'good';
  if (state === 'partial') return 'warn';
  return 'bad';
}

export function buildUpstreamPageModel(dataset) {
  const groupsById = new Map(
    (dataset.groups || []).map((group) => [
      group.id,
      {
        ...group,
        lanes: [],
      },
    ]),
  );

  const lanes = (dataset.rows || []).map((row) => {
    const lane = {
      ...row,
      label: titleizeLane(row.display_name),
      groupLabel: groupsById.get(row.group)?.label || row.group,
      publisherLabel: row.publisher_repo || 'Repo-owned placeholder',
      publishedLabel: formatDate(row.published_at),
      freshnessLabel: formatFreshness(row.freshness_age_days),
      stateTone: stateTone(row.state),
      evidenceUrl: row.source_url,
      hasEvidence: Boolean(row.source_url),
    };
    groupsById.get(row.group)?.lanes.push(lane);
    return lane;
  });

  const groups = [...groupsById.values()].map((group) => {
    const availableCount = group.lanes.filter((lane) => lane.state === 'available').length;
    const unavailableCount = group.lanes.length - availableCount;
    const freshestLane = [...group.lanes]
      .filter((lane) => typeof lane.freshness_age_days === 'number')
      .sort((left, right) => left.freshness_age_days - right.freshness_age_days)[0] || null;

    return {
      ...group,
      lanes: [...group.lanes].sort((left, right) => left.label.localeCompare(right.label)),
      availableCount,
      unavailableCount,
      totalCount: group.lanes.length,
      freshestLaneLabel: freshestLane ? `${freshestLane.label} · ${freshestLane.freshnessLabel}` : 'No published release yet',
      stateTone: unavailableCount ? (availableCount ? 'warn' : 'bad') : 'good',
    };
  });

  const missingLanes = lanes.filter((lane) => lane.state !== 'available');
  const publishedLanes = lanes.filter((lane) => lane.published_at);

  return {
    meta: {
      ...dataset._meta,
      generatedLabel: formatDate(dataset._meta?.generated_at),
      stateTone: stateTone(dataset._meta?.status),
    },
    summaryMetrics: (dataset.summary_metrics || []).map((metric) => ({
      ...metric,
      displayValue: formatMetricValue(metric),
      collectedLabel: formatDate(metric.collected_at),
      stateTone: stateTone(metric.state),
    })),
    groups,
    lanes,
    missingLanes,
    charts: {
      availability: {
        categories: groups.map((group) => group.label),
        available: groups.map((group) => group.availableCount),
        unavailable: groups.map((group) => group.unavailableCount),
      },
      freshness: {
        categories: lanes.map((lane) => lane.label),
        available: lanes.map((lane) => (lane.state === 'available' ? lane.freshness_age_days : null)),
        unavailable: missingLanes.map((lane) => ({
          name: lane.label,
          value: [0, lane.label],
          stateReason: lane.state_reason,
        })),
      },
      timeline: {
        categories: lanes.map((lane) => lane.label),
        series: groups.map((group) => ({
          name: group.label,
          data: publishedLanes
            .filter((lane) => lane.group === group.id)
            .map((lane) => ({
              name: lane.label,
              value: [lane.published_at, lane.label],
              freshnessAgeDays: lane.freshness_age_days,
              branch: lane.branch,
            })),
        })),
      },
    },
  };
}
