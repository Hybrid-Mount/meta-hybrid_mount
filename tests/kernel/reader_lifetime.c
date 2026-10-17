// SPDX-License-Identifier: GPL-2.0-only
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define assert(x) do { if (!(x)) { fprintf(stderr,"FAIL %d: %s\n",__LINE__,#x); exit(1); } } while (0)
typedef uint32_t u32;
typedef uint64_t u64;
typedef long long loff_t;
typedef unsigned int uid_t;
#define HM_ACTOR_RET int
#define HM_ACTOR_CONTINUE 0
#define container_of(p,t,m) ((t *)((char *)(p)-offsetof(t,m)))
#define likely(x) (x)
#define unlikely(x) (x)
#define READ_ONCE(x) (x)
#define smp_load_acquire(p) (*(p))
#define HYBRIDMOUNT_MAGIC_SIG 0
#define ENODATA 61
#define ENOTDIR 20
#define EOPNOTSUPP 95
#define ECHILD 10
#define LOOKUP_RCU 1
#define HM_FLAG_WHITEOUT 4
#define LINUX_VERSION_CODE 0
#define KERNEL_VERSION(a,b,c) 1
#define ERR_PTR(n) ((void *)(intptr_t)(n))
#define IS_ERR(p) ((uintptr_t)(p)>=(uintptr_t)-4095)
struct inode; struct dentry; struct file; struct dir_context;
struct qstr { const char *name; int len; };
struct inode_operations { struct dentry *(*lookup)(struct inode *,struct dentry *,unsigned int); };
struct file_operations { int (*iterate_shared)(struct file *,struct dir_context *); };
struct dentry_operations { int (*d_revalidate)(struct dentry *, unsigned int); };
struct inode { const struct inode_operations *i_op; void *i_private; };
struct dentry { struct qstr d_name; struct inode *d_inode; const struct dentry_operations *d_op; };
struct file { const struct file_operations *f_op; };
struct dir_context { int (*actor)(struct dir_context *,const char *,int,loff_t,u64,unsigned int); unsigned long pos; };
struct hybridmount_dir_node { unsigned long long bloom_mask; int refs; int seq; int rcu; void *children; };
struct path { struct dentry *dentry; void *mnt; };
struct hm_inode_info { struct hybridmount_dir_node *dir_node; int flags; struct path r_path; };
struct hm_iop { struct hybridmount_dir_node *dir_node; const struct inode_operations *orig_iop; };
struct hm_fop { struct hybridmount_dir_node *dir_node; const struct file_operations *orig_fop; };
struct hm_rule_info { struct path backing_path; int flags; unsigned long v_ino; struct hybridmount_dir_node *this_dir; struct path r_path; };
struct hybridmount_rule { uint16_t child_len; char *name; uid_t target_uid; int flags; unsigned long v_ino; struct hybridmount_dir_node *this_dir; struct path r_path; };
struct hybridmount_child_array { int count; };
#define HM_FLAG_VIRTUAL_DIR 2
#define rcu_dereference(p) (p)
#define hm_children_is_single(p) ((uintptr_t)(p)&1)
#define hm_children_single_rule(p) ((struct hybridmount_rule *)((uintptr_t)(p)&~(uintptr_t)1))
#define hm_get_child_name(p) ((p)->name)
static unsigned int read_seqcount_begin(int *p) { return 0; }
static bool read_seqcount_retry(int *p,unsigned int seq) { return false; }
static void path_get(struct path *p) {}
static struct hybridmount_rule *hybridmount_bsearch_child(struct hybridmount_child_array *a,const char *n,size_t l,u32 h,int *i) { return NULL; }
static struct hybridmount_rule *hybridmount_child_for_uid(struct hybridmount_child_array *a,const char *n,size_t l,u32 h,uid_t uid,bool exact,int *i) { return NULL; }

struct hybridmount_proxy_ctx { struct dir_context ctx; struct dir_context *orig_ctx; struct hybridmount_dir_node *dir_node; bool emitted,uid_blocked; uid_t fsuid; };
static const struct inode_operations hm_dir_iops={0}, hm_file_iops={0};
static const struct dentry_operations hm_owned_dops={0};
static int rcu_depth,fsuid_reads;
static struct hybridmount_dir_node node;
static struct hm_iop proxy_iop;
static struct hm_fop proxy_fop;
static void rcu_read_lock(void) { rcu_depth++; }
static void rcu_read_unlock(void) { assert(rcu_depth); rcu_depth--; }
static struct hm_iop *hm_get_hm_iop(const struct inode_operations *ops) { assert(rcu_depth); return &proxy_iop; }
static struct hm_fop *hm_get_hm_fop(const struct file_operations *ops) { assert(rcu_depth); return &proxy_fop; }
static int frees;
static bool refcount_inc_not_zero(int *p) { assert(rcu_depth); if(!*p) return false; ++*p; return true; }
static bool refcount_dec_and_test(int *p) { assert(*p); return !--*p; }
static void hm_dir_rcu_free(int *p) { frees++; }
static void call_rcu(int *p,void (*callback)(int *)) { callback(p); }
struct test_uid { unsigned int val; };
static struct test_uid current_fsuid(void) { fsuid_reads++; return (struct test_uid){42}; }
static bool hybridmount_is_uid_blocked(unsigned int uid) { return false; }
static u32 full_name_hash(const void *p,const char *name,int len) { return 1; }

static struct dentry *hybridmount_resolve_rule_dentry(struct inode *i,struct dentry *d,struct hybridmount_dir_node *p,u32 hash) { assert(p->refs>1 && !rcu_depth); return ERR_PTR(-ENODATA); }
static void hybridmount_hijack_dentry_ops(struct inode *i,struct dentry *d,bool owned) {}
static void d_drop(struct dentry *d) {}
static bool hm_is_virtual_pos(unsigned long pos) { return false; }
static unsigned long hm_pack_pos(int n) { return 100; }

static void hybridmount_emit_virtual_children(struct dir_context *ctx,struct hybridmount_dir_node *p,uid_t fsuid) { assert(p->refs>1); }
static int hm_call_iterate(struct file *f,struct dir_context *ctx,const struct file_operations *ops) { return ops->iterate_shared(f,ctx); }
static const struct dentry_operations *hm_get_orig_dops(struct hm_iop *iop);
#include "production.h"
static struct dentry *native_lookup(struct inode *i,struct dentry *d,unsigned int flags) { assert(node.refs>1 && !rcu_depth); return NULL; }
static int native_iterate(struct file *f,struct dir_context *ctx) {
    assert(node.refs>1 && !rcu_depth);
    for(int i=0;i<8;i++) ctx->actor(ctx,"child",5,1,7,0);
    return 0;
}
static int native_revalidate(struct dentry *d,unsigned int flags) { assert(!rcu_depth); return 1; }
static int emit(struct dir_context *ctx,const char *name,int len,loff_t offset,u64 ino,unsigned int type) { return 0; }
static const struct dentry_operations native_dops={native_revalidate};
static const struct dentry_operations *hm_get_orig_dops(struct hm_iop *iop) { assert(rcu_depth); return &native_dops; }
int main(int argc,char **argv) {
    struct inode_operations orig_iop={native_lookup};
    struct file_operations orig_fop={native_iterate};
    struct inode inode={&orig_iop,NULL};
    struct dentry dentry={.d_name={"child",5}};
    struct file file={&orig_fop}; struct dir_context ctx={.actor=emit};
    node.refs=1; node.bloom_mask=2; proxy_iop=(struct hm_iop){&node,&orig_iop}; proxy_fop=(struct hm_fop){&node,&orig_fop};
    if (argv[1][0]=='l') assert(hybridmount_hijacked_lookup(&inode,&dentry,0)==NULL);
    if (argv[1][0]=='i') assert(hybridmount_hijacked_iterate_dir(&file,&ctx)==0);
    if (argv[1][0]=='r') assert(hm_d_revalidate_common(&inode,&dentry.d_name,&dentry,0,false)==1);
    assert(node.refs==1 && !rcu_depth && !frees);
    rcu_read_lock(); assert(hm_dir_get(&node)==&node); rcu_read_unlock();
    hm_dir_put(&node); assert(node.refs==1 && !frees);
    hm_dir_put(&node); assert(!node.refs && frees==1);
    rcu_read_lock(); assert(!hm_dir_get(&node)); rcu_read_unlock();
    if(argv[1][0]=='i') assert(fsuid_reads==1);
    printf("PASS: %s entry snapshot and sleeping callback lifetime\n",argv[1]);
}
