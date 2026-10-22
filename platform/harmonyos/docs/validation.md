# 验证记录

日期：2026-10-22。上游基线：`96fd9edb83d78ddedbf70895669f7a0121c8f74b`（0.6.0）。
目标：HarmonyOS 6/API20 起，ARM64，`2in1` 与 `tablet`。当前真机均为 HarmonyOS 7/API26。

## 实现状态

默认入口为完整的上游 React 应用。183 个共享 Rust 处理函数通过生成分发器接入，
前端共用上游 200 命令 facade；本地进程等宿主专属命令不提供。生产宿主缺失时明确失败，
不存在 mock 数据或桌面路径回退。保险库、SSH/SFTP、同步/部署、AI、Skills、HTTP MCP 和 OAuth
复用上游源文件。系统能力实现与其人工验收状态分开记录。

## 构建及本机回归

- macOS ARM64，Rust 1.97.0；DevEco Studio 26.0.0.851，Native SDK 26.0.0.105。
- 锁定依赖的 OHOS ARM64 交叉编译、C++ Node-API/Asset 链接、ArkTS 编译及专用调试签名通过。
- 包内 ELF/元数据检查确认 ARM64、min API20、target API26、`2in1` 与 `tablet`，两个 Web 入口均在包内。
- `contracts.mjs --check`：200 命令合约通过；升级/本地签名脚本6项测试通过。
- `check-boundaries.mjs`：生产 OHOS 依赖中没有 Tauri、GTK、keyring、本地终端、CLI 或 test-util。
- 前端 TypeScript、ESLint、87文件/1035项测试及桌面 production build 通过。
- 桌面 Rust 377项回归通过。共享运行层295项回归通过；包括真实加密保险库、自动分发、权限边界、Agent/stdio 配置保留、
  终端额度与顺序确认，以及封装秘密的落盘、替换攻击和符号链接拒绝。
- 原生宿主6项测试通过：依赖探针、文件 FD 生命周期/权限、文档导入/撤销、旧缓存清理及初始化/页面销毁竞态。
  macOS 的 `/dev/fd` 不支持目录遍历，本机文档测试对目录使用实际临时路径；
  OHOS 真机文档检查必须使用真实 `/proc/self/fd` 目录路径。
- 整个 workspace 的 Clippy（所有 target，warnings 为错误）、rustfmt 和注释宽度检查通过。

完整 workspace 测试在本机网络环境遇到4条未修改的上游网络断言失败：
`dns_failure`、`blackholed_address_times_out`、`tcp_probe_reports_reachability_only`、
`probes_go_through_the_proxy_and_wait_for_the_server`。本机实测会解析 `nonexistent.invalid`，
使这些“域名不可达/地址黑洞”的前提不成立。`crates/hatoba-ssh` 没有移植改动，未放宽其断言。
这4条不计入通过数；其他回归单独执行。桌面子进程清理测试曾在高并发编译时出现计时失败，
降低测试并发后377项全部通过。未将默认关闭的 OpenSSH 集成环境计作真实 sshd 验收。

## 真机证据

| 设备 | 证据 | 已验证的范围 |
| --- | --- | --- |
| MateBook Pro / 2in1 / API26 | [业务 IPC](evidence/2026-10-22-2in1-business.json) | typed facade、保险库创建/重开、主机 CRUD、Agent 配置保留、锁事件、锁定拒读与解锁 |
| MatePad Pro / tablet / API26 | [业务 IPC](evidence/2026-10-22-tablet-business.json) | 同上，使用隔离诊断保险库 |
| MateBook Pro / 2in1 / API26 | [网络及文件](evidence/2026-10-22-2in1-network.json) | 15项原生检查以及 Web SSH 二进制终端消费确认 |
| MatePad Pro / tablet / API26 | [网络及文件](evidence/2026-10-22-tablet-network.json) | 以证据内实际列出的检查为准；最后一轮15项复验曾被自动锁屏阻止 |
| MateBook Pro / 2in1 / API26 | [普通入口](evidence/2026-10-22-2in1-startup.json) | 正常应用的原生初始化、能力读取、保险库状态和 React 渲染 |

