import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';

const repo = process.cwd();

function html(file) {
  return readFileSync(path.join(repo, file), 'utf8');
}

test('builds page renders triage strip, dense charts, and explicit unavailable states', () => {
  execFileSync('npm', ['run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const buildsPage = html('docs/builds/index.html');
  const overviewPage = html('docs/index.html');

  assert.match(
    buildsPage,
    /Current Status/i,
    'builds page renders the current status triage section',
  );
  assert.match(
    buildsPage,
    /Publish Plane Duration Trends/i,
    'builds page renders the publish plane chart section',
  );
  assert.match(
    buildsPage,
    /Daily Build Outcomes/i,
    'builds page renders the daily outcomes chart section',
  );
  assert.match(
    buildsPage,
    /Lab Plane Pipeline Health/i,
    'builds page renders the lab plane chart section',
  );
  assert.match(
    buildsPage,
    /Recent Terminal Runs/i,
    'builds page renders the recent runs table section',
  );
  assert.match(
    buildsPage,
    /Bluefin\s*(&mdash;|—|-)\s*stable/i,
    'builds page includes the Bluefin stable pipeline',
  );
  assert.match(
    buildsPage,
    /Bluefin LTS/i,
    'builds page includes the Bluefin LTS pipelines',
  );
  assert.match(
    buildsPage,
    /Dakota/i,
    'builds page includes the Dakota pipeline',
  );
  assert.match(
    buildsPage,
    /Duration trend for/i,
    'builds page renders accessible labels for each chart mount point',
  );
  assert.match(
    buildsPage,
    /pill--(passed|failed|pending)/,
    'builds page renders real status pills sourced from run history',
  );
  assert.match(
    buildsPage,
    /GitHub Actions/i,
    'builds page discloses the publish plane GitHub Actions source',
  );
  assert.match(
    buildsPage,
    /builds-chart-publish-lane-/,
    'builds page renders per-lane publish chart containers',
  );
  assert.match(
    buildsPage,
    /builds-chart-lab-lane-/,
    'builds page renders per-lane lab chart containers',
  );
  assert.match(
    buildsPage,
    /builds-chart-data/,
    'builds page serializes client chart data',
  );
  assert.match(
    buildsPage,
    /data\/history\/build-runs\.ndjson/,
    'builds page links the rolling history dataset',
  );
  assert.match(
    overviewPage,
    /href="[^"]*builds\/"/,
    'site nav includes a link to the Builds page',
  );
});
