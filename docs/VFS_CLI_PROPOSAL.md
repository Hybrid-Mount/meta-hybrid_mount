# VFS CLI 实现方案（已确认）

状态：用户已确认六项全部 A；运行契约与示例见 [VFS CLI 使用说明](VFS_CLI.md)。

核对日期：2026-09-21。

## 参考与范围

参考 NoMount 默认分支 `master` 的 `userspace/src/nm.c`，核对的提交为
`016375cd4a9e7da07b0519dd7bc492101de2a834`：

- https://github.com/maxsteeel/nomount
- https://github.com/maxsteeel/nomount/blob/016375cd4a9e7da07b0519dd7bc492101de2a834/userspace/src/nm.c

沿用 `rule`、`uid`、`clear`、`version` 的操作方式，使用 Rust 对接现有
Hybrid Mount `hybridmount` / `hm1` 协议。内核内置版和 LKM 使用相同控制入口。
既有无参数挂载流水线、顶层 `version`、`vfs-doctor` 和 `lkm-load` 保留。

## 已确认的六项选择

| 编号 | 决策 | A（推荐） | B | C |
| --- | --- | --- | --- | --- |
| 1 | 命令入口 | `hybrid-mount vfs ...` | 同时提供模块目录内的 `hm` 包装脚本，转发到 `hybrid-mount vfs`；不创建第二套实现、不修改全局 PATH | — |
| 2 | 命令别名 | 标准命令和 NoMount 的历史别名都支持 | 只提供 `rule` / `uid` 等标准命令 | — |
| 3 | 输出格式 | 默认可读文本，`--json` 使用固定 HM 字段 | 规则列表和 UID 列表采用 NoMount 的格式；规则支持 `--json`，UID 列表默认 JSON | 新命令默认 JSON，提供 `--text` |
| 4 | 持久化 | 只改当前内核状态，重启后由已有启动配置重新生成 | 额外提供 `--save`，保存手工规则与隔离 UID 并在启动时重放；需要增加存储格式和启动阶段 | — |
| 5 | LKM 管理范围 | 增加显式 `vfs load`，复用候选选择、熔断和加载后探测 | 在 A 基础上增加 `vfs unload --yes`，只卸载可加载版 `hybridmount` | 不增加生命周期命令，继续使用已有 `lkm-load` |
| 6 | 清空操作 | 明确指定清空范围，并要求 `--yes` | 明确指定清空范围后直接执行 | — |

用户选择：1A、2A、3A、4A、5A、6A。B/C 仅保留为决策记录，不属于本次实现范围。

## 推荐命令树

```text
hybrid-mount vfs help
hybrid-mount vfs --help
hybrid-mount vfs version [--json]
hybrid-mount vfs doctor [--json]
hybrid-mount vfs load [--json]

hybrid-mount vfs rule add <virtual> <real> [<virtual> <real> ...] [--uid UID]
hybrid-mount vfs rule add --whiteout <virtual> [<virtual> ...] [--uid UID]
hybrid-mount vfs rule add --opaque <virtual-dir> [<virtual-dir> ...] [--uid UID]
hybrid-mount vfs rule del <virtual> [<virtual> ...] [--uid UID]
hybrid-mount vfs rule list [--json]
hybrid-mount vfs rule clear --yes

hybrid-mount vfs uid add <UID> [<UID> ...]
hybrid-mount vfs uid del <UID> [<UID> ...]
hybrid-mount vfs uid list [--json]
hybrid-mount vfs uid clear --yes

hybrid-mount vfs clear rules --yes
hybrid-mount vfs clear uid --yes
hybrid-mount vfs clear all --yes
```

所有新增写入命令也支持 `--json`。独立执行 `hybrid-mount vfs` 显示帮助。
`hybrid-mount vfs version` 查询内核协议版本；顶层 `hybrid-mount version`
继续返回程序版本 JSON。

NoMount 历史命令别名全部放在 `vfs` 命名空间内：

| 别名 | 标准命令 |
| --- | --- |
| `add` / `a` | `rule add` |
| `del` / `d` | `rule del` |
| `whiteout` / `w` | `rule add --whiteout` |
| `block` / `b` | `uid add` |
| `unblock` / `u` | `uid del` |
| `list` / `l` | `rule list` |
| `list uid` / `l uid` | `uid list` |
| `version` / `v` / `-v` | 内核协议版本 |

列表命令可兼容上游的裸 `json` 参数；规则路径位置中的 `json` 不作为格式开关。
不复制上游把裸 `clear` 或未知 clear 目标默认为清空全部的行为。

