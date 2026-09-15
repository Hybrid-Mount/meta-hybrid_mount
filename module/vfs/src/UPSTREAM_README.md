# NoMount - Kernel Integration

This section contains everything related to integrating NoMount into your custom kernel. You can integrate it natively (`Built-in`) or compile it as a Loadable Kernel Module (`LKM`).

> **Note for GKI Users:** You generally do not need to do this. Pre-compiled LKMs are already provided in the NoMount release ZIP. Only proceed if you are on a legacy kernel, or a custom kernel that blocks standard LKMs (like Sultan kernels).

## Method 1: Automatic Built-in Integration (Recommended)

To seamlessly patch NoMount into your kernel source so it can be compiled built-in (`=y`) or LKM (`=m`), run the setup script at the root of your kernel tree:

```bash
curl https://raw.githubusercontent.com/maxsteeel/nomount/refs/heads/dev/kernel/setup.sh | bash -
```

If you want to integrate a specific branch, you can specific it with `bash -s <branch>`. For example:
```bash
# To integrate the dev branch
curl https://raw.githubusercontent.com/maxsteeel/nomount/refs/heads/dev/kernel/setup.sh | bash -s dev
```

Once applied, simply compile your kernel as usual.

## Method 2: Compile as LKM

If you prefer to compile NoMount as an out-of-tree module (`nomount.ko`) for your specific kernel, follow these steps:

1. **Pull the NoMount source into your kernel tree:**
(If you already have NoMount integrated in your kernel, you can skip this step).
```bash
curl https://raw.githubusercontent.com/maxsteeel/nomount/refs/heads/dev/kernel/setup.sh | bash -s dev
```

2. **Prepare the kernel headers:**
```bash
make <compiler arguments> O=out modules_prepare
```

3. **Compile the LKM:**
```bash
CONFIG_NOMOUNT=m make <compiler arguments> -C $(pwd)/out M=$(pwd)/fs/nomount/ modules
```

The compiled LKM will be located at `fs/nomount/nomount.ko`.
Rename it to match your kernel version format (e.g., `nomount-5.4.ko` or `nomount-android14-6.1.ko`) and place it inside the `lkm/` folder of the NoMount release ZIP before flashing it via KernelSU/APatch.

## Method 3: Manual Built-in Integration

If you want to integrate the code manually without the curl script:

1. **Add to fs/Kconfig:**
```kconfig
source "fs/nomount/Kconfig"
```

2. **Add to fs/Makefile:**
```make
obj-$(CONFIG_NOMOUNT) += nomount/
```

3. **Copy the necessary files:**
Transfer the NoMount code (`src/`) to the `fs/nomount/` directory of your kernel tree.
```bash
mkdir -p <your kernel source>/fs/nomount
cp -f <path of nomount>/kernel/src/* <your kernel source>/fs/nomount
```

4. **Enable NoMount:**
Enable it in your `defconfig` or via `menuconfig`:
```kconfig
CONFIG_NOMOUNT=y
```

Then compile your kernel as usual. If you followed the steps correctly, at the end of the compilation you will have a kernel with NoMount integrated!

