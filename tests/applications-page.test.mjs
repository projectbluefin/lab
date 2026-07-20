import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';

const repo = process.cwd();

function html(file) {
  return readFileSync(path.join(repo, file), 'utf8');
}

test('applications page renders live GitOps state, resource charts, and policy scorecards', () => {
  // Isolate environment for sub-process npm run build to avoid ESM/node resolution issues
  execFileSync('env', ['-i', `PATH=${process.env.PATH}`, 'npm', 'run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const applicationsPage = html('docs/applications/index.html');

  assert.match(
    applicationsPage,
    /GitOps Applications/i,
    'applications page header renders the correct title',
  );
  assert.match(
    applicationsPage,
    /Managed Applications/i,
    'applications page renders the Managed Applications KPI card',
  );
  assert.match(
    applicationsPage,
    /Policy Pass Rate/i,
    'applications page displays the policy pass rate scorecard',
  );
  assert.match(
    applicationsPage,
    /argo-workflows/i,
    'applications page lists the argo-workflows system application',
  );
  assert.match(
    applicationsPage,
    /testing-lab-infra/i,
    'applications page lists the testing-lab-infra application',
  );
  assert.match(
    applicationsPage,
    /Rollout History/i,
    'applications page renders the recent syncs rollout log',
  );
  assert.match(
    applicationsPage,
    /Policy Scorecard & Violations/i,
    'applications page renders the policy check rules section',
  );
  assert.match(
    applicationsPage,
    /app-resources-chart/,
    'applications page renders the resource consumption chart container',
  );
  assert.match(
    applicationsPage,
    /app-compliance-chart/,
    'applications page renders the compliance donut chart container',
  );
  assert.match(
    applicationsPage,
    /applications-page-data/,
    'applications page serializes client-side chart payload',
  );
});
