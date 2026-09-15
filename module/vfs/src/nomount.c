#include <linux/init.h>
#include <linux/fs.h>
#include <linux/file.h>
#include <linux/namei.h>
#include <linux/cred.h>
#include <linux/xattr.h>
#include <linux/module.h>
#include "nomount.h"

/*** Helpers ***/

static __always_inline bool nomount_is_uid_blocked(uid_t target_uid)
{
    struct nm_uid_array *arr;
    bool blocked = false;
    rcu_read_lock();
    if ((arr = rcu_dereference(nomount_uids))) {
        for (int i = 0; i < arr->count; i++) {
            if (arr->uids[i] == target_uid) {
                blocked = true;
                break;
            }
        }
    }
    rcu_read_unlock();
    return blocked;
}


static __always_inline struct nomount_rule *nomount_bsearch_child(struct nomount_child_array *arr, const char *name, size_t len, u32 hash, int *index)
{
    int l = 0, n = arr->count;
    u32 *hashes = arr->hashes;
    struct nomount_rule **rules = nm_get_child_rules(arr);

    while (n > 0) {
        int step = n >> 1, m = l + step, less = (hashes[m] < hash), mask = -less;
        l += (step + 1) & mask;
        n = step + ((n - (step << 1) - 1) & mask);
    }
    if (index) *index = l;
    while (l < arr->count && hashes[l] == hash) {
        struct nomount_rule *rule = READ_ONCE(rules[l]);
        if (likely(rule && rule->child_len == len) && !memcmp(nm_get_child_name(rule), name, len)) {
            if (index) *index = l;
            return rule;
        }
        l++;
    }
    return NULL;
}

static bool __nomount_get_rule_info(struct nomount_dir_node *dir_node, const char *name, size_t len, u32 hash, struct nm_rule_info *rule_info, bool get_path)
{
    void *children;
    struct nomount_rule *rule, *found_rule;
    unsigned int seq;
    uid_t fsuid = current_fsuid().val;

    do {
        found_rule = NULL;
        seq = read_seqcount_begin(&dir_node->seq);
        if (likely((children = rcu_dereference(dir_node->children)))) {
            if (nm_children_is_single(children)) {
                rule = nm_children_single_rule(children);
                if (rule->child_len != len || memcmp(nm_get_child_name(rule), name, len)) rule = NULL;
            } else {
                rule = nomount_bsearch_child(children, name, len, hash, NULL);
            }
            if (rule && (!rule->target_uid || rule->target_uid == fsuid))
                found_rule = rule;
        }
    } while (read_seqcount_retry(&dir_node->seq, seq));

    if (!found_rule) return false;

    if (likely(rule_info)) {
        rule_info->flags = found_rule->flags;
        rule_info->v_ino = found_rule->v_ino;
        rule_info->this_dir = found_rule->this_dir;
        if (found_rule->flags & NM_FLAG_VIRTUAL_DIR) {
            rule_info->r_path = (struct path){ .dentry = NULL, .mnt = NULL };
        } else {
            rule_info->r_path = (get_path && found_rule->r_path.dentry) ? found_rule->r_path : (struct path){ .dentry = NULL, .mnt = NULL };
            if (rule_info->r_path.dentry) path_get(&rule_info->r_path);
        }
    }

    return true;
}

static bool nomount_get_rule_info(struct nomount_dir_node *dir_node, const char *name, size_t len, u32 hash, struct nm_rule_info *rule_info, bool get_path)
{
    bool found;
    if (unlikely(!dir_node)) return false;
    rcu_read_lock();
    found = __nomount_get_rule_info(dir_node, name, len, hash, rule_info, get_path);
    rcu_read_unlock();
    return found;
}

static void nm_dir_rcu_free(struct rcu_head *head)
{
    struct nomount_dir_node *dir = container_of(head, struct nomount_dir_node, rcu);
    void *children = rcu_dereference_raw(dir->children);
    if (!nm_children_is_single(children)) kfree(children);
    kfree(dir);
}

static inline void nm_destroy_virtual_inode(struct inode *inode)
{
    struct nm_inode_info *info = inode->i_private;
    if (!info) return;
    if (info->r_path.dentry) path_put(&info->r_path);

    if (info->dir_node) {
        WRITE_ONCE(info->dir_node->v_inode, NULL);
        if (nm_dir_tag(info->dir_node) == 1UL)
            call_rcu(&info->dir_node->rcu, nm_dir_rcu_free);
    }

    kfree(info);
    inode->i_private = NULL;
}

static inline void nm_destroy_hijacked_inode(struct inode *inode, bool restore)
{
    struct nm_iop *nm_iop = nm_get_nm_iop(inode->i_op);
    struct nm_fop *nm_fop = nm_get_nm_fop(inode->i_fop);
    struct nomount_dir_node *dir_node = nm_iop ? nm_iop->dir_node : (nm_fop ? nm_fop->dir_node : NULL);

    if (nm_iop) {
        if (dir_node) RCU_INIT_POINTER(dir_node->iop, NULL);
        if (restore) smp_store_release(&inode->i_op, nm_iop->orig_iop);
        kfree_rcu(nm_iop, rcu);
    }
    if (nm_fop) {
        if (dir_node) RCU_INIT_POINTER(dir_node->fop, NULL);
        if (restore) smp_store_release(&inode->i_fop, nm_fop->orig_fop);
        kfree_rcu(nm_fop, rcu);
    }
    if (dir_node && !nm_dir_is_virtual(dir_node)) {
        smp_mb();
        if (!rcu_access_pointer(dir_node->children) &&
             cmpxchg(&dir_node->v_inode, NULL, (struct inode *)-1L) == NULL)
                call_rcu(&dir_node->rcu, nm_dir_rcu_free);
    }
}

struct nomount_proxy_ctx {
    struct dir_context ctx;
    struct dir_context *orig_ctx;
    struct nomount_dir_node *dir_node;
    bool emitted;
    bool uid_blocked;
};

static NM_ACTOR_RET nomount_actor_proxy(struct dir_context *ctx, const char *name, int namelen,
                                        loff_t offset, u64 ino, unsigned int d_type)
{
    struct nomount_proxy_ctx *proxy = container_of(ctx, struct nomount_proxy_ctx, ctx);
    NM_ACTOR_RET ret;

    if (proxy->dir_node && !proxy->uid_blocked) {
        u32 hash = full_name_hash((const void *)(unsigned long)NOMOUNT_MAGIC_SIG, name, namelen);
        if ((READ_ONCE(proxy->dir_node->bloom_mask) & (1ULL << (hash & 63))) &&
            nomount_get_rule_info(proxy->dir_node, name, namelen, hash, NULL, false)) {
            proxy->ctx.pos = offset;
            return NM_ACTOR_CONTINUE;
        }
    }

    proxy->orig_ctx->pos = proxy->ctx.pos;
    ret = proxy->orig_ctx->actor(proxy->orig_ctx, name, namelen, offset, ino, d_type);
    proxy->ctx.pos = proxy->orig_ctx->pos;
    proxy->emitted = true;

    return ret;
}

static inline void nomount_emit_virtual_children(struct dir_context *ctx, struct nomount_dir_node *dir_node)
{
	struct nomount_child_array *array;
	void *children;
	uid_t fsuid = current_fsuid().val;
	int id, srcu_idx;

	if (!dir_node || nomount_is_uid_blocked(fsuid)) return;
	if (!nm_is_virtual_pos(ctx->pos)) ctx->pos = nm_pack_pos(0);
	srcu_idx = srcu_read_lock(&nomount_srcu);
	children = srcu_dereference(dir_node->children, &nomount_srcu);
	if (children) {
		struct nomount_rule *single = nm_children_is_single(children) ? nm_children_single_rule(children) : NULL;
		struct nomount_rule **rules;
		array = single ? NULL : children;
		rules = array ? nm_get_child_rules(array) : &single;
		for (id = nm_unpack_pos(ctx->pos); id < (array ? READ_ONCE(array->count) : 1); id++) {
			struct nomount_rule *rule;
			ctx->pos = nm_pack_pos(id);
			if ((rule = READ_ONCE(rules[id])) && (rule->target_uid == 0 || rule->target_uid == fsuid)) {
				if (!(rule->flags & NM_FLAG_WHITEOUT) && !dir_emit(ctx, nm_get_child_name(rule), rule->child_len, rule->v_hash,
						(rule->flags & NM_FLAG_IS_DIR) ? DT_DIR : DT_REG)) break;
			}
			ctx->pos = nm_pack_pos(id + 1);
		}
	}
	srcu_read_unlock(&nomount_srcu, srcu_idx);
}

