## 2026-01-27 - Backend Robustness: Atomic Writes
**洞察:** `atomic_write` 实现中使用了可预测的 `SystemTime` 作为临时文件名后缀，且在 `rename` 前缺失 `sync_all()`，存在数据持久性风险。
**准则:** 原子写操作必须调用 `file.sync_all()` 确保落盘，且临时文件名应使用 `/dev/urandom` 获取强随机性。
