# GitHub Release 来源验证

2026-10-29，#5 核心实现。GUI 与浏览器持久安装验收另见对应记录。

## 核心入口

`Manager::list_releases(&CommandGh, source)` 返回分页后的正式版/预发布列表、GitHub 最新正式版标签与地址明确指定的标签。`ReleaseCatalog::preselected_asset(tag)` 仅在唯一可识别扩展 ZIP 时给出预选，仍需用户确认。

`Manager::preview_release(gh, source, &ReleaseSelection { tag, asset_id })` 获取并校验指定附件，返回 `ReleaseArtifact`。调用 `roots()` 展示有效根目录，再用 `select_root(Some(relative_path))` 获取冻结的 `Preview`；仅唯一根目录可用 `None`。空字符串表示 ZIP 根目录。预览通过现有 `install_local` 或 `prepare_update` / `apply_update` 部署，不直接修改浏览器。

`Manager::check_release(gh, installation_id)` 使用保存的附件选择与根目录检查最新正式版。`opencli-extension-v1.0.22.zip` 保存为带版本位置的选择；其他文件名按精确名称保存。匹配丢失、多个匹配或根目录改变都要求重新选择。显式切换版本重复预览流程，降级/同版本重新部署沿用已有明确确认机制。Release 标签与扩展清单版本分别保存，远程来源在重启、更新及恢复后仍可追溯。

## 认证与资源边界

所有元数据和附件通过 `gh api --hostname github.com` 参数数组调用，认证及 Token 留在 GitHub CLI。不会运行来源脚本、提取 Token 或自行请求 HTTP。缺失 CLI、401/403/404、网络失败、超时均给出阶段和后续处理指引，原始 stderr 不向 UI 透传。macOS 先按 PATH 寻找 gh；找不到时依次尝试 `/opt/homebrew/bin/gh`、`/usr/local/bin/gh`，用于 Finder 启动的环境。

每次 CLI 调用上限两分钟；元数据 16 MiB，压缩附件 100 MiB，stderr 64 KiB。只接受普通 ZIP（不接收多卷或 ZIP64 目录），最多 10000 个条目、路径 512 字节/32 层、单文件 64 MiB、累计展开和候选预览各 256 MiB。验证实际解压字节与 CRC，拒绝绝对/穿越/Windows 危险路径、重复名称、大小写/Unicode 冲突、文件目录冲突、符号链接与特殊文件。`zip` 的名称索引会合并完全重复名称，因此另外核对 ZIP 尾目录声明的条目数量，不能仅依赖索引迭代检查重复。

## 用例证据

批准测试入口：Rust 安装/更新公共用例；真实临时文件、真实 ZIP，仅替代外部 gh 响应。

`tests/remote_sources.rs` 的 10 项测试覆盖：

- 安装/更新/重启的来源元数据；公开与私有仓库相同 CLI 路径。
- 分页第二页、GitHub 最新正式版、显式预发布/旧版、编码标签与地址拒绝。
- 附件与根目录歧义，明确选择与选择持久化。
- SHA-256 失败、附件扩展版本与清单不符，旧记录与文件保留。
- 穿越、绝对路径、Windows 保留名/分隔符、Unicode/大小写与隐式父目录冲突。
- 完全重复 ZIP 条目、符号链接、损坏 ZIP、超限条目。
- 后续附件匹配丢失/歧义、根目录改变、CLI 无权限，仍可使用本地来源。
- 远程升级、显式降级、加载失败恢复，固定路径与扩展身份保持。

TDD 中观察到的失败包括分页默认版本取错、错误 SHA 被接受、附件误预选、COM¹ 与隐式父目录冲突、编码标签丢失、完全重复 ZIP 条目被索引隐藏，以及 Unicode 大小写冲突；逐项修复后通过。`cargo test -p cepm-core`、`cargo clippy -p cepm-core --all-targets -- -D warnings` 和格式检查通过。私有访问使用 CLI 边界响应验证，没有读取用户其他私有仓库；未新增 Windows/Edge 实测。

## 真实 CLI 冒烟

使用上述生产入口、实际 `CommandGh` 与公开 `jackwener/OpenCLI` Release 地址，在 `PATH=/usr/bin:/bin` 的 Finder 类环境下完成 `v1.8.6` 下载、来源 SHA-256 校验与根目录预览；结果为 `OpenCLI`、清单版本 `1.0.22`、附件 `opencli-extension-v1.0.22.zip`。此项也确认 Homebrew gh 回退路径可用。该检查没有部署到浏览器，也不代替后续持久安装/连接验收。