static void nomount_init_prealloc_inode(struct inode *inode, struct nm_inode_info *info, struct nm_rule_info *rule_info)
{
    struct inode *r_inode = NULL;
    info->flags = rule_info->flags;
    info->dir_node = rule_info->this_dir;
    if (rule_info->flags & NM_FLAG_VIRTUAL_DIR) {
        info->r_path = (struct path){ .dentry = NULL, .mnt = NULL };
    } else {
        info->r_path = rule_info->r_path.dentry ? rule_info->r_path : (struct path){ .dentry = NULL, .mnt = NULL };
        r_inode = info->r_path.dentry ? d_backing_inode(info->r_path.dentry) : NULL;
    }

    inode->i_ino = rule_info->v_ino;
    inode->i_private = info;
    inode->i_mode   = r_inode ? r_inode->i_mode    : (S_IFDIR | 0755);
    inode->i_size   = r_inode ? i_size_read(r_inode) : 4096;
    inode->i_blocks = r_inode ? r_inode->i_blocks  : 8;
    inode->i_uid    = r_inode ? r_inode->i_uid     : GLOBAL_ROOT_UID;
    inode->i_gid    = r_inode ? r_inode->i_gid     : GLOBAL_ROOT_GID;
    inode->i_op     = (r_inode && !S_ISDIR(r_inode->i_mode)) ? &nm_file_iops : &nm_dir_iops;

    if (r_inode && !S_ISDIR(r_inode->i_mode))
#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 16, 0)
        inode->i_fop = (r_inode->i_fop && r_inode->i_fop->mmap_prepare) ? &nm_file_fops_mmap_prepare : &nm_file_fops;
#else
        inode->i_fop = &nm_file_fops;
#endif
    else
        inode->i_fop = &nm_dir_fops;

    if (r_inode) nm_sync_inode_times(inode, r_inode), inode->i_mapping = r_inode->i_mapping;
    inode->i_flags |= S_PRIVATE | S_NOATIME | S_NOCMTIME | S_NOSEC;
    if (!S_ISLNK(inode->i_mode)) inode->i_opflags |= IOP_NOFOLLOW;
}

static struct dentry *nomount_resolve_rule_dentry(struct inode *dir, struct dentry *dentry, struct nomount_dir_node *dir_node, u32 hash)
{
    struct nm_inode_info *prealloc_info = NULL;
    struct inode *splice_inode = NULL, *prealloc_inode = NULL;
    struct dentry *res = ERR_PTR(-ENODATA);
    struct nm_rule_info rule_info = {0};

    rcu_read_lock();
    if (!dir_node || !__nomount_get_rule_info(dir_node, dentry->d_name.name, dentry->d_name.len, hash, &rule_info, false))
        goto unlock_out;
    if (rule_info.flags & NM_FLAG_WHITEOUT)
        goto resolve_rule;
    rcu_read_unlock();

    if (likely((prealloc_inode = new_inode(dir->i_sb)))) {
        if (unlikely(!(prealloc_info = kmalloc(sizeof(*prealloc_info), GFP_KERNEL)))) {
            iput(prealloc_inode);
            prealloc_inode = NULL;
        }
    }

    rcu_read_lock();
    if (unlikely(!__nomount_get_rule_info(dir_node, dentry->d_name.name, dentry->d_name.len, hash, &rule_info, true)))
        goto unlock_out;

resolve_rule:
    if (unlikely(nomount_is_uid_blocked(current_fsuid().val)))
        goto unlock_out;

    if (rule_info.flags & NM_FLAG_WHITEOUT) {
        nomount_hijack_dentry_ops(dir, dentry, true);
        d_add(dentry, NULL); res = NULL;
        goto unlock_out;
    }

    if (likely(prealloc_inode && ((rule_info.flags & NM_FLAG_VIRTUAL_DIR) || rule_info.r_path.dentry))) {
        if (rule_info.this_dir && (splice_inode = cmpxchg(&rule_info.this_dir->v_inode, NULL, prealloc_inode))) {
            if (splice_inode == (struct inode *)-1L) goto unlock_out;
            igrab(splice_inode);
        } else {
            nomount_init_prealloc_inode(prealloc_inode, prealloc_info, &rule_info);
            splice_inode = prealloc_inode;
            prealloc_inode = NULL; prealloc_info = NULL;
            rule_info.r_path.dentry = NULL; 
        }

        rcu_read_unlock();
        if (!IS_ERR((res = d_splice_alias(splice_inode, dentry))))
            nomount_hijack_dentry_ops(dir, res ? res : dentry, true);
            
        goto cleanup_out;
    }

unlock_out:
    rcu_read_unlock();
cleanup_out:
    if (rule_info.r_path.dentry) 
        path_put(&rule_info.r_path);

    if (prealloc_inode) {
        kfree(prealloc_info);
        iput(prealloc_inode);
    }    
    return res;
}

/*** i_op / s_op / f_op Hijacking Hooks ***/

static struct dentry *nomount_hijacked_lookup(struct inode *dir, struct dentry *dentry, unsigned int flags)
{
    struct nm_iop *nm_iop = nm_get_nm_iop(smp_load_acquire(&dir->i_op));
    struct nomount_dir_node *dir_node = nm_iop ? READ_ONCE(nm_iop->dir_node) : NULL;
    bool is_blocked = nomount_is_uid_blocked(current_fsuid().val);
    struct dentry *res;
    u32 hash = 0;

    if (unlikely(!nm_iop || !dir_node || is_blocked))
        goto do_real_lookup;

    hash = full_name_hash((const void *)(unsigned long)NOMOUNT_MAGIC_SIG, dentry->d_name.name, dentry->d_name.len);
    if (likely(!(READ_ONCE(dir_node->bloom_mask) & (1ULL << (hash & 63)))))
        goto do_real_lookup;

    if ((res = nomount_resolve_rule_dentry(dir, dentry, dir_node, hash)) != ERR_PTR(-ENODATA))
        return res;

do_real_lookup:
    if (likely(nm_iop && nm_iop->orig_iop && nm_iop->orig_iop->lookup)) {
        res = nm_iop->orig_iop->lookup(dir, dentry, flags);
        if (dir_node && !is_blocked) {
            if (!hash) hash = full_name_hash((const void *)(unsigned long)NOMOUNT_MAGIC_SIG, dentry->d_name.name, dentry->d_name.len);
            if (unlikely(nomount_get_rule_info(dir_node, dentry->d_name.name, dentry->d_name.len, hash, NULL, false))) {
                struct dentry *target = res ? res : dentry;
                if (!IS_ERR(target)) d_drop(target);
            }
        }
        if (!IS_ERR(res ? res : dentry)) nomount_hijack_dentry_ops(dir, res ? res : dentry, false);
        return res;
    }
    return ERR_PTR(-EOPNOTSUPP);
}

static int nomount_hijacked_iterate_dir(struct file *file, struct dir_context *ctx)
{
    struct nm_fop *nm_fop = nm_get_nm_fop(smp_load_acquire(&file->f_op));
    struct nomount_dir_node *dir_node = nm_fop ? READ_ONCE(nm_fop->dir_node) : NULL;
    const struct file_operations *orig_fop = nm_fop ? nm_fop->orig_fop : NULL;
    struct nomount_proxy_ctx proxy_ctx = { .ctx.actor = nomount_actor_proxy };
    bool is_blocked = nomount_is_uid_blocked(current_fsuid().val);
    int res = 0;

    if (unlikely(!orig_fop || !dir_node))
        goto do_real_iterate;

    if (unlikely(nm_is_virtual_pos(ctx->pos))) {
        if (likely(!is_blocked)) nomount_emit_virtual_children(ctx, dir_node);
        return 0;
    }

    if (unlikely(is_blocked || !READ_ONCE(dir_node->bloom_mask)))
        goto do_real_iterate;

    proxy_ctx.ctx.pos = ctx->pos;
    proxy_ctx.orig_ctx = ctx;
    proxy_ctx.dir_node = dir_node;
    proxy_ctx.emitted = false;
    proxy_ctx.uid_blocked = is_blocked;

    res = nm_call_iterate(file, &proxy_ctx.ctx, orig_fop);
    ctx->pos = proxy_ctx.ctx.pos;
    
    if (res < 0 || proxy_ctx.emitted)
        return res;

    ctx->pos = nm_pack_pos(0);
    nomount_emit_virtual_children(ctx, dir_node);
    return res;

do_real_iterate:
    if (likely(orig_fop)) 
        return nm_call_iterate(file, ctx, orig_fop);
    return -ENOTDIR;
}

static void nomount_hijacked_destroy_inode(struct inode *inode)
{
    struct nm_sop *nm_sop;
    (inode->i_op == &nm_file_iops || inode->i_op == &nm_dir_iops) ? nm_destroy_virtual_inode(inode) : nm_destroy_hijacked_inode(inode, false);

    nm_sop = nm_get_nm_sop(smp_load_acquire(&inode->i_sb->s_op));
    if (nm_sop && nm_sop->orig_sop && nm_sop->orig_sop->destroy_inode)
        nm_sop->orig_sop->destroy_inode(inode);
}