## 行为契约

- `--uid UID` 限定一条重定向规则的适用 UID；默认 0 表示全局规则。
  `uid add UID` 是把 UID 加入隔离表，两者不是同一功能。
- `--whiteout` 隐藏指定路径；`--opaque` 使用 HM 已有的 opaque 目录能力，
  目录本身可见，原生子项隐藏，仅展示注入子项。两种标志互斥。
- 同一个虚拟路径与 UID 再次 `add` 会替换对应规则；不同 UID 的记录分别寻址。
- `rule del` 按虚拟路径和 UID 精确删除，不隐式递归删除子路径；
  内核自动维护的空虚拟父目录仍按已有内核逻辑清理。
- 支持批量路径对、多个删除路径和多个 UID。先验证整条命令的参数，
  再按协议分页发送。错误的路径对数量、UID、未知选项或超长记录在发送前失败。
- 相对路径相对于当前工作目录解析；虚拟路径不要求预先存在。
  规范化不跟随符号链接，拒绝空路径和协议无法表示的参数。
- 新增 CLI 的规则删除、UID 删除采用幂等行为；UID 已存在时重复添加成功。
  只容忍相应的“不存在／已存在”结果，其余内核错误继续上报。
- 写入后回读对应规则或 UID 表，验证目标状态；不把退出码 0 当成唯一成功证据。
  协议批次不是事务：报告失败记录及已确认状态，不承诺失败时整批自动回滚。
- 查询只读取当前 Provider。普通规则／UID 命令要求已存在且兼容的 Provider，
  不隐式触发模块加载；通过显式 `vfs load` 请求自动选型和加载。
- `vfs load` 成功必须经过 `hm1` 探测；内核已内置或已加载且兼容时幂等成功。
  沿用现有外来 Provider 检测、版本检查、候选失败处理和熔断约束。
- 清空命令作用于 Provider 的共享运行态表，包含启动流水线创建的对应记录。
  清空规则不是卸载 LKM。裸 `clear` 或错误的目标只显示用法并返回错误。
- 首版推荐运行态操作，不写配置，也不把实时修改伪装成启动快照。
  实时状态通过 `vfs rule list`、`vfs uid list`、`vfs doctor` 查询；
  既有顶层 `status` 继续表达启动／挂载流水线快照。

## 推荐 JSON 与退出码

推荐规则列表采用现有 HM 字段，并补充可读标志：

```json
[
  {
    "virtual_path": "/system/etc/example.conf",
    "real_path": "/data/local/tmp/example.conf",
    "uid": 0,
    "flags": 0,
    "whiteout": false,
    "virtual_dir": false,
    "opaque": false
  }
]
```

UID 列表为整数数组；版本 JSON 为 `{ "version": "hm1" }`；
`doctor --json` 复用现有 `vfs-doctor` 的字段。
写入命令返回操作名、已确认结果及是否成功，错误写 stderr。
JSON 使用 serde 正确转义；JSON 模式 stdout 不混入人类提示和日志。

新增 VFS 命令的退出码：0 成功，1 运行／Provider／协议错误，2 参数错误。
已有命令的输出和退出行为保持兼容。

## 实现与验证范围

1. 在 `src/vfs/cli.rs` 增加参数解析、帮助、命令执行和输出；通过 `src/cli.rs` 分发。
2. 补齐规则记录的 UID 编码：当前 `EncodedRule::write_into` 固定写入 0，
   只设置 payload 顶层 `target_uid` 不足以实现 `rule add --uid`。
3. 在现有用户态后端补齐 UID 删除及三种清空操作，复用协议和分页逻辑。
4. 若选择 LKM 管理，提供能保留错误详情的加载接口；当前启动用途的
   `load_hm_vfs` 会将加载失败记日志后返回成功，CLI 必须严格报告实际结果。
5. 不预设内核协议改动；如果实测发现既有内核能力缺陷，单独记录再处理。
6. 增加命令解析、别名、UID 字节布局、批次边界、错误传播、分页读取、
   清空范围、JSON 转义和只读命令不加载模块的测试。
7. 运行 Rust 格式检查、相关单元测试和 Clippy；Android 构建按本地工具链验证。
   内置内核与 LKM 的实机加载／卸载及路径可见性验证单独标明是否执行。
8. 更新 CLI 文档和帮助。新增实现采用现有 Rust 编码风格，引用上游行为时记录来源。
