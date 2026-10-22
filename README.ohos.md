# OH-Hatoba

Hatoba 的原生 HarmonyOS 适配，目标为 HarmonyOS 6.0/API20 起的 ARM64 **2in1 与平板**。
上游历史和 README 保留；当前集成基线见 [upstream.json](platform/harmonyos/upstream.json)。

## 当前进度

目前是 **P0 原生验证台与适配基础设施**，尚不是完整可日用客户端。

- 已将上游 200 个 IPC 命令接入同一套可注入 transport 的映射；鸿蒙参数表由 bindings 自动生成。
- 已实现消息端口 transport、请求超时/清理、事件和二进制终端帧；生产 Harmony 模式不进入 mock。
- 已在本机通过 core/SSH/AI 依赖链的 OHOS ARM64 交叉编译及 C++ Node-API 链接。
- 已构建 ArkTS/ArkWeb HAP；验证台调用真实 Rust 随机数、AEAD、SQLite、Ed25519 和 Tokio 探针。
- 已用专用调试签名在 HUAWEI MateBook Pro（2in1，HarmonyOS 7/API26）安装运行，五项探针与 Web 往返通过。
- 验证台的 xterm 输入是明确标识的本地回显；SSH 网络连接、TLS、业务运行层、文件服务和真机输入验收仍待完成。

编译通过不等于真机通过；完整状态和限制见 [验证记录](platform/harmonyos/docs/validation.md)。

## 构建

需要上游要求的 Rust、Node.js、pnpm，以及 DevEco Studio 和 API26 SDK。安装 OHOS Rust 标准库：

```sh
pnpm install --frozen-lockfile
rustup target add aarch64-unknown-linux-ohos
pnpm ohos:build
```

默认使用 macOS `/Applications/DevEco-Studio.app/Contents`，其他安装位置设置
`HATOBA_DEVECO_HOME`；独立 Native SDK 可设置 `HATOBA_OHOS_NDK`。脚本明确使用当前安装 SDK，
不下载或升级 SDK。没有本地签名配置时输出未签名开发 HAP：
`platform/harmonyos/entry/build/default/outputs/default/entry-default-unsigned.hap`。

真机调试使用当前包名 `dev.ohhatoba.app` 的专用 profile。在 DevEco 生成调试签名后执行
`node scripts/harmonyos/local-signing.mjs capture`，把自动写入的凭据迁到忽略的本地配置；
Hvigor只在内存中载入该配置，后续构建无需把凭据写回受版本控制的文件。
存在本地配置时，构建同时生成并检查 `entry-default-signed.hap`，签名未生效时直接失败。
签名配置不适用于其他开发者或未纳入该 profile 的设备，需要各自生成。

安装签名包后可重复运行 P0 真机检查（会重启此应用）：

```sh
python3 scripts/harmonyos/check-device.py --target <hdc-target>
```

检查脚本核对 OHOS ARM64、上游提交、五项原生结果与 Web 接收确认。它不代替界面、IME、SSH 或 TLS 验收。

仅运行本机依赖探针：

```sh
cargo run -p hatoba-ohos --bin probe --locked
```

## 保持上游可升级

采用共享命令 facade、可注入 transport 和独立原生宿主。UI、加密和协议不建立鸿蒙副本。
升级前运行只读审计：

```sh
git fetch upstream --tags
pnpm ohos:audit -- upstream/main
```

审计列出新增/删除/变化的命令、参数、事件、DTO，以及需要复核的运行层和依赖文件。
它不会自动合并或声称变更兼容。完整流程见 [上游升级规程](platform/harmonyos/docs/upstream-updates.md)。

新增代码沿用 MIT。其他本地移植项目仅作架构与验证经验参考，未复制其桥接代码。
