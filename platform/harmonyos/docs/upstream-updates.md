# 上游升级与适配边界

## 设计约束

上游业务代码保持单一来源。鸿蒙宿主负责系统服务、生命周期和 transport，不持有第二套
SSH、保险库、同步、AI 或 React 页面实现。

| 边界 | 设计 | 升级时的作用 |
| --- | --- | --- |
| Web API | 原 `tauri.ts` 中的 `createCommandApi` 接收 transport，桌面包装器保持原入口 | 上游命令映射修改只维护一次，两个宿主共用 |
| 参数与类型 | 从 tauri-specta bindings AST 生成命令名、参数和 channel 位置；DTO 按无注释声明计算摘要 | 发现参数改名、返回值变化，以及名称未变但内部字段变化 |
| Native | 独立 OHOS workspace member + C++ Node-API 薄桥 | 不链接 Tauri/GTK，不为了鸿蒙重写协议库 |
| 业务运行层 | `hatoba-runtime` 通过 `#[path]` 编译原桌面业务源；桌面 `host` 转发 Tauri，鸿蒙注入自己的 host | 上游修改只维护同一函数，避免搬移或复制业务目录 |
| 系统能力 | 独立的原生 SystemServices 队列和本机 capability；Web 只有明确的服务白名单 | 系统 API 与平台差异集中处理，保留不可执行的同步配置 |
| 版本 | `upstream.json` 是已完成验证的上游提交；hostProtocol 独立版本 | 不因上游发版就更改桥接协议，不跳过验证推进基线 |

当前 `createCommandApi` 仍位于原有文件，以减少上游改动和未来合并位置变化。鸿蒙未伪造
`__TAURI_INTERNALS__`。新宿主仅实现现有工厂实际使用的 command、channel 与 event 三个接口。
生成器遇到未知 bindings 结构直接失败，禁止猜测 snake_case/camelCase 或静默跳过命令。

## 共享运行层

`crates/hatoba-runtime/build.rs` 读取实际命令函数的 Rust AST 和生成的 bindings 合约，
生成参数反序列化、AppHandle/State 注入、channel 绑定及 async 调用。返回值和单位值由函数类型推导。
命令不再维护第二份手写表。桌面命令宏使用 `desktop-host` 条件属性，原 Tauri 入口和测试保持可用。
当前 183 个处理函数进入鸿蒙运行层；本地进程、CLI 等宿主专属命令明确不可用。

保险库格式、SSH 算法、同步协议、AI 工具执行和 React 页面均直接复用原实现。
`hatoba-net` 只隔离 HTTP 客户端的系统信任根来源。`platform/harmonyos` 承担 Node-API、
Asset、文件 FD、生命周期和 ArkWeb。对上游的必要改动集中在 host 导入、命令条件属性和少量系统钩子。
以后上游新增命令时，先审查生成差异和它依赖的系统服务，不能只以命令名进入列表作为验收。

部分共享回归使用桌面 stdio 子进程 fixture，仅在桌面宿主运行；鸿蒙仍运行其余共享回归，
并单独验证 stdio 配置保留、秘密不回显、手动启用后仍拒绝执行。不能为了复用测试而打开生产进程能力。

## 升级步骤

1. 保留当前可构建版本，确认工作区干净，拉取 `upstream`，选定具体 tag 或 commit。
2. `node scripts/harmonyos/upstream.mjs <candidate> --check`。
   接口或核心/依赖文件改变时返回非零并给出清单；即使没有接口变化，行为变化仍需回归。
3. 在本地集成分支 merge 候选版本，保留上游历史。平台修复与通用改造分开提交，避免格式化上游文件。
4. 运行 `node scripts/harmonyos/contracts.mjs --write`，人工审查生成差异。新增命令自动进入映射；
   原生未实现的命令须明确返回不可用，不能伪装成功。输入/输出变化要同步处理并增加行为测试。
