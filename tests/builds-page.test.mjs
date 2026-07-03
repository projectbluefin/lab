import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';

const repo = process.cwd();

function html(file) {
  return readFileSync(path.join(repo, file), 'utf8');
}

test('builds page renders pipeline rows, sparkline mounts, nav link, and explicit unavailable states', () => {
  execFileSync('npm', ['run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const buildsPage = html('docs/builds/index.html');
  const overviewPage = html('docs/index.html');

  assert.match(
    buildsPage,
    /Builds at a Glance/i,
    'builds page renders the summary section title',
  );
  assert.match(
    buildsPage,
    /bluefin-qa-pipeline/i,
    'builds page includes the bluefin QA pipeline',
  );
  assert.match(
    buildsPage,
    /dakota-qa-pipeline/i,
    'builds page includes the dakota QA pipeline',
  );
  assert.match(
    buildsPage,
    /knuckle-qa-pipeline/i,
    'builds page includes the knuckle QA pipeline',
  );
  assert.match(
    buildsPage,
    /build-containerdisk/i,
    'builds page includes the containerdisk build template',
  );
  assert.match(
    buildsPage,
    /Duration trend for/i,
    'builds page renders accessible labels for each sparkline mount point',
  );
  assert.match(
    buildsPage,
    /pill--unavailable/,
    'builds page renders explicit unavailable status pills until a live snapshot lands',
  );
  assert.match(
    buildsPage,
    /Cluster snapshot is stale/i,
    'builds page discloses stale cluster snapshot state',
  );
  assert.match(
    buildsPage,
    /ghost-runners/i,
    'builds page explains the self-hosted runner bridge in the stale banner',
  );
  assert.match(
    buildsPage,
    /sparkline-/,
    'builds page renders per-row sparkline chart containers',
  );
  assert.match(
    buildsPage,
    /builds-chart-data/,
    'builds page serializes client chart data',
  );
  assert.match(
    buildsPage,
    /Template ↗/,
    'builds page links each pipeline to its workflow template source',
  );
  assert.match(
    buildsPage,
    /Data Integrity Posture/i,
    'builds page renders the data integrity disclosure panel',
  );
  assert.match(
    buildsPage,
    /data\/builds-matrix\.json/,
    'builds page links the raw dataset',
  );
  assert.match(
    overviewPage,
    /href="[^"]*builds\/"/,
    'site nav includes a link to the Builds page',
  );
});
