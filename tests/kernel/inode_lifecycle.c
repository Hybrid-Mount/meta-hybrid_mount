// SPDX-License-Identifier: GPL-3.0-only
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#undef assert
#define assert(x) do { if (!(x)) { fprintf(stderr, "FAIL line %d: %s\n", __LINE__, #x); exit(1); } } while (0)

typedef uint32_t u32;
typedef uint16_t u16;
typedef unsigned int uid_t;
#ifndef __always_inline
#define __always_inline inline
#endif
#define HM_FLAG_IS_DIR 1
#define HM_FLAG_VIRTUAL_DIR 2
#define HM_FLAG_WHITEOUT 4
#define HM_FLAG_OPAQUE 8
#define GFP_KERNEL 0
#define LOOKUP_FOLLOW 0
#ifndef ENODATA
#define ENODATA 61
#endif
#ifndef ENOMEM
#define ENOMEM 12
#endif
#ifndef ENOTDIR
#define ENOTDIR 20
#endif
#define HYBRIDMOUNT_MAGIC_SIG 0
#define likely(x) (x)
#define unlikely(x) (x)
#define READ_ONCE(x) (x)
#define WRITE_ONCE(x, v) ((x) = (v))
#define smp_load_acquire(p) (*(p))
#define smp_store_release(p, v) (*(p) = (v))
#define rcu_access_pointer(p) (p)
#define rcu_dereference(p) (p)
#define ERR_PTR(n) ((void *)(intptr_t)(n))
#define IS_ERR(p) ((uintptr_t)(p) >= (uintptr_t)-4095)
#define hm_get_vpath(r) ((r)->paths)
#define hm_get_rpath(r) ((r)->paths + (r)->v_len + 1)
#define hm_dir_tag(d) test_dir_tag(d)
#define hm_dir_set_owner(d, r) test_dir_set_owner(d, r)
#define INIT_LIST_HEAD(p) ((void)(p))
#define list_add(a, b) ((void)(a), (void)(b))
#define list_for_each_entry(r, h, member) for ((r) = NULL; (r); (r) = NULL)
#define QSTR_INIT(n, l) { (const unsigned char *)(n), (l) }

struct inode;
struct super_operations { void (*evict_inode)(struct inode *); };
struct super_block { const struct super_operations *s_op; };
struct address_space { int unused; };
struct inode {
    void *i_private;
    const void *i_op, *i_fop;
    struct super_block *i_sb;
    struct address_space i_data, *i_mapping;
    bool dying;
};
struct qstr { const unsigned char *name; size_t len; };
struct dentry { struct inode *inode; struct qstr d_name; struct super_block *d_sb; };
struct path { struct dentry *dentry; void *mnt; };
struct list_head { int unused; };
struct hybridmount_dir_node { struct inode *v_inode; uintptr_t tag; int rcu; };
struct hm_inode_info { struct path r_path; struct hybridmount_dir_node *dir_node; u16 flags; };
struct hm_rule_info { struct path r_path; struct hybridmount_dir_node *this_dir; u16 flags; };
struct hybridmount_rule {
    struct path parent_path, r_path;
    struct hybridmount_dir_node *this_dir;
    u16 v_len, flags;
    u32 v_hash;
    unsigned long v_ino;
    struct list_head list_node, topology_node;
    char paths[128];
};
struct hm_iop { struct hybridmount_dir_node *dir_node; };
struct hm_fop { struct hybridmount_dir_node *dir_node; };
struct hm_sop { const struct super_operations *orig_sop; };
struct hm_uid_array { int count; unsigned int uids[3]; };
static struct hm_uid_array *hybridmount_uids;

static const int hm_file_iops, hm_dir_iops, native_iops;
static struct hm_rule_info fixture;
static struct inode winner;
static int allocations, path_refs, early_publications, exchange_mode;
static int dropped, spliced, evicted, restored, hook_calls, topology_allocations;
static int rcu_entries;
static int freed_nodes;
static int rule_reads;
static bool delete_after_lookup, fail_allocation;
static struct dentry parent_dentry;
static struct hybridmount_dir_node *injected_parent;
static struct hm_sop super_hook;
static struct inode *retiring_inode;
static inline void hm_destroy_virtual_inode(struct inode *inode);

