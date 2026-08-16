# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount là metamodule điều phối mount cho **KernelSU** và **APatch**.
Nó hợp nhất tệp của module vào các phân vùng Android thông qua một engine chính sách thống nhất với hai backend mount:

- **OverlayFS**: mount dạng lớp để ưu tiên khả năng tương thích rộng.
- **Magic Mount**: bind mount cho thay thế đường dẫn trực tiếp hoặc fallback.

**SolidJS WebUI** tích hợp sẵn cung cấp quản lý đồ họa, theo dõi trạng thái trực tiếp và chỉnh sửa cấu hình.

Gói phát hành có hai biến thể. Trừ khi được ghi rõ, README này mô tả biến thể `lite`.

**[English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)** &nbsp; **[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)** &nbsp; **[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)** &nbsp; **[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)** &nbsp; **[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)** &nbsp; **[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)** &nbsp; **[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)** &nbsp; **[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)** &nbsp; **[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md)**

---

## Mục Lục

- [Tính năng](#tính-năng)
- [Biến thể build](#biến-thể-build)
- [Bắt đầu nhanh](#bắt-đầu-nhanh)
- [Chế độ mount](#chế-độ-mount)
- [WebUI](#webui)
- [Hỗ trợ ngôn ngữ](#hỗ-trợ-ngôn-ngữ)
- [Cấu hình](#cấu-hình)
- [Tham chiếu chính sách](#tham-chiếu-chính-sách)
- [CLI](#cli)
- [Kiến trúc](#kiến-trúc)
- [Build](#build)
- [Ghi chú vận hành](#ghi-chú-vận-hành)
- [Giấy phép](#giấy-phép)

---

## Biến thể build

| Biến thể | Binary | WebUI | Daemon / CLI | Trường hợp dùng |
|----------|--------|-------|--------------|-----------------|
| **Lite** | Có | Có | Có | Người dùng cần WebUI và engine chính sách đầy đủ nhưng không cần stealth dựa trên LKM. |
| **Nano** | Có | Không | Không | Người dùng chỉ cần điều phối mount bằng tệp cấu hình, không có runtime daemon, WebUI hoặc CLI. |

### Lite

Biến thể `lite` là biến thể mặc định. Nó giữ WebUI, daemon, CLI, OverlayFS và Magic Mount. Biến thể này phù hợp khi kernel không hỗ trợ LKM bên ngoài hoặc không cần khả năng runtime hide/spoof.

### Nano

Biến thể `nano` (`--no-default-features`, không có Cargo features) chỉ dựa trên tệp cấu hình. Nó loại bỏ WebUI, daemon, CLI và hạ tầng control plane; chỉ còn một binary nhỏ đọc `config.toml`, tạo kế hoạch mount, thực thi rồi thoát.

Nano dùng `magic` làm chế độ mặc định. Khi cài đặt, lựa chọn bằng phím âm lượng sẽ ghi marker rỗng `overlay` hoặc `magic` vào thư mục gốc của từng module được quản lý. Tên marker được so khớp không phân biệt chữ hoa chữ thường.

### Ma trận tính năng

| Tính năng | Lite | Nano |
|-----------|------|------|
| Backend OverlayFS | Có | Dựa trên marker |
| Backend Magic Mount | Có | Có, mặc định |
| WebUI | Có | Không |
| CLI | Có | Không |
| Daemon | Có | Không |
| Cache cấu hình và áp dụng runtime | Có | Không |
| Cargo features | chỉ `control-plane` | không có |
| Kích thước ZIP (xấp xỉ) | ~2 MB | ~1 MB |

## Tính năng

- **Hai backend, một engine chính sách**: gán OverlayFS hoặc Magic Mount theo từng đường dẫn.
- **Lập kế hoạch xác định**: xung đột được phát hiện trong giai đoạn lập kế hoạch.
- **WebUI tích hợp**: quản lý module, chỉnh sửa cấu hình và theo dõi trạng thái runtime.
- **Cache cấu hình**: patch tăng dần và áp dụng ngay.
- **Thân thiện với khôi phục**: tự dọn tệp runtime cũ và reset bằng `api config-reset`.
- **Dễ tự động hóa**: daemon protocol JSON-over-Unix-socket và HTTP API.

---

## Bắt đầu nhanh

1. Cài [KernelSU](https://kernelsu.org/) hoặc [APatch](https://apatch.dev/) trên thiết bị.
2. Tải ZIP `lite` hoặc `nano` từ [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases).
3. Flash ZIP qua trình cài module của root manager.
4. Khởi động lại. Hybrid Mount sẽ tự phát hiện môi trường và áp dụng chính sách overlay mặc định.

```bash
# Kiểm tra trạng thái runtime
hybrid-mount daemon status

# Liệt kê module đã phát hiện
hybrid-mount api modules-list
```

Với biến thể Lite, mở WebUI từ mục module trong KernelSU hoặc APatch.

### Đổi chế độ mount cho module

```toml
# /data/adb/hybrid-mount/config.toml
[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/problematic_binary" = "ignore"
```

---

## Chế độ mount

| Chế độ | Backend | Phù hợp với |
|--------|---------|-------------|
| `overlay` | OverlayFS | Module thêm hoặc thay thế tệp không xung đột. Chế độ mặc định. |
| `magic` | Bind mount | Thay thế trực tiếp từng tệp. |
| `ignore` | Không | Loại trừ đường dẫn cụ thể khỏi xử lý mount. |

OverlayFS hỗ trợ `ext4` làm lưu trữ bền vững mặc định và `tmpfs` làm lựa chọn tạm, nhẹ hơn.
---

## WebUI

WebUI dựa trên SolidJS được daemon phục vụ qua TCP socket cục bộ với HTTP/SSE. CLI và client tự động hóa giao tiếp qua Unix socket.

Tính năng chính:

- Dashboard trạng thái với thống kê, phân vùng, storage mode và trạng thái daemon.
- Quản lý module và chỉnh policy tương tác.
- Trình chỉnh `config.toml` có kiểm tra hợp lệ và quy tắc theo đường dẫn.

### Hỗ trợ ngôn ngữ

WebUI hiện có các locale sau:

- English (`en-US`, mặc định)
- Español (`es-ES`)
- Italiano (`it-IT`)
- 日本語 (`ja-JP`)
- Русский (`ru-RU`)
- Українська (`uk-UA`)
- Tiếng Việt (`vi-VN`)
- 简体中文 (`zh-CN`)
- 繁體中文 (`zh-TW`)

Tài liệu README có sẵn bằng [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md), [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md), [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md), [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md), [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md), [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md), [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md), [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md) và [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md).

---

## Cấu hình

Đường dẫn mặc định: `/data/adb/hybrid-mount/config.toml`.

| Trường | Kiểu | Mặc định | Mô tả |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | Thư mục nguồn module. |
| `mountsource` | string | tự phát hiện | Môi trường runtime (`KSU`, `APatch`). |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Lưu trữ upper/work của OverlayFS. |
| `disable_umount` | bool | `false` | Bỏ qua umount, chỉ dùng để debug. |
| `default_mode` | `overlay` \| `magic` | `overlay` | Chính sách toàn cục mặc định. |
| `daemon_startup_mode` | `on-demand` \| `persistent` | `on-demand` | Chế độ khởi động daemon. |
| `rules` | map | `{}` | Chính sách theo module và theo đường dẫn. |

---


## Tham chiếu chính sách

Thứ tự ưu tiên:

1. Ghi đè theo đường dẫn: `rules.<module>.paths["<path>"]`
2. Mặc định theo module: `rules.<module>.default_mode`
3. Mặc định toàn cục: `default_mode`

Các marker module được nhận diện gồm `disable`, `remove`, `skip_mount`, `mount_error`, `overlay`, `magic` và `.replace`. Tên marker được so khớp không phân biệt chữ hoa chữ thường.

---

## CLI

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

Subcommand thường dùng:

- `gen-config`: tạo cấu hình mặc định.
- `logs`: in log daemon gần đây.
- `api config-get` / `api config-set` / `api config-patch` / `api config-reset`: quản lý cấu hình.
- `api modules-list` / `api modules-apply`: xem và áp dụng policy module.
- `daemon launch` / `daemon serve` / `daemon status` / `daemon stop`: quản lý daemon.

---

## Kiến trúc

Hybrid Mount đọc `config.toml`, phát hiện inventory module, tạo kế hoạch mount theo quy tắc đường dẫn, module và toàn cục, rồi thực thi bằng OverlayFS hoặc Magic Mount. Bộ thực thi được điều khiển bởi **máy trạng thái kiểu** (`src/core/controller.rs`): `MountController<Init> → StorageReady → Planned → Executed`. Mỗi chuyển đổi đại diện cho một giai đoạn pipeline. Biến thể Lite lưu trạng thái runtime và cung cấp cho WebUI cùng CLI qua daemon.

Thư mục chính:

- `src/conf`: schema cấu hình, TOML loader, CLI và handler.
- `src/domain`: kiểu lõi, quy tắc và khớp đường dẫn.
- `src/core`: inventory, lập kế hoạch, daemon, API, startup và runtime state.
- `src/mount`: backend OverlayFS và Magic Mount.
- `src/sys`: mount syscalls.
- `webui`: SolidJS WebUI và i18n 9 ngôn ngữ.
- `xtask`: tự động hóa build và release.

---

## Build

Yêu cầu:

- Rust nightly từ `rust-toolchain.toml`
- Android NDK r27+ và `cargo-ndk`
- Node.js 20+ và pnpm cho WebUI

```bash
cargo run -p xtask -- build --release --flavor lite
cargo run -p xtask -- build --release --flavor nano
cargo run -p xtask -- build --release --skip-webui
./scripts/build-local.sh
cargo run -p xtask -- lint
cargo +nightly test
```

### CI gate và kiểm tra feature flag

Mọi thay đổi phải vượt qua: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets --workspace`, WebUI `pnpm lint` + `pnpm test`, và kiểm tra tiêu đề giấy phép. Cũng đảm bảo các tổ hợp **lite** (`--no-default-features --features control-plane`) và **nano** (`--no-default-features`) biên dịch được. Mã daemon/CLI/WebUI phải nằm sau `#[cfg(feature = "control-plane")]`.

---

## Ghi chú vận hành

- Cài đặt mới tự phát hiện `mountsource`.
- Nếu cấu hình hỏng, dùng `hybrid-mount api config-reset`, rồi áp dụng lại quy tắc từng bước.
- `api config-patch --apply-runtime` áp dụng thay đổi một phần ngay lập tức.

---

## Giấy phép

Được cấp phép theo [GPL-3.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE).
