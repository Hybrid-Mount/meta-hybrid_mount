#ifndef _LINUX_HYBRIDMOUNT_H
#define _LINUX_HYBRIDMOUNT_H

#include <linux/types.h>
#include <linux/idr.h>
#include <linux/list.h>
#include <linux/rcupdate.h>
#include <linux/rwsem.h>
#include <linux/srcu.h>
#include <linux/atomic.h>
#include <linux/file.h>
#include <linux/key-type.h>
#include <linux/highmem.h>
#include <linux/version.h>
#include <linux/compat.h>

#define HYBRIDMOUNT_VERSION "hm1"
#define HYBRIDMOUNT_MAGIC_SIG 0x4E4F4D4F554E54ULL /* upstream magic; replacement pending decision */
#define HM_FLAG_IS_DIR      (1 << 0)
#define HM_FLAG_VIRTUAL_DIR (1 << 1)
#define HM_FLAG_WHITEOUT    (1 << 2)

#define HM_CLEAR_UIDS  (1 << 0)
#define HM_CLEAR_RULES (1 << 1)
#define HM_CLEAR_EXIT  (1 << 2)

#define hm_debug(fmt, ...) printk(KERN_DEBUG "hybridmount: [DEBUG] " fmt, ##__VA_ARGS__)
#define hm_info(fmt, ...) printk(KERN_INFO "hybridmount: " fmt, ##__VA_ARGS__)
#define hm_warn(fmt, ...) printk(KERN_WARNING "hybridmount: [WARN] " fmt, ##__VA_ARGS__)
#define hm_err(fmt, ...)  printk(KERN_ERR "hybridmount: [ERROR] " fmt, ##__VA_ARGS__)

static void *hybridmount_art_root = NULL;
static struct hm_uid_array __rcu *hybridmount_uids = NULL;
static LIST_HEAD(hybridmount_rules_list);
static LIST_HEAD(hybridmount_sb_list);
static DECLARE_RWSEM(hybridmount_rwsem);
DEFINE_STATIC_SRCU(hybridmount_srcu);

/* * Helpers to dynamically calculate the memory address of the strings / structs */
#define hm_get_vpath(rule) ((rule)->paths)
#define hm_get_rpath(rule) ((rule)->paths + (rule)->v_len + 1)
#define hm_get_child_name(rule) (hm_get_vpath(rule) + (rule)->v_len - (rule)->child_len)
#define hm_get_child_rules(array) ((struct hybridmount_rule **)((array)->hashes + (array)->capacity))
#define hm_children_is_single(children) ((unsigned long)(children) & 1UL)
#define hm_children_single_rule(children) ((struct hybridmount_rule *)((unsigned long)(children) & ~1UL))
#define hm_children_from_single(rule) ((void *)((unsigned long)(rule) | 1UL))
#define hm_dir_tag(dir_node) READ_ONCE((dir_node)->_tag_ptr)
#define hm_dir_is_virtual(dir_node) (hm_dir_tag((dir_node)) & 1UL)
#define hm_dir_set_owner(dir_node, owner) WRITE_ONCE((dir_node)->_tag_ptr, (unsigned long)(owner) | 1UL)
#define hm_dir_owner(dir_node) ({ \
    unsigned long _tag = hm_dir_tag(dir_node); \
    (_tag & 1UL) ? (struct hybridmount_rule *)(_tag & ~1UL) : NULL; \
})

struct hm_iop {
    struct inode_operations fake_iop; /* MUST be exactly at offset 0 */
    const struct inode_operations *orig_iop;
    struct hybridmount_dir_node *dir_node;
    struct rcu_head rcu;

    /* Dentry Operations Hijacking */
    struct dentry_operations fake_dops;
    const struct dentry_operations *orig_dops;
};

struct hm_fop {
    struct file_operations fake_fop;  /* MUST be exactly at offset 0 */
    const struct file_operations *orig_fop;
    struct hybridmount_dir_node *dir_node;
    struct rcu_head rcu;
};

