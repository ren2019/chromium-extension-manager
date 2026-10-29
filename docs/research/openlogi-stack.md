# OpenLogi 技术栈、技能与 AST 工具调研

核查日期：2026-10-29。固定研究提交：[`0a33d385ade61a7b5ac1f9ca8840f54a46ab0937`](https://github.com/AprilNEA/OpenLogi/commit/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937)。以下事实来自该提交的源码、配置及工作流；推荐部分是针对 CEPM 的工程判断，不是已确认的技术选型。

## 结论

OpenLogi 值得参考，尤其是 Rust 核心与界面分离、按任务加载技能、把架构约束变成自动检查的做法。但其 GUI 是 **Rust + GPUI + gpui-kit**，不是 Tauri，也不是本地浏览器 Web GUI。其根工作区和桌面 crate 均直接声明 GPUI 依赖，所查树没有 `package.json` 或 Tauri 配置。GPUI 路线需要单独决定，不能把“参考 OpenLogi”自动视为接受之前的 Tauri 建议。[工作区依赖](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/Cargo.toml)、[桌面依赖](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/crates/openlogi-desktop/Cargo.toml)

## 实际依赖与版本边界

| 层次 | 已核实配置 | 对 CEPM 的意义 |
| --- | --- | --- |
| Rust 工作区 | Edition 2024；`rust-version = 1.99`；工具链通道为 `stable`；共享 workspace 依赖和 lint | 可借鉴工作区组织；工具链通道是浮动 stable，并非完整版本固定 |
| GUI 直接依赖 | `gpui` 重命名自 `gpui-pre =0.3.4`；平台层 `gpui-pre-platform =0.3.4` | 纯 Rust 原生界面路线 |
| 组件直接依赖 | gpui-component、gpui-base、gpui-kit-assets 声明上游 tag `v0.6.1`，又通过 patch 覆盖到 AprilNEA fork 的 `1ac69bd…` | 实际解析源是 fork 固定提交；不能只复制版本号而漏掉补丁负担 |
| 异步、序列化、诊断 | Tokio、Serde、serde_json、tracing、thiserror、anyhow | 适合评估用于 CEPM 核心；各项按实际需要选择 |
| 网络和文件相关 | ureq 3（rustls/platform verifier）、sha2 0.10、atomic-write-file 0.3.0 | 可参考下载、摘要和单文件写入；单文件原子写不等于完整目录部署事务 |
| 多进程通信 | tarpc 0.38 + interprocess 2 + bincode transport | 为其常驻硬件 agent 服务；CEPM 不应仅为模仿而增加常驻进程和 RPC |
| 平台层 | target cfg 下的 objc2、windows-sys 等，另有大量 HID、输入注入、权限组件 | 大部分属于硬件产品需求，CEPM 不需要照搬 |

事实依据：[根 Cargo.toml](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/Cargo.toml)、[工具链](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/rust-toolchain.toml)、[Cargo.lock](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/Cargo.lock)。所查 lock 的例子：Tokio 1.53.3、ureq 3.3.0、tarpc 0.38.0、interprocess 2.4.2；这些是该提交解析出的版本，不是建议本项目盲目固定的版本。依赖声明中的范围、精确 `=` 约束、Git patch 与 lock 结果必须分别看待。

## 架构上最值得借鉴的部分

OpenLogi 将领域类型与硬件、平台和展示层拆开；GUI、overlay 是 IPC 客户端，常驻 agent 拥有输入钩子和 HID I/O。其 core 以纯类型为主，但部分主机能力由 feature 开关控制。这个结构服务于必须持续处理硬件事件的产品，不能由此推导 CEPM 也需要三进程。[架构和 crate 职责](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/AGENTS.md)

建议 CEPM 初期保持少量明确边界：领域规则（版本、来源、操作状态）、实际安装更新服务（下载、校验、备份、部署）、浏览器适配、GUI。核心可独立验证，GUI 不自行重新判断版本或直接覆盖安装目录。是否拆成多个 crate、是否需要常驻进程，应由实际依赖和后台运行需求决定。

## Skill 是开发辅助，不是产品运行依赖

`skills-lock.json` 记录三项第三方技能来源与内容 hash：`gpui-kit`、`gpui-kit-design-guides`、`ast-grep`。根 AGENTS.md 按任务路由技能，并记录导入的上游提交；此外有本地维护的 UI 测试、变更验证、设备诊断和 fixture 贡献技能。它们指导开发代理，不是用户启动应用时要安装或加载的依赖。[技能锁](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/skills-lock.json)、[技能入口与来源](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/AGENTS.md)

共享规则以 `.agents/rules/` 为规范来源；Claude 的配置指向同一来源。根 AGENTS.md 明确指出其他客户端不能假设自动理解 Claude 的 `paths` 规则。这一点适合本项目采用：保持一个入口和明确的按需链接，不复制多份会漂移的规则。[规则装载约定](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/AGENTS.md)、[Claude 入口](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/.claude/CLAUDE.md)

建议：若选 GPUI，再引入 GPUI 的上游技能；无论选哪种 GUI，都可以借鉴“变更验证”和“界面验证”的任务拆分。硬件诊断、HID fixture 和 macOS 硬件权限技能不适合直接搬进 CEPM。此次调研没有安装或激活上述远程技能。

## “AST”具体是什么

### ast-grep：结构搜索与架构约束检查

已确认仓库使用 **ast-grep**。技能教代理编写结构查询；`sgconfig.yml` 注册根目录及各 crate 所拥有的 YAML 规则；prek 提交钩子运行 `ast-grep scan`；CI 的 ast-grep action 指定工具版本 0.45.1。它是开发和 CI 工具，`Cargo.lock` 未列入 ast-grep 包，不能把它当作扩展管理运行时引擎。[技能](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/.agents/skills/ast-grep/SKILL.md)、[规则注册](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/sgconfig.yml)、[钩子](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/prek.toml)、[CI](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/.github/workflows/ci.yml)

规则不是泛泛寻找重复字符串，而是维护职责边界。例如 `core-profile-path-owner` 禁止其他 crate 重新组合 `agent.sock`、`config.toml` 等路径常量；`workspace-no-inner-allow` 用语法结构识别模块范围的 lint 抑制。规则文件与负责该决策的模块放在一起，统一由根配置发现。[路径职责规则](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/crates/openlogi-core/.ast-grep/rules/core-profile-path-owner.yml)、[lint 规则](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/.ast-grep/rules/workspace-no-inner-allow.yml)

它要求验证三类样例：违规代码必须失败、拥有该决策的模块必须通过、无关代码必须通过，并验证跨目录选择范围。隐藏的 `.github` 需要显式扫描；本地 xtask 也同时执行普通扫描与 `.github` 扫描。这比只编写规则却不验证生效范围更值得借鉴。[规则验证流程](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/.agents/rules/ci.md#ast-grep-rules)、[本地检查实现](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/xtask/src/commands/ci/jobs/steps.rs)

对 CEPM 的建议：先使用类型系统、模块可见性和行为测试约束边界；出现确实需要的跨模块规则后，增加少量 ast-grep 检查，例如禁止 GUI 绕过安装服务写固定安装目录。不要预先复制一整套 OpenLogi 专属规则，也不要依靠 AST 扫描证明下载包安全或更新事务正确。

### syn：另一个容易混淆的 AST 依赖

仓库还包含 Rust AST 库 `syn`。`openlogi-hidpp-derive` 直接使用 syn 3.0.3、quote、proc-macro2 生成 HID++ 宏；`xtask` 直接使用 syn 2.0.117 解析 lint 属性，另外 lock 中还存在 syn 1.x 等其他依赖解析结果。这些是编译宏或维护工具用途，不代表应用在运行时解析 Chrome 扩展源码。CEPM 没有理由仅因参考该项目而加入 syn。[derive crate](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/crates/openlogi-hidpp-derive/Cargo.toml)、[xtask](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/xtask/Cargo.toml)

## 测试、跨平台与分发

- CI 有 rustfmt、Clippy、测试、rustdoc、ast-grep、typos、shell 检查及 cargo-deny。Windows 和 Linux 测试排除了 desktop crate；macOS 在 ARM64 和 x86_64 跑全工作区测试。Windows/macOS 都有全工作区 Clippy。因此不能把一次 Linux 通过当作 GUI 或跨平台验收。[CI 实际命令](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/.github/workflows/ci.yml)
- cargo-deny 当前聚焦 CLI 发布依赖闭包，注释明确未覆盖整个 GUI 依赖图。借鉴时应按 CEPM 实际发货组件确定扫描范围，不能误称其原配置已经审计全应用。[cargo-deny job](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/.github/workflows/ci.yml)
- 原生 UI 验证使用 GPUI 测试、组件 gallery、mock agent 和截图；技能明确浏览器 DOM 自动化不能测试 GPUI，mock 通过也不能证明真实硬件行为。[UI 验证技能](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/.agents/skills/testing-openlogi-ui/SKILL.md)
- 构建工作流包含 macOS ARM64/x86_64 DMG、Windows x86_64/ARM64 EXE 和 MSI；macOS 构建需要完整 Xcode/Metal 工具链。发布路径有签名、公证和分平台更新支持，macOS 使用 Sparkle。它的 AGENTS.md 将 Windows 描述为较新的已发布移植版，不能据此承诺所有系统成熟度一致。[构建分发](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/.github/workflows/build.yml)、[桌面更新依赖](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/crates/openlogi-desktop/Cargo.toml)、[项目约定](https://github.com/AprilNEA/OpenLogi/blob/0a33d385ade61a7b5ac1f9ca8840f54a46ab0937/AGENTS.md)

## 对 CEPM 的选型建议

可以直接借鉴工程方法：Rust 核心与外部操作分离、工作区统一 lint、按任务加载 skills、技能来源及内容 hash、按改动范围运行检查、Windows/macOS 分别验证、用实际回归案例建立少量 AST 规则。

GUI 保留两个真实选项：若优先全 Rust 和原生界面，GPUI + gpui-kit 值得做最小验证；若优先 Web 界面复用，则此前的 Tauri 方案仍是另一条路线。OpenLogi 本身只能证明前者已有实际应用，不能证明它对 CEPM 更省维护成本。GPUI 验证应覆盖 Windows/macOS 的文件目录选择、列表和进度、中文输入、键盘操作、可访问性及安装包，特别关注它当前需要固定版本与 fork 补丁的成本。

当前不建议引入 OpenLogi 的 HID 栈、多进程 overlay、硬件权限处理、整个发布基础设施或全部 crates。CEPM 的难点仍是浏览器安装持久性、来源认证、版本声明和安全的目录更新；这些不会因选择 GPUI 或 Tauri 自动解决。

## 证据限制

本次只读源码与配置，没有构建、运行 OpenLogi，没有执行其脚本，没有验证其当前 CI 结果、内存占用或性能。配置证明存在检查和发布路径，不等于这些路径在当前机器通过。所有依赖版本仅描述固定研究提交；采用时仍需重新检查目标版本及兼容性。此次也没有把 Rust、GPUI、Tauri 或任何新技能设为 CEPM 的已接受决策。
