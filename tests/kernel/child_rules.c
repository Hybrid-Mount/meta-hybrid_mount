// SPDX-License-Identifier: GPL-2.0-only
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define assert(x) do { if (!(x)) { fprintf(stderr,"FAIL %d: %s\n",__LINE__,#x); exit(1); } } while (0)
typedef uint32_t u32;
typedef uint64_t u64;
typedef unsigned int uid_t;
#define likely(x) (x)
#ifndef __always_inline
#define __always_inline inline
#endif
#define unlikely(x) (x)
#define READ_ONCE(x) (x)
#define WRITE_ONCE(x,v) ((x)=(v))
#define rcu_dereference(p) (p)
#define rcu_dereference_protected(p,l) (p)
#define rcu_access_pointer(p) (p)
#define rcu_assign_pointer(p,v) ((p)=(v))
#define srcu_dereference(p,s) (p)
#define kfree_rcu(p,m) free(p)
#define smp_mb() ((void)0)
#define cmpxchg(p,old,v) ({ __typeof__(*(p)) previous=*(p); if(previous==(old)) *(p)=(v); previous; })
#define GFP_KERNEL 0
#define EINVAL 22
#define ENOMEM 12
#define HYBRIDMOUNT_MAGIC_SIG 0
#define HM_FLAG_IS_DIR 1
#define HM_FLAG_VIRTUAL_DIR 2
#define HM_FLAG_WHITEOUT 4
#define DT_DIR 4
#define DT_REG 8
struct path { void *dentry,*mnt; };
struct hybridmount_rule;
struct hybridmount_dir_node {
    void *children,*iop,*fop,*v_inode;
    u64 bloom_mask;
    int seq;
};
struct hybridmount_rule {
    unsigned int target_uid;
    unsigned long v_ino;
    unsigned short child_len,flags;
    const char *name;
    struct path r_path;
    struct hybridmount_dir_node *parent_dir,*this_dir;
};
struct hybridmount_child_array { void *rcu[2]; int count,capacity; u32 hashes[]; };
struct hm_rule_info { struct path backing_path,r_path; unsigned short flags; unsigned long v_ino; struct hybridmount_dir_node *this_dir; };
struct dir_context { u32 pos; unsigned long ino; int emitted; };
#define hm_children_is_single(p) ((uintptr_t)(p)&1)
#define hm_children_single_rule(p) ((struct hybridmount_rule *)((uintptr_t)(p)&~(uintptr_t)1))
#define hm_children_from_single(p) ((void *)((uintptr_t)(p)|1))
#define hm_get_child_name(r) ((r)->name)
#define hm_get_child_rules(a) ((struct hybridmount_rule **)((a)->hashes+(a)->capacity))
static int hybridmount_srcu;
static int preempt_depth;
static bool enforce_preemption;
static void preempt_disable(void) { ++preempt_depth; }
static void preempt_enable(void) { assert(preempt_depth>0); --preempt_depth; }
static void write_seqcount_begin(int *seq) { if(enforce_preemption) assert(preempt_depth>0); ++*seq; }
static void write_seqcount_end(int *seq) { if(enforce_preemption) assert(preempt_depth>0); ++*seq; }
static unsigned int read_seqcount_begin(int *seq) { assert(!(*seq&1)); return *seq; }
static bool read_seqcount_retry(int *seq,unsigned int old) { return *seq!=(int)old; }
static void rcu_read_lock(void) {}
static void rcu_read_unlock(void) {}
static void synchronize_srcu(int *srcu) {}
static int srcu_read_lock(int *srcu) { return 0; }
static void srcu_read_unlock(int *srcu,int idx) {}
static void path_get(struct path *p) {}
static bool hm_dir_is_virtual(struct hybridmount_dir_node *dir) { return false; }
static void hm_dir_put(struct hybridmount_dir_node *dir) { assert(false); }
static void *kmalloc(size_t size,int flags) { return calloc(1,size); }
static u32 full_name_hash(const void *salt,const char *name,size_t len) { return 123; }
static bool hybridmount_is_uid_blocked(uid_t uid) { return false; }
static bool hm_is_virtual_pos(u32 pos) { return true; }
static u32 hm_pack_pos(int id) { return (u32)id; }
static u32 hm_unpack_pos(u32 pos) { return pos; }
static bool dir_emit(struct dir_context *ctx,const char *name,int len,unsigned long ino,int type) { ++ctx->emitted; ctx->ino=ino; return true; }
#include "production.h"

static void add(struct hybridmount_dir_node *dir,struct hybridmount_rule *rule) {
    assert(__hybridmount_inject_child_locked(dir,rule,rule->name,strlen(rule->name))==0);
    assert(!preempt_depth);
}
static void expect(struct hybridmount_dir_node *dir,uid_t uid,unsigned long ino) {
    struct hm_rule_info info={0};
    bool found=__hybridmount_get_rule_info(dir,"child",5,123,&info,false,uid);
    assert(found==(ino!=0));
    if(found) assert(info.v_ino==ino);
}
static void entries(struct hybridmount_dir_node *dir,uid_t uid,int count,unsigned long ino) {
    struct dir_context ctx={0};
    hybridmount_emit_virtual_children(&ctx,dir,uid);
    assert(ctx.emitted==count);
    if(count==1) assert(ctx.ino==ino);
    struct dir_context resumed={.pos=ctx.pos};
    hybridmount_emit_virtual_children(&resumed,dir,uid);
    assert(resumed.emitted==0);
}
int main(int argc,char **argv) {
    assert(argc==2);
    enforce_preemption=!strcmp(argv[1],"preemption");
    struct hybridmount_dir_node dir={.iop=(void *)1};
    struct hybridmount_rule global={.name="child",.v_ino=100};
    struct hybridmount_rule scoped={.name="child",.v_ino=200,.target_uid=42};
    struct hybridmount_rule other={.name="child",.v_ino=300,.target_uid=43};
    struct hybridmount_rule collision={.name="other",.v_ino=400,.target_uid=99};
    struct hybridmount_rule growth={.name="fifth",.v_ino=500,.target_uid=99};
    struct hybridmount_rule replacement={.name="child",.v_ino=201,.target_uid=42};
    add(&dir,&global);
    expect(&dir,42,100);
    add(&dir,&scoped);
    expect(&dir,7,100);
    expect(&dir,42,200);
    entries(&dir,42,1,200);
    add(&dir,&other);
    add(&dir,&collision);
    add(&dir,&growth);
    add(&dir,&replacement);
    expect(&dir,42,201);
    add(&dir,&scoped);
    expect(&dir,43,300);
    expect(&dir,0,100);
    entries(&dir,7,1,100);
    if(!strcmp(argv[1],"whiteout")) {
        scoped.flags=HM_FLAG_WHITEOUT;
        entries(&dir,42,0,0);
        entries(&dir,43,1,300);
    }
    __hybridmount_delete_child_locked(&scoped);
    expect(&dir,42,100);
    __hybridmount_delete_child_locked(&global);
    expect(&dir,7,0);
    expect(&dir,43,300);
    entries(&dir,43,1,300);
    __hybridmount_delete_child_locked(&collision);
    __hybridmount_delete_child_locked(&growth);
    __hybridmount_delete_child_locked(&other);
    assert(!dir.children && !preempt_depth);
    // Specific-first insertion must still choose it after a global fallback is added.
    add(&dir,&scoped);
    add(&dir,&global);
    expect(&dir,42,200);
    expect(&dir,7,100);
    __hybridmount_delete_child_locked(&global);
    __hybridmount_delete_child_locked(&scoped);
    assert(!dir.children && !preempt_depth);
    printf("PASS: child rules %s\n",argv[1]);
}
