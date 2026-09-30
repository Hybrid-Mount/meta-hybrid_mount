<!-- Copyright (c) Microsoft Corporation. Licensed under the MIT license. -->

## FFI crates follow established naming conventions (M-FFI-NAMING) { #M-FFI-NAMING }

<why>immediately recognizable crate roles across projects.</why>

Crates used for FFI should follow established naming practices:

- `-sys` for crates defining items to call into existing (C-style) libraries
- `-ffi` for crates defining (C-style) items when called from existing applications

There are slight variations of this scheme (e.g., `-sys2` when a previous `-sys` crate was abandoned and using `-` vs `_`), but overall `-ffi` clearly defines 'export' libraries, and `-sys` 'import' ones.
