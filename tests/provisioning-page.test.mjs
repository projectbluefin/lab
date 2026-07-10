import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';

const repo = process.cwd();

function html(file) {
  return readFileSync(path.join(repo, file), 'utf8');
}

test('provisioning page renders bays, charts, nav links, and integrity data', () => {
  // Trigger build to ensure pages are fresh
  execFileSync('npm', ['run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const provisioningPage = html('docs/provisioning/index.html');
  const overviewPage = html('docs/index.html');

  assert.match(
    provisioningPage,
    /Provisioning Bays/i,
    'provisioning page renders the header title',
  );

  assert.match(
    provisioningPage,
    /Bay A/i,
    'provisioning page includes Bay A',
  );

  assert.match(
    provisioningPage,
    /Bay B/i,
    'provisioning page includes Bay B',
  );

  assert.match(
    provisioningPage,
    /Bay C/i,
    'provisioning page includes Bay C',
  );

  assert.match(
    provisioningPage,
    /KubeVirt hypervisor slot monitor\./i,
    'provisioning page renders the required slots chart caption',
  );

  assert.match(
    provisioningPage,
    /btrfs reflink initialization advantage/i,
    'provisioning page renders btrfs reflink advantages text',
  );

  assert.match(
    provisioningPage,
    /provisioning-slots-chart/i,
    'provisioning page includes the slots chart container ID',
  );

  assert.match(
    provisioningPage,
    /provisioning-reflink-chart/i,
    'provisioning page includes the reflink speed chart container ID',
  );

  assert.match(
    provisioningPage,
    /Data Integrity Posture/i,
    'provisioning page renders the data integrity disclosure panel',
  );

  assert.match(
    provisioningPage,
    /data\/factory-stats\.json/i,
    'provisioning page links the raw dataset',
  );

  assert.match(
    overviewPage,
    /href="[^"]*provisioning\/"/,
    'site nav includes a link to the Provisioning page',
  );

  // Assertion: Ensure there are absolutely no emojis in the generated HTML of the provisioning page
  const emojiRegex = /[\u{1F300}-\u{1F6FF}\u{1F900}-\u{1F9FF}\u{2600}-\u{26FF}\u{2700}-\u{27BF}\u{1F1E6}-\u{1F1FF}]/u;
  assert.equal(
    emojiRegex.test(provisioningPage),
    false,
    'provisioning page does not contain any emojis',
  );
});