static void test_dir_set_owner(struct hybridmount_dir_node *node, struct hybridmount_rule *owner) {
    if (exchange_mode == 5) {
        exchange_mode = 0;
        hm_destroy_virtual_inode(retiring_inode);
    }
    node->tag = (uintptr_t)owner | 1;
}
#define smp_mb() ((void)0)
static void hm_detach_dir_node(struct hybridmount_dir_node *node) {}

static uintptr_t test_dir_tag(struct hybridmount_dir_node *node) {
    if (exchange_mode == 3 && !node->v_inode) {
        node->v_inode = &winner;
        node->tag = 1;
        exchange_mode = 0;
    }
    return node->tag;
}
static void hm_dir_rcu_free(int *p) { freed_nodes++; }
static void call_rcu(int *p, void (*callback)(int *)) { callback(p); }
static void path_get(struct path *p) { if (p->dentry) path_refs++; }
static void path_put(struct path *p) { if (p->dentry) path_refs--; }
static void kfree(void *p) { free(p); }
static void *kmalloc(size_t size, int flags) { return calloc(1, size); }
static void rcu_read_lock(void) { rcu_entries++; }
static void rcu_read_unlock(void) {}
struct test_uid { unsigned int val; };
static struct test_uid current_fsuid(void) { return (struct test_uid){0}; }
static bool __hybridmount_get_rule_info(struct hybridmount_dir_node *node,
    const unsigned char *name, size_t len, u32 hash, struct hm_rule_info *info, bool get_path) {
    if (++rule_reads == 2 && delete_after_lookup) return false;
    *info = fixture;
    if (get_path) path_get(&info->r_path);
    else info->r_path = (struct path){0};
    return true;
}
static struct inode *new_inode(struct super_block *sb) {
    if (fail_allocation) return NULL;
    struct inode *inode = calloc(1, sizeof(*inode));
    allocations++;
    inode->i_sb = sb;
    inode->i_mapping = &inode->i_data;
    return inode;
}
static void hybridmount_init_prealloc_inode(struct inode *inode,
    struct hm_inode_info *info, struct hm_rule_info *rule) {
    info->dir_node = rule->this_dir;
    info->r_path = rule->r_path;
    inode->i_private = info;
    inode->i_op = &hm_dir_iops;
}
static struct inode *igrab(struct inode *inode) {
    assert(!IS_ERR(inode));
    return inode->dying ? NULL : inode;
}
static struct inode *test_cmpxchg(struct inode **slot, struct inode *expected, struct inode *value) {
    if ((exchange_mode == 2 || exchange_mode == 4) && expected == NULL && value && !IS_ERR(value)) {
        *slot = exchange_mode == 4 ? (struct inode *)-1L : &winner;
        exchange_mode = 0;
    }
    struct inode *previous = *slot;
    if (previous == expected) {
        *slot = value;
        if (value && !IS_ERR(value) && (!value->i_private || value->i_op != &hm_dir_iops))
            early_publications++;
    }
    return previous;
}
#define cmpxchg(p, old, value) test_cmpxchg(p, old, value)
static void iput(struct inode *inode);
static void hybridmount_hijack_dentry_ops(struct inode *inode, struct dentry *dentry, bool injected) {}
static void d_add(struct dentry *dentry, struct inode *inode) { dentry->inode = inode; }
static struct dentry *d_splice_alias(struct inode *inode, struct dentry *dentry) {
    assert(inode && inode->i_private);
    spliced++;
    return NULL;
}
static struct hm_sop *hm_get_hm_sop(const struct super_operations *ops) { return &super_hook; }
static __attribute__((unused)) void hm_destroy_hijacked_inode(struct inode *inode, bool restore) {
    restored = restore;
    if (restore) inode->i_op = &native_iops;
}
static void truncate_inode_pages_final(struct address_space *mapping) {}
static void clear_inode(struct inode *inode) { evicted++; }
static struct inode *d_backing_inode(struct dentry *dentry) { return dentry->inode; }
static struct hm_iop *hm_get_hm_iop(const void *ops) { return NULL; }
static struct hm_fop *hm_get_hm_fop(const void *ops) { return NULL; }
static int kern_path(const char *name, int flags, struct path *path) {
    *path = (struct path){ &parent_dentry, NULL };
    path_get(path);
    return 0;
}
static struct hybridmount_rule *hm_tree_search_path(int len, const char *path) { return NULL; }
static struct hybridmount_dir_node *__hybridmount_alloc_dir_node(void) {
    topology_allocations++;
    return calloc(1, sizeof(struct hybridmount_dir_node));
}
static int __hybridmount_inject_child_locked(struct hybridmount_dir_node *node,
    struct hybridmount_rule *rule, const char *name, size_t len) {
    injected_parent = node;
    return 0;
}
static void hybridmount_hijack_dir_ops(struct hybridmount_dir_node *node, struct inode *inode) { hook_calls++; }
static void hybridmount_hijack_superblock(struct super_block *sb) { hook_calls++; }
static struct dentry *hm_hash_and_lookup(struct dentry *parent, struct qstr *name) { return NULL; }
static void d_drop(struct dentry *dentry) {}
static void dput(struct dentry *dentry) {}
static u32 full_name_hash(const void *salt, const char *name, size_t len) { return 1; }
static int hm_tree_insert(struct hybridmount_rule *rule) { return 0; }
static void hm_rollback_generated_topology_locked(struct hybridmount_rule *rule,
    struct list_head *generated, struct list_head *victims) { assert(false); }

