# Chromium Extension Package Manager

CEPM 是管理非商店 Chromium 扩展的 Rust + GPUI 桌面应用：把本地目录、GitHub Release ZIP 或仓库子目录中的完整扩展产物安装到固定目录，随后检查版本、更新和恢复旧文件。

先在左侧选择浏览器与 Profile，再维护该配置中的安装。多个常用配置可以置顶；重开应用会恢复最后选择。文件部署和浏览器生效分别记录，首次加载、更新后重载与版本核对通过浏览器原生扩展页完成。

Windows/macOS、Chrome/Edge 是产品目标。macOS Chrome 与 Edge 的主 Profile 已完成 OpenCLI 安装、连接和重启持久性验收；Windows 运行尚未验证，不能把跨平台构建结果视为完整兼容性保证。详见[本机主 Profile 验收](./docs/validation/main-profile-installation.md)。

## 获取与运行

[GitHub Actions](https://github.com/ren2019/chromium-extension-manager/actions/workflows/ci.yml) 在检查通过后提供 `CEPM-macos-<架构>-debug` 制品。下载并解开其中的 ZIP 后打开 `CEPM.app`。制品是运行机器架构的开发构建，**未使用 Developer ID 签名或公证**，不是通用二进制；下载件可能受到 macOS 安全策略限制。也可以按下面步骤从源码构建。

仅安装本地目录不需要 GitHub CLI。GitHub 来源要求已安装并授权的 `gh`，权限和 Token 由它管理：

```sh
gh auth login
gh auth status
```

macOS 从 Finder 启动时可能没有终端的完整 PATH。CEPM 会在找不到 `gh` 时尝试 Homebrew 常用路径 `/opt/homebrew/bin/gh` 和 `/usr/local/bin/gh`。自定义安装位置应加入启动进程的 PATH；也可从已能运行 `gh auth status` 的终端直接启动应用中的可执行文件：

```sh
/path/to/CEPM.app/Contents/MacOS/cepm-app
```

## 使用流程

1. **选择配置**：在左侧顶部添加或选择具体浏览器 Profile。可使用发现的候选，也可从 `chrome://version` / `edge://version` 复制 Profile Path，并指定浏览器应用。配置必须已存在；找不到时暂停维护，不自动改用其他配置。
2. **添加来源**：选择本地完整扩展目录；或输入 GitHub 仓库/Release 地址并选择标签和 ZIP；或指定仓库、分支/标签/提交与扩展子目录。工具不会构建源码。安装版本以 `manifest.json.version` 为准，不必等于仓库 Release 标签。
3. **确认并安装**：预览扩展名称、清单版本和目标配置，再确认。存在多个附件或扩展根目录时由你选择。CEPM 校验后复制到独立的固定安装目录，来源目录可另行移动。
4. **首次加载**：请求打开当前 Profile 的原生扩展管理页；若浏览器显示新标签页，使用旁边的“复制地址”在该窗口粘贴。开启开发者模式，选择“加载已解压的扩展程序”，使用 CEPM 提供的固定目录。核对浏览器中显示的 ID、版本和 Profile 后回到 CEPM 确认。打开管理页或完成复制都不会自动标为已生效。
5. **维护更新**：检查当前配置中的安装，预览目标版本后更新。批量操作仅执行普通升级，逐项报告结果；一项失败不撤销其他项。旧版降级与同版本内容变化均单独确认。部署后在原生页重载，再回到 CEPM 核对。

同一扩展在不同 Profile 是独立安装。原生页继续负责启停、权限和移除；CEPM 不接管已有扩展，也不会自动关闭浏览器。退出 CEPM 后固定目录仍保留，扩展可以继续使用。

## 失败与恢复

获取、归档或清单校验失败时修复提示中的原因后重试。固定安装目录被手动修改时更新会暂停，避免覆盖修改。部署后明确加载失败，可在详情中报告失败并恢复旧文件；恢复后仍需浏览器重载和核对。无法读取浏览器状态只表示待验证，不等于加载失败。

旧生效确认在更新后仅作历史记录。文件恢复不能撤销扩展自己已执行的数据迁移；不要把降级视为扩展数据的无损回滚。恢复异常会显示“恢复需要处理”，此时先保留安装目录和备份，按错误说明处理。

## 数据与开发

应用使用操作系统的用户应用数据目录，保存安装记录、Profile 偏好、固定安装目录及必要的恢复文件。**不要移动或删除该数据目录**，浏览器加载的固定路径依赖它。复制或重新构建 `CEPM.app` 不会迁移、清除这些数据。

源码构建需要 Rust 工具链和 macOS Command Line Tools，原生依赖以 GPUI 为准：

```sh
cargo run -p cepm-app --locked
```

制作本机架构的 `.app` 和 ZIP（默认 debug 开发构建；可传 `release`）：

```sh
scripts/package-macos.sh debug
open target/bundle/debug/CEPM.app
```

脚本输出到 Cargo 目标目录的 `bundle/<debug|release>/`，ZIP 附带 SHA-256 文件。它只包装本机默认目标，不执行签名或公证；设置了 `CARGO_BUILD_TARGET` 时应先取消设置。

测试数据可单独存放，正常运行应持续使用同一路径：

```sh
CEPM_DATA_DIR="$PWD/.scratch/app-data" cargo run -p cepm-app --locked
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

主要测试入口是 Rust 安装用例与真实临时文件系统，仅在外部 CLI 边界提供可控响应。原生 GUI 和实际浏览器流程采用少量 macOS 冒烟验收；通过项与限制见 [MVP 集成验收](./docs/validation/mvp-acceptance.md)。

## 规格与协作

- [PRD v0.5](./prd-chromium-extension-package-manager.md)、[MVP 实现规格](./docs/plans/mvp-spec.md)、[已批准的 A 用户旅程](./docs/plans/approved-user-journey.md)
- [规格与 5 个开发任务](https://github.com/ren2019/chromium-extension-manager/issues/2)、[GitHub Issues](https://github.com/ren2019/chromium-extension-manager/issues)
- [最小实验及未验证范围](./docs/validation/minimal-feasibility.md)、[独立 HTML 原型分支](https://github.com/ren2019/chromium-extension-manager/tree/prototype/user-journey/prototypes/user-journey)
- [AGENTS.md](./AGENTS.md)、[任务跟踪约定](./docs/agents/issue-tracker.md)、[领域术语](./GLOSSARY.md)、[架构决策](./docs/adr/)
- [OpenLogi 技术栈调研](./docs/research/openlogi-stack.md)、[扩展发布范式调研](./docs/research/extension-distribution.md)

初始 PRD 从 Downloads 导入，原文保存在首次提交中。后续访谈决策和批准的原型共同构成当前实现基线。
