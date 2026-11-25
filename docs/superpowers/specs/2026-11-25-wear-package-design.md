# Wear package split

## Intent and selected approach

Publish a normal ZIP and a separate Wear ZIP in the same GitHub Release, sharing
the release tag and version. The user selected this over independent Wear tags
and release schedules. Keep one source tree and shared backend code rather than
forking a second project.

## Package and UI behavior

- The normal package builds the phone WebUI and retains its four supported Android
  binary architectures. Remove its automatic Wear layout selection.
- The Wear package builds a dedicated Wear entry point, always opens the watch
  layout, and excludes phone layout code from its frontend dependency graph.
  Shared Material pages, translations, state, and backend APIs remain shared.
- Compile and package only `hybrid-mount-arm64` and `hybrid-mount-armv7` for Wear.
  Reject other architectures clearly during Wear installation. Retain the existing
  ARM-compatible kernel assets and their license/source provenance.
- Use the same `hybrid_mount` module ID and backend configuration paths. These are
  alternative installations; installing either replaces the other. This is an
  implementation choice to preserve existing runtime paths and configuration.
- Distinguish names and ZIP filenames with a Wear suffix. Keep the same numeric
  version code. Give Wear its own `update-wear.json`, pointing to the Wear ZIP;
  keep `update.json` pointing to the normal ZIP.

## Build and release integration

Extend `cargo xtask` with explicit normal/Wear build selection. Stage each flavor
separately so frontend files or binaries from a previous build cannot leak into
the other ZIP. Reuse compilation of the two ARM binaries where practical.

CI produces both artifacts. The release workflow validates both ZIPs, uploads both
to one draft, verifies both asset names and digests, then publishes the immutable
release. Generate both update documents from the same tag-derived version facts.
Preserve existing notification behavior while making both packages available.

## Validation and limits

Add regression checks for architecture selection, flavor metadata, separate update
URLs, and ZIP contents. Run the xtask tests, WebUI lint/type checks/tests, and both
frontend builds. Verify the Wear frontend excludes phone layouts and its ZIP has
exactly the two intended userspace binaries. Device installation and watch runtime
validation must be reported separately from host and CI checks.

No release tag or public release is created as part of implementing this change.
The dependency fix has already been committed and pushed separately.
