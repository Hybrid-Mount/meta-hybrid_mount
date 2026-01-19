## 2025-05-21 - [WebUI] TypeScript Interface Gaps
**洞察:** `webui/src/lib/store.ts` 调用了 `API.readLogs()`，但 `api.ts` 中的 `AppAPI` 接口定义缺失该方法。`tsc` 报错揭示了这一隐患，证明仅仅运行构建是不够的，必须进行严格的类型检查。
**准则:** 在修改 `store` 或 `api` 交互时，始终运行 `tsc --noEmit` 以捕获接口不匹配问题。确保 `MockAPI` 和 `RealAPI` 都实现了完整的 `AppAPI` 接口。
