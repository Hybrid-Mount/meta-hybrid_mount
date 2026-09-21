# Third-Party Notices

This file records third-party components that Hybrid Mount is derived from or distributes alongside, together with the attribution and license terms that apply. It complements [LICENSE](LICENSE) (Hybrid Mount core, GPL-3.0-only).

## NoMount — VFS kernel subsystem (`hybridmount`)

- Upstream project: NoMount, https://github.com/maxsteeel/nomount
- Upstream author: maxsteeel
- Forked commit: 016375cd4a9e7da07b0519dd7bc492101de2a834 (2026-09-13), protocol version "20"
- What is derived: the VFS kernel subsystem that Hybrid Mount ships as the `hybridmount` kernel module — a fork of NoMount's kernel sources, developed and distributed independently by Hybrid Mount.
- License: **GPL-2.0-only**. Upstream shipped a GNU General Public License version 3 text with no per-file SPDX headers, while the kernel module declared a bare `MODULE_LICENSE("GPL")`. The Hybrid Mount module resolves that conflict by carrying a GPL-2.0-only grant, stated where it is legally operative: [`module/vfs/src/LICENSE`](module/vfs/src/LICENSE) is the GPL-2.0 text and every source file has an `SPDX-License-Identifier: GPL-2.0-only` header. The module also declares `MODULE_LICENSE("GPL v2")`, which the kernel lists as a free-software ident equivalent to `"GPL"`; per `include/linux/module.h` that string marks the module as GPL for symbol-binding purposes but does not itself distinguish "v2 only" from "v2 or later". Upstream's GPL-3.0 text is no longer part of this repository.
- Attribution Hybrid Mount commits to:
  - preserve upstream copyright notices in derived files;
  - retain MODULE_AUTHOR("maxsteeel") in the kernel module;
  - record the fork commit and make the derivation explicit in design documents and release notes.
- Interoperability and non-affiliation:
  - The module is an independent implementation. It does not interoperate with NoMount's kernel or its nm CLI, and Hybrid Mount does not use the NoMount name or branding to imply endorsement.
  - That separation is enforced at the wire level, not only by naming: the module registers a different keyring key type ("hybridmount" vs "nomount"), reports the protocol version "hm1" (vs upstream "20"), and uses an HM-exclusive payload magic (0x4859425249444D4F, ASCII "HYBRIDMO") in place of upstream's 0x4E4F4D4F554E54 ("NOMOUNT"). A stock nm CLI is therefore rejected during payload validation.
  - Hybrid Mount is not affiliated with, sponsored by, or endorsed by the NoMount project.
- Distribution status: the kernel sources, build instructions and one prebuilt aarch64 module per supported Android/GKI target are committed to this repository and included in release packages. `module/vfs/binaries/list.txt` records the SHA-256 digest of every module.

## lkmloader — built-in userspace LKM loading strategy

- Upstream project: lkmloader, https://github.com/maxsteeel/lkmloader
- Upstream author: maxsteeel
- Studied revision: af7fb29222377181220d9814f46b6a8b12cb770c
- License: GPL-3.0. The Rust implementation is distributed as part of the GPL-3.0-only Hybrid Mount core.
- Scope: `src/sys/lkm_image.rs` and `src/sys/lkm_compat.rs` reimplement the undefined-symbol resolution and in-memory vermagic adaptation strategy in Rust. No upstream C executable is bundled. The implementation adds checked ELF parsing, excludes module-owned and ambiguous symbol addresses, scopes kernel diagnostics to the attempted module and vermagic, and permits only one vermagic retry after a failed insertion.
- Selection strategy: exact Android/GKI target first, then candidates for the same kernel line, based on NoMount's `module/customize.sh` at revision 016375cd4a9e7da07b0519dd7bc492101de2a834. Hybrid Mount performs discovery at boot and retains its own protocol acceptance probe and persistent crash guard.

## Mountify — ext4 sysfs LKM

- Upstream project: Mountify, https://github.com/backslashxx/mountify
- License: GPL-2.0-only
- Scope: the optional ext4 sysfs lkm/ compatibility fallback (source and prebuilt .ko files). See module/lkm/README.md and module/lkm/src/LICENSE.

## rust-skills — vendored DSH skill

- Upstream project: rust-skills, https://github.com/leonardomso/rust-skills
- Upstream author: Leonardo Maldonado
- Vendored commit: fd2a861ab0406a4ac536a55274d14ea6fd1ca9c9, vendored 2026-09-18, upstream version 1.5.1
- What is derived: the agent-facing Rust rule library under [`.dsh/skills/rust-skills/`](.dsh/skills/rust-skills/) — `rules/` (265 rule files), `SKILL.md`, `LICENSE` and the upstream `checks/validate.py` index validator.
- Modifications: `SKILL.md` frontmatter was adapted for DeepSeek Harness skill discovery (`whenToUse`, `metadata.vendored`) and prefixed with a "Hybrid Mount precedence" section recording where this repository's contracts override the generic rules. The rule files themselves are unmodified.
- License: **MIT**. Upstream `LICENSE` is retained verbatim at `.dsh/skills/rust-skills/LICENSE`.
- Attribution Hybrid Mount commits to: preserve the upstream copyright notice and license text, record the vendored commit, and state the modifications above.
- Distribution status: development-tooling only. This directory is not compiled, packaged into the module zip, or shipped in releases.

## Distribution notes

- Hybrid Mount core (Rust and module scripts) is GPL-3.0-only; the WebUI is Apache-2.0.
- The Mountify LKM and the `hybridmount` kernel module are both GPL-2.0-only independent works distributed alongside the core, not merged into a single work.
- If an attribution is missing or incorrect, please open an issue or contact the maintainers.
