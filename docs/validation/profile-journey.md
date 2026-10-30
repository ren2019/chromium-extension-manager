# 浏览器配置与 A 用户旅程实现

已批准测试入口：GUI 调用的 Rust 用例，使用真实临时目录和重开后的设置，不测试私有实现。运行 `cargo test -p cepm-core --test profiles --locked`。

逐项先红后绿实现：

- 同名但不同目录的配置身份独立；多个置顶与最后选择重开后保持，取消置顶不改变选择。
- 只从 Local State 的配置名称/目录索引发现候选；缺失目标保持原选择，不回退到其他可用配置。
- 原生扩展页请求绑定具体可执行文件与独立参数中的用户数据目录/Profile；缺失位置拒绝请求。
- 旧文本标签安装经明确定位后保留固定目录，清除原生效确认，禁止把已绑定安装当成可迁移旧记录。
- 设置损坏明确失败，不以空设置覆盖或静默回退。

GPUI 按已批准 A 重写：左侧顶部紧凑切换及置顶、按配置过滤列表、待办和记录、分步本地添加、详情、浏览器交接与核对。未定位旧记录不会自动混入当前配置。所有文件产物操作在后台执行；原生页面启动只报告请求发出，不作为生效证据。

截至本阶段，5 项配置测试、应用编译、全 workspace clippy 通过；macOS 实际 UI 冒烟由集成分支统一进行。Windows/Edge 专项验证按用户决定不执行。此记录不声称真实浏览器路径路由已完成验收。

Safe-update integration: GUI exposes frozen local version preview, explicit downgrade/same-version choice, file deployment, browser reload confirmation tied to generation, and load-failure recovery. Recovery-required records are visible and cannot be confirmed or updated. History shows the latest durable operation; it is not an exhaustive event log. Legacy binding explicitly accepts the current validated file baseline, bumps generation and clears effective state while retaining historical confirmation.

A sixth test first reproduced acceptance of an empty directory, then verified that binding rejects an empty directory and user-data root. Bind/launch require the existing Profile Preferences file without reading its contents, and reject invalid Profile directory components. All six profile tests pass with the new update core.

Remote sources now use the same add journey: GitHub Release list → arbitrary tag → explicit ZIP → explicit extension root → frozen preview; repository URL → ref → prebuilt subdirectory → frozen preview. Existing installs can choose another local or remote version, with explicit downgrade/same-version confirmation. Network/archive work remains on the background executor. Release lists have bounded scrolling; modals have bounded content scrolling.

Batch seam `Manager::check_target_updates` freezes candidates/errors only for one exact target; `apply_target_upgrades` validates that target again and deploys only Upgrade plans. Two real-filesystem tests cover local + repository + Release, a missing source, a post-preview conflict, independent successful items surviving reopen, unchanged other Profile, cross-target rejection, and skipped downgrade/redeploy. Full integrated core: 38 tests pass; workspace clippy passes.

Native routing limitation observed in root's macOS Chrome smoke: the correct isolated Profile opens, but Chrome can discard the `chrome://extensions` launch argument and display a new tab. The sidebar, detail and guide therefore expose copy-address fallback and explain pasting into the correct window. This matches Chromium's current [launch URL validation](https://github.com/chromium/chromium/blob/main/chrome/browser/ui/startup/url_util.cc): ordinary desktop command-line navigation does not allow arbitrary internal pages. A request is never represented as successful page navigation or extension verification. No browser safety flags or helper extension are added.