static int nomount_hijacked_drop_inode(struct inode *inode)
{
    struct nm_sop *nm_sop;
    if (inode->i_op == &nm_file_iops || inode->i_op == &nm_dir_iops) goto generic_fn;

    nm_sop = nm_get_nm_sop(smp_load_acquire(&inode->i_sb->s_op));
    if (nm_sop && nm_sop->orig_sop && nm_sop->orig_sop->drop_inode)
        return nm_sop->orig_sop->drop_inode(inode);

generic_fn:
    return !inode->i_nlink || inode_unhashed(inode);
}

static void nomount_hijacked_evict_inode(struct inode *inode)
{
    struct nm_sop *nm_sop;
    if (inode->i_op == &nm_file_iops || inode->i_op == &nm_dir_iops) goto generic_fn;

    nm_sop = nm_get_nm_sop(smp_load_acquire(&inode->i_sb->s_op));
    if (nm_sop && nm_sop->orig_sop && nm_sop->orig_sop->evict_inode) {
        nm_sop->orig_sop->evict_inode(inode);
    } else {
generic_fn:
        truncate_inode_pages_final(&inode->i_data);
        clear_inode(inode);
    }
}

/*** file / inode / superblock operations ***/

static int nm_open(struct inode *inode, struct file *file)
{
    struct nm_inode_info *info = inode->i_private;
    struct file *real_file;

    if (unlikely(!info)) return -ENODEV;
    if (unlikely(info->flags & NM_FLAG_VIRTUAL_DIR)) {
        file->private_data = NULL;
        return 0;
    }
    if (unlikely(!info->r_path.dentry)) return -ENODEV;

    real_file = dentry_open(&info->r_path, (file->f_flags & ~(O_CREAT | O_EXCL | O_NOCTTY)), file->f_cred);
    if (IS_ERR(real_file)) return PTR_ERR(real_file);

    file->private_data = real_file;
    return 0;
}

static int nm_release(struct inode *inode, struct file *file)
{
    struct file *real_file = file->private_data;
    if (real_file) fput(real_file), file->private_data = NULL;
    return 0;
}

static loff_t nm_llseek(struct file *file, loff_t offset, int whence)
{
    struct file *real_file = file->private_data;
    loff_t res;
    if (!real_file) return -EINVAL;

    real_file->f_pos = file->f_pos;
    res = vfs_llseek(real_file, offset, whence);
    file->f_pos = real_file->f_pos;

    return res;
}

static ssize_t nm_read_iter(struct kiocb *iocb, struct iov_iter *to)
{
    struct file *file = iocb->ki_filp;
    struct file *real_file = file->private_data;
    ssize_t ret;
    if (!real_file || !real_file->f_op->read_iter) return -EINVAL;

    iocb->ki_filp = real_file;
    ret = real_file->f_op->read_iter(iocb, to);
    iocb->ki_filp = file;

    return ret;
}

static ssize_t nm_write_iter(struct kiocb *iocb, struct iov_iter *from)
{
    struct file *file = iocb->ki_filp;
    struct file *real_file = file->private_data;
    ssize_t ret;
    if (!real_file || !real_file->f_op->write_iter) return -EINVAL;

    iocb->ki_filp = real_file;
    ret = real_file->f_op->write_iter(iocb, from);
    iocb->ki_filp = file;

    return ret;
}

static int nm_mmap(struct file *file, struct vm_area_struct *vma)
{
    int ret = generic_file_mmap(file, vma);
    return ret ? ret : (file_inode(file)->i_flags &= ~S_PRIVATE, 0);
}

#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 16, 0)
static int nm_mmap_prepare(struct vm_area_desc *desc)
{
    int ret = generic_file_mmap_prepare(desc);
    return ret ? ret : (file_inode(desc->file)->i_flags &= ~S_PRIVATE, 0);
}
#endif

static long nm_unlocked_ioctl(struct file *file, unsigned int cmd, unsigned long arg)
{
    struct file *real_file = file->private_data;
    if (!real_file || !real_file->f_op->unlocked_ioctl) return -ENOTTY;
    return real_file->f_op->unlocked_ioctl(real_file, cmd, arg);
}

#ifdef CONFIG_COMPAT
static long nm_compat_ioctl(struct file *file, unsigned int cmd, unsigned long arg)
{
    struct file *real_file = file->private_data;
    if (!real_file || !real_file->f_op->compat_ioctl) return -ENOTTY;
    return real_file->f_op->compat_ioctl(real_file, cmd, arg);
}
#endif

static ssize_t nm_splice_read(struct file *in, loff_t *ppos, struct pipe_inode_info *pipe,
                              size_t len, unsigned int flags)
{
    struct file *real_file = in->private_data;
    if (!real_file || !real_file->f_op->splice_read) return -EINVAL;
    return real_file->f_op->splice_read(real_file, ppos, pipe, len, flags);
}

static ssize_t nm_splice_write(struct pipe_inode_info *pipe, struct file *out,
                               loff_t *ppos, size_t len, unsigned int flags)
{
    struct file *real_file = out->private_data;
    if (!real_file || !real_file->f_op->splice_write) return -EINVAL;
    return real_file->f_op->splice_write(pipe, real_file, ppos, len, flags);
}

static int nm_fsync(struct file *file, loff_t start, loff_t end, int datasync)
{
    struct file *real_file = file->private_data;
    if (!real_file || !real_file->f_op->fsync) return -EINVAL;
    return real_file->f_op->fsync(real_file, start, end, datasync);
}

static ssize_t nm_listxattr(struct dentry *dentry, char *buffer, size_t size)
{
    struct nm_inode_info *info = d_backing_inode(dentry)->i_private;
    struct inode *r_inode;

    if (unlikely(!info)) return -EIO;
    if (info->flags & NM_FLAG_VIRTUAL_DIR) return 0;
    if (!info->r_path.dentry) return -EOPNOTSUPP;

    r_inode = d_backing_inode(info->r_path.dentry);
    if (!r_inode || !r_inode->i_op || !r_inode->i_op->listxattr) return 0;
    return r_inode->i_op->listxattr(info->r_path.dentry, buffer, size);
}

#if LINUX_VERSION_CODE < KERNEL_VERSION(4, 11, 0)
static int nm_getattr(struct vfsmount *mnt, struct dentry *dentry, struct kstat *stat)
#else
static int nm_getattr(IDMAP_ARG const struct path *path, struct kstat *stat, u32 request_mask, unsigned int query_flags)
#endif
{
#if LINUX_VERSION_CODE >= KERNEL_VERSION(4, 11, 0)
    struct dentry *dentry = path->dentry;
#endif
    struct inode *v_inode = d_backing_inode(dentry);
    struct nm_inode_info *info = v_inode->i_private;
    int res = 0;

    if (unlikely(!info)) return -EIO;
    if (unlikely(info->flags & NM_FLAG_VIRTUAL_DIR))
#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 3, 0)
        generic_fillattr(IDMAP_CALL request_mask, v_inode, stat);
#else
        generic_fillattr(IDMAP_CALL v_inode, stat);
#endif
    else
#if LINUX_VERSION_CODE < KERNEL_VERSION(4, 11, 0)
        res = vfs_getattr_nosec(&info->r_path, stat);
#else
        res = vfs_getattr_nosec(&info->r_path, stat, request_mask, query_flags);
#endif

    if (likely(res == 0)) {
        stat->ino = v_inode->i_ino;
        stat->dev = v_inode->i_sb->s_dev;
    }
    
    return res;
}

static int nm_setattr(IDMAP_ARG struct dentry *dentry, struct iattr *attr)
{
    struct inode *v_inode = d_inode(dentry);
    struct nm_inode_info *info = v_inode->i_private;
    struct inode *r_inode;
    int err;

    if (unlikely(!info)) return -EIO;
    if (info->flags & NM_FLAG_VIRTUAL_DIR) return 0;

    r_inode = d_backing_inode(info->r_path.dentry);
    inode_lock(r_inode);
    err = notify_change(IDMAP_CALL info->r_path.dentry, attr, NULL);
    inode_unlock(r_inode);

    if (likely(!err)) {
        if (attr->ia_valid & ATTR_SIZE) i_size_write(v_inode, i_size_read(r_inode));
        if (attr->ia_valid & ATTR_MODE) v_inode->i_mode = r_inode->i_mode;
        if (attr->ia_valid & ATTR_UID)  v_inode->i_uid = r_inode->i_uid;
        if (attr->ia_valid & ATTR_GID)  v_inode->i_gid = r_inode->i_gid;
        nm_sync_inode_times(v_inode, r_inode);
    }
    return err;
}

static const char *nm_get_link(struct dentry *dentry, struct inode *inode, struct delayed_call *done)
{
    struct nm_inode_info *info = inode->i_private;
    struct inode *real_inode;
    struct dentry *target_dentry;
    if (unlikely(!info || !info->r_path.dentry)) return ERR_PTR(-ECHILD);

    real_inode = d_backing_inode(info->r_path.dentry);
    target_dentry = dentry ? info->r_path.dentry : NULL;
    if (real_inode && real_inode->i_op && real_inode->i_op->get_link) {
        return real_inode->i_op->get_link(target_dentry, real_inode, done);
    }

    return ERR_PTR(-EINVAL);
}

