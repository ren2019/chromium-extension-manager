# 发布流程

GitHub Actions 的 [Release 工作流](https://github.com/ren2019/chromium-extension-manager/actions/workflows/release.yml) 构建优化后的 macOS Apple Silicon 和 Windows x64 包，各附一个 SHA-256 文件。

## 日常与试运行

- PR 检查继续使用 `ci.yml`。发布脚本、工作流或 Cargo 依赖变更时，还会运行双平台 Release 打包。
- 在 Actions → Release → Run workflow 选择分支，可只构建并下载制品，**不会创建 GitHub Release**。也可运行 `gh workflow run release.yml --ref main`。
- 这两种运行只有读取仓库权限。只有版本标签触发、所有构建成功后，发布 job 才获得 `contents: write`。

## 发布一个版本

1. 更新 `crates/cepm-app/Cargo.toml` 的应用版本；如核心包也变更版本，同步其清单。运行 Cargo 以更新 `Cargo.lock`，通过 PR 合入 `main`。
2. 在已更新的 `main` 上创建并推送与应用版本一致的标签，例如应用版本为 `0.1.0` 时：

   ```sh
   git switch main
   git pull --ff-only
   git tag -a v0.1.0 -m "CEPM 0.1.0"
   git push origin v0.1.0
   ```

3. 工作流验证标签、清单版本及提交已包含在 `main` 中；执行测试/检查并打包。两个平台均成功后检查 ZIP 完整性和 SHA-256，再自动创建 Release、上传四个文件并生成变更说明。

`0.2.0-rc.1` / `v0.2.0-rc.1` 这样的预发布版本会标记为 prerelease。版本标签不一致或提交未合入主分支时拒绝发布。已存在的 Release 不会被自动覆盖；失败先查 Actions 日志，可重跑失败的构建。若发布阶段已留下草稿，先检查草稿及附件再处理，不要移动已有版本标签。

## 下载内容与验证

| 文件 | 内容 |
| --- | --- |
| `CEPM-<版本>-macos-arm64.zip` | `CEPM.app`，移入 Applications |
| `CEPM-<版本>-windows-x64.zip` | `CEPM/CEPM.exe` 和运行说明，解压后运行 |
| 每个 ZIP 的 `.sha256` | 对应归档的 SHA-256 摘要 |

macOS 可运行 `shasum -a 256 -c <文件名>.sha256`。Windows 可用 `Get-FileHash <文件名>.zip -Algorithm SHA256`，与摘要文件比较。

现有 macOS 本机打包入口 `scripts/package-macos.sh release` 继续有效；Windows 使用 PowerShell 7 执行 `./scripts/package-windows.ps1 -Profile release`。两者均构建当前机器的原生目标并输出到 Cargo 目标目录 `bundle/release/`。Debug 包仍保留原文件名。

## 当前边界

- 未配置 Apple Developer ID 签名/公证或 Windows 代码签名；Release 说明明确标注。无需添加发布 PAT 或签名密钥，发布使用 GitHub 自动提供的任务令牌。
- Windows 这里只验证编译和打包，未恢复 Windows 原生界面或浏览器专项验收。运行时可能需要微软官方 Visual C++ x64 Redistributable，包内说明提供入口。
- 不包含应用自动升级服务；升级时替换程序文件，保留用户应用数据目录。
- 第一次配置只试运行构建，不擅自创建版本标签或发布首个版本。

实现依据：[GitHub runner 规格](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)、[制品上传](https://github.com/actions/upload-artifact)、[GitHub CLI 发布命令](https://cli.github.com/manual/gh_release_create)。
