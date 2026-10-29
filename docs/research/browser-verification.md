# 日常浏览器中的扩展验证与重载

核查日期：2026-10-29。范围：Windows/macOS、Chrome/Edge、持久安装、管理器无需常驻；仅查官方资料，未访问真实浏览器或执行兼容性测试。

后续访谈已确认：首版接受人工重载和版本核对兜底，明确标记用户确认；不把额外辅助扩展作为首版前置要求。以下保留各技术路径的研究，实际范围以 PRD v0.3 为准。

## 已确认事实

- `chrome.management.getAll()` 返回已安装扩展列表，`get(id)` 返回指定扩展信息；需要 `management` 权限。信息含 ID、名称、版本、启用状态、安装类型；`development` 表示开发者模式下加载的已解压扩展。[Management API](https://developer.chrome.com/docs/extensions/reference/api/management)
- `ExtensionInfo` 没有安装目录或浏览器配置路径字段。因此该接口本身不能证明某条记录对应 CEPM 的固定安装目录；也不能仅凭名称和版本排除另一份同名安装。此项是对官方字段表的推论。[字段定义](https://developer.chrome.com/docs/extensions/reference/api/management#type-ExtensionInfo)
- Management 提供安装、卸载、启用和禁用事件。事件可辅助维护状态，但停机后应重新读取快照；快照同步属于本产品方案，不是事件永不丢失的保证。[Management 事件](https://developer.chrome.com/docs/extensions/reference/api/management#events)
- `setEnabled(id, enabled)` 是启用/禁用操作，文档说明通常需要用户手势，且可能弹出浏览器确认。官方 Management API 没有 `reload(id)`；禁用再启用不能作为“必然重新读取所有磁盘文件”的契约。[setEnabled](https://developer.chrome.com/docs/extensions/reference/api/management#method-setEnabled)
- `chrome.runtime.reload()` 没有目标 ID 参数，作用于调用它的扩展；辅助扩展调用它只会重载自身。第三方扩展若自行实现外部消息处理，可以协作执行自重载，但普通扩展没有统一的这类协议。[Runtime API](https://developer.chrome.com/docs/extensions/reference/api/runtime#method-reload)、[外部消息](https://developer.chrome.com/docs/extensions/reference/api/runtime#event-onMessageExternal)
- Chrome 官方指引要求修改 manifest、service worker、content scripts 后重载扩展；content scripts 还需要刷新宿主网页。扩展页面的刷新图标是明确支持的人工路径。[Hello World：重载](https://developer.chrome.com/docs/extensions/get-started/tutorial/hello-world#reload)
- “启用”不是功能健康证明。MV3 service worker 可以正常休眠；没有活跃 worker 不能直接判失败，处于启用状态也不能证明业务逻辑成功。这是结合状态字段和生命周期的推论。[Service worker 生命周期](https://developer.chrome.com/docs/extensions/develop/concepts/service-workers/lifecycle)

## 辅助扩展与 Native Messaging

- 辅助扩展可用 `management` 读取浏览器状态，再用 `nativeMessaging` 交给 Rust host；这两个权限需要显式声明。Native Messaging 是扩展与本机程序间的传输通道，不增加读取或重载其他扩展的权限。[Chrome Native Messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)
- `sendNativeMessage()` 每次启动一个 host；`connectNative()` 启动 host 并维持到端口断开。因此可以设计按需验证，无需管理器永久运行；若保持连接则 host 会持续存在。[进程生命周期](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging#native-messaging-protocol)
- Windows/macOS 的常规连接由扩展发起；`runtime.onConnectNative` 反向连接事件只支持 ChromeOS。桌面 GUI 想立即唤起未连接的辅助扩展，需要另设计明确触发方式，不能假设 Native Messaging 自带桌面主动呼叫能力。[Runtime 原生连接](https://developer.chrome.com/docs/extensions/reference/api/runtime#event-onConnectNative)
- Host 需要安装注册，并用 `allowed_origins` 限定辅助扩展 ID；Windows 使用注册表位置，macOS 使用规定目录下的 host manifest。Edge 另有对应注册位置，不能只注册 Chrome 就假定 Edge 可用。[Chrome 注册](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging#native-messaging-host-location)、[Edge 注册](https://learn.microsoft.com/en-us/microsoft-edge/extensions/developer-guide/native-messaging)
- Edge 官方列出 Windows/Mac 支持 `management`、`runtime`；其 Native Messaging 文档覆盖两平台。用户级 host 可受策略限制，安装后的连通性仍须检查。[Edge API 支持](https://learn.microsoft.com/en-us/microsoft-edge/extensions/developer-guide/api-support)、[用户级 host 策略](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-policies/nativemessaginguserlevelhosts)

## 不安装辅助扩展的路径

- **人工验证方案**：引导用户在目标配置的扩展页面完成首次加载、Reload、核对 ID/版本/启用状态。管理器记录为“用户确认”，不能标成自动验证；磁盘版本变化单独记录为“文件已更新”。
- **按需调试方案**：Chrome 144 起官方提供开启远程调试、逐次批准连接的现有会话流程，无需以调试参数重新启动；它有用户确认及自动控制提示，不是静默访问。CEPM 自己接入、目标扩展检查及 Edge 等价能力尚未验证。[Chrome 官方连接流程](https://developer.chrome.com/blog/chrome-devtools-mcp-debug-your-browser-session)
- 当前实验性 CDP `Extensions.getExtensions` 返回已解压扩展的 ID、路径、版本、启用状态；这些比 Management 更利于路径关联，但仍不证明所有功能正常。该域没有独立 `reload` 方法，是否可在目标会话中调用须实测。[Extensions 协议](https://chromedevtools.github.io/devtools-protocol/tot/Extensions/)
- 不把 `loadUnpacked` 当作持久安装或重载捷径：Chromium 已实现下次启动移除 CDP 安装扩展；不能为自动化更新破坏已确认的持久安装要求。[官方实现](https://github.com/chromium/chromium/commit/03338921851f5a0773c4d6b68586133851e2ca4e)

## 产品方案建议（待决策）

1. 区分磁盘安装版本、浏览器报告版本、用户确认结果和扩展专用健康检查；浏览器报告版本不自动等同于词汇表中要求“实际加载并运行”的生效版本。
2. 若选择辅助扩展，先定位为按需状态观察器：用户从目标配置点击“验证”，host 回传带时间与配对标识的结果；首次配对解决配置/安装关联，不能按同名自动接管已有安装。
3. 通用更新保留人工 Reload；功能健康通过可选的目标专用探针验证，例如 OpenCLI 自身的连接诊断。失败或无探针时如实报告验证范围。
4. 若要求从 GUI 一次点击完成全部重载与验证，需单独验证浏览器调试路径或第三方协作协议；辅助扩展加 Native Messaging 本身不足以保证它。

## Windows/macOS × Chrome/Edge 实测清单

- 首次手动安装后，管理器退出、浏览器彻底重启：安装、扩展 ID、用户配置及功能均保持。
- 原目录替换后的旧缓存、manifest/worker/content-script 更新，人工 Reload 与宿主网页刷新后的版本一致性。
- 辅助扩展跨配置配对、host 安装/卸载、用户手势与确认弹窗、权限增加、目标禁用/缺失、host 无连接和状态过期。
- 若试验禁用再启用：验证是否重新读取磁盘及是否保留权限/配置；通过前不宣传为通用 Reload。
- 若试验按需 CDP：验证具体稳定版本、批准流程、`getExtensions` 可用性、对持久安装无副作用；Edge 独立测证。
