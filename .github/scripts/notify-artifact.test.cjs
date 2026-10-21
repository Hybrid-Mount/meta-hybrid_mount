// SPDX-License-Identifier: GPL-3.0-only

const assert = require('node:assert/strict');
const test = require('node:test');
const { findArtifact } = require('./notify-artifact.cjs');
const sourceHash = 'a'.repeat(64);
const name = `notify-linux-x86_64-${sourceHash}`;

function scenario(artifacts, runs) {
  return {
    sourceHash,
    context: { repo: { owner: 'Hybrid-Mount', repo: 'meta-hybrid_mount' }, payload: { repository: { id: 123 } } },
    github: {
      paginate: async () => artifacts,
      rest: { actions: {
        listArtifactsForRepo: () => {},
        getWorkflowRun: async ({ run_id }) => ({ data: runs[run_id] }),
      } },
    },
  };
}

const artifact = (id = 1) => ({ id, name, expired: false, workflow_run: { id: 10, head_repository_id: 123 } });
const run = () => ({ path: '.github/workflows/build.yml', event: 'push', head_branch: 'dev', head_repository: { id: 123 } });

test('reuses the exact artifact from a trusted build even while the package is still building', async () => {
  assert.deepEqual(await findArtifact(scenario([artifact()], { 10: { ...run(), status: 'in_progress' } })), { id: 1, runId: 10 });
});

test('expired artifacts and different source hashes cannot be reused', async () => {
  assert.equal(await findArtifact(scenario([{ ...artifact(), expired: true }, { ...artifact(2), name: 'other-source' }], {})), undefined);
});

test('forks, pull requests and unrelated workflows cannot supply a notification executable', async () => {
  for (const changed of [
    { ...run(), event: 'pull_request' },
    { ...run(), path: '.github/workflows/untrusted.yml' },
    { ...run(), head_repository: { id: 999 } },
    { ...run(), head_branch: 'feature' },
  ]) {
    assert.equal(await findArtifact(scenario([artifact()], { 10: changed })), undefined);
  }
  assert.equal(await findArtifact(scenario([{ ...artifact(), workflow_run: { id: 10, head_repository_id: 999 } }], {})), undefined);
});

test('release-tag builds can reuse the same source build', async () => {
  assert.deepEqual(await findArtifact(scenario([artifact()], { 10: { ...run(), path: '.github/workflows/release.yml', head_branch: 'v6.2.5-rc.1' } })), { id: 1, runId: 10 });
});

test('API errors fail instead of silently choosing an unverified artifact', async () => {
  const input = scenario([artifact()], {});
  input.github.rest.actions.getWorkflowRun = async () => { throw new Error('API unavailable'); };
  await assert.rejects(findArtifact(input), /API unavailable/);
});
