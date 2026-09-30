<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

## Custom panics have a helpful message (M-PANIC-MESSAGE) { #M-PANIC-MESSAGE }

<why>faster bug diagnosis.</why>

When code panics intentionally (via `panic!`, `assert!`, `unreachable!`, `todo!`, or similar), a message must be present to clearly state what went wrong and, where applicable, include relevant values.

```rust,ignore
// Bad, the panic gives the developer little to act on.
assert!(buffer.len() >= HEADER_SIZE);

// Good, message contains reason and actual values.
assert!(buffer.len() >= HEADER_SIZE, "buffer too small for header: got {} bytes, need {HEADER_SIZE}", buffer.len());
```

Messages related to API misuse should be useful to the end user. Messages indicating bugs should be helpful to you-as-the-author, or whoever maintains the project after you, to quickly identify the underlying cause.

Panic messages in tests are not generally needed.
