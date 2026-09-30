<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

# Checklist

- **Universal**
  - [ ] Follow the upstream guidelines ([M-UPSTREAM-GUIDELINES])
  - [ ] Use static verification ([M-STATIC-VERIFICATION])
  - [ ] Lint overrides should use `#[expect]` ([M-LINT-OVERRIDE-EXPECT])
  - [ ] Public types are Debug ([M-PUBLIC-DEBUG])
  - [ ] Public types meant to be read are Display ([M-PUBLIC-DISPLAY])
  - [ ] If in doubt, split the crate ([M-SMALLER-CRATES])
  - [ ] Names are free of weasel words ([M-WEASEL-WORDS])
  - [ ] Names of items are short ([M-SHORT-NAMES])
  - [ ] Prefer regular over associated functions ([M-REGULAR-FN])
  - [ ] Magic values are documented ([M-DOCUMENTED-MAGIC])
  - [ ] Use structured logging with message templates ([M-LOG-STRUCTURED])
- **Library / Interoperability**
  - [ ] Types are Send ([M-TYPES-SEND])
  - [ ] Native escape hatches ([M-ESCAPE-HATCHES])
  - [ ] Don't leak external types ([M-DONT-LEAK-TYPES])
  - [ ] Items come from their original crate ([M-FOREIGN-REEXPORTS])
  - [ ] Accept `impl AsRef<>` where feasible ([M-IMPL-ASREF])
  - [ ] Accept `impl RangeBounds<>` where feasible ([M-IMPL-RANGEBOUNDS])
  - [ ] Accept `impl 'IO'` where feasible ('sans IO') ([M-IMPL-IO])
- **Library / UX**
  - [ ] Abstractions don't visibly nest ([M-SIMPLE-ABSTRACTIONS])
  - [ ] Avoid smart pointers and wrappers in APIs ([M-AVOID-WRAPPERS])
  - [ ] Prefer types over generics, generics over dyn traits ([M-DI-HIERARCHY])
  - [ ] Errors are canonical structs ([M-ERRORS-CANONICAL-STRUCTS])
  - [ ] Canonical error conversion uses `From`, not `map_err` ([M-FROM-ERROR])
  - [ ] Complex type construction has builders ([M-INIT-BUILDER])
  - [ ] Complex type initialization hierarchies are cascaded ([M-INIT-CASCADED])
  - [ ] Services are Clone ([M-SERVICES-CLONE])
  - [ ] Essential functionality should be inherent ([M-ESSENTIAL-FN-INHERENT])
  - [ ] Modules are balanced in size and scope ([M-BALANCED-MODULES])
  - [ ] Don't define preludes ([M-NO-PRELUDE])
  - [ ] Parameter ordering is consistent ([M-PARAMETER-CONSISTENCY])
  - [ ] Collections implement the appropriate iter traits ([M-COLLECTION-TRAITS])
  - [ ] Functions are `async` over returning a Future ([M-ASYNC-FN])
- **Library / Resilience**
  - [ ] I/O and system calls are mockable ([M-MOCKABLE-SYSCALLS])
  - [ ] Test utilities are feature gated ([M-TEST-UTIL])
  - [ ] Integration tests live under `tests/` ([M-INTEGRATION-TESTS])
  - [ ] Use the proper type family ([M-STRONG-TYPES])
  - [ ] Newtypes guard their invariants ([M-STRONG-TYPES-GUARD])
  - [ ] Builders validate in final `.build()` ([M-BUILD-RESULT])
  - [ ] Don't glob re-export items ([M-NO-GLOB-REEXPORTS])
  - [ ] Avoid statics ([M-AVOID-STATICS])
  - [ ] Production code uses telemetry, not println ([M-LOG-NOT-PRINT])
- **Library / Building**
  - [ ] Libraries work out of the box ([M-OOBE])
  - [ ] Native `-sys` crates compile without dependencies ([M-SYS-CRATES])
  - [ ] Features are additive ([M-FEATURES-ADDITIVE])