15项检查覆盖：系统 CA 的公网 HTTPS 正向连接、自签名证书拒绝、SSH 主机密钥拒绝和密码认证、
exec、PTY 非 UTF-8/中文/emoji/resize、SFTP 512KiB 上传下载及 FD 提交字节一致性/预取消、
local/remote/dynamic 三类转发、AI HTTP 模型列表与 Unicode 流/预取消、HTTP MCP 初始化/列工具/
调用/关闭、OAuth PKCE/state 校验/loopback 回调拒绝与取消，以及配置/Skills ZIP/目录导入和授权撤销。
Web 另经真实 typed facade、共享业务后端与 SSH 连接传输约96KiB中文/emoji终端数据，
等待 xterm 解析回调完成并核对全部字节。该项覆盖真实 WebMessagePort 二进制通道及消费后 ACK。

SSH/SFTP 服务只使用生成数据和内存文件，不执行主机 shell；转发限定测试目标。
HTTP/OAuth 不登录真实账号；自签名证书仅供负向测试，不加入系统信任。
FD 测试由原生创建临时文件描述符，不能代替用户实际操作系统文件选择器。
证据只保存布尔结果与版本/设备类别，不含设备地址、用户保险库、签名材料、凭据或终端内容。

## 真机发现并修复

- ArkWeb 的 `resource://` ES Module/CSS 被 CORS 拦截。采用包内资源拦截的固定虚拟 HTTPS origin，
  拒绝外部请求和目录穿越，保持浏览器安全检查。
- Vite/Rolldown 多入口曾将公共 helper 放进应用入口，导致诊断页隐式启动第二个宿主。
  保留严格入口签名，独立 runtime chunk；不再出现重复握手和请求 ID 冲突。
- 鸿蒙拒绝在沙箱内创建原文件授权层所需符号链接。读取改为持有并复制选择器授予的 FD，
  直接通过 `/proc/self/fd` 访问，单独保留原文件名；上传不复制大文件。
- 桌面 SSH config 读取函数忽略传入路径，已支持用户明确选中的文件，并不把外来 `~` 路径
  展开到鸿蒙应用目录。导出文件内的私钥仍由上游解析；其他路径不能绕过本机授权。
- `READ_PASTEBOARD` 属受限 ACL，普通调试应用安装被系统拒绝。移除该权限，
  使用系统 PasteButton 的用户点击授权，不用宽权限或静默剪贴板读绕过。
- 终端 ACK 从“收到帧即确认”改为等待 xterm 消费完成；原生按帧限制在途字节并验证确认顺序。
- OHOS 链接 chrono 时显式加入 `libtime_service_ndk.so`；Asset Store 显式链接 `libasset_ndk.z.so`。
- 签名只从忽略且0600的本地配置加载；构建前移除旧签名产物，签名未生效时构建失败。

## 未完成的验收

- HarmonyOS 6/API20 基线设备实际运行。API26 工具链与两类 API26 设备不是 API20 运行证据。
- 触控布局、软键盘/中文 IME、文字选择、外接键盘、窗口连续缩放及 WebGL 回退体验。
- 真正的系统文件选择器、PasteButton、通知点击、Asset 生物识别认证/取消/系统认证变更。
- 真实外部 AI 账号、OAuth 系统浏览器往返、Cloudflare 账户部署和多设备同步。
- 锁屏/后台、断网重连、网络切换、页面重建、系统回收后的恢复与活动流中断。

用户明确选择“暂不允许投屏，先继续自动化测试”；上述交互不因编译或启动标记通过。
后台只申请系统允许的延时/数据传输任务，不承诺无限后台 SSH。
本地终端、stdio MCP、SSH Agent/CLI、托盘和桌面更新器属于本版明确不提供的本机能力，
同步来的配置保留；其他功能的人工验收仍是完成发布前的必要工作。
