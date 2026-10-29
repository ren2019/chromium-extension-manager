# Chromium Extension Package Manager

非商店 Chromium 扩展包管理器（CEPM），为预构建扩展提供安装、版本检查、更新和结果验证能力。

## 项目起点

[产品需求文档（PRD v0.3）](./prd-chromium-extension-package-manager.md) 从 Downloads 导入，初始原文保存在首次提交中；设计访谈决策已合入正文，第 17 节汇总当前基线。

设计访谈已收口：核心确定使用 Rust，GUI 优先验证 GPUI/gpui-kit。最小实验已通过 macOS 交互、双平台构建及 macOS Chrome 持久安装验证；Windows 桌面交互和 Edge 持久性仍待验证，产品实现尚未开始。

## MVP 范围

- 目标平台：Windows 和 macOS，优先支持 Microsoft Edge 和 Google Chrome。
- 扩展来源：公开及私有 GitHub Release ZIP、仓库内可直接加载的扩展子目录、本地目录；首版不自动构建源码。
- 远程获取与认证：调用相应外部 CLI，由 CLI 管理 Token；本项目不提供登录或凭据管理。
- 核心流程：添加来源、下载校验、固定目录安装、检查更新、选择版本（含降级）和生效版本验证。
- 浏览器加载：优先服务现有日常配置，允许首次手动加载；受管环境作为可选的自动化模式。
- 生效确认：可靠自动检查不可用时，允许用户手动重载并核对版本，结果明确标记为用户确认。
- 可靠性要求：更新前备份，失败不破坏旧版，检测本地修改，区分磁盘版本与实际生效版本。
- 首个验收案例：OpenCLI Browser Bridge。

完整需求、优先级和验收标准以 PRD 为准。

## 实施起点

先完成 PRD 第 15 节列出的技术验证。当前实验支持“固定安装目录 + 首次手动加载 + 更新后手动重载”的最小路径；磁盘安装版本与生效版本必须分开记录。

- [最小验证结果、证据与剩余检查](./docs/validation/minimal-feasibility.md)
- [实验代码及运行方法](https://github.com/ren2019/chromium-extension-manager/tree/prototype/gpui-browser-feasibility/prototypes)（仅在实验分支）
- [Windows / macOS 构建与可下载制品](https://github.com/ren2019/chromium-extension-manager/actions/runs/40826295319)

## 协作配置

任务与规格使用 [GitHub Issues](https://github.com/ren2019/chromium-extension-manager/issues) 跟踪。

Agent 入口为 [AGENTS.md](./AGENTS.md)，具体约定见：

- [任务跟踪](./docs/agents/issue-tracker.md)
- [分流标签](./docs/agents/triage-labels.md)
- [领域文档](./docs/agents/domain.md)

这些配置可直接编辑。术语表和架构决策记录在实际需要时创建。

## 设计调研

- [OpenLogi 技术栈、技能与 AST 工具](./docs/research/openlogi-stack.md)
- [GitHub 扩展发布范式样本](./docs/research/extension-distribution.md)
- [本地目录版本声明](./docs/research/local-directory-version.md)
- [私有 GitHub 认证方案](./docs/research/private-github-auth.md)
- [浏览器状态验证与重载](./docs/research/browser-verification.md)
- [已确认：扩展安装必须独立于管理器持续可用](./docs/adr/0001-persistent-browser-installation.md)

调研文档中的建议不等同于已确认决策；产品范围以 PRD 中的访谈记录为准。
