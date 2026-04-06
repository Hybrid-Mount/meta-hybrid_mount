// SPDX-License-Identifier: GPL-2.0-only
// nuke_ext4_sysfs KPM for APatch/KernelPatch.

#include <linux/kernel.h>
#include <linux/errno.h>
#include <linux/string.h>
#include <linux/namei.h>
#include <linux/path.h>
#include <linux/fs.h>

#include <kpmodule.h>

KPM_NAME("nuke_ext4_sysfs");
KPM_VERSION("0.1.0");
KPM_LICENSE("GPL v2");
KPM_AUTHOR("Hybrid Mount Developers");
KPM_DESCRIPTION("Expose nuke_ext4_sysfs for Hybrid Mount in APatch env");

extern void ext4_unregister_sysfs(struct super_block *sb);

static long do_nuke_ext4_sysfs(const char *path) {
    struct path fs_path;
    struct super_block *sb;
    const char *name;
    int err;

    if (!path || !path[0]) {
        return -EINVAL;
    }

    err = kern_path(path, 0, &fs_path);
    if (err) {
        pr_err("[hm-kpm] kern_path failed for %s: %d\n", path, err);
        return err;
    }

    sb = fs_path.dentry->d_inode->i_sb;
    name = sb->s_type->name;
    if (strcmp(name, "ext4") != 0) {
        pr_info("[hm-kpm] skip non-ext4 target: %s (%s)\n", path, name);
        path_put(&fs_path);
        return -EINVAL;
    }

    ext4_unregister_sysfs(sb);
    path_put(&fs_path);
    pr_info("[hm-kpm] nuke done: %s\n", path);
    return 0;
}

static long hm_control(const char *args, char *out_msg, int outlen) {
    long rc = do_nuke_ext4_sysfs(args);

    if (out_msg && outlen > 0) {
        scnprintf(out_msg, outlen, "rc=%ld", rc);
    }
    return rc;
}

static long hm_syscall(const char *arg0, const char *arg1, const char *arg2,
                       const char *arg3, const char *arg4,
                       const char *arg5, const char *arg6) {
    (void)arg1;
    (void)arg2;
    (void)arg3;
    (void)arg4;
    (void)arg5;
    (void)arg6;

    return do_nuke_ext4_sysfs(arg0);
}

KPM_INIT(hm_init) {
    pr_info("[hm-kpm] init\n");
    return 0;
}

KPM_CTL0(hm_control);
KPM_SYSCALL(hm_syscall);

KPM_EXIT(hm_exit) {
    pr_info("[hm-kpm] exit\n");
}
