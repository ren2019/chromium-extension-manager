# 使用 Rust 核心，界面框架单独验证

核心使用 Rust，并与 GUI 分离，使来源解析、版本比较和安装更新行为能独立验证。GUI 采用 GPUI/gpui-kit 开始实现，使用已完成最小实验的上游依赖，不直接照搬 OpenLogi 的 fork 补丁。

最初要求 Windows/macOS 交互验证后才采用。2026-10-29，macOS 交互和双平台构建通过后，用户明确要求“edge 和 windows 不再验证了。往下推进”，因此取消剩余 Windows/Edge 验证作为开工门槛。Windows/macOS、Chrome/Edge 仍是产品目标；未测项保留为未知，不声明已验证兼容。核心独立于界面，使以后需要替换 GUI 时仍能保留安装行为。
