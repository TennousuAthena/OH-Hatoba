# 验证记录

更新日期：2026-11-13。上游基线：`951de88b40523b0b4a5cb0344c2bc26015899057`（0.8.5）。
目标：HarmonyOS 6/API20 起，ARM64，`2in1` 与 `tablet`。当前真机均为 HarmonyOS 7/API26。

## 实现状态

默认入口为完整的上游 React 应用。202 个共享 Rust 处理函数通过生成分发器接入，
前端共用上游 228 命令 facade；本地进程等宿主专属命令不提供。生产宿主缺失时明确失败，
不存在 mock 数据或桌面路径回退。保险库、SSH/SFTP、同步/部署、AI、Skills、HTTP MCP 和 OAuth
复用上游源文件。系统能力实现与其人工验收状态分开记录。

## 构建及本机回归

### 2026-11-13 / 未签名 release 导出

- 使用 `scripts/harmonyos/build-hap.sh --release --unsigned` 导出 ARM64 release HAP，
  包名 `moe.kiwi.ohhatoba`，版本 `0.8.5`/`80500`，min API20、target API26，支持 `2in1` 与 `tablet`。
  Rust、CMake 和 ArkTS 均采用 release 模式；实际 CMake 编译包含 `-O2`、`NDEBUG`，并链接
  release Rust 静态库。14 项构建/合约/签名脚本检查及 5 项 HAP 校验测试通过，详见
  [release 构建审计](evidence/2026-11-13-release-085-export.json)。
- 发布的未签名 HAP 大小为 30,354,950 字节，SHA-256 为
  `649f03da46f60a7dda74a202b401f99dc2ad49bc4dd74303b60dd0b0e84cbd83`。
  本地真机测试使用签名 release 包，大小为 30,564,270 字节，SHA-256 为
  `1f4f2175bd0d1fd566ff93cc80ff8977da3b57db949dc76b96285fd9c447476c`；该签名包仅供本地验证。
  ZIP 内容比对确认未签名包的 45 个条目与签名包对应条目的解压字节全部一致，
  签名包仅多出 `.pages.info` 条目。未签名包需自行签名后安装。
- 两类 API26 真机均安装上述签名 release 包并通过独立端到端复验：
  [MateBook Pro（2in1）](evidence/2026-11-13-2in1-release-085-e2e.json)、
  [MatePad Pro（平板）](evidence/2026-11-13-tablet-release-085-e2e.json)。
  15 项原生网络/文件检查、隔离保险库及主机/历史 CRUD、SSH/Telnet 各 768 KiB 逐字节回显与
  xterm 消费额度回收、AI 发送/保存/取消生命周期均通过，安装摘要与本次签名 release 包一致。
- 两台设备的普通入口也已复验：
  [2in1](evidence/2026-11-13-2in1-release-085-startup.json)、
  [平板](evidence/2026-11-13-tablet-release-085-startup.json)均确认原生初始化、React 渲染和窗口背景同步。
- 首次日志采集时，两台设备的全局 hilog 级别均为 W，INFO 成功标记未被收集。
  仅将本应用日志 tag 临时设为 I 后重新验证通过，没有修改安装包或全局日志级别。
  完成后将该 tag 设回 W，与测试前观察到的全局有效等级一致；此临时覆盖在设备重启后失效。
  两台设备已回到普通入口，临时端口映射和生成数据服务均已清理。
  本次没有投屏；API20 实际运行及触控/IME、真实选择器、生物识别、外部账号、系统恢复等人工验收
  仍未完成。下方记录的 5 项上游环境依赖断言失败保持原结论，不计为通过。

### 2026-11-13 / MateBook Pro（2in1）端到端加严复验

- 在 MateBook Pro/API26 安装与平板加严复验**完全相同的 v0.8.5 签名包**，SHA-256 为
  `1ba632f307d274fb5caa629442cfc903601d0cdfb0d916b67e183a3e7b877731`。
  [2in1 端到端证据](evidence/2026-11-13-2in1-upstream-085-e2e.json)记录本次独立启动标识和实际安装摘要。
- 隔离保险库、SSH/Telnet 主机和历史 CRUD，以及 15 项原生网络/文件检查全部通过；
  包括 TLS 信任/拒绝、SSH 认证和主机密钥拒绝、SFTP FD 传输及取消、三类转发、AI/MCP/OAuth 协议链路。
- Web SSH 与 Telnet 各通过 8 × 96 KiB（768 KiB）逐字节回显、xterm 消费和额度回收，
  同时完成 resize/close 调用。真实 AI typed facade 的发送、Unicode 流、会话保存、中途取消、
  空闲状态和本轮测试记录清理全部通过，结果与平板一致。
