# nuke

A small LKM to unregister an ext4 sysfs node.

Usage:

```shell
#!/bin/sh

kptr_set=$(cat /proc/sys/kernel/kptr_restrict)
echo 1 > /proc/sys/kernel/kptr_restrict
ptr_address=$(grep " ext4_unregister_sysfs$" /proc/kallsyms | awk {'print "0x"$1'})
insmod nuke.ko mount_point="/data/adb/modules" symaddr="$ptr_address"
echo $kptr_set > /proc/sys/kernel/kptr_restrict
```

Compatibility:

- Linux 4.4+.
- `CONFIG_KALLSYMS=y`

[Older version](https://github.com/backslashxx/mountify/tree/f0108a151e3be123e4aaf7319890db160692c309/nuke_ext4_lkm)
uses `kp.addr` / `kallsyms_lookup_name`.

Credits:

- KernelSU — [`de29115`](https://github.com/tiann/KernelSU/commit/de291151f1c2bd63cae1f797d938bfb14cbf2dc0)
