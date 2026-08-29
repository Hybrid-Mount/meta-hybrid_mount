// SPDX-License-Identifier: GPL-2.0-only

#include <linux/module.h>
#include <linux/init.h>
#include <linux/fs.h>
#include <linux/path.h>
#include <linux/namei.h>
#include <linux/string.h>
#include <linux/version.h>
#include <linux/kallsyms.h>

#ifndef MODULE
#error "This is for LKM builds only. Do not compile built-in (CONFIG_NUKE_EXT4_SYSFS=y). Its bullshit."
#endif

/*
 * USAGE:
 *
 * kptr_set=$(cat /proc/sys/kernel/kptr_restrict)
 * echo 1 > /proc/sys/kernel/kptr_restrict
 * ptr_address=$(grep ext4_unregister_sysfs /proc/kallsyms | awk {'print "0x"$1'})
 * insmod nuke.ko mount_point="/data/adb/modules" symaddr="$ptr_address"
 * echo $kptr_set > /proc/sys/kernel/kptr_restrict
 *
 */

static unsigned long symaddr;
module_param(symaddr, ulong, 0000);
MODULE_PARM_DESC(symaddr, "ext4_unregister_sysfs symbol address");

static char *mount_point = "/data/adb/modules";
module_param(mount_point, charp, 0000);
MODULE_PARM_DESC(mount_point, "nuke an ext4 sysfs node");

#ifndef __nocfi
#define __nocfi
#endif

static void (*ext4_unregister_sysfs_ptr)(struct super_block *) = NULL;

static __nocfi int ext4_unregister_sysfs_fn(struct super_block *sb)
{
	const char *sym = "ext4_unregister_sysfs";
	char buf[KSYM_SYMBOL_LEN] = {0};

	if (!symaddr) {
		pr_info("mountify/nuke_ext4: symaddr not provided!\n");
		return -EINVAL;
	}

	// https://elixir.bootlin.com/linux/v6.17.1/source/kernel/kallsyms.c#L474
	// turns out we can confirm the symbol!
	sprint_symbol(buf, symaddr);
	buf[KSYM_SYMBOL_LEN - 1] = '\0';

	// if strstarts symbol
	// output is like "ext4_unregister_sysfs+0x0/0x70"
	if (!!strncmp(buf, sym, strlen(sym))) {
		pr_info("mountify/nuke_ext4: wrong symbol!? %s found!\n", buf);
		return -EINVAL;
	}

	pr_info("mountify/nuke_ext4: sprint_symbol 0x%lx: %s\n", symaddr, buf);
	ext4_unregister_sysfs_ptr = (void (*)(struct super_block *))symaddr;
	ext4_unregister_sysfs_ptr(sb);
	return 0;
}

static int __init nuke_entry(void)
{
	struct path path;
	pr_info("mountify/nuke_ext4: init with symaddr: 0x%lx mount_point: %s\n", symaddr, mount_point);

	// kang from ksu
	int err = kern_path(mount_point, 0, &path);
	if (err) {
		pr_info("mountify/nuke_ext4: kern_path failed: %d\n", err);
		return -EAGAIN;
	}

	struct super_block* sb = path.dentry->d_inode->i_sb;
	const char* name = sb->s_type->name;
	if (strcmp(name, "ext4") != 0) {
		pr_info("mountify/nuke_ext4: not ext4\n");
		path_put(&path);
		return -EAGAIN;
	}

	pr_info("mountify/nuke_ext4: unregistering sysfs node for ext4 volume (%s)\n", sb->s_id);
	int ret = ext4_unregister_sysfs_fn(sb);
	if (ret) {
		path_put(&path);
		return -EAGAIN;
	}

	// now recheck if the node still exists
	// this is on /proc/fs/ext4
	char procfs_path[64] = {0};
	snprintf(procfs_path, sizeof(procfs_path), "/proc/fs/ext4/%s", sb->s_id);

	// release ref here, we now have a copy of sb->s_id on procfs_path
	path_put(&path);

	// reuse &path
	err = kern_path(procfs_path, 0, &path);
	if (!err) {
		pr_info("mountify/nuke_ext4: procfs node still exists at %s\n", procfs_path);
		path_put(&path);
	} else
		pr_info("mountify/nuke_ext4: procfs node nuked (%s is gone)\n", procfs_path);

	pr_info("mountify/nuke_ext4: unload\n");
	return -EAGAIN;
}

static void __exit nuke_exit(void) { __builtin_unreachable(); }

module_init(nuke_entry);
module_exit(nuke_exit);

MODULE_LICENSE("GPL");
MODULE_AUTHOR("xx");
MODULE_DESCRIPTION("nuke ext4 sysfs");

#if LINUX_VERSION_CODE >= KERNEL_VERSION(5, 0, 0)
MODULE_IMPORT_NS(VFS_internal_I_am_really_a_filesystem_and_am_NOT_a_driver);
#endif