- **Macros**
  - [ ] Macros are a last resort ([M-MACRO-LAST-RESORT])
  - [ ] Prefer 'macros by example' over proc macros ([M-EXAMPLE-OVER-PROC])
  - [ ] Macros don't lie about signatures ([M-MACROS-DONT-LIE])
  - [ ] Macros assume main crate ([M-MACRO-MAIN-CRATE])
  - [ ] Pin supporting proc macro crates ([M-MACRO-VERSION-PIN])
  - [ ] Third party items come from hidden `_private` module ([M-MACRO-HELPERS])
  - [ ] Proc macros should have separate impl crate incl. tests ([M-PROC-IMPL])
  - [ ] Proc macros don't produce implied or hidden items ([M-PROC-IMPLIED-ITEMS])
- **Applications**
  - [ ] Use mimalloc for apps ([M-MIMALLOC-APPS])
  - [ ] Applications may use Anyhow or derivatives ([M-APP-ERROR])
  - [ ] Applications target highest viable target-cpu ([M-TARGET-CPU])
- **FFI**
  - [ ] Isolate DLL state between FFI libraries ([M-ISOLATE-DLL-STATE])
  - [ ] Business logic belongs in core crates, FFI only translates ([M-FFI-TRANSLATES])
  - [ ] FFI crates follow established naming conventions ([M-FFI-NAMING])
- **Correctness**
  - [ ] Unsafe needs reason, should be avoided ([M-UNSAFE])
  - [ ] Unsafe implies undefined behavior ([M-UNSAFE-IMPLIES-UB])
  - [ ] All code must be sound ([M-UNSOUND])
  - [ ] Panic means 'stop the program' ([M-PANIC-IS-STOP])
  - [ ] Detected programming bugs are panics, not errors ([M-PANIC-ON-BUG])
  - [ ] Panic continuation is last resort ([M-PANIC-CONTINUATION])
  - [ ] Custom panics have a helpful message ([M-PANIC-MESSAGE])
- **Performance**
  - [ ] Optimize for throughput, avoid empty cycles ([M-THROUGHPUT])
  - [ ] Identify, profile, optimize the hot path early ([M-HOTPATH])
  - [ ] Long-running tasks should have yield points ([M-YIELD-POINTS])
  - [ ] Reuse allocations where possible ([M-MEM-REUSE])
  - [ ] Library telemetry does not tank performance ([M-LOG-OVERHEAD])
  - [ ] Nested type hierarchies should avoid needless indirection ([M-AVOID-INDIRECTION])
  - [ ] Use boxed slices and strings for immutable owned sequences ([M-BOX-DST])
  - [ ] Shrink collections to fit after building ([M-SHRINK-TO-FIT])
  - [ ] Use a fast hasher where possible ([M-FAST-HASHER])
  - [ ] Collections are created with sufficient initial capacity ([M-INITIAL-CAPACITY])
  - [ ] Hot `async` functions reduce stack size ([M-ASYNC-STACK-SIZE])
- **Project**
  - [ ] Common settings come from the workspace Cargo.toml ([M-CARGO-WORKSPACE])
  - [ ] The workspace lists and versions all crates ([M-CRATES-IN-WORKSPACE])
  - [ ] All crates are siblings in one folder ([M-CRATES-FLAT-FOLDER])
  - [ ] New crates target latest edition ([M-LATEST-EDITION])
  - [ ] MSRV is conservatively updated ([M-MSRV])
- **Documentation**
  - [ ] First sentence is one line; approx. 15 words ([M-FIRST-DOC-SENTENCE])
  - [ ] Has comprehensive module documentation ([M-MODULE-DOCS])
  - [ ] Documentation has canonical sections ([M-CANONICAL-DOCS])
  - [ ] Mark `pub use` items with `#[doc(inline)]` ([M-DOC-INLINE])
- **AI**
  - [ ] Design with AI use in mind ([M-DESIGN-FOR-AI])
  - [ ] Items are only visible through one path ([M-SINGLE-ITEM-PATH])
  - [ ] Avoid meta design documentation ([M-NO-META-DESIGN-DOCUMENTATION])
  - [ ] Tests do not assert ground truth ([M-TAUTOLOGICAL-TESTS])
  - [ ] Rust code solves Rust problems ([M-RUST-SHAPED])

