# VFS source identity comparison

Reviewed 2026-11-06 against the local implementation based on d39c5925.

The current Hybrid Mount rule pins `kern_path(..., LOOKUP_FOLLOW)`'s path. Inode generations retain their own immutable backing path; replacing a rule can publish a new generation without rebinding an open old inode. `hm_open` opens that pinned path. This is object identity, rather than automatic following of a source pathname after atomic replacement.

| Operation after registration | Existing FD / pinned path model | Fresh path lookup |
| --- | --- | --- |
| Rename source | Same object | Rule still refers to pinned object |
| Unlink and recreate pathname | Old object remains referenced | No automatic rebind to recreated object |
| Atomic rename over source | Old object remains referenced | No automatic rebind to replacement |
| Source through symlink | Referent captured by LOOKUP_FOLLOW | Retargeting the symlink does not rebind the rule |
| Explicit rule replacement | Old FD retains its generation | New generation uses the newly resolved source |

These are conclusions from owning source paths, **not device observations**. File content can still change in place. Native permissions and open error handling remain applicable. Lookup/getattr and concurrent rename need execution on a kernel running this exact module before their observed results can be recorded.

## Upstream dependency chain

- [66978f9c8d7a](https://github.com/maxsteeel/nomount/commit/66978f9c8d7a) removes lookup-time rebinding and keeps inode bindings immutable. Hybrid Mount already uses immutable generations; copying its implementation would duplicate behavior and disturb our ownership model.
- [940efd11b225](https://github.com/maxsteeel/nomount/commit/940efd11b225) obtains the real basename from the pinned dentry instead of the spelling of a source path, including symlink sources.
- [0fb89eac1a5b](https://github.com/maxsteeel/nomount/commit/0fb89eac1a5b) captures the parent and basename together under a d_seq retry. It changes snapshot cleanup as well as capture.
- [ab8c05775122](https://github.com/maxsteeel/nomount/commit/ab8c05775122) corrects the name_snapshot layout difference on kernels older than 5.3. The earlier patch cannot be imported alone as an old-kernel fix.

Hybrid Mount does not currently store this parent/basename source constraint. Adding it would change when a registered rule becomes unusable after source rename/unlink. Keep the existing binding semantics for this delivery; a change needs an explicit follow-versus-fixed design and concurrent device evidence. Host generation tests prove replacement lifetime, not rename behavior on the actual hooked filesystem.
