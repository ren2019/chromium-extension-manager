# GPUI 与浏览器持久安装：最小验证

验证日期：2026-10-29。此记录是一次可行性实验，不是 CEPM 产品验收。

后续决定（同日）：用户要求“edge 和 windows 不再验证了。往下推进”。据此停止补测并采用 GPUI 开始实现，任务 #1 按调整后的范围关闭。下文保留实验当时的事实与未验证项；这些未测项不再是开工门槛，也没有被改写为通过。

## 结论

GPUI/gpui-kit 已通过 macOS 原生交互验证，以及 macOS ARM64 / Windows x64 构建和打包验证。Windows 桌面交互尚未实测；实验当时据此暂缓最终选型，后续用户取消该门槛，详见上方决定。

macOS Chrome 的“固定安装目录 + 首次手动加载未打包扩展 + 更新后手动重载”路径通过本次持久安装实验：浏览器完整退出再启动，扩展仍可运行；升级和降级保持相同扩展 ID 与测试存储。该路径不依赖管理器常驻，也不依赖 CDP 临时加载。

确认了磁盘安装版本和生效版本可能不一致，甚至同一个扩展页面可以读到新文件、却仍得到旧的运行时清单版本。产品必须继续明确区分这两个状态。

## 原始材料

- [可复现的实验代码与操作说明](https://github.com/ren2019/chromium-extension-manager/tree/prototype/gpui-browser-feasibility/prototypes)，仅保留在实验分支。
- [首次双平台 CI](https://github.com/ren2019/chromium-extension-manager/actions/runs/40826295319)，代码提交 `a62afbb`，两个任务均成功。
- [macOS unsigned app ZIP](https://github.com/ren2019/chromium-extension-manager/actions/runs/40826295319/artifacts/12876593221)。
- [Windows portable EXE ZIP](https://github.com/ren2019/chromium-extension-manager/actions/runs/40826295319/artifacts/12876214977)。CI 制品有保留期限，可从实验分支重新运行工作流。
- [验证任务及剩余检查 #1](https://github.com/ren2019/chromium-extension-manager/issues/1)。

## GPUI

使用 crates.io 上游 `gpui-kit = 0.8.1`、锁定解析的 `gpui-pre = 0.3.10`、`rfd = 0.15.4`，没有采用 OpenLogi 的 fork 补丁。界面只有输入框、回显、原生目录选择、100 条测试列表和按钮驱动的模拟进度。

| 检查 | macOS | Windows |
| --- | --- | --- |
| 锁定依赖构建 | 通过，本机和 CI | 通过，CI |
| 可执行文件 `--version` | 通过 | 通过 |
| 打包 | unsigned `.app` ZIP | portable `.exe` ZIP |
| 原生窗口实际显示 | 通过，本机 | 未验证 |
| 中文输入法组合输入 | 通过：输入 `zhongwen`，观察预编辑，再提交为“中文” | 未验证 |
| 中文文本粘贴与回显 | 通过：“中文扩展管理器” | 未验证 |
| 原生目录选择与中文路径 | 通过：选取 `.scratch/目录选择验证`，回显准确 | 未验证 |
| 列表滚动 | 通过：从第 001 条滚动至第 052 条附近 | 未验证 |
| 进度刷新 | 通过：两次操作至 50%，文字和进度控件一致 | 未验证 |
| 签名、公证或正式安装器 | 未验证 | 未验证 |

本机为 macOS 26.6.2（25G83）ARM64，Rust 1.99.1；仅安装 Command Line Tools，没有完整 Xcode。CI 使用 `macos-26-arm64` 与 `windows-2025-vs2026` 镜像，Rust 均为 1.99.1。`--version` 在 GUI 初始化前退出，不能据此声称 Windows 窗口、显卡兼容或中文输入通过。未做性能、窗口缩放和正常关闭验收。

## Chrome 持久安装

浏览器为 macOS Chrome 156.0.8078.74 ARM64。通过 `chrome://version` 确认专用测试配置位于 `.scratch/browser-persistence/chrome-profile/Default`，启动参数未包含 `--load-extension`、远程调试或 CDP 加载选项。扩展安装目录固定为 `.scratch/browser-persistence/installed-extension`。

测试扩展仅申请 `storage`，无网站访问权限、无联网行为。正常 UI 开启开发者模式并“加载未打包的扩展程序”。扩展自己的状态页显示运行时 ID、清单版本、页面构建版本与 `chrome.storage.local` 标记。未修改浏览器 Preferences / Secure Preferences。

| 步骤 | 运行时版本 | 页面文件版本 | 存储标记 | 结果 |
| --- | --- | --- | --- | --- |
| 首次加载 | 1.0.0 | 1.0.0 | initial | 成功 |
| 页面按钮保存标记 | 1.0.0 | 1.0.0 | cepm-persistence-survived | 成功 |
| 退出测试程序，完整退出 Chrome 后重新启动相同配置 | 1.0.0 | 1.0.0 | cepm-persistence-survived | 保留且页面可运行 |
| 固定目录写入 1.1.0，仅刷新状态页，未重载扩展 | **1.0.0** | **1.1.0** | cepm-persistence-survived | 复现磁盘/生效不一致 |
| 在扩展管理页点击重新加载，再打开状态页 | 1.1.0 | 1.1.0 | cepm-persistence-survived | 升级通过 |
| 再次完整退出 Chrome 并重新启动 | 1.1.0 | 1.1.0 | cepm-persistence-survived | 升级后持久性通过 |
| 固定目录写回 1.0.0，重载并重新打开状态页 | 1.0.0 | 1.0.0 | cepm-persistence-survived | 显式降级通过 |

以上所有阶段 ID 均为 `hkhohgenacglcgoipclmohopoliaoace`。此 ID 是当前机器与固定路径下的观测值，不保证换目录或换机器仍相同。退出通过 Chrome 菜单完成，重启前检查主浏览器进程已消失；GPUI 测试程序也已停止，文件准备脚本执行后立即退出，没有常驻安装服务。

首次运行 Chrome 时系统要求人工处理打开确认，用户完成后才继续。该环境步骤已解决。

## 尚未证明的部分

- Windows 原生窗口、中文输入、目录选择、列表及进度；正式可分发安装包。
- Windows Chrome / Edge 持久性，以及 macOS Edge 持久性。Edge 已安装，但桌面工具无法可靠绑定独立测试实例；未操作用户日常 Edge 窗口或配置。
- 日常配置中的实际扩展兼容性、OpenCLI Browser Bridge 连接验收、扩展自身的数据迁移和降级兼容性。
- 自动安装、自动重载、通用自动版本读取、异常加载后的文件恢复；本次是普通浏览器 UI 路径的实验，并不等于产品已具备这些能力。
- 完整操作系统重启、浏览器自动升级和不同企业策略环境。

本次观测支持继续实现“固定目录 + 明确等待重载/核对”的最小流程。它不支持把“文件已写入”直接标记为“更新成功”，也不支持把同一测试扩展的数据保留推断为任意扩展都能无损降级。

## 本地环境改动与复现

为执行实验安装了官方 Google Chrome 和 Homebrew Rust（含其工具链依赖）。Homebrew 安装时自动更新并升级了依赖；其自动清理移除的 `duti` 已恢复。测试配置、生成扩展、构建日志与本地 `.app` 保留在忽略提交的 `.scratch/` 中，不含于公开仓库。

实验当时建议继续 Windows 桌面交互及 Chrome / Edge 同路径检查；后续用户取消这些验证，因此停止该后续计划，保留本记录以说明现有证据边界。