- [2in1 普通入口证据](evidence/2026-11-13-2in1-upstream-085-e2e-startup.json)确认同一安装包
  正常启动、React 渲染和原生背景同步。当前两类 API26 设备均完成此版本的自动化复验。
- 本轮没有修改应用实现或重建安装包。结束后已回到普通入口，移除四个临时映射并停止生成数据服务。
  未投屏；窗口布局、触控/中文 IME、真实选择器、生物识别与系统恢复仍需人工验收，API20 仍缺真机证据。

### 2026-11-13 / 平板端到端加严复验

- 同一上游 v0.8.5 基线，重新构建并安装含加强诊断的签名包；
  [本轮端到端证据](evidence/2026-11-13-tablet-upstream-085-e2e.json)记录实际安装摘要，
  [本轮普通入口](evidence/2026-11-13-tablet-upstream-085-e2e-startup.json)记录相同设备的启动与背景同步。
- SSH 与 Telnet 均通过真实 typed facade 分 8 次发送 96 KiB，共 **768 KiB/协议**，
  超过单通道 512 KiB 额度。逐字节比对回显，并等待 xterm 消费完成，验证 ACK 后额度可继续使用。
  同时完成 resize/close 调用；不把返回成功视为真实窗口尺寸的视觉验收。
- AI 新增完整业务路径：在隔离诊断保险库内发现/保存生成模型 provider，调用 `ai_send`，
  经共享 assistant 和 Web channel 验证 Unicode 响应、完成事件及会话持久化；
  慢流收到首片段后调用 `ai_stop`，确认停止原因、空闲状态与已收文本保留，最后删除本次记录。
  此项补足原有直接协议库的 streaming/cancel 检查，不使用真实账号。
- 原有 15 项原生网络/文件检查、保险库与主机/历史 CRUD、普通入口、React 渲染、原生背景同步全部通过。
- 每次启动携带独立验证标识，先观察本次标记再接受对应进程结果；拒绝旧成功日志、当前错误和中途换进程。
  真机暴露的日志环形缓冲淘汰启动标记问题已修复：观察标记后，允许同一进程的后续日志继续完成验证。
  新增 5 项生成日志回放测试覆盖上述判定，并加入 HarmonyOS CI。TypeScript、定向 lint、15 项 IPC、
  6 项合约/签名脚本检查及签名 HAP 构建通过。此次变更仅加强诊断与结果采集，未更改生产业务协议。
- 测试后回到普通应用入口，移除本轮四个测试映射并停止服务。没有投屏，触控/中文 IME、
  真实选择器、生物识别、外部账号与生命周期恢复仍不在本次自动化验收范围内。

### 2026-11-13 / 上游 v0.8.5

- 集成正式 tag `v0.8.5`（`951de88b40523b0b4a5cb0344c2bc26015899057`），保留上游提交历史。
  [兼容审计](evidence/2026-11-13-upstream-085-audit.json)记录 27 个新增命令、4 个新增事件和 DTO 变化；
  原有命令参数未改变。重新生成 228 命令合约，202 个处理函数进入共享运行层。
- AI 与 IPC 直接使用上游新拆出的 `hatoba-assistant`、`hatoba-ipc`，不保留旧业务副本。
  `local-processes` 仅在桌面启用；鸿蒙继续拒绝本地进程与 stdio MCP，并保留同步配置。
  新 Telnet、终端历史/建议、SSH 登录消息和应用连接代理复用上游实现。
- Telnet 和 SSH 共用二进制帧、消费完成 ACK、传输额度和页面销毁清理。诊断保险库新增
  Telnet 主机 CRUD、按主机隔离的命令历史增查删清空、空格开头不入历史验证。
  新回环服务只协商 Telnet 协议并回显生成字节，Web 侧核对 96 KiB Unicode/IAC 数据与 xterm 消费。
- 外部程序编辑器在鸿蒙明确不可用，菜单与程序选择入口按本机能力隐藏，保存的配置不删除。
  鸿蒙同步/AI 默认直连，可选择已保存 HTTP/SOCKS5 代理；不自动读取系统代理，界面已明确说明。
- 应用名与图标保持上游 Hatoba，包名 `moe.kiwi.ohhatoba`，版本 `0.8.5`/`80500`。
  HAP 检查同时核对应用、workspace 与上游基线版本，安装验证记录实际安装文件的摘要。
- TypeScript、ESLint、Prettier、桌面构建通过；桌面 1193、共享 UI 28、站点 31、Worker 168、
  上游脚本 88 项测试通过。独立鸿蒙运行层 139 项、无本地进程的 AI 共享层 170 项测试通过。
  OHOS 库 6 项、Telnet 46 项、Rust 文档检查，以及 workspace/独立 AI/独立鸿蒙宿主三组
  Clippy（`-D warnings`）、rustfmt、注释宽度、依赖边界检查通过。桌面 bindings 导出无漂移。
