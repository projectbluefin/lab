import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

export interface GitOpsApp {
  name: string;
  namespace: string;
  sync_status: string;
  health_status: string;
  target_revision: string;
  path: string;
  repo_url: string;
  destination_namespace: string;
  drifted_count: number;
  drifted_resources: Array<{
    group: string;
    kind: string;
    name: string;
    namespace: string;
    status: string;
  }>;
  collected_at: string;
  // Merged metrics
  pods_count: number;
  cpu: {
    usage: number;
    request: number;
    limit: number;
  };
  memory: {
    usage: number;
    request: number;
    limit: number;
  };
}

export interface ComplianceRule {
  id: string;
  name: string;
  description: string;
  status: string;
  total_checked: number;
  violations_count: number;
  violations: Array<{
    source: string;
    detail: string;
  }>;
}

export interface ComplianceData {
  score: number;
  rules: ComplianceRule[];
  _meta: {
    git_manifests_scanned: number;
    live_pods_scanned: number;
  };
}

export interface DeploymentEvent {
  app: string;
  id: number;
  revision: string;
  started_at: string;
  finished_at: string;
  status: string;
}

export interface GitOpsPageModel {
  applications: GitOpsApp[];
  compliance: ComplianceData;
  deployments: DeploymentEvent[];
  summary: {
    total_apps: number;
    synced_apps: number;
    outofsync_apps: number;
    healthy_apps: number;
    degraded_apps: number;
    total_pods: number;
    total_cpu_cores_used: number;
    total_mem_mib_used: number;
    compliance_score: number;
    generated_at: string;
  };
}

function readJson<T>(path: string, fallback: T): T {
  if (!existsSync(path)) {
    return fallback;
  }
  try {
    return JSON.parse(readFileSync(path, 'utf8')) as T;
  } catch {
    return fallback;
  }
}

export function loadGitOpsPageModel(repoRoot: string): GitOpsPageModel {
  const statusPath = join(repoRoot, 'docs/data/gitops-status.json');
  const resourcesPath = join(repoRoot, 'docs/data/app-resource-usage.json');
  const compliancePath = join(repoRoot, 'docs/data/policy-compliance.json');
  const deploymentsPath = join(repoRoot, 'docs/data/gitops-deployments.json');

  const statusData = readJson<{ applications: any[], _meta: any }>(statusPath, { applications: [], _meta: { generated_at: "" } });
  const resourcesData = readJson<{ applications: any[] }>(resourcesPath, { applications: [] });
  const complianceData = readJson<ComplianceData>(compliancePath, {
    score: 100.0,
    rules: [],
    _meta: { git_manifests_scanned: 0, live_pods_scanned: 0 }
  });
  const deploymentsData = readJson<{ deployments: DeploymentEvent[] }>(deploymentsPath, { deployments: [] });

  const resourcesMap = Object.fromEntries(
    resourcesData.applications.map((app) => [app.name, app])
  );

  const applications: GitOpsApp[] = statusData.applications.map((app) => {
    const res = resourcesMap[app.name] || {
      pods_count: 0,
      cpu: { usage: 0.0, request: 0.0, limit: 0.0 },
      memory: { usage: 0.0, request: 0.0, limit: 0.0 }
    };

    return {
      ...app,
      pods_count: res.pods_count || 0,
      cpu: res.cpu || { usage: 0.0, request: 0.0, limit: 0.0 },
      memory: res.memory || { usage: 0.0, request: 0.0, limit: 0.0 }
    };
  });

  // Calculate summaries
  const total_apps = applications.length;
  const synced_apps = applications.filter(a => a.sync_status === 'Synced').length;
  const outofsync_apps = total_apps - synced_apps;
  const healthy_apps = applications.filter(a => a.health_status === 'Healthy').length;
  const degraded_apps = total_apps - healthy_apps;

  const total_pods = applications.reduce((sum, a) => sum + a.pods_count, 0);
  const total_cpu_cores_used = applications.reduce((sum, a) => sum + a.cpu.usage, 0);
  const total_mem_mib_used = applications.reduce((sum, a) => sum + a.memory.usage, 0);

  return {
    applications,
    compliance: complianceData,
    deployments: deploymentsData.deployments || [],
    summary: {
      total_apps,
      synced_apps,
      outofsync_apps,
      healthy_apps,
      degraded_apps,
      total_pods,
      total_cpu_cores_used: roundTo(total_cpu_cores_used, 3),
      total_mem_mib_used: roundTo(total_mem_mib_used, 1),
      compliance_score: complianceData.score,
      generated_at: statusData._meta?.generated_at || new Date().toISOString()
    }
  };
}

function roundTo(num: number, dec: number): number {
  const exp = Math.pow(10, dec);
  return Math.round(num * exp) / exp;
}