static int nm_dir_iterate_dir(struct file *file, struct dir_context *ctx)
{
    struct nm_inode_info *info = file_inode(file)->i_private;
    struct nomount_dir_node *dir_node = info ? info->dir_node : NULL;
    struct file *real_file = file->private_data;
    int res = 0;
    if (unlikely(nm_is_virtual_pos(ctx->pos))) goto emit_virtual;

    if (real_file) {
        struct nomount_proxy_ctx proxy_ctx = {
            .ctx.actor = nomount_actor_proxy, .ctx.pos = ctx->pos, .orig_ctx = ctx,
            .dir_node = dir_node, .emitted = false, .uid_blocked = nomount_is_uid_blocked(current_fsuid().val)
        };
        res = nm_call_iterate(real_file, &proxy_ctx.ctx, real_file->f_op);
        ctx->pos = proxy_ctx.ctx.pos;
        if (res < 0 || proxy_ctx.emitted) return res;
        ctx->pos = nm_pack_pos(0);
    } else if (info && (info->flags & NM_FLAG_VIRTUAL_DIR)) {
        if (ctx->pos < 2 && !dir_emit_dots(file, ctx)) return 0;
        ctx->pos = nm_pack_pos(0);
    } else {
        return -ENOTDIR;
    }

emit_virtual:
    nomount_emit_virtual_children(ctx, dir_node);
    return res;
}

static struct dentry *nm_dir_lookup(struct inode *dir, struct dentry *dentry, unsigned int flags)
{
    struct nm_inode_info *info = dir->i_private; 
    struct dentry *res;

    if (info->dir_node) {
        u32 v_hash = full_name_hash((const void *)(unsigned long)NOMOUNT_MAGIC_SIG, dentry->d_name.name, dentry->d_name.len);
        if (READ_ONCE(info->dir_node->bloom_mask) & (1ULL << (v_hash & 63)) &&
            (res = nomount_resolve_rule_dentry(dir, dentry, info->dir_node, v_hash)) != ERR_PTR(-ENODATA))
                return res;
    }

    if (info->flags & NM_FLAG_VIRTUAL_DIR)
        goto negative_dentry;

    if (info->r_path.dentry) {
        struct inode *r_dir = d_backing_inode(info->r_path.dentry);
        if (r_dir->i_op->lookup) {
            res = r_dir->i_op->lookup(r_dir, dentry, flags);
            if (!IS_ERR(res ? res : dentry)) nomount_hijack_dentry_ops(dir, res ? res : dentry, false);
            return res;
        }
    }
    return ERR_PTR(-EOPNOTSUPP);

negative_dentry:
    nomount_hijack_dentry_ops(dir, dentry, true);
    d_add(dentry, NULL);
    return NULL;
}

struct nm_xattr_proxy {
    struct xattr_handler fake;
    const struct xattr_handler *orig;
};

static int nm_xattr_get(const struct xattr_handler *handler, struct dentry *dentry, struct inode *inode, const char *name, void *buffer, size_t size FLAGS_ARG)
{
    struct nm_xattr_proxy *proxy = container_of(handler, struct nm_xattr_proxy, fake);
    if (inode->i_op == &nm_file_iops || inode->i_op == &nm_dir_iops) {
        struct nm_inode_info *info = inode->i_private;
        if (unlikely(!info || !info->r_path.dentry)) return -ENODATA;
        return __vfs_getxattr(info->r_path.dentry, d_inode(info->r_path.dentry), xattr_full_name(handler, name), buffer, size FLAGS_VAL);
    }

    return proxy->orig->get(proxy->orig, dentry, inode, name, buffer, size FLAGS_VAL);
}

static int nm_xattr_set(const struct xattr_handler *handler, IDMAP_ARG struct dentry *dentry, struct inode *inode, const char *name, const void *buffer, size_t size, int flags)
{
    struct nm_xattr_proxy *proxy = container_of(handler, struct nm_xattr_proxy, fake);
    if (inode->i_op == &nm_file_iops || inode->i_op == &nm_dir_iops) {
        struct nm_inode_info *info = inode->i_private;
        if (unlikely(!info || !info->r_path.dentry)) return -ENODATA;
        return __vfs_setxattr(IDMAP_PATH(info->r_path) info->r_path.dentry, d_inode(info->r_path.dentry), xattr_full_name(handler, name), buffer, size, flags);
    }
    return proxy->orig->set(proxy->orig, IDMAP_CALL dentry, inode, name, buffer, size, flags);
}

#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 13, 0)
static int nm_d_revalidate(struct inode *parent_inode, const struct qstr *name, struct dentry *dentry, unsigned int flags)
#else
static int nm_d_revalidate(struct dentry *dentry, unsigned int flags)
#endif
{
    struct nomount_dir_node *parent_dir = NULL;
    const struct dentry_operations *orig_dops;
    struct nm_rule_info rule_info;
    struct inode *inode;
    struct nm_iop *iop = NULL;
    bool injected, has_rule = false;

#if LINUX_VERSION_CODE < KERNEL_VERSION(6, 13, 0)
    struct inode *parent_inode = d_inode(READ_ONCE(dentry->d_parent));
    const struct qstr *name = &dentry->d_name;
#endif
    if (unlikely(!parent_inode)) return 1;

    if (parent_inode->i_op == &nm_dir_iops) {
        parent_dir = ((struct nm_inode_info *)parent_inode->i_private)->dir_node;
    } else {
        iop = nm_get_nm_iop(smp_load_acquire(&parent_inode->i_op));
        parent_dir = iop ? iop->dir_node : NULL;
    }

    inode = READ_ONCE(dentry->d_inode);
    injected = inode && (inode->i_op == &nm_file_iops || inode->i_op == &nm_dir_iops);

    if (parent_dir) {
        u32 hash = full_name_hash((const void *)(unsigned long)NOMOUNT_MAGIC_SIG, name->name, name->len);
        has_rule = nomount_get_rule_info(parent_dir, name->name, name->len, hash, &rule_info, false);
    }

    if (nomount_is_uid_blocked(current_fsuid().val)) {
        if (injected) goto drop_it;
        goto orig_dops;
    }

    if (has_rule) {
        if (rule_info.flags & NM_FLAG_WHITEOUT) return !inode;
        if (injected) return 1;
        goto drop_it;
    }

    if (injected || (!inode && has_rule))
        goto drop_it;

orig_dops:
    if ((orig_dops = nm_get_orig_dops(iop)) && orig_dops->d_revalidate) {
#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 13, 0)
        return orig_dops->d_revalidate(parent_inode, name, dentry, flags);
#else
        return orig_dops->d_revalidate(dentry, flags);
#endif
    }
    return 1;

drop_it:
    if (flags & LOOKUP_RCU) return -ECHILD;
    d_drop(dentry);
    return 0;
}

#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 16, 0)
static const struct file_operations nm_file_fops_mmap_prepare = {
    .owner = THIS_MODULE,
    .llseek = nm_llseek,
    .open = nm_open,
    .release = nm_release,
    .read_iter = nm_read_iter,
    .write_iter = nm_write_iter,
    .mmap_prepare = nm_mmap_prepare,
    .unlocked_ioctl = nm_unlocked_ioctl,
#ifdef CONFIG_COMPAT
    .compat_ioctl = nm_compat_ioctl,
#endif
    .splice_read = nm_splice_read,
    .splice_write = nm_splice_write,
    .fsync = nm_fsync,
};
#endif

static const struct file_operations nm_file_fops = {
    .owner = THIS_MODULE,
    .llseek = nm_llseek,
    .open = nm_open,
    .release = nm_release,
    .read_iter = nm_read_iter,
    .write_iter = nm_write_iter,
    .mmap = nm_mmap,
    .unlocked_ioctl = nm_unlocked_ioctl,
#ifdef CONFIG_COMPAT
    .compat_ioctl = nm_compat_ioctl,
#endif
    .splice_read = nm_splice_read,
    .splice_write = nm_splice_write,
    .fsync = nm_fsync,
};

static const struct inode_operations nm_file_iops = {
    .getattr = nm_getattr,
    .setattr = nm_setattr,
    .listxattr = nm_listxattr,
    .get_link = nm_get_link,
};

static const struct file_operations nm_dir_fops = {
    .owner = THIS_MODULE,
    .open = nm_open,
    .release = nm_release,
    .llseek = default_llseek,
    .read = generic_read_dir,
    .iterate_shared = nm_dir_iterate_dir,
#if LINUX_VERSION_CODE < KERNEL_VERSION(6, 6, 0)
    .iterate = nm_dir_iterate_dir,
#endif
};

static const struct inode_operations nm_dir_iops = {
    .lookup = nm_dir_lookup,
    .getattr = nm_getattr,
    .setattr = nm_setattr,
    .listxattr = nm_listxattr,
};

