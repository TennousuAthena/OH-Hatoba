# OH-Hatoba

Hatoba 的原生 HarmonyOS 适配，目标为 HarmonyOS 6.0/API20 起的 ARM64 **2in1 与平板**。
上游历史和 README 保留；当前集成基线见 [upstream.json](platform/harmonyos/upstream.json)。

## 当前进度

默认入口已经是完整的 Hatoba React 应用。鸿蒙宿主编译同一份 Rust 业务源文件，
通过生成的分发器接入 183 个处理函数；前端仍共用上游的 200 命令 facade。

- SSH、PTY、SFTP、三类端口转发、保险库、同步/Worker 部署、远程 AI、Skills、HTTP MCP 和 OAuth
  使用上游实现，没有建立鸿蒙业务副本。
- ArkTS 提供系统文件选择、剪贴板粘贴按钮、外部浏览器、通知、生命周期与后台传输申请。
  文件通过用户授权的描述符和不透明句柄访问，Web 不能指定应用私有路径。
- TLS 使用鸿蒙 Certificate Manager 的系统 CA；设备密钥使用 Asset Store，其他原生密文采用
  上游 AEAD。生物识别保险库密钥使用独立的认证保护 Asset，认证结果不经过 Web。
- 终端保持二进制帧，按 xterm 消费完成确认额度；触屏设备提供辅助按键，软键盘使用内容缩放避让。
- 两类 API26 真机已经运行生成数据自动化。完整证据、测试范围和未验收项目见
  [验证记录](platform/harmonyos/docs/validation.md)。实现接入不代表每项系统交互已经验收。

本版不提供本地终端、stdio MCP、桌面 SSH Agent/CLI、托盘和桌面自动更新。
来自其他设备的这些配置会保留；本机能力控制执行和界面入口。
后台申请由系统决定，不能保证应用被回收后继续保持 SSH 连接。

当前产物为开发调试 HAP。API20 基线设备、触控/中文 IME、真实文件选择器、生物识别和系统恢复
仍需完成人工验收；用户目前选择暂停投屏，不将这些项目记作通过。

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

安装签名包后可重复运行真机检查（仅重启此应用）：

```sh
python3 scripts/harmonyos/check-device.py --target <hdc-target>
python3 scripts/harmonyos/check-device.py --target <hdc-target> --app
```

默认检查核对 OHOS ARM64、上游提交、五项依赖结果，以及通过真实 typed facade 运行的保险库和主机 CRUD。
`--app` 检查普通入口的原生初始化、能力读取、保险库状态和 React 渲染，不操作用户保险库。
`--network` 额外要求下面的生成数据服务；先在本机分别运行，再通过 hdc 临时反向映射端口：

```sh
cargo run -p hatoba-ohos --features test-tools --bin fixture-server --locked
python3 scripts/harmonyos/fixture-http.py
python3 scripts/harmonyos/fixture-tls.py
# 对每个测试设备分别执行；不影响其他端口映射。
hdc -t <hdc-target> rport tcp:22222 tcp:22222
hdc -t <hdc-target> rport tcp:28080 tcp:28080
hdc -t <hdc-target> rport tcp:28443 tcp:28443
python3 scripts/harmonyos/check-device.py --target <hdc-target> --network
```

SSH 服务只回显生成数据、使用内存 SFTP，并将转发限定在固定测试目标；不执行主机 shell 或读取主机文件。
HTTP 服务不使用真实账号。TLS 负向服务使用临时自签名证书，不安装信任根。
这些服务与 `test-tools` 二进制不打入 HAP，生产业务没有测试服务回退。
两台设备应顺序测试，结束后删除这三个临时反向映射并关闭服务。
检查不代替触控、IME、真实选择器或生物识别验收。

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
