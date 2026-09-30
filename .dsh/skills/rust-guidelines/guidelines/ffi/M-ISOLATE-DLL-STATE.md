<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

## Isolate DLL state between FFI libraries (M-ISOLATE-DLL-STATE) { #M-ISOLATE-DLL-STATE }

<why>data integrity and defined behavior across DLL boundaries.</why>

When loading multiple Rust-based dynamic libraries (DLLs) within one application, you may only share 'portable' state between these libraries.
Likewise, when authoring such libraries, you must only accept or provide 'portable' data from foreign DLLs.

Portable here means data that is safe and consistent to process regardless of its origin. By definition, this is a subset of FFI-safe types.
A type is portable if it is `#[repr(C)]` (or similarly well-defined), and _all_ of the following:

- It must not have any interaction with any `static` or thread local.
- It must not have any interaction with any `TypeId`.
- It must not contain any value, pointer or reference to any non-portable data (it is valid to point into portable data within non-portable data, such as
  sharing a reference to an ASCII string held in a `Box`).

_Interaction_ means any computational relationship, and therefore also relates to how the type is used. Sending a `u128` between DLLs is OK, using it to
exchange a transmuted `TypeId` isn't.

The underlying issue stems from the Rust compiler treating each DLL as an entirely new compilation artifact, akin to a standalone application. This means each DLL:

- has its own set of `static` and thread-local variables,
- the type layout of any `#[repr(Rust)]` type (the default) can differ between compilations,
- has its own set of unique type IDs, differing from any other DLL.

Notably, this affects:

- ⚠️ any allocated instance, e.g., `String`, `Vec<u8>`, `Box<Foo>`, ...
- ⚠️ any library relying on other statics, e.g., `tokio`, `log`,
- ⚠️ any struct not `#[repr(C)]`,
- ⚠️ any data structure relying on consistent `TypeId`.

In practice, transferring any of the above between libraries leads to data loss, state corruption, and usually undefined behavior.

Take particular note that this may also apply to types and methods that are invisible at the FFI boundary:

```rust,ignore
/// A method in DLL1 that wants to use a common service from DLL2
#[ffi_function]
fn use_common_service(common: &CommonService) {
    // This has at least two issues:
    // - `CommonService`, or ANY type nested deep within might have
    //   a different type layout in DLL2, leading to immediate
    //   undefined behavior (UB) ⚠️
    // - `do_work()` here looks like it will be invoked in DLL2, but
    //   the code executed will actually come from DLL1. This means that
    //   `do_work()` invoked here will see a data structure coming from
    //   DLL2, but will use statics from DLL1 ⚠️
    common.do_work();
}
```
