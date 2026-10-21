# AtomGit 同步与维护

项目地址：<https://atomgit.com/YuzakiKokuban/meta-hybrid_mount>

GitHub 是代码集成和发行包发布入口。`.github/workflows/sync-atomgit.yml`
在 GitHub 分支或标签推送后向 AtomGit 同步代码，也支持手动执行与每日补同步。
每日补同步可以捕获使用 `GITHUB_TOKEN` 产生、不会触发其他工作流的发布提交。

## 启用自动同步

在 GitHub 仓库的 Settings → Secrets and variables → Actions 中添加
`ATOMGIT_TOKEN`，值为拥有目标 AtomGit 仓库写权限的访问令牌。
令牌仅保存在 Secret 中，不要加入代码、文档或仓库 URL。
在 Actions 中手动运行 **Sync to AtomGit**，检查日志与两端提交是否一致。

同步范围为分支、提交和标签，不包含 Issues、PR、Release 元数据或发行附件。
AtomGit 独立分支和标签不会被删除；分支分叉或标签冲突会导致同步报错，
需要维护者处理后重新运行。GitHub 重写历史也不会强制覆盖 AtomGit。

## 本机推送

本机保留 `origin` 指向 GitHub，`atomgit` 指向 AtomGit：

```sh
git push origin HEAD
git push atomgit HEAD
git push atomgit --tags
```

日常先将提交集成到 GitHub，再由工作流同步。直接推送 AtomGit 的提交也应
推送回 GitHub，避免两端分叉。不要同时开启另一套会重写同一分支的双向镜像。