- Rust 全量执行有 5 项未修改的上游环境依赖断言失败，不计为通过：子进程 PID 文件未生成、
  本机 uptime 缺失，以及 TCP 黑洞/不可达探测/代理旁路三项测试在当前网络实际连通。
  完整名称、观察结果和其余分组通过情况见 [本轮构建验证](evidence/2026-11-13-upstream-085-validation.json)。
  未配置的 OpenSSH/live Worker 集成不会算作外部服务验收，也没有为通过检查放宽断言。
- 最终签名 HAP 已安装至 MatePad Pro/API26，[端到端证据](evidence/2026-11-13-tablet-upstream-085-network.json)
  覆盖 15 项原生检查、Web SSH/Telnet 数据与消费确认，以及隔离保险库业务；
  [普通入口](evidence/2026-11-13-tablet-upstream-085-startup.json)确认 React 渲染和原生窗口背景同步。
  安装包摘要与测试记录一致；本轮没有投屏，未将日志标记作为触控/IME/视觉验收。
  初始升级验证时 MateBook 与 API20 设备尚未复验；后续 MateBook 结果见上方 2in1 加严复验记录，
  API20 仍未验证。该轮测试完成后已移除四个反向端口并停止生成数据服务。

### 2026-10-24 / 上游 v0.7.1

- 候选提交为 tag `v0.7.1`（`e1c7d171f1a7ee85c08f3d3d01a7c12f8b4c3d22`）。只读审计发现
  新增 `windowZoom` 命令与 `window://zoom` 事件，新增 `HostsView`、`ZoomView`，
  删除 `DashboardLayout`，并扩展 AI 设置与本机偏好 DTO。合约重新生成后为 201 个命令、
  184 个共享处理函数，人工复核无其他命令形状漂移。
- 新的界面缩放在鸿蒙侧接入 ArkWeb：Webview 打开受控 `zoomAccess`，按上游 10 档
  67%–200% 取最近值，监听实际缩放比例换算目标倍率，启动时沿用 `ui_zoom` 并保存结果；
  共享 Rust 分发器走宿主服务白名单，不复制桌面 Tauri 实现。`ZoomView` 在非桌面宿主
  可反序列化，桌面事件属性仍由 `desktop-host` 控制。
- workspace TypeScript、ESLint、Prettier、桌面 1057 项与站点 31 项测试通过；
  IPC/合约/边界检查通过。完整 Rust workspace 的 Clippy、rustfmt、注释宽度检查通过，
  测试在本宿主排除两项依赖本机 shell/运行环境指标的上游测试后完成：
  `stats::tests::the_script_reads_this_machine`（本环境未报告 uptime）与
  `platform::login_shell::tests::shell::what_the_shell_started_is_killed_with_it`
  （本环境缺少其 shell fixture 路径）。两者均为未修改的上游测试；此前单独重跑的
  SOCKS 不可达断言通过。签名 OHOS HAP 构建和包结构检查通过，摘要见
  [升级验证记录](evidence/2026-10-24-upstream-071-validation.json)。
- 新包名 `moe.kiwi.ohhatoba`、`0.7.1`/`70100` 的签名包已构建，并在 MateBook Pro 完成安装后
  复验：业务 IPC 与普通入口启动均通过，普通入口同时确认 React 渲染和原生窗口背景同步，
  记录见 [MateBook 业务链路](evidence/2026-10-24-2in1-upstream-071-newbundle-business.json)和
  [MateBook 普通入口](evidence/2026-10-24-2in1-upstream-071-newbundle-startup.json)。
- MatePad Pro 的旧无线调试端口已失效，仍待重新连接后完成同版本安装与业务/普通入口复验。

### 2026-10-22 / 初始基线

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
- 本机检查与交付 HAP 的摘要见 [结构化记录](evidence/2026-10-22-host-validation.json)。
- 整个 workspace 的 Clippy（所有 target，warnings 为错误）、rustfmt 和注释宽度检查通过。

完整 workspace 测试在本机网络环境遇到4条未修改的上游网络断言失败：
`dns_failure`、`blackholed_address_times_out`、`tcp_probe_reports_reachability_only`、
`probes_go_through_the_proxy_and_wait_for_the_server`。本机实测会解析 `nonexistent.invalid`，
使这些“域名不可达/地址黑洞”的前提不成立。`crates/hatoba-ssh` 没有移植改动，未放宽其断言。
这4条不计入通过数；其他 workspace 回归在显式排除这4条后执行完成。桌面子进程清理测试曾在高并发编译时出现计时失败，
降低测试并发后377项全部通过。未将默认关闭的 OpenSSH 集成环境计作真实 sshd 验收。

