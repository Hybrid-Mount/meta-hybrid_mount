# VFS CLI

`hybrid-mount vfs` 管理当前内核中的 Hybrid Mount VFS Provider。内核内置版和
LKM 共用 `hybridmount` keyring / `hm1` 协议。命令参考 NoMount `nm` 的操作方式，
但不操作 NoMount Provider。

在设备 root shell 中设置：

```sh
HM=/data/adb/modules/hybrid_mount/hybrid-mount
"$HM" vfs help
```

单独执行 `"$HM" vfs` 显示帮助；单独执行 `"$HM"` 仍然运行完整挂载流水线。
无需独立可执行文件或 PATH 修改。

## 命令

| 命令（以下均接在 `hybrid-mount vfs` 后） | 行为 |
| --- | --- |
| `help` / `--help` / `-h` | 显示帮助，不访问内核。 |
| `version [--json]` | 读取内核协议版本；顶层 `hybrid-mount version` 继续返回程序版本。 |
| `doctor [--json]` | 诊断存在方式、版本、响应、规则、隔离 UID 与错误。 |
| `load [--json]` | 显式自动选择、加载并验证随附 VFS LKM；已有兼容 Provider 时不重复加载。 |
| `rule add <virtual> <real> [...] [--uid UID]` | 添加一组或多组虚拟路径→真实路径规则。 |
| `rule add --whiteout <virtual> [...] [--uid UID]` | 隐藏一个或多个路径。 |
| `rule add --opaque <directory> [...] [--uid UID]` | 保留目录本身，隐藏原生子项，仅展示注入子项。 |
| `rule del <virtual> [...] [--uid UID]` | 按路径与 UID 精确删除规则，不递归删除子路径规则。 |
| `rule list [--json]` | 从 Provider 分页读取全部当前规则。 |
| `rule clear --yes` | 清空全部规则，保留隔离 UID。 |
| `uid add <UID> [...]` | 将 UID 加入隔离表，使其看到原生文件系统。 |
| `uid del <UID> [...]` | 将 UID 移出隔离表。 |
| `uid list [--json]` | 读取全部隔离 UID。 |
| `uid clear --yes` | 清空隔离 UID，保留规则。 |
| `clear rules --yes` | 等同 `rule clear --yes`。 |
| `clear uid --yes` | 等同 `uid clear --yes`。 |
| `clear all --yes` | 清空规则和隔离 UID。 |

所有新增写入命令也接受 `--json`，默认输出可读文本。清空作用于共享 Provider
中的对应表，包含启动流程创建的记录；裸 `clear`、未知目标、缺少 `--yes` 均为参数错误。

`uid add 10234` 与 `rule add ... --uid 10234` 的含义不同：前者让 UID 绕开 VFS，
后者限定该规则的适用 UID。规则默认 UID 为 0，表示全局，而不是仅 root。

## 示例

```sh
# 查询与加载
"$HM" vfs doctor
"$HM" vfs load
"$HM" vfs version --json

# 真实文件需要已存在；虚拟路径允许是新路径
"$HM" vfs rule add /system/etc/example.conf /data/local/tmp/example.conf

# 两组路径，只对指定 UID 生效
"$HM" vfs rule add /system/etc/a /data/local/tmp/a \
  /system/etc/b /data/local/tmp/b --uid 10234 --json

# 隐藏路径，或创建只展示注入子项的目录
"$HM" vfs rule add --whiteout /system/etc/unwanted.conf
"$HM" vfs rule add --opaque /system/etc/example-dir

# 运行态隔离 UID
"$HM" vfs uid add 10234 10235
"$HM" vfs uid list --json
"$HM" vfs uid del 10234

# 查看与精确删除
"$HM" vfs rule list --json
"$HM" vfs rule del /system/etc/a /system/etc/b --uid 10234

# 明确清空范围
"$HM" vfs rule clear --yes
"$HM" vfs uid clear --yes
"$HM" vfs clear all --yes --json
```

## 别名和参数

所有别名位于 `vfs` 命名空间内：

| 别名 | 标准命令 |
| --- | --- |
| `add` / `a` | `rule add` |
| `del` / `d` | `rule del` |
| `whiteout` / `w` | `rule add --whiteout` |
| `block` / `b` | `uid add` |
| `unblock` / `u` | `uid del` |
| `list` / `l` | `rule list` |
| `list uid` / `l uid` | `uid list` |
| `v` / `-v` | `version` |

例如 `"$HM" vfs a /virtual /real`，`"$HM" vfs l json`。
列表命令接受裸 `json` 作为 `--json` 的别名。路径参数中的 `json` 保持路径含义。
使用 `--` 结束选项解析，以便传入以 `-` 开头的相对路径。

