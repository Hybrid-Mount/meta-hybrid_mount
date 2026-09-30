<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

## Follow the upstream guidelines (M-UPSTREAM-GUIDELINES) { #M-UPSTREAM-GUIDELINES }

<why>a codebase that reflects community lessons and does not surprise users or contributors.</why>

The guidelines in this book complement existing Rust guidelines, in particular:

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
- [Rust Style Guide](https://doc.rust-lang.org/nightly/style-guide/)
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns//intro.html)
- [Rust Reference - Undefined Behavior](https://doc.rust-lang.org/reference/behavior-considered-undefined.html)

We recommend you read through these as well, and apply them in addition to this book's items. Pay special attention to the ones below, as they are frequently forgotten:

- [ ] [C-CONV](https://rust-lang.github.io/api-guidelines/naming.html#ad-hoc-conversions-follow-as_-to_-into_-conventions-c-conv) - Ad-hoc conversions
  follow  `as_`, `to_`, `into_` conventions
- [ ] [C-GETTER](https://rust-lang.github.io/api-guidelines/naming.html#getter-names-follow-rust-convention-c-getter) - Getter names follow Rust convention
- [ ] [C-COMMON-TRAITS](https://rust-lang.github.io/api-guidelines/interoperability.html#c-common-traits) - Types eagerly implement common traits
  - `Copy`, `Clone`, `Eq`, `PartialEq`, `Ord`, `PartialOrd`, `Hash`, `Default`, `Debug`
  - `Display` where type wants to be displayed
- [ ] [C-CTOR](https://rust-lang.github.io/api-guidelines/predictability.html?highlight=new#constructors-are-static-inherent-methods-c-ctor) -
  Constructors are static, inherent methods
  - In particular, have `Foo::new()`, even if you have `Foo::default()`
- [ ] [C-FEATURE](https://rust-lang.github.io/api-guidelines/naming.html#feature-names-are-free-of-placeholder-words-c-feature) - Feature names
  are free of placeholder words
