# Hybrid Mount

Hybrid Mount là siêu mô-đun gắn kết kết hợp dành cho KernelSU và APatch. Trong quá trình khởi động, mô-đun quét các mô-đun khác rồi chọn OverlayFS, Magic Mount hoặc bỏ qua cho từng mục dựa trên quy tắc toàn cục, quy tắc mô-đun và quy tắc đường dẫn. Thư mục nguồn của mô-đun luôn được xem là dữ liệu đầu vào chỉ đọc.

## Tính năng

- Có thể kết hợp OverlayFS và Magic Mount theo từng mô-đun hoặc từng đường dẫn.
- Quy tắc đường dẫn được ưu tiên hơn mặc định của mô-đun, và mặc định của mô-đun được ưu tiên hơn mặc định toàn cục.
- OverlayFS hỗ trợ cả hai chế độ lưu trữ tmpfs và ext4.
- Với vùng staging ext4, KernelSU sử dụng ioctl chính thức để ẩn các nút sysfs; APatch và các môi trường không phải KSU mặc định sử dụng LKM tương thích đi kèm.
- Magic Mount hỗ trợ tệp, thư mục, liên kết tượng trưng, `.replace` và ngữ nghĩa whiteout.
- WebUI cung cấp hai giao diện MD3 (mặc định) và Miuix.
- Hỗ trợ arm64, armv7 và x86_64; trình cài đặt tự động chọn tệp nhị phân phù hợp.

## Cài đặt

Tải tệp ZIP từ [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) rồi cài đặt bằng trình quản lý KernelSU hoặc APatch. Trong lần cài đặt đầu tiên, dùng phím âm lượng để chọn backend mặc định. Khi nâng cấp, tệp `/data/adb/hybrid-mount/config.toml` vẫn được giữ nguyên.

## Cấu hình

Cấu hình mặc định:

```toml
moduledir = "/data/adb/modules"
mountsource = "KSU"
overlay_mode = "ext4" # ext4 | tmpfs
disable_umount = false
default_mode = "overlay" # overlay | magic

[rules.example_module]
default_mode = "magic"

[rules.example_module.paths]
"system/etc/hosts" = "overlay"
```

Đường dẫn trong quy tắc được viết tương đối với thư mục gốc của mô-đun. Quy tắc cấp mô-đun và cấp đường dẫn cũng có thể dùng `ignore`; backend mặc định toàn cục chỉ chấp nhận `overlay` hoặc `magic`. Không thể gán cùng một đường dẫn tệp cho cả hai backend gắn kết. Các thư mục thông thường có thể được hai backend dùng chung làm nút cấu trúc, còn xung đột về tệp, kiểu hoặc `.replace` sẽ khiến giai đoạn lập kế hoạch khi khởi động báo lỗi ngay lập tức. Thay đổi cấu hình có hiệu lực sau khi khởi động lại.

Cơ chế phân luồng này không thay đổi bước kiểm tra khả năng `CONFIG_TMPFS_XATTR` hiện có của dự án. Trên KernelSU, quá trình cài đặt xóa toàn bộ thư mục `lkm/` của mô-đun và khi chạy chỉ sử dụng ioctl `NukeExt4Sysfs` chính thức. Các bản cài đặt APatch và môi trường không phải KSU giữ lại LKM và mặc định thử dùng nó sau khi gắn kết vùng staging ext4. Các tệp `.ko` đi kèm chỉ hỗ trợ aarch64. Việc chọn tự động yêu cầu dòng kernel và thẻ Android/GKI khớp chính xác; các tổ hợp không xác định sẽ bị từ chối. LKM dựng sẵn vẫn phải được kiểm tra khả năng tương thích ABI trên thiết bị thực tương ứng. Nếu thiết bị gặp sự cố trong lúc chạy `insmod`, một dấu ngắt mạch bền vững sẽ ngăn LKM được tải lại ở lần khởi động tiếp theo mà vẫn giữ các chức năng khác của Hybrid Mount. Xem [`module/lkm/README.md`](../module/lkm/README.md) để biết ma trận hỗ trợ, checksum, nguồn và giấy phép.

## Phản hồi

Trước khi cài đặt hoặc báo cáo sự cố, hãy đọc [Lưu ý sử dụng](../USAGE_NOTICE.md). Vui lòng đính kèm bugreport KernelSU/APatch, phiên bản mô-đun và các bước tái hiện. Liên hệ với chúng tôi qua [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) hoặc [nhóm Telegram](https://t.me/hybridmountchat).

## Ngôn ngữ / Languages

- [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_EN.md)
- [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ES.md)
- [Français](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_FR.md)
- [Bahasa Indonesia](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ID.md)
- [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_IT.md)
- [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_JA.md)
- [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_RU.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)
- [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_UK.md)
- [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_VI.md)
- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH_TW.md)

## Giấy phép

- Phần lõi (Rust và các tập lệnh mô-đun): GPL-3.0-only (xem [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (xem [`webui/LICENSE`](../webui/LICENSE)).
- LKM sysfs ext4 tùy chọn (mã nguồn và tệp `.ko` dựng sẵn): GPL-2.0-only, bắt nguồn từ [Mountify](https://github.com/backslashxx/mountify); xem [`module/lkm/README.md`](../module/lkm/README.md) và [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
