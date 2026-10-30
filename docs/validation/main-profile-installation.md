# 本机安装与主 Profile 验收

2026-10-29。当前目标明确要求 macOS Chrome 和 Edge 的主 Profile，覆盖此前停止 Edge 专项验证的范围决定。旧隔离 Chrome 验收没有替代本次主 Profile 安装。

## 安装与身份

- 简化后的应用安装到 `/Applications/CEPM.app`，通过 LaunchServices 启动；使用操作系统应用数据目录，未复用隔离验收数据。
- 两个浏览器分别通过 version 页核对日常用户数据目录下的 `Default`。CEPM 分别绑定具体应用、用户数据目录和 Profile，两个配置均置顶，重开恢复最后选择。
- 两次安装均从应用的 GitHub Release 流程获取 `jackwener/OpenCLI` v2.0.2 的 `opencli-extension-v1.0.26.zip`，校验后复制到独立固定目录。不是直接写入安装记录。
- 浏览器正常「加载已解压的扩展」后，分别核对独立 ID、1.0.26 和启用状态，并在 CEPM 保存为用户确认。既有 Edge 商店版 1.0.24 和其数据保留，未卸载或覆盖。
- 各自 OpenCLI 页面显示独立连接身份和 daemon 连接；分别限定身份的实时 Connectivity 探测成功。未修改浏览器内部配置文件，也未升级已有 CLI 或重启已有 daemon。

## 重启持久性

1. 通过应用关闭按钮退出本机 CEPM 和旧验收版，通过浏览器菜单正常退出两个主 Profile 的浏览器进程。
2. 检查两个主浏览器进程已退出；各自健康查询均变为 `profile-disconnected`。另一项无关的 Edge 原型进程保留。
3. 重开 CEPM，两个安装记录、置顶与最后选择保留；分别启动绑定的 Chrome、Edge 主 Profile，然后退出 CEPM。
4. 确认没有 CEPM 进程；两个浏览器主 Profile 以新进程运行。此前记录的两个连接身份分别恢复 `ready`、`extensionConnected: true`、`extensionVersion: 1.0.26`。
5. 再分别执行限定连接身份的 OpenCLI doctor，两条实时 Connectivity 均成功。

## 诊断限制

现有 OpenCLI CLI 1.8.6 的 doctor 在实时探测后调用未指定身份的 `getDaemonHealth()`。本机多个连接同时存在时，该调用返回 `profile-required`，却被 doctor 的 `extensionFlaky` 分支显示为 unstable。已核对本机源码及实际返回；分别指定两个已核验身份的健康查询持续为 ready。因此不声称 doctor 全部项目无警告，也不为消除显示警告修改全局默认 Profile。

## 开发验证

- `cargo fmt --all --check`、`cargo test --workspace --locked`、workspace all-targets Clippy 通过；46 项核心测试通过。
- 最终本机二进制与打包二进制 SHA-256 相同：`d9e5f6983e297ec75018dcc6581ccad4dde2d9c58988fdd14ae48c6cff9e42e6`。
- 原生界面实测简化列表、分步加载、首次 ID 确认、两个配置独立记录、多项置顶及重开恢复。版本核对仍是明确的用户确认，不冒充自动读取。
- 本轮是 macOS 主 Profile 验收；Windows 运行、Developer ID 签名和公证仍不在通过范围内。

## 简化交互的最终复核

- 无更新时直接回到列表，仅显示“没有发现新版本”，不出现额外批量操作页。
- 任意版本列表可展开/收起；只有一个有效扩展目录时直接进入安装预览，确认页只有一个“安装”主操作。预览验证退出后未创建额外安装记录。
- 后续加载指引不再要求输入 ID/版本或勾选声明，最终按钮明确询问指定版本是否已显示。
- Standards 审查：硬性违反 0、判断性问题 0。Spec 审查发现 1 项 P2：启动失败后的手动继续缺失；已在 `d72c02b` 修复并复核。
- 用独立验收记录实际点击“遇到问题 → 已手动打开，继续 → 已加载，下一步”，到达版本确认页；未点击最终确认，两份记录 SHA-256 均保持不变。这证明推进引导没有提前记录生效。