struct hm_sop {
    struct super_operations fake_sop; /* MUST be exactly at offset 0 */
    const struct super_operations *orig_sop;
    const struct xattr_handler **orig_xattr;
    const struct xattr_handler **fake_xattr;
    struct super_block *sb;
    struct rcu_head rcu;
    struct list_head list;
};

struct hm_inode_info {
    struct path r_path;
    struct hybridmount_dir_node *dir_node;
    u8 flags;
};

struct hybridmount_child_array {
    struct rcu_head rcu;
    int count;
    int capacity;
    u32 hashes[];
};

struct hybridmount_dir_node {
    struct rcu_head rcu;
    void __rcu *children;
    u64 bloom_mask;
    struct inode *v_inode;
    union {
        unsigned long _tag_ptr;
        struct {
            struct hm_iop __rcu *iop;
            struct hm_fop __rcu *fop;
        };
    };
    seqcount_t seq;
};

struct hybridmount_rule {
    struct path r_path;
    struct hybridmount_dir_node *this_dir;
    unsigned long v_ino;
    u32 v_hash;
    unsigned int target_uid;
    u16 v_len;
    u16 r_len;
    u16 child_len;
    u16 flags;

    struct hybridmount_dir_node *parent_dir;
    struct list_head list_node;
    struct hybridmount_rule *next_uid;
    char paths[];
};

struct hm_rule_info {
    u16 flags;
    unsigned long v_ino;
    struct path r_path;
    struct hybridmount_dir_node *this_dir;
};

struct hm_uid_array {
    struct rcu_head rcu;
    int count;
    uid_t uids[];
};

/*** Operation Vectors ***/
#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 16, 0)
static const struct file_operations hm_file_fops_mmap_prepare;
#endif
static const struct file_operations hm_file_fops;
static const struct inode_operations hm_file_iops;
static const struct file_operations hm_dir_fops;
static const struct inode_operations hm_dir_iops;

/*** forward declarations ***/
static struct dentry *hybridmount_hijacked_lookup(struct inode *dir, struct dentry *dentry, unsigned int flags);
static int hybridmount_hijacked_iterate_dir(struct file *file, struct dir_context *ctx);
static void hybridmount_hijacked_destroy_inode(struct inode *inode);
static void hybridmount_hijack_dentry_ops(struct inode *dir, struct dentry *dentry, bool injected);
static void hm_free_rule(struct hybridmount_rule *rule);

/* =====================================================================
 * Hybrid Mount VFS Offset Protocol
 * =====================================================================
 * 64-bit layout: [ 16-bit 'nm' ][ 16-bit 0 ][ 32-bit ID ] 
 * 32-bit layout: [ 16-bit 'nm' ][ 16-bit ID ]
 */
#define HM_SIG_16 0x686DULL /* "hm" in hex */
static inline bool hm_is_virtual_pos(loff_t pos) {
#ifdef CONFIG_COMPAT
    if (in_compat_syscall()) return (pos & 0xFFFF0000ULL) == (HM_SIG_16 << 16);
#endif
    return (pos & 0xFFFFFFFF00000000ULL) == (HM_SIG_16 << 48);
}

static inline loff_t hm_pack_pos(int id) {
#ifdef CONFIG_COMPAT
    if (in_compat_syscall()) return (HM_SIG_16 << 16) | (id & 0xFFFF);
#endif
    return (HM_SIG_16 << 48) | (id & 0xFFFFFFFF);
}

static inline int hm_unpack_pos(loff_t pos) {
#ifdef CONFIG_COMPAT
    if (in_compat_syscall()) return (int)(pos & 0xFFFF);
#endif
    return (int)(pos & 0xFFFFFFFF);
}

/**** Adaptive Radix Tree Protocol ****/
// ref: https://db.in.tum.de/~leis/papers/ART.pdf

#define ART_NODE4   1
#define ART_NODE16  2
#define ART_NODE48  3
#define ART_NODE256 4

