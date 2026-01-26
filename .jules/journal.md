## 2026-01-26 - [Atomic Write Security]
**洞察:** atomic_write 原实现使用了可预测的 SystemTime 和 PID 作为临时文件名，且在重命名文件前缺少 sync_all，存在安全隐患和数据丢失风险。
**准则:** 临时文件必须使用 /dev/urandom 生成随机后缀；在 fs::rename 之前必须调用 file.sync_all() 确保数据持久化。
