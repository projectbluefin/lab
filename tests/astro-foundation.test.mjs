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
    'docs/bluefin/index.html',
    'docs/tests/index.html',
    'docs/applications/index.html',
    'docs/homebrew/index.html',
    'docs/adoption/index.html',
    'docs/userspace/index.html',
    'docs/about/index.html',
  ];

  for (const file of expectedFiles) {
    assert.equal(existsSync(path.join(repo, file)), true, `${file} should exist after build`);
  }

  assert.match(html('docs/index.html'), /factory-dashboard/, 'overview keeps the dashboard shell');
  assert.doesNotMatch(html('docs/index.html'), /factory-dashboard\.js/, 'legacy dashboard script is removed');
  assert.doesNotMatch(html('docs/index.html'), /factory-dashboard\.css/, 'legacy dashboard style is removed');
  assert.match(html('docs/index.html'), /class="kpi-grid"/, 'overview renders build-time KPI cards');
  assert.match(html('docs/index.html'), /class="nodes-grid"/, 'overview renders contributor nodes');
  assert.match(html('docs/index.html'), /class="image-status-grid"/, 'overview renders image status section');
  assert.match(html('docs/index.html'), /href="\/upstream\/"/, 'overview links to upstream at domain root');
  assert.match(html('docs/index.html'), /site-nav__link[^>]*>Overview</, 'top nav shows Overview tab');
  assert.match(html('docs/tests/index.html'), /src="\/_astro\/tests-charts\.[^"]+" data-cfasync="false"/, 'tests page keeps Cloudflare-safe chart script');
  assert.match(html('docs/upstream/index.html'), /src="\/_astro\/upstream-page\.[^"]+" data-cfasync="false"/, 'upstream page keeps Cloudflare-safe chart script');
  assert.match(html('docs/bluefin/index.html'), /src="\/_astro\/upstream-page\.[^"]+" data-cfasync="false"/, 'bluefin page keeps Cloudflare-safe chart script');
  assert.match(html('docs/homebrew/index.html'), /data-cfasync="false"/, 'homebrew page keeps Cloudflare-safe chart script');
  assert.match(html('docs/adoption/index.html'), /data-cfasync="false"/, 'adoption page keeps Cloudflare-safe chart script');
  assert.match(html('docs/upstream/index.html'), /Upstream/, 'upstream page renders');
  assert.match(html('docs/bluefin/index.html'), /Bluefin upstream/i, 'bluefin page renders');
  assert.match(html('docs/tests/index.html'), /Tests/, 'tests page renders');
  assert.match(html('docs/applications/index.html'), /Applications/, 'applications page renders');
  assert.match(html('docs/homebrew/index.html'), /Homebrew/, 'homebrew page renders');
  assert.match(html('docs/adoption/index.html'), /Adoption/, 'adoption page renders');
  assert.match(html('docs/about/index.html'), /Bluefin QA — Methodology/i, 'about page renders methodology');
  assert.match(html('docs/applications/index.html'), /Bazaar/, 'applications page calls out Bazaar scope');
  assert.match(html('docs/index.html'), /href="\/homebrew\/"/, 'overview links to homebrew at domain root');
  assert.match(html('docs/index.html'), /href="\/adoption\/"/, 'overview links to adoption at domain root');
  assert.match(html('docs/index.html'), /href="\/userspace\/"/, 'overview links to userspace at domain root');
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
  
  // Premium features assertions
  assert.match(testsPage, /Tests at a Glance/i, 'tests page renders beautiful title for KPI scorecard');
  assert.match(testsPage, /Verified scenarios/i, 'tests page renders Verified scenarios KPI card');
  assert.match(testsPage, /progress-bar/i, 'tests page renders inline progress bars for pass rates');
  assert.match(testsPage, /Data Integrity Posture/i, 'tests page renders Data Integrity Posture section');
  assert.match(testsPage, /Evidence-backed authenticity/i, 'tests page renders evidence-backed authenticity disclosure');
});

