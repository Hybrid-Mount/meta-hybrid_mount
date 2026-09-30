<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

## Magic values are documented (M-DOCUMENTED-MAGIC) { #M-DOCUMENTED-MAGIC }

<why>maintainability and safe refactoring.</why>

Hardcoded _magic_ values in production code must be accompanied by a comment. The comment should outline:

- why this value was chosen,
- non-obvious side effects if that value is changed,
- external systems that interact with this constant.

You should prefer named constants over inline values.

```rust, ignore
// Bad: it's relatively obvious that this waits for a day, but not why
wait_timeout(60 * 60 * 24).await // Wait at most a day

// Better
wait_timeout(60 * 60 * 24).await // Large enough value to ensure the server
                                 // can finish. Setting this too low might
                                 // make us abort a valid request. Based on
                                 // `api.foo.com` timeout policies.

// Best

/// How long we wait for the server.
///
/// Large enough value to ensure the server
/// can finish. Setting this too low might
/// make us abort a valid request. Based on
/// `api.foo.com` timeout policies.
const UPSTREAM_SERVER_TIMEOUT: Duration = Duration::from_secs(60 * 60 * 24);
```
