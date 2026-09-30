<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

## Lint overrides should use `#[expect]` (M-LINT-OVERRIDE-EXPECT) { #M-LINT-OVERRIDE-EXPECT }

<why>a current, tidy lint set.</why>

When overriding project-global lints inside a submodule or item, you should do so via `#[expect]`, not `#[allow]`.

Expected lints emit a warning if the marked warning was not encountered, thus preventing the accumulation of stale lints.
That said, `#[allow]` lints are still useful when applied to generated code, and can appear in macros.

Overrides should be accompanied by a `reason`:

```rust,edition2021
#[expect(clippy::unused_async, reason = "API fixed, will use I/O later")]
pub async fn ping_server() {
  // Stubbed out for now
}
```
