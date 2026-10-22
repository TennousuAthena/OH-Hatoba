# 验证记录

日期：2026-10-22。上游基线：`96fd9edb83d78ddedbf70895669f7a0121c8f74b`（0.6.0）。

## 已验证

- macOS ARM64，Rust 1.97.0；本机 native probe 两项测试通过。
- core、ssh、ai 的锁定依赖链可编译为 `aarch64-unknown-linux-ohos` staticlib。
- DevEco Studio API26 / SDK `26.0.0.105`；C++ Node-API 动态库及 ArkTS HAP 构建成功。
- API20 compatible，target/compile API26；HAP声明保留 `2in1` 和 `tablet`。
- 共享桌面映射与鸿蒙 transport 测试通过，覆盖参数、错误、早期二进制帧、事件清理、超时与断开。
- 升级审计测试覆盖参数改名、DTO内部字段变化、命令/事件增删、未知代码生成格式拒绝。
- Native依赖树未包含 Tauri/GTK/keyring/本机终端等桌面宿主组件。
- 桌面全部86个测试文件、1028项测试通过；桌面production build、TypeScript、ESLint、native Clippy通过。
- 后补的协议版本不匹配、队列上限、取消发送失败测试通过；IPC定向套件共11项通过。
- 最终HAP包内元数据与ELF检查通过：API20下限、API26 target、两类设备与ARM64动态库一致。
- 专用调试签名 HAP 在 HUAWEI MateBook Pro（2in1，DevEco显示HarmonyOS 7.0.0/API26）安装并启动成功。
- 真机 Node-API/Rust 探针返回 `aarch64 / linux / ohos`，五项真实测试全部通过；Web 收到并渲染结果。
  可重复检查脚本已再次运行通过，结构化证据见 [2in1 P0记录](evidence/2026-10-22-2in1-probe.json)。
- 新增本地资源边界测试后，IPC/资源定向套件14项通过；升级与本地签名脚本6项测试通过。

## 已发现并处理

- OHOS staticlib 链接到 C++ 宿主时需要显式链接 `libtime_service_ndk.so`，供上游
  chrono → iana-time-zone 使用。SDK头文件标注相关符号从API12可用，满足API20基线。
- SDK向 clang 注入的 `--gcc-toolchain` 会触发 unused-command-line-argument；仅屏蔽这项
  SDK参数警告，其他 C++ 编译警告仍按错误处理。
- Hvigor的 HarmonyOS 构建读取 `DEVECO_SDK_HOME`；构建脚本显式设置，不依赖 IDE 会话环境。
- 真机 ArkWeb 会对 `resource://` 中 Vite生成的ES Module脚本和CSS报CORS错误。
  按[官方页面加载文档](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/web-page-loading)的
  本地资源拦截方式，改为虚拟HTTPS同源地址，全部资源由包内提供，拒绝其他请求，不关闭安全检查。
- DevEco可能只生成签名材料而未关联产品；捕获工具处理唯一候选，歧义时拒绝。
  签名须在根节点求值完成、子模块任务创建前注入。已实际生成签名包，并保留共享配置为未签名状态。

## 待验证／待实现

- API20基线设备与两类设备的真实运行；API26工具链编译不是API20运行证据。
- SSH密码/密钥认证、主机密钥确认、交互认证、PTY resize、代理及转发。
- ArkWeb中文 IME、触屏选择、软键盘、外接键盘、WebGL回退和二进制持续流背压。
- 系统CA加载、TLS正负测试、文件授权/FD/SFTP、Asset Store与系统认证。
- 共享应用运行层、同步/部署、完整AI审批流程、HTTP MCP与OAuth浏览器回调。
- 后台/锁屏、网络切换、页面重建、系统回收后的恢复与取消。
- DevEco投屏停留在连接中；本次用进程内探针及Web渲染确认验证往返，未据此标记视觉与输入体验通过。
- GitHub远程fork尚未创建：当前`gh`认证返回HTTP401。本地保留完整上游历史，开发分支为`ohos/main`。

当前HAP是明确标识的开发验证台，不是功能完整的客户端。网络与凭据行为不能根据编译结果标记通过。