#include "production.h"

static void iput(struct inode *inode) {
    if (!inode) return;
    dropped++;
    if (inode->i_private) hybridmount_hijacked_evict_inode(inode);
    free(inode);
}
static void native_evict(struct inode *inode) {
    assert(restored && inode->i_op == &native_iops);
    evicted++;
}
static void reset(void) {
    allocations = path_refs = early_publications = dropped = spliced = evicted = restored = 0;
    hook_calls = topology_allocations = exchange_mode = 0;
    rcu_entries = 0;
    freed_nodes = 0;
    rule_reads = 0;
    delete_after_lookup = fail_allocation = false;
    hybridmount_uids = NULL;
    memset(&fixture, 0, sizeof(fixture));
    memset(&winner, 0, sizeof(winner));
    super_hook.orig_sop = NULL;
}
static void lookup_case(const char *mode) {
    reset();
    struct hybridmount_dir_node node = {0};
    struct super_block sb = {0};
    struct inode dir = {.i_sb = &sb};
    struct dentry source = {0}, target = {.d_name = QSTR_INIT("child", 5)};
    fixture.this_dir = &node;
    fixture.r_path.dentry = &source;
    if (!strcmp(mode, "whiteout")) fixture.flags = HM_FLAG_WHITEOUT;
    if (!strcmp(mode, "opaque")) {
        fixture.flags = HM_FLAG_VIRTUAL_DIR | HM_FLAG_OPAQUE;
        fixture.r_path.dentry = NULL;
    }
    delete_after_lookup = !strcmp(mode, "deleted");
    fail_allocation = !strcmp(mode, "allocation");
    struct hm_inode_info winner_info = {.dir_node = &node};
    winner.i_private = &winner_info;
    winner.i_op = &hm_dir_iops;
    if (!strcmp(mode, "existing") || !strcmp(mode, "dying")) node.v_inode = &winner;
    if (!strcmp(mode, "dying") || !strcmp(mode, "race-dying")) winner.dying = true;
    if (!strcmp(mode, "race") || !strcmp(mode, "race-dying")) exchange_mode = 2;
    if (!strcmp(mode, "retired")) exchange_mode = 4;
    struct dentry *result = hybridmount_resolve_rule_dentry(&dir, &target, &node, 0);
    assert(!early_publications);
    if (!strcmp(mode, "dying") || !strcmp(mode, "race-dying") ||
        !strcmp(mode, "retired") || delete_after_lookup || fail_allocation) {
        assert(result == ERR_PTR(-ENODATA) && !spliced && path_refs == 0);
    } else if (!strcmp(mode, "whiteout")) {
        assert(result == NULL && !spliced && !allocations && path_refs == 0);
    } else {
        assert(result == NULL && spliced == 1);
    }
    if (!strcmp(mode, "existing")) assert(allocations == 0);
    if (!strcmp(mode, "race")) assert(node.v_inode == &winner && dropped == 1 && path_refs == 0);
    if (!strcmp(mode, "new") || !strcmp(mode, "opaque")) {
        assert(node.v_inode && node.v_inode != &winner);
        assert(path_refs == (!strcmp(mode, "opaque") ? 0 : 1));
        iput(node.v_inode);
        assert(path_refs == 0 && node.v_inode == NULL);
    }
    printf("PASS: inode lookup %s\n", mode);
}
int main(int argc, char **argv) {
    const char *mode = argc > 1 ? argv[1] : "new";
    if (!strcmp(mode, "evict")) {
        reset();
        struct super_block sb = {0};
        struct inode inode = {.i_sb = &sb};
        struct address_space borrowed = {0};
        inode.i_op = &hm_file_iops;
        inode.i_mapping = &borrowed;
        inode.i_private = calloc(1, sizeof(struct hm_inode_info));
        hybridmount_hijacked_evict_inode(&inode);
        assert(!inode.i_private && inode.i_mapping == &inode.i_data && evicted == 1);
        struct super_operations native = {.evict_inode = native_evict};
        super_hook.orig_sop = &native;
        inode.i_op = &native_iops;
        hybridmount_hijacked_evict_inode(&inode);
        assert(evicted == 2);
        puts("PASS: cleanup before native eviction and borrowed mapping release");
    } else if (!strcmp(mode, "retired-republish") || !strcmp(mode, "orphan")) {
        reset();
        struct hybridmount_dir_node node = {0};
        struct inode inode = {0};
        struct hm_inode_info *info = calloc(1, sizeof(*info));
        info->dir_node = &node;
        inode.i_private = info;
        node.v_inode = &inode;
        bool orphan = !strcmp(mode, "orphan");
        if (orphan) node.tag = 1;
        else exchange_mode = 3;
        hm_destroy_virtual_inode(&inode);
        if (orphan) {
            assert(freed_nodes == 1 && node.v_inode == (struct inode *)-1L);
            puts("PASS: orphan topology is retired once and rejects new creators");
        } else {
            assert(!freed_nodes && node.v_inode == &winner);
            puts("PASS: old eviction cannot retire a newly published inode's topology");
        }
    } else if (!strcmp(mode, "retire-eviction")) {
        reset();
        struct hybridmount_dir_node node = {0};
        struct inode inode = {0};
        struct hm_inode_info *info = calloc(1, sizeof(*info));
        struct hybridmount_rule *rule = calloc(1, sizeof(*rule));
        info->dir_node = &node;
        inode.i_private = info;
        node.v_inode = &inode;
        node.tag = (uintptr_t)rule | 1;
        rule->this_dir = &node;
        rule->flags = HM_FLAG_VIRTUAL_DIR;
        retiring_inode = &inode;
        exchange_mode = 5;
        hm_free_rule(rule);
        assert(freed_nodes == 1 && node.v_inode == (struct inode *)-1L && !inode.i_private);
        puts("PASS: retirement racing with eviction does not leak orphan topology");
    } else if (!strcmp(mode, "topology") || !strcmp(mode, "real-parent")) {
        reset();
        struct hybridmount_dir_node node = {0};
        struct hm_inode_info info = {.dir_node = &node};
        struct inode inode = {.i_op = &hm_dir_iops, .i_private = &info};
        bool real = !strcmp(mode, "real-parent");
        if (real) inode.i_op = &native_iops;
        parent_dentry.inode = &inode;
        struct hybridmount_rule rule = {.v_len = 13};
        memcpy(rule.paths, "/parent/child", sizeof("/parent/child"));
        struct list_head generated = {0}, victims = {0};
        assert(hybridmount_generate_virtual_topology(&rule, &generated, &victims) == 0);
        if (real) {
            assert(hook_calls == 2 && topology_allocations == 1);
            assert(path_refs == 1 && rule.parent_path.dentry == &parent_dentry);
            path_put(&rule.parent_path);
            free(injected_parent);
            puts("PASS: real parent retains its path reference");
        } else {
            assert(!hook_calls && !topology_allocations && injected_parent == &node);
            assert(path_refs == 0 && !rule.parent_path.dentry);
            puts("PASS: existing virtual topology is reused without recursive hooks");
        }
    } else if (!strcmp(mode, "uids")) {
        reset();
        assert(!hybridmount_is_uid_blocked(3) && rcu_entries == 0);
        struct hm_uid_array uids = {.count = 3, .uids = {1, 3, 7}};
        hybridmount_uids = &uids;
        assert(hybridmount_is_uid_blocked(3));
        assert(!hybridmount_is_uid_blocked(2));
        assert(rcu_entries == 2);
        puts("PASS: empty UID table avoids RCU and populated table still isolates");
    } else {
        lookup_case(mode);
    }
    return 0;
}
