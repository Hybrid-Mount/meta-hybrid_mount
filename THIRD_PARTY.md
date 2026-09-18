# Third-Party Notices

This file records third-party components that Hybrid Mount is derived from or distributes alongside, together with the attribution and license terms that apply. It complements [LICENSE](LICENSE) (Hybrid Mount core, GPL-3.0-only).

## NoMount — VFS kernel subsystem (K2)

- Upstream project: NoMount, https://github.com/maxsteeel/nomount
- Upstream author: maxsteeel
- Forked commit: 016375cd4a9e7da07b0519dd7bc492101de2a834 (2026-09-13), protocol version "20"
- What is derived: the VFS kernel subsystem called K2 in Hybrid Mount's design — a fork of NoMount's kernel sources, developed and distributed independently by Hybrid Mount.
- License as declared upstream (currently inconsistent; to be clarified with the author before any compiled K2 artifact is distributed):
  - the repository ships a GNU General Public License version 3 text in LICENSE, with no per-file SPDX-License-Identifier headers and no project-level "or later" notice;
  - the kernel module declares MODULE_LICENSE("GPL"), which in Linux kernel convention means GPL-2.0-or-later.
- Attribution Hybrid Mount commits to:
  - preserve the upstream license text and any copyright notices in derived files;
  - retain MODULE_AUTHOR("maxsteeel") in the kernel module;
  - record the fork commit and make the derivation explicit in design documents and release notes.
- Interoperability and non-affiliation:
  - K2 is an independent implementation. It does not interoperate with NoMount's kernel or its nm CLI, and Hybrid Mount does not use the NoMount name or branding to imply endorsement.
  - That separation is enforced at the wire level, not only by naming: K2 registers a different keyring key type ("hybridmount" vs "nomount"), reports the protocol version "hm1" (vs upstream "20"), and uses an HM-exclusive payload magic (0x4859425249444D4F, ASCII "HYBRIDMO") in place of upstream's 0x4E4F4D4F554E54 ("NOMOUNT"). A stock nm CLI is therefore rejected during payload validation.
  - Hybrid Mount is not affiliated with, sponsored by, or endorsed by the NoMount project.
- Distribution status: K2 kernel sources and build instructions are part of this repository, but no compiled K2 artifact is committed, uploaded by CI, or included in a release while the upstream licence declaration remains unresolved.

## Mountify — ext4 sysfs LKM

- Upstream project: Mountify, https://github.com/backslashxx/mountify
- License: GPL-2.0-only
- Scope: the optional ext4 sysfs lkm/ compatibility fallback (source and prebuilt .ko files). See module/lkm/README.md and module/lkm/src/LICENSE.

## Distribution notes

- Hybrid Mount core (Rust and module scripts) is GPL-3.0-only; the WebUI is Apache-2.0.
- The Mountify LKM (GPL-2.0-only) and the K2 kernel module are independent works distributed alongside the core, not merged into a single work.
- If an attribution is missing or incorrect, please open an issue or contact the maintainers.
