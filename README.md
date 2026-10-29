# Chromium Extension Package Manager

非商店 Chromium 扩展包管理器（CEPM），为预构建扩展提供安装、版本检查、更新和结果验证能力。

## 项目起点

[产品需求文档（PRD，Draft v0.1）](./prd-chromium-extension-package-manager.md) 是项目的初始需求基线，原文从 Downloads 导入，未作修改。

当前处于需求阶段，尚未选择技术栈或开始实现。

## MVP 范围

- 目标平台：Windows，优先支持 Microsoft Edge 和 Google Chrome。
- 扩展来源：作者发布的预构建 GitHub Release ZIP（Manifest V3）。
- 核心流程：添加来源、下载校验、固定目录安装、检查更新、一键更新和生效版本验证。
- 浏览器加载：受管配置自动加载；现有日常配置按实际能力提供一次性手动降级。
- 可靠性要求：更新前备份，失败不破坏旧版，检测本地修改，区分磁盘版本与实际生效版本。
- 首个验收案例：OpenCLI Browser Bridge。

完整需求、优先级和验收标准以 PRD 为准。

## 实施起点

先验证 PRD 第 15 节列出的技术问题，尤其是目标浏览器对 `Extensions.loadUnpacked` 的支持、扩展 ID 持久化和重载后的版本验证，再确定实现方案。

## 协作配置

任务与规格使用 [GitHub Issues](https://github.com/ren2019/chromium-extension-manager/issues) 跟踪。

Agent 入口为 [AGENTS.md](./AGENTS.md)，具体约定见：

- [任务跟踪](./docs/agents/issue-tracker.md)
- [分流标签](./docs/agents/triage-labels.md)
- [领域文档](./docs/agents/domain.md)

这些配置可直接编辑。术语表和架构决策记录在实际需要时创建。
