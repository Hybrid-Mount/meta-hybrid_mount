# SPDX-License-Identifier: GPL-2.0-only
#
# Out-of-tree build of the Hybrid Mount VFS kernel module (K2).
#
# DDK (https://github.com/Ylarod/ddk) images export KDIR, ARCH, LLVM and the
# Clang toolchain and mount the checkout at /build, so from the repository root:
#
#   ddk build --target android14-6.1 -- -C module/vfs/src
#
# Without DDK, point KDIR at a prepared kernel tree:
#
#   make -C module/vfs/src KDIR=/path/to/kernel
#
# Built-in integration is separate: setup.sh copies the sources into the kernel
# tree and writes the in-tree fragment (obj-$(CONFIG_HYBRIDMOUNT)) there.

MODULE_NAME := hybridmount
obj-m += $(MODULE_NAME).o

# Kernels before 5.18 default to gnu89, which rejects the C99 declarations used
# throughout the sources; the upstream build always set -std=gnu11.
ccflags-y += -std=gnu11 -Wno-declaration-after-statement

KDIR ?= /lib/modules/$(shell uname -r)/build
MDIR := $(realpath $(dir $(abspath $(lastword $(MAKEFILE_LIST)))))

.PHONY: all clean

all:
	@if [ ! -d "$(KDIR)" ]; then \
		echo "KDIR is not a directory: $(KDIR)" >&2; \
		echo "Build through DDK, or pass KDIR=<kernel tree>." >&2; \
		exit 1; \
	fi
	$(MAKE) -C $(KDIR) M=$(MDIR) modules

clean:
	$(MAKE) -C $(KDIR) M=$(MDIR) clean
	@rm -f "$(MDIR)/$(MODULE_NAME).ko"
