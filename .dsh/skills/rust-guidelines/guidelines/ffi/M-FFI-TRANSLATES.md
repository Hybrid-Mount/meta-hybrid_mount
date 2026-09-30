<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

## Business logic belongs in core crates, FFI only translates (M-FFI-TRANSLATES) { #M-FFI-TRANSLATES }

<why>maximal safe code and a clean separation of concerns.</why>

When Rust is used to create FFI libraries, there should be a clear separation of concerns between the core _business logic_ crate `foo` and the glue crate `foo-ffi`.

Any operational functionality belongs in the core crate and should be expressed as idiomatic, safe, testable Rust. The FFI crate exists only to translate between native Rust and C constructs, and the core crate must not be infected with interop concerns, even if this means repeating, and slightly adjusting, type and function signatures. For example, given the following type in the core crate `foo`:

```rust,ignore
pub struct Message {
    destination: [u8; 8],
    data: Vec<u8>,
}

impl Message {
    pub fn new(destination: [u8; 8], data: Vec<u8>) -> Self { /* ... */ }
    pub fn transmit(&self) -> Result<(), TransmitError> { /* ... */ }
}
```

A proper separation of concerns might collapse construction and transmission into a single FFI entry point in `foo-ffi`:

```rust,ignore
#[no_mangle]
pub unsafe extern "C" fn transmit_message(
    destination: *const [u8; 8],
    data: *const u8,
    data_len: usize,
) -> u8 {
    let data = std::slice::from_raw_parts(data, data_len).to_vec();
    match Message::new(*destination, data).transmit() {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
```

However, it would be improper to leak FFI requirements into `foo` itself: ownership, data models and signatures do not translate seamlessly between the two worlds. Any time _saved_ by skipping a clean split will have to be paid back many times over during refactorings down the line.

```rust
#[repr(C)]
pub struct Message {
    pub destination: [u8; 8],
    pub data_ptr: *mut u8,
    pub data_len: usize,
    pub data_cap: usize,
}
```
