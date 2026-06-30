import { readFileSync } from 'node:fs';

interface SummaryMetric {
  id: string;
  label: string;
  value: number;
  unit: string;
  state: string;
  state_reason: string | null;
  source_url: string;
  collected_at: string;
  derivation: string;
}

interface TapEntry {
  id: string;
  name: string;
  url: string;
  description: string | null;
  state: string;
  state_reason: string | null;
  source_url: string;
  collected_at: string;
  derivation: string;
}

interface HomebrewRow {
  id: string;
  variant: string;
  branch: string;
  tap_name: string | null;
  tap_url: string | null;
  install_count: number | null;
  download_count: number | null;
  state: string;
  state_reason: string;
  source_url: string;
  collected_at: string;
  derivation: string;
}

interface HomebrewDataset {
  schema_version: string;
  _meta: {
    page: string;
    description: string;
    generated_at: string;
    starter_artifact: boolean;
    status: string;
  };
  summary_metrics: SummaryMetric[];
  taps: TapEntry[];
  rows: HomebrewRow[];
}

export interface HomebrewPageModel {
  dataset: HomebrewDataset;
  summaryMetrics: SummaryMetric[];
  summaryMetricMap: Record<string, SummaryMetric | undefined>;
  taps: TapEntry[];
  rows: HomebrewRow[];
  availableRows: HomebrewRow[];
  unavailableRows: HomebrewRow[];
  chartData: {
    laneStatus: Array<{
      id: string;
      label: string;
      stateScore: number;
      stateLabel: string;
      installCount: number | null;
      downloadCount: number | null;
      sourceUrl: string;
    }>;
  };
}

function readJson<T>(path: string): T {
  return JSON.parse(readFileSync(path, 'utf8')) as T;
}

export function loadHomebrewPageModel(datasetPath: string): HomebrewPageModel {
  const dataset = readJson<HomebrewDataset>(datasetPath);

  const rows = dataset.rows;
  const availableRows = rows.filter((row) => row.state === 'available');
  const unavailableRows = rows.filter((row) => row.state !== 'available');

  const summaryMetricMap = Object.fromEntries(
    dataset.summary_metrics.map((metric) => [metric.id, metric]),
  ) as Record<string, SummaryMetric | undefined>;

  const chartData = {
    laneStatus: rows.map((row) => ({
      id: row.id,
      label: `${row.variant}/${row.branch}`,
      stateScore: row.state === 'available' ? (row.install_count !== null ? 2 : 1) : 0,
      stateLabel:
        row.state === 'available'
          ? row.install_count !== null
            ? 'Homebrew data available'
            : 'Lane tracked, no install data'
          : 'Awaiting Homebrew data',
      installCount: row.install_count,
      downloadCount: row.download_count,
      sourceUrl: row.source_url,
    })),
  };

  return {
    dataset,
    summaryMetrics: dataset.summary_metrics,
    summaryMetricMap,
    taps: dataset.taps,
    rows,
    availableRows,
    unavailableRows,
    chartData,
  };
}
