// SPDX-License-Identifier: GPL-3.0-only
const assert = require('node:assert/strict');
const test = require('node:test');
const { validateReleaseAssets } = require('./release-assets.cjs');
const normal = { name: 'Hybrid-Mount-6.2.6-2000.zip', sha256: 'a'.repeat(64) };
const wear = { name: 'Hybrid-Mount_Wear-6.2.6-2000.zip', sha256: 'b'.repeat(64) };
const expected = [normal, wear];
const draft = { draft: true, tag_name: 'v6.2.6' };
const assets = expected.map(p => ({ name: p.name, digest: `sha256:${p.sha256}`, state: 'uploaded' }));

test('both exact package digests may be published in either order', () => {
  validateReleaseAssets(draft, assets, 'v6.2.6', expected);
  validateReleaseAssets(draft, [...assets].reverse(), 'v6.2.6', expected);
});

test('missing, extra, duplicate, unfinished or altered assets block publication', () => {
  for (const candidate of [assets.slice(0, 1), [...assets, assets[0]], [assets[0], assets[0]],
    [assets[0], { ...assets[1], state: 'new' }],
    [assets[0], { ...assets[1], digest: `sha256:${normal.sha256}` }],
    [assets[0], { ...assets[1], name: 'unknown.zip' }]]) {
    assert.throws(() => validateReleaseAssets(draft, candidate, 'v6.2.6', expected));
  }
});

test('a published release, wrong tag or incomplete expectations block publication', () => {
  assert.throws(() => validateReleaseAssets({ ...draft, draft: false }, assets, 'v6.2.6', expected));
  assert.throws(() => validateReleaseAssets(draft, assets, 'v6.2.7', expected));
  assert.throws(() => validateReleaseAssets(draft, assets, 'v6.2.6', [normal, normal]));
});