<!-- Universal  -->
[M-UPSTREAM-GUIDELINES]: ../universal/#M-UPSTREAM-GUIDELINES
[M-STATIC-VERIFICATION]: ../universal/#M-STATIC-VERIFICATION
[M-LINT-OVERRIDE-EXPECT]: ../universal/#M-LINT-OVERRIDE-EXPECT
[M-PUBLIC-DEBUG]: ../universal/#M-PUBLIC-DEBUG
[M-PUBLIC-DISPLAY]: ../universal/#M-PUBLIC-DISPLAY
[M-SMALLER-CRATES]: ../universal/#M-SMALLER-CRATES
[M-WEASEL-WORDS]: ../universal/#M-WEASEL-WORDS
[M-SHORT-NAMES]: ../universal/#M-SHORT-NAMES
[M-REGULAR-FN]: ../universal/#M-REGULAR-FN
[M-DOCUMENTED-MAGIC]: ../universal/#M-DOCUMENTED-MAGIC
[M-LOG-STRUCTURED]: ../universal/#M-LOG-STRUCTURED
[M-LOG-NOT-PRINT]: ../libs/resilience/#M-LOG-NOT-PRINT

<!-- Libs -->
[M-TYPES-SEND]: ../libs/interop/#M-TYPES-SEND
[M-DONT-LEAK-TYPES]: ../libs/interop/#M-DONT-LEAK-TYPES
[M-FOREIGN-REEXPORTS]: ../libs/interop/#M-FOREIGN-REEXPORTS
[M-ESCAPE-HATCHES]: ../libs/interop/#M-ESCAPE-HATCHES
[M-STRONG-TYPES]: ../libs/resilience/#M-STRONG-TYPES
[M-STRONG-TYPES-GUARD]: ../libs/resilience/#M-STRONG-TYPES-GUARD
[M-NO-GLOB-REEXPORTS]: ../libs/resilience/#M-NO-GLOB-REEXPORTS
[M-ESSENTIAL-FN-INHERENT]: ../libs/ux/#M-ESSENTIAL-FN-INHERENT
[M-MOCKABLE-SYSCALLS]: ../libs/resilience/#M-MOCKABLE-SYSCALLS
[M-SIMPLE-ABSTRACTIONS]: ../libs/ux/#M-SIMPLE-ABSTRACTIONS
[M-AVOID-WRAPPERS]: ../libs/ux/#M-AVOID-WRAPPERS
[M-DI-HIERARCHY]: ../libs/ux/#M-DI-HIERARCHY
[M-ERRORS-CANONICAL-STRUCTS]: ../libs/ux/#M-ERRORS-CANONICAL-STRUCTS
[M-FROM-ERROR]: ../libs/ux/#M-FROM-ERROR
[M-INIT-BUILDER]: ../libs/ux/#M-INIT-BUILDER
[M-BUILD-RESULT]: ../libs/resilience/#M-BUILD-RESULT
[M-INIT-CASCADED]: ../libs/ux/#M-INIT-CASCADED
[M-SERVICES-CLONE]: ../libs/ux/#M-SERVICES-CLONE
[M-IMPL-ASREF]: ../libs/interop/#M-IMPL-ASREF
[M-IMPL-RANGEBOUNDS]: ../libs/interop/#M-IMPL-RANGEBOUNDS
[M-IMPL-IO]: ../libs/interop/#M-IMPL-IO
[M-BALANCED-MODULES]: ../libs/ux/#M-BALANCED-MODULES
[M-NO-PRELUDE]: ../libs/ux/#M-NO-PRELUDE
[M-PARAMETER-CONSISTENCY]: ../libs/ux/#M-PARAMETER-CONSISTENCY
[M-COLLECTION-TRAITS]: ../libs/ux/#M-COLLECTION-TRAITS
[M-ASYNC-FN]: ../libs/ux/#M-ASYNC-FN
[M-TEST-UTIL]: ../libs/resilience/#M-TEST-UTIL
[M-INTEGRATION-TESTS]: ../libs/resilience/#M-INTEGRATION-TESTS
[M-AVOID-STATICS]: ../libs/resilience/#M-AVOID-STATICS
[M-OOBE]: ../libs/building/#M-OOBE
[M-SYS-CRATES]: ../libs/resilience/#M-SYS-CRATES
[M-FEATURES-ADDITIVE]: ../libs/building/#M-FEATURES-ADDITIVE

