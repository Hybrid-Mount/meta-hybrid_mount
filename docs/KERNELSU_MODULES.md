# KernelSU Modules distribution

Hybrid Mount publishes from this public repository. Its permanent module ID is
`hybrid_mount`; it does not need to match the repository name. The root
`module.yaml` supplies the catalog metadata from the default branch, `main`.
Versions and download URLs come from the release ZIP's generated `module.prop`.
The manifest declares API 26 as the module minimum; the riscv64 binary uses
API 35, while arm64, armv7, and x86_64 use API 26.

## Initial registration

1. Enable **Settings → General → Releases → Enable release immutability** before
   publishing a new release. Existing releases do not become immutable.
2. Install the read-only [KernelSU Modules GitHub App](https://developers.kernelsu.org/)
   for `Hybrid-Mount/meta-hybrid_mount`. An organization owner may need to approve it.
3. In the [developer portal](https://developers.kernelsu.org/), open **My repositories**,
   run **Precheck**, confirm `hybrid_mount`, and select **Submit for inclusion**.
4. Wait for the first submission's maintainer review. A successful package precheck
   or GitHub release does not itself mean the module has been listed.

## Releases

Integrate `dev` into `main` and wait for **Build Hybrid Mount** to pass before
tagging. The release workflow resolves the tag through `cargo xtask release-version`,
builds all four Android binaries, and verifies the prebuilt VFS source digests.
The official `KernelSU-Modules/tools/actions/module-release` action is pinned to a
full commit SHA. It validates the ZIP and `module.yaml`, creates a draft, uploads
one module ZIP, publishes, and verifies `immutable: true`.

The asset name is `hybrid_mount-{versionCode}-{version}.zip`. The workflow uses
the action's actual asset name when generating `update.json`, so manager updates
resolve to the same uploaded file. Telegram notification runs after version sync
so a notification failure cannot leave manager updates pointing at the previous release.
Keep the existing self-hosted `updateJson`
until catalog registration has completed.

For `v6.2.3`, `version=6.2.3` and `versionCode=602003999`, higher than
`v6.2.3-rc.3` (`602003303`). Never move or reuse a published tag or replace its
ZIP; corrections require a new version and a higher `versionCode`.

The **KernelSU Modules** Check Run and the portal show catalog validation after
submission. Android/GKI ABI and device behavior still require device validation.

References: [Getting started](https://developers.kernelsu.org/docs/zh-CN/getting-started),
[module.yaml](https://developers.kernelsu.org/docs/zh-CN/module-yaml),
[Releasing](https://developers.kernelsu.org/docs/zh-CN/releasing).
