# 代码审查报告（严格/全面）

## 审查范围
- Rust 主程序：配置解析、模块扫描、挂载计划与执行、Magic Mount。
- WebUI：API 层命令构造与运行时兼容性。
- 工程质量门禁：`cargo test`、`cargo clippy`、`pnpm lint`、`pnpm build`。

## 结论摘要
本次审查识别到 **3 个高优先级问题（2 个 High，1 个 Medium）**，其中 2 个位于 Rust 核心执行路径，1 个位于 WebUI 命令拼接逻辑。

---

## 发现 1（High）
### 标题
`OverlayMode::Erofs` 路径下 `magic_workspace` 未创建时不会挂载 tmpfs，可能导致 Magic Mount 直接失败。

### 证据
在 `mount_magic` 中，当 `overlay_mode` 为 `Erofs` 时，仅在 `magic_ws_path.exists()` 为真时才执行 `mount_tmpfs`；若路径不存在，分支没有创建目录也不会挂载。随后无条件调用 `magic_mount::magic_mount(&magic_ws_path, ...)`。这会让后续流程拿到不存在或未初始化的工作目录。

### 风险
- 在 `Erofs` 场景中，首次运行或清理后运行可能失败。
- 失败后会导致模块挂载降级/缺失，表现为功能异常或开机后模块行为不一致。

### 建议
- 在 `Erofs` 分支先确保目录存在（`create_dir_all`），再执行 `mount_tmpfs`。
- 补充一个最小回归测试：`magic_workspace` 初始不存在时，`mount_magic` 仍能成功进入后续流程。

---

## 发现 2（High）
### 标题
十六进制 payload 解码缺少奇数长度校验，可能触发切片越界 panic（CLI DoS）。

### 证据
`handle_save_config` 与 `handle_save_module_rules` 都使用 `(0..payload.len()).step_by(2)` 并访问 `payload[i..i+2]`；当 payload 长度为奇数时，末次迭代会越界，直接 panic。

### 风险
- 任意错误输入可导致进程崩溃（拒绝服务）。
- 影响 `save-config` / `save-module-rules` 两个入口。

### 建议
- 解码前增加 `payload.len() % 2 == 0` 校验，不满足时返回结构化错误。
- 建议将 hex 解码提取为统一函数，避免重复与遗漏。

---

## 发现 3（Medium）
### 标题
`openLink` 使用 shell 字符串拼接，仅转义双引号，存在命令注入面。

### 证据
`openLink` 将 URL 插入 `am start ... -d "..."` 的命令字符串，只做了 `"` 处理。若底层执行器按 shell 解释，`$(...)`、反引号等在双引号中仍可能被解释。

### 风险
- 若 URL 来源被污染（包括未来功能扩展后的用户输入），可能触发命令注入。
- 该调用运行在高权限环境（KSU），风险放大。

### 建议
- 使用参数化执行（argv 形式）替代字符串拼接。
- 若受限于 API 只能传字符串，至少做白名单校验（仅允许 `https?://`）并进行 shell-safe 转义（覆盖 `$`、`` ` ``、`\` 等）。

---

## 质量门禁结果
- `cargo test`：通过（当前无单元测试用例）。
- `cargo clippy --all-targets --all-features`：通过。
- `pnpm -C webui build`：通过。
- `pnpm -C webui lint`：失败，原因是 `eslint.config.js` 依赖 `typescript-eslint` 包未解析到（环境/依赖配置问题）。

## 后续建议（按优先级）
1. 先修复发现 1、2（运行时稳定性与可用性）。
2. 同步加回归测试（尤其是奇数长度 payload 与 EROFS 初始化路径）。
3. 修复 WebUI 命令构造方式（参数化执行）并恢复 lint 依赖可用性。
