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

function normalizeTerminology(value) {
  if (!value) return value;
  return value.replace(/\blanes\b/gi, 'streams').replace(/\blane\b/gi, 'stream');
}

export function buildUpstreamPageModel(dataset, options = {}) {
  const includeGroups = new Set(options.includeGroups || []);
  const excludeGroups = new Set(options.excludeGroups || []);
  const hasIncludeFilter = includeGroups.size > 0;
  const groupOrder = options.groupOrder || [];
  const groupRank = new Map(groupOrder.map((id, index) => [id, index]));

  const shouldIncludeGroup = (groupId) => {
    if (excludeGroups.has(groupId)) return false;
    if (hasIncludeFilter && !includeGroups.has(groupId)) return false;
    return true;
  };

  const groupsById = new Map(
    (dataset.groups || []).map((group) => [
      group.id,
      {
        ...group,
        description: normalizeTerminology(group.description),
        lanes: [],
      },
    ]),
  );

  const lanes = (dataset.rows || []).filter((row) => shouldIncludeGroup(row.group)).map((row) => {
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
      state_reason: normalizeTerminology(row.state_reason),
    };
    groupsById.get(row.group)?.lanes.push(lane);
    return lane;
  });

  const groups = [...groupsById.values()].filter((group) => shouldIncludeGroup(group.id)).map((group) => {
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
  }).sort((left, right) => {
    const leftRank = groupRank.has(left.id) ? groupRank.get(left.id) : Number.MAX_SAFE_INTEGER;
    const rightRank = groupRank.has(right.id) ? groupRank.get(right.id) : Number.MAX_SAFE_INTEGER;
    if (leftRank !== rightRank) return leftRank - rightRank;
    return left.label.localeCompare(right.label);
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
      label: normalizeTerminology(metric.label),
      derivation: normalizeTerminology(metric.derivation),
      state_reason: normalizeTerminology(metric.state_reason),
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
