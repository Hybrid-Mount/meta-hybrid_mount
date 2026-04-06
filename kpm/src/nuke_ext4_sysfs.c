// SPDX-License-Identifier: GPL-2.0-only
// nuke_ext4_sysfs KPM for APatch/KernelPatch.

#include <linux/kernel.h>
#include <linux/errno.h>
#include <linux/string.h>
#include <linux/printk.h>

#include <kpmodule.h>

KPM_NAME("nuke_ext4_sysfs");
KPM_VERSION("0.1.0");
KPM_LICENSE("GPL v2");
KPM_AUTHOR("Hybrid Mount Developers");
KPM_DESCRIPTION("Expose nuke_ext4_sysfs for Hybrid Mount in APatch env");

static long do_nuke_ext4_sysfs(const char *path) {
    if (!path || !path[0]) {
        return -EINVAL;
    }

    pr_info("[hm-kpm] request: %s\n", path);
    return -EOPNOTSUPP;
}

static long hm_control(const char *args, char *out_msg, int outlen) {
    long rc = do_nuke_ext4_sysfs(args);

    if (out_msg && outlen > 0) {
        scnprintf(out_msg, outlen, "rc=%ld", rc);
    }
    return rc;
}

static long hm_control_nr(void *a1, void *a2, void *a3) {
    (void)a2;
    (void)a3;
    return do_nuke_ext4_sysfs((const char *)a1);
}

static long hm_init(const char *args, const char *event, void *reserved) {
    (void)args;
    (void)event;
    (void)reserved;
    pr_info("[hm-kpm] init\n");
    return 0;
}

static long hm_exit(void *reserved) {
    (void)reserved;
    pr_info("[hm-kpm] exit\n");
    return 0;
}

KPM_CTL0(hm_control);
KPM_CTL1(hm_control_nr);
KPM_INIT(hm_init);
KPM_EXIT(hm_exit);
