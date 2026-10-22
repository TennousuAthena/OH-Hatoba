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

## 已发现并处理

- OHOS staticlib 链接到 C++ 宿主时需要显式链接 `libtime_service_ndk.so`，供上游
  chrono → iana-time-zone 使用。SDK头文件标注相关符号从API12可用，满足API20基线。
- SDK向 clang 注入的 `--gcc-toolchain` 会触发 unused-command-line-argument；仅屏蔽这项
  SDK参数警告，其他 C++ 编译警告仍按错误处理。
- Hvigor的 HarmonyOS 构建读取 `DEVECO_SDK_HOME`；构建脚本显式设置，不依赖 IDE 会话环境。

## 待验证／待实现

- 调试签名、真机原生库装载与实际探针结果。本机已有其他应用的 profile，不能用于当前包名。
- API20基线设备与两类设备的真实运行；API26工具链编译不是API20运行证据。
- SSH密码/密钥认证、主机密钥确认、交互认证、PTY resize、代理及转发。
- ArkWeb中文 IME、触屏选择、软键盘、外接键盘、WebGL回退和二进制持续流背压。
- 系统CA加载、TLS正负测试、文件授权/FD/SFTP、Asset Store与系统认证。
- 共享应用运行层、同步/部署、完整AI审批流程、HTTP MCP与OAuth浏览器回调。
- 后台/锁屏、网络切换、页面重建、系统回收后的恢复与取消。
- GitHub远程fork尚未创建：当前`gh`认证返回HTTP401。本地保留完整上游历史，开发分支为`ohos/main`。

当前HAP是明确标识的开发验证台，不是功能完整的客户端。网络与凭据行为不能根据编译结果标记通过。
