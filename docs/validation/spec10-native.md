# Spec #10 原生验收记录

2026-10-30，macOS，原生 GPUI 正常窗口 1120×780。设计依据为 #10 固定的获批原型 959eefc。下述状态来自原生应用，非 HTML。

## 已验证：浏览与首次加载交接

使用独立 CEPM_DATA_DIR，经 Manager 公开用例创建两份同来源 OpenCLI 和一份本地长名称扩展；这些是界面验收记录，不冒称它们已在浏览器真实加载。

- 扩展库显示实际清单图标，缺图标使用拼图；空配置显示添加入口。
- Profile 浮层不推动布局；两项置顶不改变当前目标；切换至空配置关闭详情。重开恢复上次选择及两项置顶。
- 右侧全页详情可返回；各配置安装情况默认折叠，展开并点击另一配置后，目标、详情和数量同步切换。
- 长配置名称省略且有完整可读名称；导航、置顶、跨配置入口均有可读名称。单一浅色标题栏显示 Logo 与全称；关于页显示实际 0.1.0 版本与项目/反馈入口。
- 首次加载无 ID 输入。点击复制后实际剪贴板内容与本记录的完整绝对安装目录逐字相同，无引号、换行或省略；复制后记录仍 Pending。
- 在独立验收记录中提交人工确认后，状态为 UserConfirmed、ID 仍为空、确认版本 1.0.26，保留待识别；另一配置和本地扩展仍 Pending。此项仅验证用户确认工作流，不代表自动读取浏览器。

图像：

- [空列表](spec10-images/11-empty.png)
- [Profile 浮层](spec10-images/11-profile-menu.png)
- [完整详情](spec10-images/11-detail.png)
- [关于](spec10-images/11-about.png)
- [无 ID 人工确认](spec10-images/12-manual-confirmed.png)

## 已验证：更新失败、重装和恢复

在独立本地扩展记录中确认 2.1.0，再把受控来源更新为 2.2.0。通过原生界面执行检查、部署，结果为单次提醒；反馈失败后，实际固定目录仍是 2.2.0、原备份仍是 2.1.0，历史确认保留，主动作是重新安装。明确同版本重新部署后，关闭提醒不提交成功；再次从详情显式恢复，实际文件与记录均为 2.1.0、generation=4、Pending。安装时间不变，成功恢复刷新更新时间。整个过程没有调用真实浏览器。

- [紧凑加载交接](spec10-images/12-compact-load.png)
- [更新结果单次确认](spec10-images/13-update-confirm.png)
- [失败处理详情](spec10-images/13-update-failed.png)

键盘原生检查：Tab 后 Return 打开 Profile 菜单，菜单内可循环导航，Escape 返回触发按钮后可再次 Return 打开；焦点框可见。随后统一为蓝色焦点框。

- [菜单键盘焦点](spec10-images/11-keyboard-menu.png)

## 已验证：手动卸载后的清理重试

只对上述独立测试记录操作，未触及日常 Edge。将临时安装目录设为不可读后，在原生卸载确认中勾选浏览器已移除并提交；清理失败保留 `BrowserRemoved` 阶段、错误和记录，界面提供“重试清理”，不再要求重复确认浏览器移除。恢复该临时目录权限后重试成功：固定目录和记录消失，来源目录仍存在，两份 OpenCLI 的记录哈希完全不变，操作历史保留一条成功卸载。

- [清理失败与重试](spec10-images/15-cleanup-failed.png)

## 已验证：原生添加与批量结果

在原生添加流程通过系统文件夹选择器选择带中文的临时来源，摘要确认后创建新受管安装，关闭加载提醒仍为 Pending。暂时移开此测试来源后批量检查：本地来源明确失败，另一份 OpenCLI 独立报告无需更新。恢复来源并变更至 2.3.0 后再次批量部署：仅本地项的实际文件与记录变为 2.3.0 / Pending；OpenCLI 跳过，其他配置不变。该验证未操作真实浏览器。

- [安装摘要](spec10-images/16-add-summary.png)
- [检查失败与无需更新并存](spec10-images/16-mixed-check.png)
- [部署与跳过分别反馈](spec10-images/16-batch-result.png)

## 第一轮修复与本机安装包复验（7548e6f）

最终代码 `7548e6f`，已按正常打包脚本生成 Apple Silicon debug 包，并替换 `/Applications/CEPM.app`。原程序副本保留在本机临时验收目录以便回退。安装后二进制与生成包 SHA-256 均为 `940c60e339e8844adfb87de483cda284e78314f7db7a417819f0619cd0a74f0c`。

- 更新后的实际应用成功打开既有数据；全称、Logo、OpenCLI 自身图标、全页详情、折叠的两配置对照及关于页正常。
- 在独立测试数据再次打开恢复对话框，详情仍在背景；Escape 返回原详情，未执行恢复。
- 再次制造临时目录权限错误：卸载清理错误只显示一次，详情保留“清理未完成”；恢复权限重试成功，列表数量减少，历史仍显示已移除扩展的完整名称及此前事件。
- GitHub Release 原生预览从 `jackwener/OpenCLI` 获取 `v2.0.2`，正确解析附件的扩展版本 `1.0.26`；关闭预览不部署。
- 更新本机程序前后，两份日常受管安装（Edge / Chrome）的记录与全部安装文件哈希逐一相同。本次未卸载、重装、重启日常浏览器，也未开启日常调试连接。
- 四项规格审查问题修复后已做针对性复审，无遗留代码问题；规范审查的两项维护建议已修复。见 [审查记录](spec10-review.md)。

截图：

- [旧版原生界面](spec10-images/00-installed-before.png)
- [安装后的扩展库](spec10-images/17-installed-library.png)
- [安装后的真实详情](spec10-images/17-installed-detail.png)
- [安装后的关于页](spec10-images/17-installed-about.png)
- [保留详情背景的恢复确认](spec10-images/17-restore-dialog-final.png)
- [最终清理失败提示](spec10-images/17-cleanup-error-final.png)
- [卸载后可辨认的历史](spec10-images/17-uninstall-history-final.png)
- [官方 Release 预览](spec10-images/16-release-preview.png)

## 最终 Edge 验收

2026-10-30 收到用户行动前授权后，已完成独立配置真实持久安装/自动卸载/重启缺席、其他配置保留、共享多 Profile 拒绝，以及日常 Edge 的 OpenCLI 卸载、原官方来源重装、重启持久性和实际连接。临时调试已关闭；Chrome 的受管记录与全部文件哈希未变。详见 [Edge 验收记录](spec10-edge-acceptance.md)。

应用默认启用已验收的窄范围卸载适配器；只允许已验证 macOS Edge 156.0.4314.50、单普通 Profile 根目录和精确受管安装，其他情况保留手动流程。首次持久加载仍使用浏览器原生入口。
