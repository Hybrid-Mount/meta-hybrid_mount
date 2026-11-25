// SPDX-License-Identifier: GPL-3.0-only

function validateReleaseAssets(draft, assets, tag, expected) {
  if (!draft.draft || draft.tag_name !== tag) {
    throw new Error('Expected a draft for the current release tag; refusing to publish');
  }
  const digests = new Map(expected.map(p => [p.name, p.sha256]));
  if (digests.size !== 2 || expected.length !== 2 || assets.length !== 2 ||
      new Set(assets.map(a => a.name)).size !== 2) {
    throw new Error('Expected exactly the normal and Wear module ZIPs');
  }
  for (const asset of assets) {
    const digest = digests.get(asset.name);
    if (!/^[a-f0-9]{64}$/.test(digest ?? '') || asset.state !== 'uploaded' ||
        asset.digest !== `sha256:${digest}`) {
      throw new Error('Uploaded package identity or digest differs from validated pair');
    }
  }
}

module.exports = { validateReleaseAssets };
