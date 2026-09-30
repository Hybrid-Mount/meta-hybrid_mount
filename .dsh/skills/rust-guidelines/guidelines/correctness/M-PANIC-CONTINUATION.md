<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

## Panic continuation is last resort (M-PANIC-CONTINUATION) { #M-PANIC-CONTINUATION }

<why>state integrity and freedom from subtle bugs.</why>

Panic recovery via `catch_unwind()` is a matter of last resort and must generally be followed by a controlled application restart.

Panics indicate the program has reached an unrecoverable state (compare [M-PANIC-IS-STOP](./#M-PANIC-IS-STOP) and [M-PANIC-ON-BUG](./#M-PANIC-ON-BUG)). Library code in particular should not attempt to catch a panic and continue execution, as there is a risk of observing otherwise impossible state:

```rust,ignore
thread_local! {
    static ALWAYS_EQUAL: RefCell<(i32, i32)> = RefCell::new((0, 0));
}

fn main() {
    let _ = panic::catch_unwind(|| {
        ALWAYS_EQUAL.with_borrow_mut(|p| {
            p.0 += 1;        
            panic!("Assume some user-provided closure failed here");  
            p.1 += 1;
        });
    });

    ALWAYS_EQUAL.with_borrow(|p| {
        assert_eq!(p.0, p.1);  // Broken!
    });
}
```

Although the example above is slightly contrived, the side effects and interactions of a caught panic can be harder to identify, can have wide blast radius, and be subtle.

Systems where many unrelated tasks are in flight (e.g., server request handlers) can use `catch_unwind` on a per-request basis, but should still promote an application restart after a request handler caused a panic. The purpose of `catch_unwind` here is not to continue execution indefinitely, but to allow all other requests to gracefully finish.
