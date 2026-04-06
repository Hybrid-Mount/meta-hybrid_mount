KPM_NAME := nuke_ext4_sysfs
KPM_VERSION ?= 0.1.0
KPM_OUT := out/$(KPM_NAME)_$(KPM_VERSION).kpm

KP_DIR ?= $(HOME)/AndroidPatch/kpm

OS_NAME := $(shell uname | tr A-Z a-z)
MACHINE := $(shell uname -m)
NDK_BIN_DIR := toolchains/llvm/prebuilt/$(OS_NAME)-$(MACHINE)/bin

ifdef ANDROID_NDK_LATEST_HOME
NDK_PATH ?= $(ANDROID_NDK_LATEST_HOME)/$(NDK_BIN_DIR)
else ifdef ANDROID_NDK
NDK_PATH ?= $(ANDROID_NDK)/$(NDK_BIN_DIR)
endif

ifdef TARGET_COMPILE
CC := $(TARGET_COMPILE)gcc
STRIP := $(TARGET_COMPILE)strip
else ifdef NDK_PATH
CC := $(NDK_PATH)/aarch64-linux-android31-clang
STRIP := $(NDK_PATH)/llvm-strip
endif

CC ?= gcc
STRIP ?= strip

CFLAGS := -Wall -O2 -fno-PIC -fno-asynchronous-unwind-tables -fno-stack-protector -fno-common
INCLUDE_DIRS := . include patch/include linux/include linux/arch/arm64/include linux/tools/arch/arm64/include kpm/include
INCLUDE_FLAGS := $(foreach dir,$(INCLUDE_DIRS),-I$(KP_DIR)/kernel/$(dir))
SRC := src/$(KPM_NAME).c

.PHONY: all clean check-env

all: check-env $(KPM_OUT)

check-env:
	@test -d "$(KP_DIR)/kernel" || (echo "[ERR] KP_DIR invalid: $(KP_DIR)" && exit 1)
	@test -f "$(SRC)" || (echo "[ERR] source not found: $(SRC)" && exit 1)

$(KPM_OUT): $(SRC)
	@mkdir -p out
	@$(CC) $(CFLAGS) $(INCLUDE_FLAGS) $^ -r -o $@
	@$(STRIP) -g --strip-unneeded $@
	@echo "[OK] built $@"

clean:
	rm -rf out