struct art_node {
    u8 type;
    u8 num_children;
    u16 prefix_len;
};

struct art_node4 {
    struct art_node n;
    u8 keys[4];
    void *children[4];
};

struct art_node16 {
    struct art_node n;
    u8 keys[16];
    void *children[16];
};

struct art_node48 {
    struct art_node n;
    u8 child_index[256];
    void *children[48];
};

struct art_node256 {
    struct art_node n;
    void *children[256];
};

#define ART_IS_LEAF(x)   ((unsigned long)(x) & 1UL)
#define ART_GET_LEAF(x)  ((struct hybridmount_rule *)((unsigned long)(x) & ~1UL))
#define ART_MAKE_LEAF(x) ((void *)((unsigned long)(x) | 1UL))

static void **hm_art_find_child(struct art_node *n, u8 c)
{
    switch (n->type) {
        case ART_NODE4: {
            struct art_node4 *n4 = (struct art_node4 *)n;
            for (int i = 0; i < n->num_children; i++)
                if (n4->keys[i] == c) return &n4->children[i];
            break;
        }
        case ART_NODE16: {
            struct art_node16 *n16 = (struct art_node16 *)n;
            void *match = memchr(n16->keys, c, n->num_children);
            if (match) return &n16->children[(u8 *)match - n16->keys];
            break;
        }
        case ART_NODE48: {
            struct art_node48 *n48 = (struct art_node48 *)n;
            u8 i = n48->child_index[c];
            if (i) return &n48->children[i - 1];
            break;
        }
        case ART_NODE256: {
            struct art_node256 *n256 = (struct art_node256 *)n;
            if (n256->children[c]) return &n256->children[c];
            break;
        }
    }
    return NULL;
}

static struct hybridmount_rule *hm_tree_search_path(u16 len, const char *path)
{
    void *node = hybridmount_art_root;
    void **child;
    int depth = 0;

    while (node) {
        if (ART_IS_LEAF(node)) {
            struct hybridmount_rule *r = ART_GET_LEAF(node);
            if (likely(r->v_len == len && !memcmp(hm_get_vpath(r), path, len))) return r;
            return NULL;
        }

        struct art_node *n = node;
        depth += n->prefix_len;
        if (unlikely(depth > len)) return NULL;
        child = hm_art_find_child(n, path[depth]);
        node = child ? *child : NULL;
        if (likely(node)) depth++;
    }
    return NULL;
}

static struct hybridmount_rule *hm_tree_search_exact(u16 len, const char *path, unsigned int uid)
{
    struct hybridmount_rule *r = hm_tree_search_path(len, path);
    while (r) {
        if (r->target_uid == uid) return r;
        r = r->next_uid;
    }
    return NULL;
}

static void hm_art_add_child(void **ref, u8 c, void *child)
{
    void *node = *ref;
    struct art_node *n = node;

    if (n->type == ART_NODE4) {
        struct art_node4 *n4 = (struct art_node4 *)n;
        if (n->num_children < 4) {
            n4->keys[n->num_children] = c;
            n4->children[n->num_children] = child;
            n->num_children++;
            return;
        }

        struct art_node16 *n16 = kzalloc(sizeof(*n16), GFP_KERNEL);
        n16->n = n4->n;
        n16->n.type = ART_NODE16;
        memcpy(n16->keys, n4->keys, 4);
        for (int i = 0; i < 4; i++)
            n16->children[i] = n4->children[i];
        n16->keys[4] = c;
        n16->children[4] = child;
        n16->n.num_children++;
        *ref = n16;
        kfree(n4);
    } else if (n->type == ART_NODE16) {
        struct art_node16 *n16 = (struct art_node16 *)n;
        if (n->num_children < 16) {
            n16->keys[n->num_children] = c;
            n16->children[n->num_children] = child;
            n->num_children++;
            return;
        }

        struct art_node48 *n48 = kzalloc(sizeof(*n48), GFP_KERNEL);
        n48->n = n16->n;
        n48->n.type = ART_NODE48;
        for (int i = 0; i < 16; i++) {
            n48->child_index[n16->keys[i]] = i + 1;
            n48->children[i] = n16->children[i];
        }
        n48->child_index[c] = 17;
        n48->children[16] = child;
        n48->n.num_children++;
        *ref = n48;
        kfree(n16);
    } else if (n->type == ART_NODE48) {
        struct art_node48 *n48 = (struct art_node48 *)n;
        if (n->num_children < 48) {
            int pos = 0;
            while (n48->children[pos]) pos++;
            n48->child_index[c] = pos + 1;
            n48->children[pos] = child;
            n->num_children++;
            return;
        }

        struct art_node256 *n256 = kzalloc(sizeof(*n256), GFP_KERNEL);
        n256->n = n48->n;
        n256->n.type = ART_NODE256;
        for (int i = 0; i < 256; i++) {
            if (n48->child_index[i])
                n256->children[i] = n48->children[n48->child_index[i] - 1];
        }
        n256->children[c] = child;
        n256->n.num_children++;
        *ref = n256;
        kfree(n48);
    } else if (n->type == ART_NODE256) {
        struct art_node256 *n256 = (struct art_node256 *)n;
        n256->children[c] = child;
        n256->n.num_children++;
    }
}

