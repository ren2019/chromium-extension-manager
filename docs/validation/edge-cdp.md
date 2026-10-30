# Edge 按需连接与扩展操作验证

2026-10-30，macOS。用户明确主要使用 Edge，重新授权这次 Edge 最小验证；Windows 验证仍不恢复。结果不代表原生 CEPM 已接入 CDP。

## 结果

| 环境 / 操作 | 实测结果 |
| --- | --- |
| Edge 156.0.4314.50，独立临时配置，读取扩展 | `Extensions.getExtensions` 成功，返回 ID、路径、版本、enabled。 |
| 同环境，`Extensions.loadUnpacked` | 成功；本次不需要 `--enable-unsafe-extension-debugging`。仅测试一个 MV3 样例。 |
| 加载后彻底关闭并重启同一测试配置 | 列表为空，临时加载不持久，不能作为 CEPM 首次安装完成。 |
| 重新加载样例后 `Extensions.uninstall` | 成功；回读列表为空，再次重启仍为空。没有操作用户扩展。 |
| 日常 Edge 156.0.4314.50，用户在 inspect 页面开启并批准连接 | 从该日常用户数据目录的 DevToolsActivePort 连接成功，无需重启。 |
| 日常配置只读 `getExtensions` | 返回一份未打包扩展，安装路径属于 CEPM；版本 1.0.26、enabled=true，ID 与路径字段均可用。没有写入、卸载或重载现有扩展。 |
| 结束 | 探针关闭 WebSocket；用户确认取消浏览器调试开关，9222 监听消失。旧测试实例经用户单独要求关闭。 |

复现：在原型分支运行 `node prototypes/visual-refresh/probes/edge-cdp.mjs`。脚本创建全新临时配置和 MV3 样例，输出报告，关闭它自己启动的浏览器；不访问日常配置。本次去除绝对临时目录后的结果见 `edge-cdp-result.json`（同原型目录的 probes 下）。日常只读探针不包含在自动复现脚本中，以免脚本擅自接入真实浏览器。

## 尚未证明的能力

- 日常配置中的真实卸载未测试；独立配置只卸载了 CDP 加载的样例。正式上线前还需独立配置内测试“UI 持久加载 → CDP 卸载”及清理失败。
- 单个浏览器实例中的多个 Profile 绑定与隔离尚未验证。`Extensions` 接口没有 Profile 参数，不能将当前 UI 选择直接视作 CDP 操作目标。必须核对配置绑定、固定路径和 ID；无法证明时禁止自动写操作。
- 自动持久加载与不改变安装性质的自动重载未验证。不能用重新调用 `loadUnpacked` 冒充普通更新重载。
- `getExtensions` 的 enabled 和版本不是扩展全部功能的健康证明，也不是任意商店扩展的清单。
- Windows、其他 Edge 版本、企业策略限制没有实测；Chrome 的能力也不从 Edge 结果反推。

## 连接生命周期

按用户希望，在安装、核对或卸载需要时请求本次连接；完成、取消、超时或失联后结束 APP 会话，不后台自动重连。首次开通浏览器调试需要用户在浏览器批准，不能由尚未连接的 CDP 自行开启。

“APP 断开”不等于“浏览器关闭调试”。Chromium 将此开关注册为 Local State 布尔偏好，启动时读取并决定是否监听；因此不能向用户保证重启自动关闭。Edge 本轮未为验证开关持久性而重启日常浏览器。关闭开关仍通过浏览器界面，或将来经单独验证的官方能力；不修改内部配置文件，也不为了关闭开关擅自重启浏览器。

产品不以常驻管理器或开机重新注入来规避持久安装要求；这保持 ADR 0001 的决定。

## 主要依据

- [微软：连接正在运行的 Edge](https://learn.microsoft.com/en-us/microsoft-edge/web-platform/devtools-mcp-server#auto-connect-to-a-running-edge-instance)
- [CDP Extensions 定义](https://chromedevtools.github.io/devtools-protocol/tot/Extensions/)
- [Chromium：重启移除 CDP 安装扩展](https://github.com/chromium/chromium/commit/03338921851f5a0773c4d6b68586133851e2ca4e)
- [调试开关偏好注册](https://github.com/chromium/chromium/blob/main/chrome/browser/browser_process_impl.cc)、[启动读取偏好及连接批准](https://github.com/chromium/chromium/blob/main/chrome/browser/devtools/remote_debugging_server.cc)
