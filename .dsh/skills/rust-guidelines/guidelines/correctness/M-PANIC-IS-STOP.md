<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

## Panic means 'stop the program' (M-PANIC-IS-STOP) { #M-PANIC-IS-STOP }

<why>soundness and predictability.</why>

Panics are not exceptions. Instead, they suggest immediate program termination.

Although your code must be [_minimally_ panic-safe](https://doc.rust-lang.org/nomicon/exception-safety.html) (i.e., a survived panic may not lead to
undefined state), invoking a panic means _this program should stop now_. It is not valid to:

- use panics to communicate (errors) upstream,
- use panics to handle self-inflicted error conditions,
- assume panics will be caught, even by your own code.

For example, if the application calling you is compiled with a `Cargo.toml` containing

```toml
[profile.release]
panic = "abort"
```

then any invocation of panic will cause an otherwise functioning program to needlessly abort. Valid reasons to panic are:

- when encountering a programming error, e.g., `x.expect("must never happen")`,
- anything invoked from const contexts, e.g., `const { foo.unwrap() }`,
- when user requested, e.g., providing an `unwrap()` method yourself,
- when encountering a poison, e.g., by calling `unwrap()` on a lock result (a poisoned lock signals another thread has panicked already).

Any of those are directly or indirectly linked to programming errors.