static void *hm_art_first_child(struct art_node *n)
{
    if (n->type == ART_NODE4) return ((struct art_node4 *)n)->children[0];
    if (n->type == ART_NODE16) return ((struct art_node16 *)n)->children[0];
    void **children = n->type == ART_NODE48 ? ((struct art_node48 *)n)->children : ((struct art_node256 *)n)->children;
    int capacity = n->type == ART_NODE48 ? 48 : 256;
    for (int i = 0; i < capacity; i++) if (children[i]) return children[i];
    return NULL;
}

static struct hybridmount_rule *hm_art_minimum(void *node)
{
    while (!ART_IS_LEAF(node)) node = hm_art_first_child(node);
    return ART_GET_LEAF(node);
}

static void hm_tree_insert(struct hybridmount_rule *new_rule)
{
    const char *key = hm_get_vpath(new_rule);
    void **child, **node_ref = &hybridmount_art_root;
    void *node;
    int depth = 0;

    list_add_tail(&new_rule->list_node, &hybridmount_rules_list);
    while ((node = *node_ref)) {
        if (ART_IS_LEAF(node)) {
            struct hybridmount_rule *existing = ART_GET_LEAF(node);
            if (existing->v_len == new_rule->v_len && !memcmp(hm_get_vpath(existing), key, new_rule->v_len)) {
                new_rule->next_uid = existing->next_uid;
                existing->next_uid = new_rule;
                return;
            }

            struct art_node4 *n4 = kzalloc(sizeof(*n4), GFP_KERNEL);
            n4->n.type = ART_NODE4;
            n4->n.num_children = 2;

            const char *existing_key = hm_get_vpath(existing);
            int p = 0, max_cmp = min_t(int, existing->v_len, new_rule->v_len) - depth;
            while (p < max_cmp && existing_key[depth + p] == key[depth + p]) p++;

            n4->n.prefix_len = p;
            depth += p;

            n4->keys[0] = existing_key[depth];
            n4->children[0] = node;
            n4->keys[1] = key[depth];
            n4->children[1] = ART_MAKE_LEAF(new_rule);
            *node_ref = n4;
            return;
        }

        struct art_node *n = node;
        if (n->prefix_len > 0) {
            struct hybridmount_rule *borrowed = hm_art_minimum(node);
            const char *borrowed_key = hm_get_vpath(borrowed);
            int p = 0, max_cmp = min_t(int, n->prefix_len, new_rule->v_len - depth);

            while (p < max_cmp && borrowed_key[depth + p] == key[depth + p]) p++;
            if (p < n->prefix_len) {
                struct art_node4 *n4 = kzalloc(sizeof(*n4), GFP_KERNEL);
                n4->n.type = ART_NODE4;
                n4->n.num_children = 2;
                n4->n.prefix_len = p;
                n->prefix_len -= (p + 1);
                n4->keys[0] = borrowed_key[depth + p];
                n4->children[0] = node;
                n4->keys[1] = key[depth + p];
                n4->children[1] = ART_MAKE_LEAF(new_rule);
                *node_ref = n4;
                return;
            }
        }

        depth += n->prefix_len;
        if ((child = hm_art_find_child(n, key[depth]))) {
            node_ref = child;
            depth++;
        } else {
            hm_art_add_child(node_ref, key[depth], ART_MAKE_LEAF(new_rule));
            return;
        }
    }
    *node_ref = ART_MAKE_LEAF(new_rule);
}