## 真机证据

| 设备 | 证据 | 已验证的范围 |
| --- | --- | --- |
| MateBook Pro / 2in1 / API26 / v0.8.5 加严复验 | [端到端](evidence/2026-11-13-2in1-upstream-085-e2e.json)、[普通入口](evidence/2026-11-13-2in1-upstream-085-e2e-startup.json) | 与平板相同签名包；15 项原生检查、SSH/Telnet 各 768 KiB 与额度回收、真实 AI 生命周期、业务 CRUD、启动与背景同步 |
| MatePad Pro / tablet / API26 / v0.8.5 加严复验 | [端到端](evidence/2026-11-13-tablet-upstream-085-e2e.json)、[普通入口](evidence/2026-11-13-tablet-upstream-085-e2e-startup.json) | SSH/Telnet 各 768 KiB 与额度回收、真实 AI 业务发送/保存/取消、既有 15 项原生检查与业务检查 |
| MatePad Pro / tablet / API26 / v0.8.5 | [网络与业务](evidence/2026-11-13-tablet-upstream-085-network.json)、[普通入口](evidence/2026-11-13-tablet-upstream-085-startup.json) | 最终签名包的 15 项原生检查、Web SSH/Telnet、主机/历史 CRUD、普通入口与背景同步 |
| MateBook Pro / 2in1 / API26 | [业务 IPC](evidence/2026-10-22-2in1-business.json) | typed facade、保险库创建/重开、主机 CRUD、Agent 配置保留、锁事件、锁定拒读与解锁 |
| MatePad Pro / tablet / API26 | [业务 IPC](evidence/2026-10-22-tablet-business.json) | 同上，使用隔离诊断保险库 |
| MateBook Pro / 2in1 / API26 | [网络及文件](evidence/2026-10-22-2in1-network.json) | 15项原生检查以及 Web SSH 二进制终端消费确认 |
| MatePad Pro / tablet / API26 | [网络及文件](evidence/2026-10-22-tablet-network.json) | 15项原生检查以及 Web SSH 二进制终端消费确认 |
| MateBook Pro / 2in1 / API26 | [普通入口](evidence/2026-10-22-2in1-startup.json) | 正常应用的原生初始化、能力读取、保险库状态和 React 渲染 |
| MatePad Pro / tablet / API26 | [普通入口](evidence/2026-10-22-tablet-startup.json) | 同上，无需投屏或操作用户保险库 |

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

- 2026-10-23：应用显示名称恢复为上游 `Hatoba`，启动器和启动窗口复用桌面原始 PNG 图标，
  删除自创 SVG；页面标题、生物识别提示及诊断页可见名称也同步恢复。Hvigor 包含 IDE 构建的
  自动同步，HAP 检查核对上游产品名称、图标字节和旧图标移除。TypeScript、ESLint、合约、
  宿主边界及本地签名脚本检查通过；签名包已经覆盖安装到
  [MateBook Pro](evidence/2026-10-23-2in1-upstream-branding-startup.json) 与
  [MatePad Pro](evidence/2026-10-23-tablet-upstream-branding-startup.json)，普通启动、React 渲染及
  原生背景同步均通过。没有投屏或操作启动器，不将包资源核对记作视觉验收。

- 平板系统导航区出现整块白色背景带：Web 深浅主题没有同步到原生窗口的安全区背景。
  现在读取共享 CSS 的 `--win`，在启动、手动主题和系统主题变化时，通过隔离的宿主服务同步
  窗口、ArkUI 根容器及 Web 背景；三键导航模式也设置相应背景与文字颜色。
  保留系统导航手势、指示线与原有安全区避让，不使用全屏覆盖内容。
  生产 CSS 会把 `#ffffff` 压缩为 `#fff`，同步前会展开短格式颜色，避免只在开发构建生效。
  TypeScript、ESLint、1035项前端测试、6项原生测试、Clippy、合约与边界检查通过；新签名包已
  覆盖安装到平板。[普通入口及背景同步复验](evidence/2026-10-22-tablet-navigation-surface-startup.json)
  确认原生背景 API 完成并返回请求颜色；没有投屏检查可见导航区，不将启动标记作为视觉验收。

- 鸿蒙界面曾显示上游的最小化/最大化/关闭按钮：原条件只排除了 macOS，按钮依赖的 Tauri
  窗口接口在鸿蒙不可用。现在仅 Windows/Linux 渲染这组按钮，鸿蒙引导/锁屏页也不再渲染
  空的桌面标题栏覆盖层。TypeScript、ESLint、1035项前端测试通过，新签名包已覆盖安装到平板；
  [普通入口复验与包校验和](evidence/2026-10-22-tablet-window-controls-startup.json)已记录，未投屏验收。

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