/* --- Hijacking Management --- */

static inline void nomount_hijack_superblock(struct super_block *sb)
{
    struct nm_sop *nm_sop;
    int count = 0;

    if (unlikely(!sb || !sb->s_op || nm_get_nm_sop(smp_load_acquire(&sb->s_op)) ||
                 !(nm_sop = kmalloc(sizeof(*nm_sop), GFP_KERNEL)))) return;

    nm_sop->fake_sop = *(sb->s_op);
    nm_sop->orig_sop = sb->s_op;
    nm_sop->orig_xattr = nm_sop->fake_xattr = NULL;
    nm_sop->sb = sb;
    nm_sop->fake_sop.destroy_inode = nomount_hijacked_destroy_inode;
    nm_sop->fake_sop.drop_inode = nomount_hijacked_drop_inode;
    nm_sop->fake_sop.evict_inode = nomount_hijacked_evict_inode;

    if (sb->s_xattr) {
        const struct xattr_handler **new_array;
        struct nm_xattr_proxy *proxies;

        while (sb->s_xattr[count]) count++;
        if ((new_array = kmalloc((count + 1) * sizeof(void *) + (count * sizeof(*proxies)), GFP_KERNEL))) {
            proxies = (void *)(new_array + count + 1);
            new_array[count] = NULL;
            for (int i = 0; i < count; i++) {
                proxies[i].orig = sb->s_xattr[i];
                proxies[i].fake = *sb->s_xattr[i];
                if (proxies[i].fake.get) proxies[i].fake.get = nm_xattr_get;
                if (proxies[i].fake.set) proxies[i].fake.set = nm_xattr_set;
                new_array[i] = &proxies[i].fake;
            }
            nm_sop->orig_xattr = (const struct xattr_handler **)sb->s_xattr;
            nm_sop->fake_xattr = new_array;
            smp_store_release((const struct xattr_handler ***)&sb->s_xattr, new_array);
            nm_debug("xattr handlers successfully hijacked for dev: 0x%x\n", sb->s_dev);
        }
    }

    list_add_tail_rcu(&nm_sop->list, &nomount_sb_list);
    smp_store_release(&sb->s_op, &nm_sop->fake_sop);
    nm_debug("Superblock successfully hijacked for dev: 0x%x\n", sb->s_dev);
}

static inline void nomount_hijack_dir_ops(struct nomount_dir_node *dir_node, struct inode *inode)
{
    struct nm_iop *nm_iop = NULL;
    struct nm_fop *nm_fop = NULL;

    if (inode->i_op && !nm_get_nm_iop(smp_load_acquire(&inode->i_op))) {
        if (likely((nm_iop = kmalloc(sizeof(*nm_iop), GFP_KERNEL)))) {
            nm_iop->fake_iop = *(inode->i_op);
            nm_iop->orig_iop = inode->i_op;
            nm_iop->dir_node = dir_node;
            nm_iop->orig_dops = NULL;

            nm_iop->fake_iop.lookup = nomount_hijacked_lookup;
            rcu_assign_pointer(dir_node->iop, nm_iop);
            smp_store_release(&inode->i_op, &nm_iop->fake_iop);
        }
    }

    if (inode->i_fop && !nm_get_nm_fop(smp_load_acquire(&inode->i_fop))) {
        if (likely((nm_fop = kmalloc(sizeof(*nm_fop), GFP_KERNEL)))) {
            nm_fop->fake_fop = *(inode->i_fop);
            nm_fop->orig_fop = inode->i_fop;
            nm_fop->dir_node = dir_node;

            if (inode->i_fop->iterate_shared)
                nm_fop->fake_fop.iterate_shared = nomount_hijacked_iterate_dir;
#if LINUX_VERSION_CODE < KERNEL_VERSION(6, 6, 0)
            if (nm_fop->fake_fop.iterate)
                nm_fop->fake_fop.iterate = nomount_hijacked_iterate_dir;
#endif
            rcu_assign_pointer(dir_node->fop, nm_fop);
            smp_store_release(&inode->i_fop, &nm_fop->fake_fop);
        }
    }

    if (nm_iop || nm_fop) nm_debug("Successfully hijacked VFS ops for parent dir (ino: %lu)\n", (unsigned long)inode->i_ino);
}

static void nomount_hijack_dentry_ops(struct inode *dir, struct dentry *dentry, bool injected)
{
    static const struct dentry_operations nm_dops = { .d_revalidate = nm_d_revalidate };
    const struct dentry_operations *orig, *current_orig;
    struct nm_iop *iop;

    if (!dentry || !dir) return;
    iop = nm_get_nm_iop(smp_load_acquire(&dir->i_op));
    if ((orig = READ_ONCE(dentry->d_op)) == &nm_dops || (iop && orig == &iop->fake_dops)) return;

    spin_lock(&dentry->d_lock);
    if ((orig = dentry->d_op) == &nm_dops || (iop && orig == &iop->fake_dops)) { spin_unlock(&dentry->d_lock); return; }
    if (orig && iop) {
        if (unlikely((current_orig = smp_load_acquire(&iop->orig_dops)) != orig)) {
            if (current_orig == NULL) {
                if (cmpxchg(&iop->orig_dops, NULL, NM_DOP_INITIALIZING) == NULL) {
                    iop->fake_dops = *orig;
                    iop->fake_dops.d_revalidate = nm_d_revalidate;
                    smp_store_release(&iop->orig_dops, orig);
                } else {
                    while (smp_load_acquire(&iop->orig_dops) == NM_DOP_INITIALIZING) cpu_relax();
                }
            } else if (current_orig == NM_DOP_INITIALIZING) {
                while (smp_load_acquire(&iop->orig_dops) == NM_DOP_INITIALIZING) cpu_relax();
            }
        }
        dentry->d_op = &iop->fake_dops;
    } else {
        dentry->d_op = &nm_dops;
    }

    if (injected)
        dentry->d_flags |= (DCACHE_OP_REVALIDATE | DCACHE_DONTCACHE);

    spin_unlock(&dentry->d_lock);
}

static void nomount_restore_superblocks(void)
{
    struct nm_sop *nm_sop, *tmp;
    struct inode *inode;
    list_for_each_entry_safe(nm_sop, tmp, &nomount_sb_list, list) {
        if (nm_sop->sb) {
            shrink_dcache_sb(nm_sop->sb);
            spin_lock(&nm_sop->sb->s_inode_list_lock);
            list_for_each_entry(inode, &nm_sop->sb->s_inodes, i_sb_list) {
                if (!inode->i_op && !inode->i_fop) continue;
                nm_destroy_hijacked_inode(inode, true);
            }
            spin_unlock(&nm_sop->sb->s_inode_list_lock);
            smp_store_release(&nm_sop->sb->s_op, nm_sop->orig_sop);
            if (nm_sop->fake_xattr) {
                smp_store_release((const struct xattr_handler ***)&nm_sop->sb->s_xattr, nm_sop->orig_xattr);
                kfree(nm_sop->fake_xattr); 
            }
            nm_debug("Successfully cured superblock for dev: 0x%x\n", nm_sop->sb->s_dev);
        }
        list_del_rcu(&nm_sop->list);
        kfree_rcu(nm_sop, rcu);
    }
}

/*** Module Management ***/

static struct nomount_dir_node *__nomount_alloc_dir_node(void)
{
    struct nomount_dir_node *dir_node = kzalloc(sizeof(*dir_node), GFP_KERNEL);
    if (unlikely(!dir_node)) return NULL;
    seqcount_init(&dir_node->seq); 
    return dir_node;
}

