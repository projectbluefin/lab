import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';

const repo = process.cwd();

function html(file) {
  return readFileSync(path.join(repo, file), 'utf8');
}

test('provisioning page renders architecture, nodes, containerDisks, and evidence links', () => {
  execFileSync('npm', ['run', 'build'], {
    cwd: repo,
    stdio: 'pipe',
    encoding: 'utf8',
  });

  const provisioningPage = html('docs/provisioning/index.html');
  const overviewPage = html('docs/index.html');

  assert.match(
    provisioningPage,
    /VM Provisioning/i,
    'provisioning page renders the header title',
  );

  assert.match(
    provisioningPage,
    /containerDisk/i,
    'provisioning page explains containerDisk provisioning',
  );

  assert.match(
    provisioningPage,
    /No host-side btrfs reflink/i,
    'provisioning page explicitly corrects the btrfs reflink misconception',
  );

  assert.match(
    provisioningPage,
    /Hypervisor Nodes/i,
    'provisioning page renders the hypervisor nodes section',
  );

  assert.match(
    provisioningPage,
    /ContainerDisk Inventory/i,
    'provisioning page renders the containerDisk inventory',
  );

  assert.match(
    provisioningPage,
    /Guest Filesystems/i,
    'provisioning page renders the guest filesystems section',
  );

  assert.match(
    provisioningPage,
    /provisioning-capacity-chart/i,
    'provisioning page includes the capacity chart container',
  );

  assert.match(
    provisioningPage,
    /provisioning-filesystems-chart/i,
    'provisioning page includes the filesystems chart container',
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

  // No emojis in generated HTML.
  const emojiRegex = /[\u{1F300}-\u{1F6FF}\u{1F900}-\u{1F9FF}\u{2600}-\u{26FF}\u{2700}-\u{27BF}\u{1F1E6}-\u{1F1FF}]/u;
  assert.equal(
    emojiRegex.test(provisioningPage),
    false,
    'provisioning page does not contain any emojis',
  );
});
