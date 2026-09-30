<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

## Names of items are short (M-SHORT-NAMES) { #M-SHORT-NAMES }

<why>idiomatic code.</why>

The Rust convention that item identifiers are short should be followed:

- identifiers should not compound more than 2 short words (`AppConfig` over `GlobalApplicationConfig`),
- module or crate information shouldn't be baked into prefixes (`foo::Id` over `foo::FooId`), in particular when the direct 'super' item is sufficiently descriptive - in these cases users are expected to disambiguate items locally via qualifiers where needed (`fn convert(foo::Id) -> bar::Id`).
- abbreviations are preferred (`CallbackFn` over `CallbackFunction`),

Any of these rules can be broken where it makes local sense, but on a per-crate bases these exceptions should be _exceptional_ and well motivated.
