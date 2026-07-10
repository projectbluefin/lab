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
    'docs/images/index.html',
    'docs/tests/index.html',
    'docs/applications/index.html',
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
  assert.match(html('docs/index.html'), /href="\/images\/"/, 'overview links to images at domain root');
  assert.match(html('docs/index.html'), /site-nav__link[^>]*>Overview</, 'top nav shows Overview tab');
  assert.match(html('docs/tests/index.html'), /src="\/_astro\/TestsCharts\.[^"]+"/, 'tests page keeps bundled chart script');
  assert.match(html('docs/images/index.html'), /src="\/_astro\/upstream-page\.[^"]+"[^>]* data-cfasync="false"/, 'images page keeps Cloudflare-safe chart script');
  const adoptionPage = html('docs/adoption/index.html');
  assert.match(adoptionPage, /data-cfasync="false"/, 'adoption page keeps Cloudflare-safe chart script');
  assert.match(html('docs/images/index.html'), /Image status/, 'images page renders');
  assert.match(html('docs/tests/index.html'), /Tests/, 'tests page renders');
  assert.match(html('docs/applications/index.html'), /Applications/, 'applications page renders');
  assert.match(adoptionPage, /Homebrew/, 'adoption page renders integrated Homebrew content');
  assert.match(adoptionPage, /Adoption/, 'adoption page renders');
  assert.match(html('docs/about/index.html'), /Bluefin QA — Methodology/i, 'about page renders methodology');
  assert.match(html('docs/applications/index.html'), /Bazaar/, 'applications page calls out Bazaar scope');
  assert.match(html('docs/index.html'), /href="\/adoption\/"/, 'overview links to adoption at domain root');
  assert.match(html('docs/index.html'), /href="\/userspace\/"/, 'overview links to userspace at domain root');
  assert.match(html('docs/index.html'), /Ryzen AI MAX\+/i, 'overview renders control-plane Ryzen AI specs');
  assert.match(html('docs/index.html'), /Zot OCI Registry Cache & Heat/i, 'overview renders Zot OCI cache section');
  // Accept either :30501 (live LAN data) or :30500 (fallback when LAN unreachable, e.g. GitHub Actions)
  assert.match(html('docs/index.html'), /:305(?:00|01)/i, 'overview renders zot-cache port details');
  assert.match(html('docs/images/index.html'), /Unavailable|pending|coming soon/i, 'subpages show explicit unavailable state');

  // Regression guard: chart scripts must load echarts globally from a CDN classic script
  // and never rely on a bare `import ... from 'echarts'` inside a type="module" script
  // whose src came from a `?url` import. Astro never bundles/resolves that pattern, so the
  // import throws `Failed to resolve module specifier "echarts"` in every real browser and
  // the chart silently fails to render (caught this only via a real headless-browser check,
  // never via these text assertions alone).
  function decodeInlineScriptSource(pageHtml, matchToken) {
    // The chart script may be emitted either as a separate built asset (its filename
    // contains matchToken) or inlined as a base64 data: URI when small enough for
    // Vite's asset-inlining threshold — a data: URI never contains the original
    // filename, so match either shape rather than requiring the token in the src.
    const allScriptSrcs = [...pageHtml.matchAll(/<script src="([^"]+)"[^>]*>/g)].map((m) => m[1]);
    const src = allScriptSrcs.find((s) => s.includes(matchToken))
      ?? allScriptSrcs.find((s) => !s.includes('cdn.jsdelivr.net'));
    assert.ok(src, `expected to find a local chart script tag near ${matchToken}`);
    if (src.startsWith('data:')) {
      const base64 = src.split(',')[1];
      return Buffer.from(base64, 'base64').toString('utf8');
    }
    return readFileSync(path.join(repo, 'docs', src), 'utf8');
  }

  for (const [page, scriptToken] of [
    ['docs/images/index.html', 'upstream-page'],
    ['docs/builds/index.html', 'builds-charts'],
  ]) {
    const pageHtml = html(page);
    assert.match(
      pageHtml,
      /cdn\.jsdelivr\.net\/npm\/echarts/,
      `${page} loads echarts globally from CDN before its chart script`,
    );
    assert.doesNotMatch(
      pageHtml,
      new RegExp(`type="module"[^>]*${scriptToken}`),
      `${page} chart script is not an unresolved ES module import`,
    );
    const scriptSource = decodeInlineScriptSource(pageHtml, scriptToken);
    assert.doesNotMatch(
      scriptSource,
      /import\s+\*\s+as\s+echarts\s+from\s+['"]echarts['"]/,
      `${scriptToken}.js has no unresolved bare echarts import`,
    );
  }
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
  assert.match(testsPage, /Triage & Local Execution Runbook/i, 'tests page renders triage runbook section');
  assert.match(testsPage, /Permission denied \(publickey\) at SSH wait/i, 'tests page runbook lists common publickey log symptom');
});

test('images page renders grouped views, chart mounts, evidence links, and unavailable states', () => {
  execFileSync('npm', ['run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const imagesPage = html('docs/images/index.html');

  assert.match(imagesPage, /Stream availability by family/i, 'images page shows grouped availability chart section');
  assert.match(imagesPage, /Release freshness by stream/i, 'images page shows freshness chart section');
  assert.match(imagesPage, /Release timeline/i, 'images page shows release timeline chart section');
  assert.match(imagesPage, /Dakota testing/i, 'images page includes projectbluefin streams');
  assert.match(imagesPage, /https:\/\/github\.com\/(orgs\/projectbluefin\/packages\/container\/dakota|projectbluefin\/dakota\/releases)/i, 'images page links projectbluefin evidence');
  assert.match(imagesPage, /No published release timestamp is present in docs\/data\/factory-stats\.json for this stream\./i, 'images page keeps unavailable reason explicit');
  assert.match(imagesPage, /https:\/\/github\.com\/ublue-os\/aurora\/releases/i, 'images page links non-bluefin evidence');
  assert.match(imagesPage, /Fedora Silverblue|Fedora Kinoite/i, 'images page references Silverblue and Kinoite upstream parent OSes');
  assert.match(imagesPage, /https:\/\/fedoraproject\.org\/silverblue\/|https:\/\/fedoraproject\.org\/kinoite\//i, 'images page links upstream Silverblue and Kinoite homepages');
  assert.match(imagesPage, /upstream-availability-chart|upstream-freshness-chart|upstream-timeline-chart|upstream-distribution-chart|upstream-brackets-chart/, 'images page renders chart containers');
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
