// SPDX-License-Identifier: GPL-3.0-only
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#undef assert
#define assert(x) do { if (!(x)) { fprintf(stderr, "FAIL %d: %s\n", __LINE__, #x); exit(1); } } while (0)
typedef uint16_t u16;
typedef uint32_t u32;
typedef unsigned int uid_t;
#define loff_t int64_t
static bool compat_mode;
static bool in_compat_syscall(void) { return compat_mode; }
#define KERNEL_VERSION(a,b,c) (((a)<<16)|((b)<<8)|(c))
#define HM_FLAG_IS_DIR 1
#define HM_FLAG_VIRTUAL_DIR 2
#define HM_FLAG_WHITEOUT 4
#define HM_FLAG_OPAQUE 8
#define S_PRIVATE 512
#define S_ISDIR(mode) ((mode) == 1)
#define GFP_KERNEL 0
#define LOOKUP_FOLLOW 0
#define ENOMEM 12
#define ENOENT 2
#ifndef ENODATA
#define ENODATA 61
#endif
#define EINVAL 22
#define EOPNOTSUPP 95
#define HYBRIDMOUNT_MAGIC_SIG 0
#define likely(x) (x)
#define unlikely(x) (x)
#define ERR_PTR(n) ((void *)(intptr_t)(n))
#define IS_ERR(p) ((uintptr_t)(p) >= (uintptr_t)-4095)
#define PTR_ERR(p) ((int)(intptr_t)(p))
#define READ_ONCE(x) (x)
#define WRITE_ONCE(x,v) ((x)=(v))
#define RCU_INIT_POINTER(x,v) ((x)=(v))
#define rcu_access_pointer(p) (p)
#define smp_load_acquire(p) (*(p))
#define smp_store_release(p,v) (*(p)=(v))
#define smp_mb() ((void)0)
#define hm_debug(...) ((void)0)
#define container_of(p,t,m) ((t *)((char *)(p)-offsetof(t,m)))
#define FLAGS_ARG
#define FLAGS_VAL
#define DT_DIR 4
#define DT_REG 8
#define hm_get_vpath(r) ((r)->paths)
#define hm_get_rpath(r) ((r)->paths+(r)->v_len+1)
#define hm_get_child_name(r) (hm_get_vpath(r)+(r)->v_len-(r)->child_len)
#define hm_children_is_single(p) ((uintptr_t)(p)&1)
#define hm_children_single_rule(p) ((struct hybridmount_rule *)((uintptr_t)(p)&~(uintptr_t)1))
#define hm_get_child_rules(a) ((a)->rules)
#define hm_dir_set_owner(d,r) ((d)->owner=(r))
#define hm_dir_is_virtual(d) ((d)->owner != NULL)
#define INIT_LIST_HEAD(p) memset(p,0,sizeof(*(p)))
#define list_add_tail_rcu(p,h) ((void)(p),(void)(h))
#define list_del_init(p) ((void)(p))
#define list_move(p,h) ((void)(p),(void)(h))

struct inode_operations { int lookup; };
struct file_operations { int iterate; };
struct inode;
struct super_operations { int (*drop_inode)(struct inode *); void (*evict_inode)(struct inode *); };
struct dentry;
struct xattr_handler {
    int (*get)(const struct xattr_handler *, struct dentry *, struct inode *, const char *, void *, size_t);
    void (*set)(void);
#if LINUX_VERSION_CODE < KERNEL_VERSION(5,4,0)
    int (*__get)(const struct xattr_handler *, struct dentry *, struct inode *, const char *, void *, size_t);
#endif
};
struct super_block { const struct super_operations *s_op; const struct xattr_handler **s_xattr; int s_active; };
static void atomic_inc(int *value) { ++*value; }
struct inode { const struct inode_operations *i_op; const struct file_operations *i_fop; unsigned long i_flags,i_ino; int i_mode; void *i_private; };
struct dentry { struct inode *inode; };
struct path { struct dentry *dentry; void *mnt; };
struct list_head { int unused; };
struct hybridmount_rule;
struct hm_iop;
struct hm_fop;
struct hybridmount_dir_node {
    void *children; struct inode *v_inode;
    struct hm_iop *iop; struct hm_fop *fop;
    struct hybridmount_rule *owner; int rcu;
};
struct hybridmount_rule {
    struct path r_path,parent_path; struct hybridmount_dir_node *this_dir,*parent_dir;
    unsigned long v_ino; u32 v_hash; unsigned int target_uid;
    u16 v_len,r_len,child_len,flags;
    struct list_head list_node,topology_node;
    struct hybridmount_rule *next_uid;
    char paths[128];
};
struct hybridmount_child_array { int count; u32 hashes[2]; struct hybridmount_rule *rules[2]; };
static struct hybridmount_rule *hybridmount_child_for_uid(struct hybridmount_child_array *arr,
    const char *name,size_t len,u32 hash,uid_t uid,bool exact,int *index) { return NULL; }
