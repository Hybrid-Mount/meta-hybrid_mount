<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

## Unsafe implies undefined behavior (M-UNSAFE-IMPLIES-UB) { #M-UNSAFE-IMPLIES-UB }

<why>semantic consistency without warning fatigue.</why>

The marker `unsafe` may only be applied to functions and traits if misuse implies the risk of undefined behavior (UB).
It must not be used to mark functions that are dangerous to call for other reasons.

```rust
// Valid use of unsafe
unsafe fn print_string(x: *const String) { }

// Invalid use of unsafe
unsafe fn delete_database() { }
```