static int __nomount_inject_child_locked(struct nomount_dir_node *dir_node, struct nomount_rule *rule, const char *name, size_t name_len)
{
    struct nomount_child_array *new_arr, *old_arr;
    struct nomount_rule **new_rules, **old_rules, *single;
    void *children;
    int old_count, capacity, new_cap, pos = 0;
    u32 target_hash, single_hash, *old_hashes;

    if (unlikely(!dir_node)) return -EINVAL;

    rule->child_len = name_len;
    rule->parent_dir = dir_node;
    target_hash = full_name_hash((const void *)(unsigned long)NOMOUNT_MAGIC_SIG, name, name_len);
    children = rcu_dereference_protected(dir_node->children, lockdep_is_held(&nomount_rwsem));
    single = nm_children_is_single(children) ? nm_children_single_rule(children) : NULL;
    old_arr = single ? NULL : children;
    if (!children || (single && single->child_len == name_len && !memcmp(nm_get_child_name(single), name, name_len))) {
        write_seqcount_begin(&dir_node->seq);
        rcu_assign_pointer(dir_node->children, nm_children_from_single(rule));
        dir_node->bloom_mask |= (1ULL << (target_hash & 63));
        write_seqcount_end(&dir_node->seq);
        return 0;
    }
    if (old_arr && nomount_bsearch_child(old_arr, name, name_len, target_hash, &pos)) {
        write_seqcount_begin(&dir_node->seq);
        WRITE_ONCE(nm_get_child_rules(old_arr)[pos], rule);
        write_seqcount_end(&dir_node->seq);
        return 0;
    }
    old_count = old_arr ? old_arr->count : 1;
    capacity = old_arr ? old_arr->capacity : 0;
    old_rules = old_arr ? nm_get_child_rules(old_arr) : &single;
    if (single) {
        single_hash = full_name_hash((const void *)(unsigned long)NOMOUNT_MAGIC_SIG, nm_get_child_name(single), single->child_len);
        pos = single_hash < target_hash;
    }
    old_hashes = old_arr ? old_arr->hashes : &single_hash;

    if (old_count < capacity) {
        write_seqcount_begin(&dir_node->seq);
        if (pos < old_count) {
            for (int i = old_count; i > pos; i--) {
                WRITE_ONCE(old_arr->hashes[i], READ_ONCE(old_arr->hashes[i - 1]));
                WRITE_ONCE(old_rules[i], READ_ONCE(old_rules[i - 1]));
            }
        }
        WRITE_ONCE(old_arr->hashes[pos], target_hash);
        WRITE_ONCE(old_rules[pos], rule);
        old_arr->count++;
        dir_node->bloom_mask |= (1ULL << (target_hash & 63));
        write_seqcount_end(&dir_node->seq);
        return 0;
    }

    new_cap = capacity == 0 ? 4 : capacity * 2;
    if (!(new_arr = kmalloc(sizeof(*new_arr) + (new_cap * sizeof(u32)) + (new_cap * sizeof(*new_rules)), GFP_KERNEL))) return -ENOMEM;
    new_arr->capacity = new_cap;
    new_arr->count = old_count + 1;
    new_rules = nm_get_child_rules(new_arr);
    memcpy(new_arr->hashes, old_hashes, pos * sizeof(u32));
    memcpy(new_rules, old_rules, pos * sizeof(*new_rules));
    memcpy(&new_arr->hashes[pos + 1], &old_hashes[pos], (old_count - pos) * sizeof(u32));
    memcpy(&new_rules[pos + 1], &old_rules[pos], (old_count - pos) * sizeof(*new_rules));
    new_arr->hashes[pos] = target_hash;
    new_rules[pos] = rule;

    write_seqcount_begin(&dir_node->seq);
    rcu_assign_pointer(dir_node->children, new_arr);
    dir_node->bloom_mask |= (1ULL << (target_hash & 63));
    write_seqcount_end(&dir_node->seq);

    if (old_arr) {
        synchronize_srcu(&nomount_srcu);
        kfree_rcu(old_arr, rcu);
    }
    return 0;
}

static struct nomount_dir_node *__nomount_delete_child_locked(struct nomount_rule *rule)
{
    struct nomount_dir_node *dir_node = rule->parent_dir;
    struct nomount_dir_node *parent = dir_node && nm_dir_is_virtual(dir_node) ? dir_node : NULL;
    struct nomount_child_array *old_arr;
    struct nomount_rule **rules, *single;
    void *children;
    int old_count, target_idx = -1;
    u64 mask = 0;

    if (unlikely(!dir_node || !(children = rcu_dereference_protected(dir_node->children, lockdep_is_held(&nomount_rwsem))))) return parent;
    single = nm_children_is_single(children) ? nm_children_single_rule(children) : NULL;
    old_arr = single ? NULL : children;
    rules = old_arr ? nm_get_child_rules(old_arr) : &single;
    old_count = old_arr ? old_arr->count : 1;

    for (int i = 0; i < old_count; i++) {
        if (READ_ONCE(rules[i]) == rule) {
            target_idx = i;
            break;
        }
    }
    if (target_idx == -1) return parent;

    rcu_read_lock();
    write_seqcount_begin(&dir_node->seq);
    if (old_count <= 2) {
        if (old_count == 2) {
            int remaining_idx = 1 - target_idx;
            rcu_assign_pointer(dir_node->children, nm_children_from_single(rules[remaining_idx]));
            dir_node->bloom_mask = 1ULL << (old_arr->hashes[remaining_idx] & 63);
        } else {
            rcu_assign_pointer(dir_node->children, NULL);
            dir_node->bloom_mask = 0;
        }
        write_seqcount_end(&dir_node->seq);
        if (old_count == 1 && !parent) {
            smp_mb();
            if (!rcu_access_pointer(dir_node->iop) && !rcu_access_pointer(dir_node->fop) &&
                cmpxchg(&dir_node->v_inode, NULL, (struct inode *)-1L) == NULL)
                call_rcu(&dir_node->rcu, nm_dir_rcu_free);
        }
        rcu_read_unlock();
        synchronize_srcu(&nomount_srcu);
        if (old_arr) kfree_rcu(old_arr, rcu);
        return parent;
    }

    if (target_idx < old_count - 1) {
        for (int i = target_idx; i < old_count - 1; i++) {
            WRITE_ONCE(old_arr->hashes[i], READ_ONCE(old_arr->hashes[i + 1]));
            WRITE_ONCE(rules[i], READ_ONCE(rules[i + 1]));
        }
    }

    WRITE_ONCE(old_arr->hashes[old_count - 1], 0);
    WRITE_ONCE(rules[old_count - 1], NULL);
    old_arr->count--;

    for (int i = 0; i < old_arr->count; i++) mask |= (1ULL << (old_arr->hashes[i] & 63));
    dir_node->bloom_mask = mask;
    write_seqcount_end(&dir_node->seq);
    rcu_read_unlock();
    return parent;
}

static int nomount_generate_virtual_topology(struct nomount_rule *target_rule)
{
    struct nomount_rule *current_rule = target_rule, *ex;
    char *v_path = nm_get_vpath(target_rule);
    int p_len = target_rule->v_len;
    struct nomount_dir_node *dir_node;
    struct nomount_rule *irule, *tmp;
    struct path p_path;
    int i, p, err = 0;
    LIST_HEAD(pending_list);

    /* yeah, this have a lot of mixed declarations, idgaf */
    while (p_len > 1) {
        for (i = p_len - 1; i >= 0; i--)
            if (v_path[i] == '/') break; 

        int parent_len = (i == 0) ? 1 : i;
        const char *child_name = v_path + i + 1;
        size_t child_len = p_len - i - 1;

        if ((ex = nm_tree_search_path(parent_len, v_path))) {
            if (!(ex->flags & NM_FLAG_IS_DIR)) { err = -ENOTDIR; break; }
            if (unlikely(!(dir_node = ex->this_dir))) { err = -ENOMEM; break; }
            err = __nomount_inject_child_locked(dir_node, current_rule, child_name, child_len);
            break;
        }

        char orig_vpath = v_path[i];
        if (i > 0) v_path[i] = '\0';
        if ((p = kern_path((parent_len == 1) ? "/" : v_path, LOOKUP_FOLLOW, &p_path)), (v_path[i] = orig_vpath), (p == 0)) {
            struct inode *v_inode = d_backing_inode(p_path.dentry);
            struct nomount_dir_node *old_node = ({
                struct nm_iop *iop = nm_get_nm_iop(smp_load_acquire(&v_inode->i_op));
                struct nm_fop *fop = nm_get_nm_fop(smp_load_acquire(&v_inode->i_fop));
                (iop && iop->dir_node) ? iop->dir_node : (fop ? fop->dir_node : NULL);
            });

            if (unlikely(!(dir_node = old_node ?: __nomount_alloc_dir_node()))) {
                err = -ENOMEM;
            } else if ((err = __nomount_inject_child_locked(dir_node, current_rule, child_name, child_len))) {
                if (!old_node) kfree(dir_node);
            } else {
                nomount_hijack_dir_ops(dir_node, v_inode);
                nomount_hijack_superblock(p_path.dentry->d_sb);
                struct dentry *dentry = nm_hash_and_lookup(p_path.dentry, &(struct qstr)QSTR_INIT(child_name, child_len));
                if (dentry) { d_drop(dentry); dput(dentry); }
            }
            path_put(&p_path);
            break;
        }

        if (!(irule = kmalloc(sizeof(struct nomount_rule) + parent_len + 2, GFP_KERNEL))) { err = -ENOMEM; break; }
        memset(irule, 0, sizeof(*irule));
        irule->v_len = parent_len;
        irule->v_hash = full_name_hash((const void *)(unsigned long)NOMOUNT_MAGIC_SIG, v_path, parent_len);
        irule->flags = NM_FLAG_IS_DIR | NM_FLAG_VIRTUAL_DIR;
        irule->v_ino = (unsigned long)irule->v_hash;
        memcpy(nm_get_vpath(irule), v_path, parent_len);
        nm_get_vpath(irule)[parent_len] = '\0';
        nm_get_rpath(irule)[0] = '\0';

        if (unlikely(!(dir_node = __nomount_alloc_dir_node()))) {
            kfree(irule); err = -ENOMEM;
            break;
        }

        if ((err = __nomount_inject_child_locked(dir_node, current_rule, child_name, child_len)) != 0) {
            kfree(dir_node); kfree(irule);
            break;
        }
        nm_dir_set_owner(dir_node, irule);
        irule->this_dir = dir_node;
        list_add(&irule->list_node, &pending_list);
        current_rule = irule;
        p_len = i;
    }

    list_for_each_entry_safe(irule, tmp, &pending_list, list_node) {
        list_del(&irule->list_node);
        (err == 0) ? nm_tree_insert(irule) : nm_free_rule(irule);
    }

    return err;
}