static void hm_art_free_tree(void *node)
{
    if (!node || ART_IS_LEAF(node)) return;

    struct art_node *n = node;
    if (n->type == ART_NODE4) {
        struct art_node4 *n4 = (struct art_node4 *)n;
        for (int i = 0; i < n->num_children; i++) hm_art_free_tree(n4->children[i]);
    } else if (n->type == ART_NODE16) {
        struct art_node16 *n16 = (struct art_node16 *)n;
        for (int i = 0; i < n->num_children; i++) hm_art_free_tree(n16->children[i]);
    } else if (n->type == ART_NODE48) {
        struct art_node48 *n48 = (struct art_node48 *)n;
        for (int i = 0; i < 256; i++) {
            if (n48->child_index[i]) hm_art_free_tree(n48->children[n48->child_index[i] - 1]);
        }
    } else if (n->type == ART_NODE256) {
        struct art_node256 *n256 = (struct art_node256 *)n;
        for (int i = 0; i < 256; i++) {
            if (n256->children[i]) hm_art_free_tree(n256->children[i]);
        }
    }
    kfree(n);
}

static void hm_art_remove_child(void **parent_ref, void **child_ref, u8 key)
{
    struct art_node *n = *parent_ref;
    int last = --n->num_children;

    if (n->type == ART_NODE4) {
        struct art_node4 *n4 = (struct art_node4 *)n;
        int pos = child_ref - n4->children;
        n4->keys[pos] = n4->keys[last];
        n4->children[pos] = n4->children[last];
    } else if (n->type == ART_NODE16) {
        struct art_node16 *n16 = (struct art_node16 *)n;
        int pos = child_ref - n16->children;
        n16->keys[pos] = n16->keys[last];
        n16->children[pos] = n16->children[last];
    } else if (n->type == ART_NODE48) {
        ((struct art_node48 *)n)->child_index[key] = 0;
    }

    if (n->num_children == 1) {
        void *child = hm_art_first_child(n);
        if (!ART_IS_LEAF(child))
            ((struct art_node *)child)->prefix_len += n->prefix_len + 1;
        *parent_ref = child;
        kfree(n);
    }
}

static void hm_art_remove_leaf(void **ref, struct hybridmount_rule *target)
{
    void *node;
    void **parent_ref = NULL;
    const char *key = hm_get_vpath(target);
    int depth = 0;

    while ((node = *ref)) {
        if (ART_IS_LEAF(node)) {
            struct hybridmount_rule *r = ART_GET_LEAF(node);
            if (r->v_len == target->v_len && !memcmp(hm_get_vpath(r), key, target->v_len)) {
                if (r == target) {
                    if (target->next_uid) *ref = ART_MAKE_LEAF(target->next_uid);
                    else {
                        *ref = NULL;
                        if (parent_ref) hm_art_remove_child(parent_ref, ref, key[depth - 1]);
                    }
                } else {
                    struct hybridmount_rule *prev = r;
                    while (prev->next_uid && prev->next_uid != target) prev = prev->next_uid;
                    if (prev->next_uid == target) prev->next_uid = target->next_uid;
                }
            }
            return;
        }

        struct art_node *n = node;
        depth += n->prefix_len;
        if (depth > target->v_len) return;
        parent_ref = ref;
        ref = hm_art_find_child(n, key[depth]);
        if (!ref) return;
        depth++;
    }
}