struct hm_iop { struct inode_operations fake_iop; const struct inode_operations *orig_iop; struct hybridmount_dir_node *dir_node; int rcu; };
struct hm_fop { struct file_operations fake_fop; const struct file_operations *orig_fop; struct hybridmount_dir_node *dir_node; int rcu; };
struct hm_inode_info { struct path r_path; };
struct hm_sop { struct super_operations fake_sop; const struct super_operations *orig_sop; const struct xattr_handler **orig_xattr,**fake_xattr; struct super_block *sb; struct list_head list; };
struct hm_xattr_proxy { struct xattr_handler fake; const struct xattr_handler *orig; };
struct dir_context { loff_t pos; unsigned long ino; };
static int hybridmount_srcu,hybridmount_sb_list;
static void *hybridmount_art_root __attribute__((unused));
static const struct inode_operations hm_file_iops={1},hm_dir_iops={2},native_iops={3};
static const struct file_operations native_fops={3};
static struct inode source_inode={.i_mode=1,.i_ino=900,.i_op=&native_iops};
static struct dentry source_dentry={&source_inode};
static struct hybridmount_rule *indexed,*published;
static bool allocation_fail,reenter;
static int fail_at, allocation_count, live_allocations;
static int path_refs,detaches,retirements,drops;
static struct hm_iop *observed_iop;
static struct hm_fop *observed_fop;
static struct inode *teardown_inode;
static int iop_frees,fop_frees;
static void hm_destroy_hijacked_inode(struct inode *,bool);
static void *kmalloc(size_t size,int flags) {
    if (allocation_fail || ++allocation_count == fail_at) return NULL;
    void *p=calloc(1,size); if(p) live_allocations++; return p;
}
static void kfree(void *p) { if(p) live_allocations--; free(p); }
static void path_get(struct path *p) { if(p->dentry) path_refs++; }
static void path_put(struct path *p) { if(p->dentry) path_refs--; }
static int kern_path(const char *path,int flags,struct path *out) {
    if(!strcmp(path,"/parent") || !strcmp(path,"/parent/child")) return -ENOENT;
    *out=(struct path){&source_dentry,NULL}; path_get(out); return 0;
}
static struct inode *d_backing_inode(struct dentry *d) { return d->inode; }
static struct inode *d_inode(struct dentry *d) { return d->inode; }
static void d_drop(struct dentry *d) { drops++; }
static u32 full_name_hash(const void *salt,const char *name,size_t len) { return 123; }
static struct hybridmount_dir_node *__hybridmount_alloc_dir_node(void) { return kmalloc(sizeof(struct hybridmount_dir_node),0); }
static struct hybridmount_rule *hm_tree_search_exact(u16 len,const char *name,unsigned int uid) { return indexed && indexed->target_uid==uid ? indexed : NULL; }
static int hm_tree_insert(struct hybridmount_rule *rule) { published=rule; return 0; }
static int __hybridmount_inject_child_locked(struct hybridmount_dir_node *node,struct hybridmount_rule *rule,const char *name,size_t len) { rule->parent_dir=node; return 0; }
static void hm_art_remove_leaf(void **root,struct hybridmount_rule *rule) { if(indexed==rule) indexed=published; }
static void hm_detach_rule_locked(struct hybridmount_rule *rule,struct list_head *victims,bool prune) { detaches++; indexed=NULL; }
static void rcu_read_lock(void) {}
static void rcu_read_unlock(void) {}
static void hm_dir_rcu_free(int *p) { retirements++; }
static void hm_dir_put(struct hybridmount_dir_node *p) { if(p) retirements++; }
static void call_rcu(int *p,void (*f)(int *)) { f(p); }
static void test_retire(void *p) { if(p==observed_iop) iop_frees++; if(p==observed_fop) fop_frees++; }
#define kfree_rcu(p,m) test_retire(p)
#define cmpxchg(p,old,value) ({ __typeof__(*(p)) previous=*(p); if(previous==(old)) *(p)=(value); previous; })
#define xchg(p,value) ({ __typeof__(*(p)) previous=*(p); *(p)=(value); previous; })
static struct hm_iop *hm_get_hm_iop(const struct inode_operations *ops) {
    struct hm_iop *result=observed_iop && ops==&observed_iop->fake_iop ? observed_iop : NULL;
    if(result && reenter) { reenter=false; hm_destroy_hijacked_inode(teardown_inode,true); }
    return result;
}
static struct hm_fop *hm_get_hm_fop(const struct file_operations *ops) { return observed_fop && ops==&observed_fop->fake_fop ? observed_fop : NULL; }
static bool hybridmount_is_uid_blocked(uid_t uid) { return false; }
struct test_uid { unsigned int val; };
static struct test_uid current_fsuid(void) { return (struct test_uid){42}; }
static int srcu_read_lock(int *srcu) { return 0; }
static void srcu_read_unlock(int *srcu,int index) {}
#define srcu_dereference(p,s) (p)
static bool dir_emit(struct dir_context *ctx,const char *name,int len,unsigned long ino,int type) { ctx->ino=ino; return true; }
static int native_get(const struct xattr_handler *h,struct dentry *d,struct inode *i,const char *n,void *b,size_t s) { assert(i && i->i_op==&native_iops); return 71; }
static int __vfs_getxattr(struct dentry *d,struct inode *i,const char *n,void *b,size_t s) { return native_get(NULL,d,i,n,b,s); }
static const char *xattr_full_name(const struct xattr_handler *h,const char *n) { return n; }
static void hm_xattr_set(void) {}
static int hybridmount_hijacked_drop_inode(struct inode *i) { return 0; }
static void hybridmount_hijacked_evict_inode(struct inode *i) {}
static struct hm_sop *hm_get_hm_sop(const struct super_operations *ops) { return NULL; }