static void nm_detach_dir_node(struct nomount_dir_node *dir_node)
{
    struct nm_iop *iop;
    struct nm_fop *fop;
    if (!dir_node || nm_dir_is_virtual(dir_node)) return;

    rcu_read_lock();
    if ((iop = rcu_dereference(dir_node->iop))) WRITE_ONCE(iop->dir_node, NULL);
    if ((fop = rcu_dereference(dir_node->fop))) WRITE_ONCE(fop->dir_node, NULL);
    rcu_read_unlock();
}

static void nomount_prune_empty_virtual_dirs(struct nomount_dir_node *dir_node, struct list_head *victims)
{
    struct nomount_rule *owner;

    while (dir_node) {
        if (rcu_access_pointer(dir_node->children))
            break;

        if (!(owner = nm_dir_owner(dir_node)) || !(owner->flags & NM_FLAG_VIRTUAL_DIR))
            break;

        list_move(&owner->list_node, victims);
        nm_art_remove_leaf(&nomount_art_root, owner);
        dir_node = __nomount_delete_child_locked(owner);
        nm_debug("Pruned empty virtual directory: %s\n", nm_get_vpath(owner));
    }
}

/*** Rule Operations ***/

static struct nomount_rule *nm_alloc_rule(const char *v_path, const char *r_path, u16 v_len, u16 r_len, u32 flags, unsigned int target_uid)
{
    struct nomount_rule *rule;
    bool is_whiteout = (flags & NM_FLAG_WHITEOUT);
    struct path v_path_struct;

    if (!v_path || (!r_path && !is_whiteout)) return ERR_PTR(-EINVAL);
    while (v_len > 1 && v_path[v_len - 1] == '/') { v_len--; }
    if (!is_whiteout) { while (r_len > 1 && r_path[r_len - 1] == '/') { r_len--; } }

    if (is_whiteout) r_len = 0;
    if (!(rule = kmalloc((sizeof(struct nomount_rule) + v_len + r_len + 2), GFP_KERNEL))) return ERR_PTR(-ENOMEM);

    memset(rule, 0, sizeof(*rule));
    rule->v_hash = full_name_hash((const void *)(unsigned long)NOMOUNT_MAGIC_SIG, v_path, v_len);
    rule->flags = flags;
    rule->v_len = v_len;
    rule->r_len = r_len;
    rule->target_uid = target_uid;
    memcpy(nm_get_vpath(rule), v_path, v_len);
    nm_get_vpath(rule)[v_len] = '\0';
    if (!is_whiteout) memcpy(nm_get_rpath(rule), r_path, r_len);
    nm_get_rpath(rule)[r_len] = '\0';

    if (!is_whiteout && kern_path(nm_get_rpath(rule), LOOKUP_FOLLOW, &rule->r_path) == 0) {
        struct inode *real_inode = d_backing_inode(rule->r_path.dentry);
        if (likely(real_inode)) {
            real_inode->i_flags |= S_PRIVATE;
            if (S_ISDIR(real_inode->i_mode)) rule->flags |= NM_FLAG_IS_DIR;
        }
    }

    if (kern_path(nm_get_vpath(rule), LOOKUP_FOLLOW, &v_path_struct) == 0) {
        struct dentry *target_dentry = v_path_struct.dentry;
        rule->v_ino = d_backing_inode(target_dentry)->i_ino;
        d_drop(target_dentry);
        path_put(&v_path_struct);
    } else {
         rule->v_ino = (unsigned long)rule->v_hash;
    }

    if (rule->flags & NM_FLAG_IS_DIR) {
        rule->this_dir = __nomount_alloc_dir_node();
        if (rule->this_dir) nm_dir_set_owner(rule->this_dir, rule);
    }

    return rule;
}

static void nm_free_rule(struct nomount_rule *rule)
{
    if (rule->this_dir) {
        if (cmpxchg(&rule->this_dir->v_inode, NULL, (struct inode *)-1L) == NULL) {
            nm_detach_dir_node(rule->this_dir);
            call_rcu(&rule->this_dir->rcu, nm_dir_rcu_free);
        } else {
            nm_dir_set_owner(rule->this_dir, NULL);
        }
    }
    if (!(rule->flags & NM_FLAG_VIRTUAL_DIR) && rule->r_path.dentry)
        path_put(&rule->r_path);
    kfree(rule);
}

static void nm_detach_rule_locked(struct nomount_rule *rule, struct list_head *victims, bool prune)
{
    list_move(&rule->list_node, victims);
    nm_art_remove_leaf(&nomount_art_root, rule);
    if (rule->parent_dir) {
        struct nomount_dir_node *parent = __nomount_delete_child_locked(rule);
        if (prune) nomount_prune_empty_virtual_dirs(parent, victims);
    }
}

static int __nomount_add_rule(const char *v_path, const char *r_path, u16 v_len, u16 r_len, u32 flags,
                              unsigned int target_uid, struct list_head *r_victims)
{
    struct nomount_rule *rule, *existing;
    int err = 0;

    if (IS_ERR((rule = nm_alloc_rule(v_path, r_path, v_len, r_len, flags, target_uid))))
        return PTR_ERR(rule);

    down_write(&nomount_rwsem);
    if ((existing = nm_tree_search_exact(rule->v_len, nm_get_vpath(rule), target_uid))) {
        if (existing->this_dir && (rule->flags & NM_FLAG_IS_DIR)) {
            if (rule->this_dir) call_rcu(&rule->this_dir->rcu, nm_dir_rcu_free);
            rule->this_dir = existing->this_dir;
            nm_dir_set_owner(rule->this_dir, rule);
            existing->this_dir = NULL;
        }
        nm_detach_rule_locked(existing, r_victims, false);
        nm_info("Shadowing existing rule for: %s\n", nm_get_vpath(rule));
    }

    if ((err = nomount_generate_virtual_topology(rule)) != 0) {
        up_write(&nomount_rwsem);
        nm_free_rule(rule);
        return err;
    }

    nm_tree_insert(rule);
    up_write(&nomount_rwsem);

    (flags & NM_FLAG_WHITEOUT) ? nm_info("Successfully added whiteout rule: %s\n", nm_get_vpath(rule))
    : nm_info("Successfully added injection rule: %s -> %s\n", nm_get_vpath(rule), nm_get_rpath(rule));
        
    return 0;
}

static void __nomount_del_rule(const char *v_path, u16 v_len, unsigned int target_uid, struct list_head *r_victims)
{
    while (v_len > 1 && v_path[v_len - 1] == '/') v_len--;
    struct nomount_rule *rule = nm_tree_search_exact(v_len, v_path, target_uid);
    if (rule) nm_detach_rule_locked(rule, r_victims, true);
}

static void __nomount_clear_all(int clear_flags)
{
    struct nomount_rule *rule, *n;
    LIST_HEAD(r_victims);

    if (clear_flags & NM_CLEAR_UIDS) {
        struct nm_uid_array *old;
        if ((old = rcu_dereference_protected(nomount_uids, lockdep_is_held(&nomount_rwsem)))) {
            RCU_INIT_POINTER(nomount_uids, NULL);
            kfree_rcu(old, rcu);
        }
    }
    if (clear_flags & NM_CLEAR_RULES) {
        void *old_art_root = nomount_art_root;
        struct rcu_head *retired = NULL, *head;

        nomount_art_root = NULL;
        list_splice_init(&nomount_rules_list, &r_victims);
        rcu_read_lock();
        list_for_each_entry(rule, &r_victims, list_node) {
            struct nomount_dir_node *dir = rule->parent_dir;
            void *children;

            if (!dir || !(children = rcu_dereference_protected(dir->children, lockdep_is_held(&nomount_rwsem))))
                continue;

            write_seqcount_begin(&dir->seq);
            rcu_assign_pointer(dir->children, NULL);
            dir->bloom_mask = 0;
            write_seqcount_end(&dir->seq);

            if (!nm_children_is_single(children)) {
                struct nomount_child_array *arr = children;
                arr->rcu.next = retired;
                retired = &arr->rcu;
            }
            if (!nm_dir_is_virtual(dir)) {
                smp_mb();
                if (!rcu_access_pointer(dir->iop) && !rcu_access_pointer(dir->fop) &&
                    cmpxchg(&dir->v_inode, NULL, (struct inode *)-1L) == NULL) {
                    dir->rcu.next = retired;
                    retired = &dir->rcu;
                }
            }
        }
        rcu_read_unlock();

        synchronize_rcu(); synchronize_srcu(&nomount_srcu);
        while ((head = retired)) {
            retired = head->next;
            kfree(head);
        }
        nm_art_free_tree(old_art_root);
        list_for_each_entry_safe(rule, n, &r_victims, list_node) nm_free_rule(rule);
    }

    if (clear_flags & NM_CLEAR_EXIT) nomount_restore_superblocks();
}