/* --- UIDs Array RCU Management --- */
static inline int hm_uid_add(uid_t target)
{
    struct hm_uid_array *old, *new_arr;
    int count = 0;
    if ((old = rcu_dereference_protected(hybridmount_uids, lockdep_is_held(&hybridmount_rwsem)))) {
        for (int i = 0; i < (count = old->count); i++) if (old->uids[i] == target) return -EEXIST;
    }

    if (!(new_arr = kmalloc(sizeof(*new_arr) + (count + 1) * sizeof(uid_t), GFP_KERNEL))) return -ENOMEM;
    new_arr->count = count + 1;
    if (old) memcpy(new_arr->uids, old->uids, count * sizeof(uid_t));
    new_arr->uids[count] = target;
    rcu_assign_pointer(hybridmount_uids, new_arr);
    if (old) kfree_rcu(old, rcu);
    return 0;
}

static inline int hm_uid_del(uid_t target)
{
    struct hm_uid_array *old, *new_arr = NULL;
    int count, target_idx = -1;

    if (!(old = rcu_dereference_protected(hybridmount_uids, lockdep_is_held(&hybridmount_rwsem)))) return -ENOENT;
    for (int i = 0; i < (count = old->count); i++) if (old->uids[i] == target) { target_idx = i; break; }
    if (target_idx < 0) return -ENOENT;

    if (count > 1) {
        if (!(new_arr = kmalloc(sizeof(*new_arr) + (count - 1) * sizeof(uid_t), GFP_KERNEL))) return -ENOMEM;
        new_arr->count = count - 1;
        if (target_idx > 0) memcpy(new_arr->uids, old->uids, target_idx * sizeof(uid_t));
        if (target_idx < count - 1) memcpy(new_arr->uids + target_idx, old->uids + target_idx + 1, (count - target_idx - 1) * sizeof(uid_t));
    }
    rcu_assign_pointer(hybridmount_uids, new_arr);
    kfree_rcu(old, rcu);
    return 0;
}

/* ============================ */
/* HYBRIDMOUNT PAYLOAD PROTOCOL     */
/* ============================ */

enum {
    HM_CMD_UNSPEC = 0,
    HM_CMD_GET_VERSION,
    HM_CMD_ADD_RULE,
    HM_CMD_DEL_RULE,
    HM_CMD_ADD_UID,
    HM_CMD_DEL_UID,
    HM_CMD_CLEAR_ALL,
    HM_CMD_CLEAR_RULES,
    HM_CMD_CLEAR_UIDS,
    HM_CMD_GET_LIST,
    HM_CMD_GET_UIDS,
};

struct hm_payload {
    u64 magic;
    u32 cmd;
    u32 target_uid;
    int status;
    u32 arg1;
    u32 data_size;
    char buffer[4068];
} __attribute__((packed));

struct hm_rule_hdr {
	u32 flags;
	u32 uid;
	u16 v_len;
	u16 r_len;
} __attribute__((packed));

struct hm_del_hdr {
	u32 uid;
	u16 v_len;
} __attribute__((packed));

/* * Compat macros * */
#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 3, 0)
    #define IDMAP_PATH(path) mnt_idmap((path).mnt),
    #define IDMAP_ARG struct mnt_idmap *idmap,
    #define IDMAP_CALL idmap,
#elif LINUX_VERSION_CODE >= KERNEL_VERSION(5, 12, 0)
    #define IDMAP_PATH(path) mnt_user_ns((path).mnt),
    #define IDMAP_ARG struct user_namespace *mnt_userns,
    #define IDMAP_CALL mnt_userns,
#else
    #define IDMAP_PATH(path)/* Nothing */
    #define IDMAP_ARG /* Nothing */
    #define IDMAP_CALL /* Nothing */
#endif

#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 1, 0)
    #define HM_ACTOR_RET bool
    #define HM_ACTOR_CONTINUE true