5. 执行下面的自动检查及上游桌面 CI。涉及 Rust 依赖时重新构建 OHOS，不只依赖本机测试。
6. 根据变更执行真机矩阵：终端/IME、文件、TLS/同步、AI 审批、生命周期。记录 SDK、系统与设备类型。
7. 全部通过后才更新 `upstream.json` 的 commit/version，并在验证记录列出兼容例外。

```sh
node scripts/harmonyos/contracts.mjs --check
node --test scripts/harmonyos/contracts.test.mjs
node scripts/harmonyos/check-boundaries.mjs
pnpm --filter @hatoba/desktop typecheck
pnpm --filter @hatoba/desktop lint
pnpm --filter @hatoba/desktop exec vitest run src/ipc/tauri.test.ts src/ipc/harmonyos/bridge.test.ts
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
node scripts/ci/comment-width.mjs
pnpm ohos:build
```

`check-boundaries` 需要依赖已获取，可先运行 native build。新增依赖解析必须继续使用锁文件；
协议、加密数据格式和云端 API 不得随平台适配自行分叉。

## 原生宿主协议 v1

页面使用固定虚拟 origin `https://hatoba.invalid`。`onInterceptRequest` 将合法 GET 映射到包内
rawfiles，其他请求直接返回403，缺失资源404；没有监听端口、外部站点或 DNS 依赖。路径检查拒绝
编码、查询、目录穿越和不同 origin。导航仅开放入口文档与 fragment。CSP 禁止远程脚本和 frames。

页面加载后，ArkTS 向这个固定 origin 投递 `oh-hatoba-host/v1` 与一个 WebMessagePort。
使用虚拟 HTTPS origin 是为了兼容 Vite ES Module 和动态 chunk 的同源加载；`resource://` 在
真机受到 CORS 拦截，不能通过关闭浏览器安全检查解决。普通浏览器不会得到伪造的 native 结果。

- JSON 消息携带 `v=1`；调用为 `type=call,id,command,args`，返回为 `type=reply,id,ok,result/error`。
- 事件为 `type=event,event,payload`；结构化流为 `type=channel,channel,payload`。
- 二进制消息前四字节为 little-endian channel ID，后面保持上游原始 TermFrame，不能先转 UTF-8。
- 消费后发送 `type=ack,channel,bytes`；后续 Rust 流生产者必须按确认额度限制在途数据。
- 最多64个待回复请求、128个 channel；二进制 payload 最大256 KiB，JSON 最多1,048,576个 JS 字符。
- 超时发送 cancel，原生取消实际分发任务。页面销毁关闭队列、取消请求、锁定保险库并关闭会话。
  成功创建的长连接流在释放时发送 channel_close。二进制额度上限512 KiB，按帧确认且必须按序；
  Web 在 xterm 消费回调完成后确认，生产者在无额度时等待并有超时。

普通入口使用共享业务分发器。`host_probe` 与独立诊断页用于生成数据验证；网络诊断必须显式打开
诊断 Want 参数，且使用隔离的诊断保险库。原生系统调用走另一条队列，不向 Web 暴露生物识别 token 或密钥。
Web 只得到文件选择器产生的不透明 grant；上传直接读 FD，下载先写私有暂存并提交到获授权 FD。
上游导出的内嵌私钥只从已选文件解析；配置中引用的其他文件需要在本机另行选择，不能凭路径读取。

请求/回复仍受有界消息和队列限制，超大结果应给出明确错误。应用回收后不恢复秘密或伪造活跃会话。
页面正常重建会关闭旧运行层，异步初始化使用 generation/epoch 防止旧页重新附着。

## 本地签名

DevEco生成当前包名的专用调试配置后，运行 `local-signing.mjs capture`。唯一产品/唯一签名时
补全 IDE 可能遗漏的关联；多个候选时拒绝猜测。配置放在忽略文件中，权限0600。
Hvigor在根节点 `afterNodeEvaluate` 中合并本地签名，早于子模块创建签名任务；
`nodesEvaluated` 时再改产品已经太晚，会构建成功但跳过签名。
构建前清除旧签名产物，本地签名开启时必须检查本次新生成的签名 HAP。