<!-- Apps -->
[M-APP-ERROR]: ../apps/#M-APP-ERROR
[M-MIMALLOC-APPS]: ../apps/#M-MIMALLOC-APPS
[M-TARGET-CPU]: ../apps/#M-TARGET-CPU

<!-- FFI -->
[M-ISOLATE-DLL-STATE]: ../ffi/#M-ISOLATE-DLL-STATE
[M-FFI-TRANSLATES]: ../ffi/#M-FFI-TRANSLATES
[M-FFI-NAMING]: ../ffi/#M-FFI-NAMING

<!-- Correctness -->
[M-UNSAFE]: ../correctness/#M-UNSAFE
[M-UNSAFE-IMPLIES-UB]: ../correctness/#M-UNSAFE-IMPLIES-UB
[M-UNSOUND]: ../correctness/#M-UNSOUND
[M-PANIC-IS-STOP]: ../correctness/#M-PANIC-IS-STOP
[M-PANIC-ON-BUG]: ../correctness/#M-PANIC-ON-BUG
[M-PANIC-CONTINUATION]: ../correctness/#M-PANIC-CONTINUATION
[M-PANIC-MESSAGE]: ../correctness/#M-PANIC-MESSAGE

<!-- Performance -->
[M-HOTPATH]: ../performance/#M-HOTPATH
[M-THROUGHPUT]: ../performance/#M-THROUGHPUT
[M-YIELD-POINTS]: ../performance/#M-YIELD-POINTS
[M-MEM-REUSE]: ../performance/#M-MEM-REUSE
[M-LOG-OVERHEAD]: ../performance/#M-LOG-OVERHEAD
[M-AVOID-INDIRECTION]: ../performance/#M-AVOID-INDIRECTION
[M-BOX-DST]: ../performance/#M-BOX-DST
[M-SHRINK-TO-FIT]: ../performance/#M-SHRINK-TO-FIT
[M-FAST-HASHER]: ../performance/#M-FAST-HASHER
[M-INITIAL-CAPACITY]: ../performance/#M-INITIAL-CAPACITY
[M-ASYNC-STACK-SIZE]: ../performance/#M-ASYNC-STACK-SIZE

<!-- Project -->
[M-CARGO-WORKSPACE]: ../project/#M-CARGO-WORKSPACE
[M-CRATES-IN-WORKSPACE]: ../project/#M-CRATES-IN-WORKSPACE
[M-CRATES-FLAT-FOLDER]: ../project/#M-CRATES-FLAT-FOLDER
[M-LATEST-EDITION]: ../project/#M-LATEST-EDITION
[M-MSRV]: ../project/#M-MSRV

<!-- Docs -->
[M-FIRST-DOC-SENTENCE]: ../docs/#M-FIRST-DOC-SENTENCE
[M-MODULE-DOCS]: ../docs/#M-MODULE-DOCS
[M-CANONICAL-DOCS]: ../docs/#M-CANONICAL-DOCS
[M-DOC-INLINE]: ../docs/#M-DOC-INLINE

<!-- Macros -->
[M-MACRO-LAST-RESORT]: ../macros/#M-MACRO-LAST-RESORT
[M-EXAMPLE-OVER-PROC]: ../macros/#M-EXAMPLE-OVER-PROC
[M-MACROS-DONT-LIE]: ../macros/#M-MACROS-DONT-LIE
[M-MACRO-MAIN-CRATE]: ../macros/#M-MACRO-MAIN-CRATE
[M-MACRO-VERSION-PIN]: ../macros/#M-MACRO-VERSION-PIN
[M-MACRO-HELPERS]: ../macros/#M-MACRO-HELPERS
[M-PROC-IMPL]: ../macros/#M-PROC-IMPL
[M-PROC-IMPLIED-ITEMS]: ../macros/#M-PROC-IMPLIED-ITEMS

<!-- AI -->
[M-DESIGN-FOR-AI]: ../ai/#M-DESIGN-FOR-AI
[M-SINGLE-ITEM-PATH]: ../ai/#M-SINGLE-ITEM-PATH
[M-NO-META-DESIGN-DOCUMENTATION]: ../ai/#M-NO-META-DESIGN-DOCUMENTATION
[M-TAUTOLOGICAL-TESTS]: ../ai/#M-TAUTOLOGICAL-TESTS
[M-RUST-SHAPED]: ../ai/#M-RUST-SHAPED