#else
    #define HM_ACTOR_RET int
    #define HM_ACTOR_CONTINUE 0
#endif

#if LINUX_VERSION_CODE < KERNEL_VERSION(5, 12, 0) && LINUX_VERSION_CODE >= KERNEL_VERSION(5, 2, 0)
    #define FLAGS_ARG , int flags
    #define FLAGS_VAL , flags
#else
    #define FLAGS_ARG /* Nothing */
    #define FLAGS_VAL /* Nothing */
#endif

#ifndef DCACHE_DONTCACHE
# define DCACHE_DONTCACHE 0
#endif

static inline void hm_sync_inode_times(struct inode *v_inode, struct inode *r_inode)
{
#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 12, 0)
    v_inode->i_atime_sec = r_inode->i_atime_sec;
    v_inode->i_atime_nsec = r_inode->i_atime_nsec;
    v_inode->i_mtime_sec = r_inode->i_mtime_sec;
    v_inode->i_mtime_nsec = r_inode->i_mtime_nsec;
    v_inode->i_ctime_sec = r_inode->i_ctime_sec;
    v_inode->i_ctime_nsec = r_inode->i_ctime_nsec;
#elif LINUX_VERSION_CODE >= KERNEL_VERSION(6, 6, 0)
    v_inode->i_atime = r_inode->i_atime;
    v_inode->i_mtime = r_inode->i_mtime;
    inode_set_ctime_to_ts(v_inode, inode_get_ctime(r_inode));
#else
    v_inode->i_atime = r_inode->i_atime;
    v_inode->i_mtime = r_inode->i_mtime;
    v_inode->i_ctime = r_inode->i_ctime;
#endif
}

static inline int hm_call_iterate(struct file *file, struct dir_context *ctx, const struct file_operations *fop)
{
    if (fop->iterate_shared)
        return fop->iterate_shared(file, ctx);
#if LINUX_VERSION_CODE < KERNEL_VERSION(6, 6, 0)
    else if (fop->iterate)
        return fop->iterate(file, ctx);
#endif
    return -ENOTDIR;
}

static inline struct dentry *hm_hash_and_lookup(struct dentry *dir, struct qstr *n) {
    n->hash = full_name_hash(dir, n->name, n->len);
    return (unlikely(dir->d_flags & DCACHE_OP_HASH) && dir->d_op->d_hash(dir, n) < 0) ? NULL : d_lookup(dir, n);
}

static inline struct hm_iop *hm_get_hm_iop(const struct inode_operations *iop) {
    if (likely(iop) && iop->lookup == hybridmount_hijacked_lookup)
        return container_of(iop, struct hm_iop, fake_iop);
    return NULL;
}

static inline struct hm_fop *hm_get_hm_fop(const struct file_operations *fop) {
    if (unlikely(!fop)) return NULL;
    if (fop->iterate_shared == hybridmount_hijacked_iterate_dir)
        return container_of(fop, struct hm_fop, fake_fop);
#if LINUX_VERSION_CODE < KERNEL_VERSION(6, 6, 0)
    if (fop->iterate == hybridmount_hijacked_iterate_dir)
        return container_of(fop, struct hm_fop, fake_fop);
#endif
    return NULL;
}

static inline struct hm_sop *hm_get_hm_sop(const struct super_operations *sop) {
    if (likely(sop) && sop->destroy_inode == hybridmount_hijacked_destroy_inode)
        return container_of(sop, struct hm_sop, fake_sop);
    return NULL;
}

#define HM_DOP_INITIALIZING ((const struct dentry_operations *)1L)
static inline const struct dentry_operations *hm_get_orig_dops(struct hm_iop *iop)
{
    const struct dentry_operations *dops;
    if (!iop) return NULL;
    dops = smp_load_acquire(&iop->orig_dops);
    return (dops == HM_DOP_INITIALIZING) ? NULL : dops;
}

#endif /* _LINUX_HYBRIDMOUNT_H */