test('upstream page renders grouped views, chart mounts, evidence links, and unavailable states', () => {
  execFileSync('npm', ['run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const upstreamPage = html('docs/upstream/index.html');

  assert.match(upstreamPage, /Stream availability by family/i, 'upstream page shows grouped availability chart section');
  assert.match(upstreamPage, /Release freshness by stream/i, 'upstream page shows freshness chart section');
  assert.match(upstreamPage, /Release timeline/i, 'upstream page shows release timeline chart section');
  assert.doesNotMatch(upstreamPage, /Dakota testing/i, 'upstream page excludes projectbluefin streams');
  assert.match(upstreamPage, /No published release timestamp is present in docs\/data\/factory-stats\.json for this stream\./i, 'upstream page keeps unavailable reason explicit');
  assert.match(upstreamPage, /https:\/\/github\.com\/ublue-os\/aurora\/releases/i, 'upstream page links non-bluefin evidence');
  assert.match(upstreamPage, /Fedora Silverblue|Fedora Kinoite/i, 'upstream page references Silverblue and Kinoite upstream parent OSes');
  assert.match(upstreamPage, /https:\/\/fedoraproject\.org\/silverblue\/|https:\/\/fedoraproject\.org\/kinoite\//i, 'upstream page links upstream Silverblue and Kinoite homepages');
  assert.match(upstreamPage, /upstream-availability-chart|upstream-freshness-chart|upstream-timeline-chart/, 'upstream page renders chart containers');
});

test('bluefin page renders bluefin-family streams with explicit unavailable states and evidence links', () => {
  execFileSync('npm', ['run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const bluefinPage = html('docs/bluefin/index.html');

  assert.match(bluefinPage, /Bluefin upstream/i, 'bluefin page title renders');
  assert.match(bluefinPage, /Dakota testing/i, 'bluefin page includes dakota stream');
  assert.match(bluefinPage, /bluefin-lts testing/i, 'bluefin page includes bluefin-lts stream');
  assert.match(bluefinPage, /No published release timestamp is present in docs\/data\/factory-stats\.json for this stream\./i, 'bluefin page keeps unavailable reason explicit');
  assert.match(bluefinPage, /https:\/\/github\.com\/projectbluefin\/dakota\/releases/i, 'bluefin page links projectbluefin evidence');
  assert.doesNotMatch(bluefinPage, /ublue-os\/aurora/i, 'bluefin page excludes non-projectbluefin streams');
});

test('userspace page renders FSDK containers, registry metadata, and charts', () => {
  execFileSync('npm', ['run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const userspacePage = html('docs/userspace/index.html');

  assert.match(userspacePage, /Freedesktop SDK Container Images/i, 'userspace page renders main title');
  assert.match(userspacePage, /FSDK images built/i, 'userspace page renders build KPI card');
  assert.match(userspacePage, /OCI local storage/i, 'userspace page renders Zot OCI local storage card');
  assert.match(userspacePage, /Registry Layers/i, 'userspace page renders Zot layers count card');
  assert.match(userspacePage, /OCI Compliance/i, 'userspace page renders OCI Compliance specification card');
  assert.match(userspacePage, /Freedesktop SDK custom containers/i, 'userspace page renders tracked custom containers section');
  assert.match(userspacePage, /fsdk\/lab-runner/i, 'userspace page includes lab-runner OCI image');
  assert.match(userspacePage, /elements\/oci\/lab-runner\.bst/i, 'userspace page includes buildstream element path');
  assert.match(userspacePage, /Curated Resource Optimization/i, 'userspace page explains why some elements are unbuilt');
  assert.match(userspacePage, /podman pull 192\.168\.1\.102:30500\/fsdk\/lab-runner:latest/i, 'userspace page renders copyable podman pull commands');
  assert.match(userspacePage, /GitHub Source/i, 'userspace page links FSDK containers directly to GitHub');
  assert.match(userspacePage, /userspace-registry-dist-chart|userspace-fsdk-status-chart/, 'userspace page renders chart containers');
  assert.match(userspacePage, /FSDK version/i, 'userspace page renders Freedesktop SDK version label');
  assert.match(userspacePage, /Digest/i, 'userspace page renders OCI Digest label');
  assert.match(userspacePage, /Git Commit Ref/i, 'userspace page renders Git Commit Reference label');
  assert.match(userspacePage, /Registry Heat \/ Activity/i, 'userspace page renders Registry Heat column header');
  assert.match(userspacePage, /Sizzling/i, 'userspace page renders sizzling heat metric label');
  assert.match(userspacePage, /heat-glowing-bar/i, 'userspace page renders glowing and sizzling progress bar elements');
  assert.match(userspacePage, /Data Integrity Posture/i, 'userspace page renders Data Integrity Posture section');
});
