# GitHub 仓库目录来源

仓库入口只接收 GitHub 仓库、用户选择的分支/标签/提交和相对扩展子目录。`Manager::preview_repository` 先通过已授权的 `gh` 解析完整提交，再获取该提交的 ZIP 归档；两次请求之间分支变化不会改变本次选择的产物。认证继续由 GitHub CLI 管理。

归档经共用安全检查解压到临时目录后，必须具有唯一仓库根目录。仅检查用户指定的子目录，不从其他目录猜测扩展，不执行仓库脚本或构建。扩展清单及声明资源验证成功后返回与本地安装相同的 `Preview`。

界面调用顺序：

```rust,ignore
let preview = manager.preview_repository(
    &gh, "owner/repository", "main", "packages/browser-extension",
)?;
// 显示名称、清单版本、origin() 中的引用和具体提交，再让用户确认目标。
let installation = manager.install_local(preview, target)?;

// 再次检查会重新解析已保存的引用，包括移动分支。
let plan = manager.check_repository(&gh, &installation.id)?;
// 根据 plan.kind() 展示升级、降级或同版本重新部署，并要求相应选择。
let installation = manager.apply_update(plan, selected_kind)?;
```

显式切换版本使用新的 `preview_repository` 输入（例如旧标签或提交），然后调用 `prepare_update` 和现有 `apply_update`。安装记录保存仓库、原始引用、具体提交、子目录及产物摘要；归档临时路径不作为后续更新来源。部署仍使用固定安装目录，部署成功后保持待浏览器重载/用户核对。失败恢复复用同一备份和记录流程。

测试通过 Rust 安装用例验证真实 ZIP、临时文件和重开记录；仅替换外部 `gh` 响应。私有仓库测试覆盖 CLI 已授权返回内容及访问失败，不访问额外的真实私有仓库。没有新增 Windows/Edge 验证。