#include "production.h"

int main(int argc,char **argv) {
    assert(argc==2); const char *mode=argv[1];
    if(!strncmp(mode,"hook-alloc",10)) {
        struct super_operations native = {0};
        struct xattr_handler handler = {.get=native_get};
        const struct xattr_handler *handlers[] = {&handler, NULL};
        struct super_block sb = {.s_op=&native, .s_xattr=handlers};
        fail_at = !strcmp(mode,"hook-alloc-sop") ? 1 : 2;
        assert(hybridmount_hijack_superblock(&sb) == -ENOMEM);
        assert(sb.s_op == &native && sb.s_xattr == handlers && !live_allocations);
    } else if(!strcmp(mode,"hook-pin")) {
        struct super_operations native = {0};
        struct super_block sb = {.s_op=&native, .s_active=1};
        assert(hybridmount_hijack_superblock(&sb)==0);
        assert(sb.s_active==2);
    } else if(!strcmp(mode,"source-flags")) {
        source_inode.i_flags=32;
        struct hybridmount_rule *rule=hm_alloc_rule("/parent","/source",7,7,0,42);
        assert(!IS_ERR(rule)); assert(source_inode.i_flags==32);
        path_put(&rule->r_path); free(rule->this_dir); free(rule); assert(!path_refs);
    } else if(!strcmp(mode,"dirent") || !strcmp(mode,"dirent-compat")) {
        compat_mode=!strcmp(mode,"dirent-compat");
        struct hybridmount_rule rule={.v_ino=900,.v_hash=123,.v_len=7,.child_len=6,.target_uid=42};
        memcpy(rule.paths,"/parent",8);
        struct hybridmount_dir_node node={.children=(void *)((uintptr_t)&rule|1)};
        struct dir_context ctx={0}; hybridmount_emit_virtual_children(&ctx,&node,42);
        assert(ctx.ino==rule.v_ino && ctx.pos==(compat_mode ? 0x686D0001LL : 0x686D000000000001LL));
    } else if(!strncmp(mode,"cursor-",7)) {
        struct hybridmount_rule rule={.v_ino=900,.target_uid=42};
        struct hybridmount_dir_node node={.children=(void *)((uintptr_t)&rule|1)};
        struct dir_context ctx={.pos=!strcmp(mode,"cursor-negative") ?
            0x686D000080000000LL : 0x686D0000FFFFFFFFLL};
        if(!strcmp(mode,"cursor-compat-max")) { compat_mode=true; ctx.pos=0x686DFFFFLL; }
        hybridmount_emit_virtual_children(&ctx,&node,42);
        assert(ctx.ino==0);
    } else if(!strncmp(mode,"parent",6)) {
        struct hybridmount_rule original={.v_len=7,.v_ino=900,.child_len=6,.target_uid=42,.flags=HM_FLAG_IS_DIR,.r_len=7};
        memcpy(original.paths,"/parent\0/source",16);
        struct hybridmount_dir_node node={.children=&original,.owner=&original},outer={0};
        original.this_dir=&node; original.parent_dir=&outer;
        original.r_path.dentry=&source_dentry; path_refs=1;
        if(!strcmp(mode,"parent-opaque")) original.flags|=HM_FLAG_OPAQUE|HM_FLAG_VIRTUAL_DIR;
        if(!strcmp(mode,"parent-empty")) node.children=NULL;
        allocation_fail=!strcmp(mode,"parent-allocation"); indexed=&original;
        struct list_head victims={0};
        __hybridmount_del_rule("/parent",7,42,&victims);
        if(allocation_fail) { assert(indexed==&original && !detaches && node.owner==&original); }
        else if(!node.children) { assert(!indexed && detaches==1); }
        else {
            assert(indexed && indexed!=&original && indexed->this_dir==&node);
            assert(indexed->flags==(HM_FLAG_IS_DIR|HM_FLAG_VIRTUAL_DIR) && !indexed->r_len && !indexed->r_path.dentry);
            assert(node.owner==indexed && indexed->v_ino==900 && !original.this_dir);
            assert(original.r_path.dentry==&source_dentry && path_refs==1);
            if(!strcmp(mode,"parent-repeat")) { struct hybridmount_rule *saved=indexed; __hybridmount_del_rule("/parent",7,42,&victims); assert(indexed==saved); }
            free(indexed);
        }
    } else if(!strcmp(mode,"teardown")) {
        struct hybridmount_dir_node node={0};
        struct hm_iop iop={.orig_iop=&native_iops,.dir_node=&node};
        struct hm_fop fop={.orig_fop=&native_fops,.dir_node=&node};
        node.iop=&iop; node.fop=&fop;
        struct inode inode={.i_op=&iop.fake_iop,.i_fop=&fop.fake_fop};
        observed_iop=&iop; observed_fop=&fop; teardown_inode=&inode; reenter=true;
        hm_destroy_hijacked_inode(&inode,true);
        assert(iop_frees==1 && fop_frees==0 && retirements==1);
        assert(!fop.dir_node);
        assert(inode.i_op==&native_iops && inode.i_fop==&native_fops);
    } else {
        struct xattr_handler orig={.get=native_get};
#if LINUX_VERSION_CODE >= KERNEL_VERSION(4,14,0) && LINUX_VERSION_CODE < KERNEL_VERSION(5,4,0)
        orig.__get=native_get;
#endif
        const struct xattr_handler *handlers[]={&orig,NULL}; struct super_operations ops={0};
        struct super_block sb={.s_op=&ops,.s_xattr=handlers}; hybridmount_hijack_superblock(&sb);
        struct hm_inode_info info={.r_path={&source_dentry,NULL}};
        struct inode inode={.i_op=&hm_file_iops,.i_private=&info}; struct dentry d={&inode};
        if(!strcmp(mode,"xattr-native")) inode.i_op=&native_iops;
        if(!strcmp(mode,"xattr-virtual")) info.r_path.dentry=NULL;
        int result;
#if LINUX_VERSION_CODE >= KERNEL_VERSION(4,14,0) && LINUX_VERSION_CODE < KERNEL_VERSION(5,4,0)
        result=sb.s_xattr[0]->__get(sb.s_xattr[0],&d,&inode,"security.selinux",NULL,0);
#else
        result=sb.s_xattr[0]->get(sb.s_xattr[0],&d,&inode,"security.selinux",NULL,0);
#endif
        assert(result==(!strcmp(mode,"xattr-virtual") ? -ENODATA : 71));
        free((void *)sb.s_xattr); free(container_of(sb.s_op,struct hm_sop,fake_sop));
    }
    printf("PASS: VFS semantics %s\n",mode); return 0;
}