`--uid UID` 与 `--uid=UID` 均支持；UID 是十进制 u32，负值、溢出、非数字和
重复 `--uid` 均被拒绝。`--whiteout` 和 `--opaque` 互斥。

相对路径以当前工作目录为基准，词法合并 `.`、`..`、重复 `/` 和尾部 `/`，
不跟随符号链接。路径必须是有效 UTF-8，非空且不含 NUL。规范化路径短于 4096
字节，包含头部与路径的单条协议记录不超过 4068 字节。
整条命令通过参数验证后才接触 Provider；错误的后续路径或 UID 不会造成前面的参数先被执行。

## 生效范围与错误

- 修改立即作用于运行态，不写配置、不提供 `--save`。重启后由现有启动配置重新生成。
- 同路径、同 UID 再次添加会替换对应规则；删除不存在的规则／UID、重复加入隔离 UID
  是幂等操作。同次命令重复添加同一规则时，回读验证最后请求的值。
- 每次写入后回读目标表，规则验证同时检查路径、UID、真实路径和 whiteout／opaque
  语义。同一路径但 UID 或来源不符，不计为成功。
- 批次不是事务。发生内核错误时停止后续批次，再回读目标状态；已执行批次和失败批次内
  的部分记录可能已经生效，不自动回滚。结果逐项区分 `confirmed` 与 `unconfirmed`。
  `confirmed` 表示回读时目标状态满足请求，不代表此次请求一定改变了该记录。
- 多个进程并发修改时，回读只是当时观测；不承诺跨进程事务或持续不变的快照。
- 规则／UID 查询和修改不隐式加载 LKM；Provider 不可用时明确报错。
  `vfs load` 沿用候选选型、外来 Provider 检测和
  `/data/adb/hybrid-mount/vfs_lkm_boot_guard` 熔断，失败保留诊断。
  已存在的不兼容 Provider 不被覆盖或卸载。
- `vfs load` 需要 Linux／Android；随附 LKM 仅 aarch64。已有兼容内置 Provider 的
  控制命令不受随附 LKM 架构限制。本版不提供 `vfs unload`。
- 顶层 `status` 仍是启动／挂载流程快照，手工操作后查看实时状态使用 `vfs doctor`、
  `vfs rule list`、`vfs uid list`。旧 `vfs-doctor` JSON 契约保留。

退出码：0 成功，1 运行／Provider／协议错误，2 新增 VFS 命令参数错误。
`doctor` 是诊断报告命令，即使 Provider 不可用也可成功输出报告；检查 `responds`、
`probe_error` 和 `list_error` 判断设备状态。

## JSON 契约

JSON 模式 stdout 仅输出 JSON；诊断错误写 stderr。参数错误或执行前 Provider
检查失败时 stdout 为空。已经开始执行的写入请求即使部分失败，也先输出结果对象，
再以退出码 1 结束。

规则列表使用 HM 字段，保留原始 flags 并提供可读标志：

```json
[
  {
    "flags": 0,
    "uid": 0,
    "virtual_path": "/system/etc/example.conf",
    "real_path": "/data/local/tmp/example.conf",
    "whiteout": false,
    "virtual_dir": false,
    "opaque": false
  }
]
```

UID 列表为整数数组，例如 `[10234, 10235]`。协议版本输出为 `{ "version": "hm1" }`。
`doctor --json` 与旧 `vfs-doctor` 使用相同字段；`load --json` 输出
`operation`、`ok`、`version`、`presence`，其成功必须经过受支持版本探测。

写入结果示例：

```json
{
  "operation": "rule.add",
  "ok": true,
  "results": [
    { "target": "/system/etc/example.conf", "uid": 0, "confirmed": true }
  ],
  "error": null,
  "readback_error": null
}
```

清空操作的 `target` 为 `rules`／`uids`，省略 `uid`。JSON 字符串通过 serde 转义。

## 开发与验证

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo check --locked -p hybrid-mount --target aarch64-linux-android
cargo check --locked -p hybrid-mount --target armv7-linux-androideabi
cargo check --locked -p hybrid-mount --target x86_64-linux-android
```

用户态测试覆盖解析、别名、退出码、UID 字节布局、分页、幂等、部分失败、清空范围、
JSON 转义及回读一致性。宿主机模拟传输测试和交叉编译不替代内置内核／LKM 的设备验证；
设备验证应单独检查路径可见性、UID 隔离、加载后协议响应与重启行为。

## 模块级热操作

`vfs` 子命令保留低层调试语义；模块级 load/unload/reload 使用 `hybrid-mount runtime`，共享启动所有权、操作锁和软重启清理。请参阅 [RUNTIME.md](RUNTIME.md)。低层修改受管规则可能导致后续 runtime 操作因所有权漂移而拒绝执行。