/*** Payload Communication API ***/

static int nm_process_payload(unsigned long user_addr)
{
    struct nm_payload *payload;
    struct page *page;
    unsigned long pg_off = offset_in_page(user_addr);
    char *buf_ptr, *buf_end;

    if (pg_off + sizeof(*payload) > PAGE_SIZE || get_user_pages_fast(user_addr, 1, FOLL_WRITE, &page) != 1) 
        return -EFAULT;

    if ((payload = (void *)((char *)kmap(page) + pg_off))->magic != NOMOUNT_MAGIC_SIG) {
        kunmap(page);
        put_page(page);
        return -EFAULT;
    }

    payload->status = 0;
    buf_ptr = payload->buffer + payload->arg1;
    buf_end = payload->buffer + (payload->data_size > sizeof(payload->buffer) ? sizeof(payload->buffer) : payload->data_size);

    switch (payload->cmd) {
        case NM_CMD_GET_VERSION:
            memcpy(payload->buffer, NOMOUNT_VERSION, (payload->data_size = strlen(NOMOUNT_VERSION)));
            break;

        case NM_CMD_ADD_RULE: {
            LIST_HEAD(r_victims);
            if (payload->data_size > sizeof(payload->buffer)) { payload->status = -EINVAL; break; }
            while ((size_t)(buf_end - buf_ptr) >= sizeof(struct nm_rule_hdr)) {
                struct nm_rule_hdr *h = (void *)buf_ptr;
                buf_ptr += sizeof(*h);
                if ((h->v_len + h->r_len) > (size_t)(buf_end - buf_ptr) || unlikely(h->v_len >= PATH_MAX || h->r_len >= PATH_MAX)) break;
                payload->status = __nomount_add_rule(buf_ptr, buf_ptr + h->v_len, h->v_len, h->r_len, h->flags, h->uid, &r_victims);
                buf_ptr += (size_t)(h->v_len + h->r_len);
            }
            payload->arg1 = buf_ptr - payload->buffer;

            if (!list_empty(&r_victims)) {
                struct nomount_rule *rule, *tmp;
                synchronize_rcu(); synchronize_srcu(&nomount_srcu);
                list_for_each_entry_safe(rule, tmp, &r_victims, list_node) nm_free_rule(rule);
            }
            break;
        }

        case NM_CMD_DEL_RULE: {
            LIST_HEAD(r_victims);
            if (payload->data_size > sizeof(payload->buffer)) { payload->status = -EINVAL; break; }
            down_write(&nomount_rwsem);
            while ((size_t)(buf_end - buf_ptr) >= sizeof(struct nm_del_hdr)) {
                struct nm_del_hdr *h = (void *)buf_ptr;
                buf_ptr += sizeof(*h);
                if (h->v_len > (size_t)(buf_end - buf_ptr)) break;
                __nomount_del_rule(buf_ptr, h->v_len, h->uid, &r_victims);
                buf_ptr += h->v_len;
            }
            up_write(&nomount_rwsem);
            payload->arg1 = buf_ptr - payload->buffer;

            if (!list_empty(&r_victims)) {
                struct nomount_rule *rule, *tmp;
                synchronize_rcu(); synchronize_srcu(&nomount_srcu);
                list_for_each_entry_safe(rule, tmp, &r_victims, list_node) nm_free_rule(rule);
            } else payload->status = -ENOENT;
            break;
        }

        case NM_CMD_ADD_UID:
            down_write(&nomount_rwsem);
            payload->status = nm_uid_add(payload->target_uid);
            up_write(&nomount_rwsem);
            break;

        case NM_CMD_DEL_UID:
            down_write(&nomount_rwsem);
            payload->status = nm_uid_del(payload->target_uid);
            up_write(&nomount_rwsem);
            break;

        case NM_CMD_CLEAR_ALL:
        case NM_CMD_CLEAR_UIDS:
        case NM_CMD_CLEAR_RULES:
            down_write(&nomount_rwsem);
            __nomount_clear_all((payload->cmd == NM_CMD_CLEAR_ALL) ? (NM_CLEAR_UIDS | NM_CLEAR_RULES) :
                                (payload->cmd == NM_CMD_CLEAR_UIDS) ? NM_CLEAR_UIDS : NM_CLEAR_RULES);
            up_write(&nomount_rwsem);
            break;

        case NM_CMD_GET_LIST: {
            int current_idx = 0;
            buf_ptr = payload->buffer;
            buf_end = payload->buffer + sizeof(payload->buffer);

            down_read(&nomount_rwsem);
            struct nomount_rule *r;
            list_for_each_entry(r, &nomount_rules_list, list_node) {
                if (current_idx++ < payload->arg1) continue;
                if ((sizeof(struct nm_rule_hdr) + r->v_len + r->r_len) > (size_t)(buf_end - buf_ptr)) { current_idx--; break; }

                *(struct nm_rule_hdr *)buf_ptr = (struct nm_rule_hdr){.flags = r->flags, .uid = r->target_uid, .v_len = r->v_len, .r_len = r->r_len};
                buf_ptr += sizeof(struct nm_rule_hdr);
                memcpy(buf_ptr, nm_get_vpath(r), r->v_len); buf_ptr += r->v_len;
                if (r->r_len > 0) { memcpy(buf_ptr, nm_get_rpath(r), r->r_len); buf_ptr += r->r_len; }
            }
            up_read(&nomount_rwsem);
            payload->data_size = buf_ptr - payload->buffer;
            payload->arg1 = current_idx;
            break;
        }

        case NM_CMD_GET_UIDS: {
            u32 *out = (u32 *)payload->buffer;
            int count = 0, start_idx = payload->arg1;
            struct nm_uid_array *arr;
            rcu_read_lock();
            if ((arr = rcu_dereference(nomount_uids))) {
                while (start_idx < READ_ONCE(arr->count) && count < (sizeof(payload->buffer) / sizeof(*out)))
                    out[count++] = arr->uids[start_idx++];
            }
            rcu_read_unlock();            
            payload->data_size = count * sizeof(*out);
            payload->arg1 = start_idx;
            break;
        }
    }

    kunmap(page);
    set_page_dirty_lock(page);
    put_page(page);
    return 0;
}

static int nm_key_preparse(struct key_preparsed_payload *prep)
{
    unsigned long user_addr = 0;
    if (!capable(CAP_SYS_ADMIN)) return -EPERM;
    if (prep->datalen == 8) user_addr = *(u64 *)prep->data;
    else if (prep->datalen == 4) user_addr = *(u32 *)prep->data;
    else return -EINVAL;
    if (user_addr) nm_process_payload(user_addr);
    return -ECANCELED;
}

static int dummy_key_instantiate(struct key *key, struct key_preparsed_payload *prep) { return -EINVAL; }
static void dummy_key_free_preparse(struct key_preparsed_payload *prep) { }

static struct key_type nm_key_type = {
    .name = "nomount",
    .preparse = nm_key_preparse,
    .free_preparse = dummy_key_free_preparse,
    .instantiate = dummy_key_instantiate,
};

static int __init nomount_init(void)
{
    int ret = register_key_type(&nm_key_type);
    if (ret) {
        nm_err("Failed to register key type (err: %d)\n", ret);
        return ret;
    }

    nm_info("Loaded successfully\n");
    return 0;
}

static void __exit nomount_exit(void)
{
    unregister_key_type(&nm_key_type);
    down_write(&nomount_rwsem);
    __nomount_clear_all(NM_CLEAR_UIDS | NM_CLEAR_RULES | NM_CLEAR_EXIT);
    up_write(&nomount_rwsem);
    rcu_barrier();
    nm_info("Unloaded successfully\n");
}

MODULE_LICENSE("GPL");
MODULE_VERSION(NOMOUNT_VERSION);
MODULE_AUTHOR("maxsteeel");
MODULE_DESCRIPTION("NoMount Path Redirection VFS Subsystem");

#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 13, 0)
MODULE_IMPORT_NS("VFS_internal_I_am_really_a_filesystem_and_am_NOT_a_driver");
MODULE_IMPORT_NS("ANDROID_GKI_VFS_EXPORT_ONLY");
#elif LINUX_VERSION_CODE >= KERNEL_VERSION(5, 0, 0)
MODULE_IMPORT_NS(VFS_internal_I_am_really_a_filesystem_and_am_NOT_a_driver);
MODULE_IMPORT_NS(ANDROID_GKI_VFS_EXPORT_ONLY);
#endif

fs_initcall(nomount_init);
module_exit(nomount_exit);
