// SPDX-License-Identifier: GPL-2.0-only
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#define assert(x) do { if(!(x)) { fprintf(stderr,"FAIL %d: %s\n",__LINE__,#x); exit(1); } } while(0)
#define KERNEL_VERSION(a,b,c) (((a)<<16)|((b)<<8)|(c))
#define LINUX_VERSION_CODE KERNEL_VERSION(5,10,0)
#define GFP_KERNEL 0
#define ENOMEM 12
#define ENODEV 19
#define EOPNOTSUPP 95
#define likely(x) (x)
#define unlikely(x) (x)
#define smp_load_acquire(p) (*(p))
#define smp_store_release(p,v) (*(p)=(v))
#define rcu_assign_pointer(p,v) ((p)=(v))
#define hm_debug(...) ((void)0)
struct module { int refs; };
static struct module ours, backing;
#define THIS_MODULE (&ours)
struct inode; struct dentry; struct file; struct dir_context;
struct inode_operations { struct dentry *(*lookup)(struct inode *,struct dentry *,unsigned int); };
struct file_operations { struct module *owner; int (*iterate_shared)(struct file *,struct dir_context *); int (*iterate)(struct file *,struct dir_context *); };
struct super_operations { int (*drop_inode)(struct inode *); void (*evict_inode)(struct inode *); };
struct xattr_handler { int (*get)(void); int (*set)(void); };
struct super_block { const struct super_operations *s_op; const struct xattr_handler **s_xattr; };
struct inode { const struct inode_operations *i_op; const struct file_operations *i_fop; struct super_block *i_sb; };
struct list_head { int dummy; };
struct hybridmount_dir_node { struct hm_iop *iop; struct hm_fop *fop; };
struct hm_iop { struct inode_operations fake_iop; const struct inode_operations *orig_iop; struct hybridmount_dir_node *dir_node; const void *orig_dops; };
struct hm_fop { struct file_operations fake_fop; const struct file_operations *orig_fop; struct hybridmount_dir_node *dir_node; struct list_head list; };
struct hm_sop { struct super_operations fake_sop; const struct super_operations *orig_sop; const struct xattr_handler **orig_xattr,**fake_xattr; struct super_block *sb; struct list_head list; };
struct hm_xattr_proxy { const struct xattr_handler *orig; struct xattr_handler fake; };
static int fail_at,allocations,live,listed; static int hybridmount_sb_list,hybridmount_fop_list __attribute__((unused));
static void *kmalloc(size_t size,int flags) { if(++allocations==fail_at) return NULL; void *p=calloc(1,size); if(p) live++; return p; }
static void kfree(void *p) { if(p) live--; free(p); }
static void list_add_tail_rcu(struct list_head *l,int *head) { listed++; }
static void list_add_tail(struct list_head *l,int *head) { listed++; }
static int try_module_get(struct module *m) { if(fail_at==5) return 0; if(m) m->refs++; return 1; }
static void module_put(struct module *m) { if(m) m->refs--; }
static struct hm_iop *hm_get_hm_iop(const struct inode_operations *ops) { return NULL; }
static struct hm_fop *hm_get_hm_fop(const struct file_operations *ops) { return NULL; }
static struct hm_sop *hm_get_hm_sop(const struct super_operations *ops) { return NULL; }
static struct dentry *hybridmount_hijacked_lookup(struct inode *i,struct dentry *d,unsigned int flags) { return NULL; }
static int hybridmount_hijacked_iterate_dir(struct file *f,struct dir_context *c) { return 0; }
static int hybridmount_hijacked_drop_inode(struct inode *i) { return 0; }
static void hybridmount_hijacked_evict_inode(struct inode *i) {}
static int hm_xattr_get(void) { return 0; }
static int hm_xattr_set(void) { return 0; }
#include "production.h"
int main(int argc,char **argv) {
    fail_at=atoi(argv[1]);
    struct inode_operations iop={hybridmount_hijacked_lookup};
    struct file_operations fop={.owner=&backing,.iterate_shared=hybridmount_hijacked_iterate_dir};
    struct super_operations sop={0}; struct xattr_handler handler={.get=hm_xattr_get};
    const struct xattr_handler *xattrs[]={&handler,NULL};
    struct super_block sb={&sop,xattrs}; struct inode inode={&iop,&fop,&sb};
    struct hybridmount_dir_node node={0};
    hybridmount_hijack_dir_ops(&node,&inode);
    if(fail_at) {
        assert(inode.i_op==&iop && inode.i_fop==&fop && sb.s_op==&sop && sb.s_xattr==xattrs);
        assert(!node.iop && !node.fop && !live && !listed && !backing.refs);
    } else {
        assert(node.iop && node.fop && listed==2 && backing.refs==1);
        assert(inode.i_fop->owner==THIS_MODULE && node.fop->orig_fop->owner==&backing);
    }
    printf("PASS: hook transaction allocation/module point %d\n",fail_at);
}
