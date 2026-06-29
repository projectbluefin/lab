import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const repo = process.cwd();

function html(file) {
  return readFileSync(path.join(repo, file), 'utf8');
}

test('Astro build emits multipage factory routes into docs', () => {
  execFileSync('npm', ['run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const expectedFiles = [
    'docs/index.html',
    'docs/upstream/index.html',
    'docs/tests/index.html',
    'docs/applications/index.html',
  ];

  for (const file of expectedFiles) {
    assert.equal(existsSync(path.join(repo, file)), true, `${file} should exist after build`);
  }

  assert.match(html('docs/index.html'), /factory-dashboard/, 'overview keeps the dashboard shell');
  assert.match(html('docs/index.html'), /href="\/testing-lab\/upstream\/"/, 'overview links to upstream with Pages base');
  assert.match(html('docs/index.html'), /src="\/testing-lab\/assets\/factory-dashboard\.js" defer data-cfasync="false"/, 'overview keeps Cloudflare-safe dashboard script');
  assert.match(html('docs/tests/index.html'), /src="\/testing-lab\/_astro\/tests-charts\.[^"]+" data-cfasync="false"/, 'tests page keeps Cloudflare-safe chart script');
  assert.match(html('docs/upstream/index.html'), /src="\/testing-lab\/_astro\/upstream-page\.[^"]+" data-cfasync="false"/, 'upstream page keeps Cloudflare-safe chart script');
  assert.match(html('docs/upstream/index.html'), /Upstream/, 'upstream page renders');
  assert.match(html('docs/tests/index.html'), /Tests/, 'tests page renders');
  assert.match(html('docs/applications/index.html'), /Applications/, 'applications page renders');
  assert.match(html('docs/applications/index.html'), /Bazaar/, 'applications page calls out Bazaar scope');
  assert.match(html('docs/upstream/index.html'), /Unavailable|pending|coming soon/i, 'subpages show explicit unavailable state');
});

test('tests page renders matrix views, chart mounts, evidence links, and unavailable states', () => {
  execFileSync('npm', ['run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const testsPage = html('docs/tests/index.html');

  assert.match(testsPage, /Reliability trends/i, 'tests page shows reliability trend chart section');
  assert.match(testsPage, /Failure concentration/i, 'tests page shows failure concentration chart section');
  assert.match(testsPage, /Suite\/variant view/i, 'tests page shows suite variant chart section');
  assert.match(testsPage, /bluefin-testing-smoke/i, 'tests page renders available matrix row details');
  assert.match(testsPage, /results\/bluefin-testing-smoke\.json/i, 'tests page links results evidence');
  assert.match(testsPage, /Result file exists, but no completed run is published for this matrix cell yet\./i, 'tests page keeps unavailable states explicit');
});

test('upstream page renders grouped views, chart mounts, evidence links, and unavailable states', () => {
  execFileSync('npm', ['run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const upstreamPage = html('docs/upstream/index.html');

  assert.match(upstreamPage, /Lane availability by family/i, 'upstream page shows grouped availability chart section');
  assert.match(upstreamPage, /Release freshness by lane/i, 'upstream page shows freshness chart section');
  assert.match(upstreamPage, /Release timeline/i, 'upstream page shows release timeline chart section');
  assert.match(upstreamPage, /Dakota testing/i, 'upstream page keeps unavailable lanes visible');
  assert.match(upstreamPage, /No published release timestamp is present in docs\/data\/factory-stats\.json for this lane\./i, 'upstream page keeps unavailable reason explicit');
  assert.match(upstreamPage, /https:\/\/github\.com\/projectbluefin\/dakota\/releases/i, 'upstream page links lane evidence');
  assert.match(upstreamPage, /upstream-availability-chart|upstream-freshness-chart|upstream-timeline-chart/, 'upstream page renders chart containers');
});
