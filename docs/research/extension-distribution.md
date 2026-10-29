# GitHub Chromium 扩展的发布与安装范式

核查日期：2026-10-29。状态：事实调查及设计建议，不代表用户已批准新的范围。

## 结论

**预构建 Release ZIP 是重要入口，但仅支持它会漏掉“仓库目录本身就是可加载扩展”的项目。** 本次 9 组目的性样本覆盖成熟扩展、单仓多产品、官方样例和框架样例；其中 6 组在所查 Release 中提供浏览器扩展 ZIP，一组明确标注仅供本地测试。这个选择偏向可观察、维护活跃的项目，不能用来估计整个 GitHub 的比例，更不能推断私有仓库的分布。

建议首版统一处理三种**已经构建完成的扩展产物**：Release 附件、仓库指定引用及子目录、本地目录。私有/公开属于访问认证维度，不是另一种产物格式。源码构建、商店抓包和 Actions 构建产物发现另行设计。

## 样本与可复核证据

表中 Release tag 为本次核查时固定的版本定位；分支样本另固定 commit。没有下载执行这些扩展，也未逐一解压 Release 附件，所以“ZIP”表示作者公布的附件与安装指引，不表示已通过 CEPM 的安全校验或兼容性验收。

| 样本与固定引用 | 实际观察到的分发/安装方式 | 版本及选择陷阱 |
| --- | --- | --- |
| [OpenCLI v2.0.2](https://github.com/jackwener/OpenCLI/releases/tag/v2.0.2) | Release 附件 `opencli-extension-v1.0.26.zip`；[前一发布说明](https://github.com/jackwener/OpenCLI/releases/tag/v2.0.1) 同时提到商店审核 | [扩展 manifest](https://github.com/jackwener/OpenCLI/blob/v2.0.2/extension/manifest.json) 是 MV3、`1.0.26`，与主产品 Release `2.0.2` 不同。两次主产品发布可以沿用同一扩展版本。 |
| [Dark Reader v4.9.134](https://github.com/darkreader/darkreader/releases/tag/v4.9.134) | 同时提供 `darkreader-chrome-mv3.zip`、`darkreader-chrome.zip`、Firefox/Thunderbird XPI；[README](https://github.com/darkreader/darkreader/blob/v4.9.134/README.md) 提供商店入口及 Node/Deno 构建方法 | [package.json](https://github.com/darkreader/darkreader/blob/v4.9.134/package.json) 声明 `4.9.134`。源码里的浏览器 manifest 是组合模板；不能仅按 `.zip` 选择第一个附件，也不能把源码目录当作成品。 |
| [uBlock Origin Lite 2026.1026.1812](https://github.com/uBlockOrigin/uBOL-home/releases/tag/2026.1026.1812) | 同一 Release 提供 Chromium ZIP、Edge ZIP、Firefox XPI、Safari ZIP；正文推荐对应商店 | 使用日期风格版本；Tag 与附件名对应，但仍应以解包后 manifest 为扩展版本。必须区分目标浏览器，Safari ZIP 不是 Chromium 包。 |
| [SingleFile-MV3 a13f696](https://github.com/gildas-lormeau/SingleFile-MV3/tree/a13f6964d788126679c1c41dc2f1f946a39d5c67) | [官方 README](https://github.com/gildas-lormeau/SingleFile-MV3/blob/a13f6964d788126679c1c41dc2f1f946a39d5c67/README.MD) 直接指导下载仓库归档、解压并加载；仓库含打包后的 `lib/` 文件。[主项目 README](https://github.com/gildas-lormeau/SingleFile/blob/v1.33.0/README.MD) 把 Chrome/Edge 用户引导到此仓库 | [根 manifest](https://github.com/gildas-lormeau/SingleFile-MV3/blob/a13f6964d788126679c1c41dc2f1f946a39d5c67/manifest.json) 是 MV3、`1.33.0`。主项目 `singlefile-extension-firefox.zip` 不应误选为 Chromium 包；此例说明 repository archive 也可能包含可直接加载产物。 |
| [Bitwarden browser-v2026.10.0](https://github.com/bitwarden/clients/releases/tag/browser-v2026.10.0) | 提供 `dist-chrome-2026.10.0.zip`、Edge/Firefox/Opera ZIP、source ZIP，以及部分 `bit-` 前缀产物；[浏览器 README](https://github.com/bitwarden/clients/blob/browser-v2026.10.0/apps/browser/README.md) 推荐商店并指向构建说明 | 同仓库另有 `web-v*`、`desktop-v*`，核查时较新的发布属于其他产品。必须先选 browser 产品线，不能直接取仓库 latest；[manifest 转换代码](https://github.com/bitwarden/clients/blob/browser-v2026.10.0/apps/browser/webpack/manifest.js) 还处理浏览器前缀及通道。 |
| [Violentmonkey v2.50.0](https://github.com/violentmonkey/violentmonkey/releases/tag/v2.50.0) | 同时提供 MV2 CRX、MV3 ZIP、webext ZIP，另有预发布 beta；[构建脚本](https://github.com/violentmonkey/violentmonkey/blob/v2.50.0/package.json) 区分普通与 MV3 构建 | [源码 manifest.yml](https://github.com/violentmonkey/violentmonkey/blob/v2.50.0/src/manifest.yml) 的版本为 `__VERSION__`，package 版本为 `2.50.0`。必须使用最终产物，不能把模板直接加载，也不能把所有 ZIP 当作同一目标。 |
| [Refined GitHub 26.10.26](https://github.com/refined-github/refined-github/releases/tag/26.10.26) | Release 提供 `refined-github-26.10.26-for-local-testing-only.zip`；[README](https://github.com/refined-github/refined-github/blob/26.10.26/readme.md) 提供商店入口；[package](https://github.com/refined-github/refined-github/blob/26.10.26/package.json) 有编译及打包任务 | 作者明确限定附件用途为本地测试，CEPM 应展示这一用途，不应静默当正式安装包。[源码 manifest](https://github.com/refined-github/refined-github/blob/26.10.26/source/manifest.json) 为 MV3，版本却是 `0.0.0`，说明仓库模板版本不是发布版真值。 |
| [Google Chrome 官方样例 2bc864f](https://github.com/GoogleChrome/chrome-extensions-samples/tree/2bc864f7e432d829ccd7a91790d98b6504ba56ee) | [README](https://github.com/GoogleChrome/chrome-extensions-samples/blob/2bc864f7e432d829ccd7a91790d98b6504ba56ee/README.md) 指导 clone 仓库后 Load unpacked；[action 样例目录](https://github.com/GoogleChrome/chrome-extensions-samples/tree/2bc864f7e432d829ccd7a91790d98b6504ba56ee/api-samples/action) 内已有 manifest、后台脚本及页面 | 此目录 manifest 为 MV3、`1.0`。一个仓库包含很多独立扩展，不能把仓库根目录视作唯一安装单位。该仓库本次 Releases 查询为空。 |
| [WXT 官方 examples 0f61ce4](https://github.com/wxt-dev/examples/tree/0f61ce4a03b3eaeccf44746138e7733243d8bccf) | [vue-overlay 示例](https://github.com/wxt-dev/examples/tree/0f61ce4a03b3eaeccf44746138e7733243d8bccf/examples/vue-overlay) 要先安装依赖；[package](https://github.com/wxt-dev/examples/blob/0f61ce4a03b3eaeccf44746138e7733243d8bccf/examples/vue-overlay/package.json) 提供 `wxt build` / `wxt zip` | package 声明 `0.0.0`；[WXT 文档](https://wxt.dev/guide/essentials/config/manifest.html) 说明 manifest 由构建生成，版本来自 package/config。仓库源码不等于可加载目录；本次 Releases 查询为空。 |

## 共性与反例

### 发布渠道与产物结构不是同一个问题

多个成熟项目同时提供商店安装和 GitHub 附件；商店审核与 GitHub 发布节奏可以不同。Release 可包含多个浏览器、MV2/MV3、源码包和测试包；仓库归档既可能是需构建的源码，也可能已经包含可加载产物。应验证内容及作者声明，而不是用“ZIP 后缀”或“Source code 标签”直接判断可安装性。上表 Dark Reader、SingleFile、Violentmonkey、Refined GitHub 是不同方向的反例。

### “最新 Release”并不一定是该扩展的最新版本

Bitwarden 是多产品 Release 命名空间；OpenCLI 的主产品版本与扩展版本独立；WXT 和 Refined GitHub 的源码版本可能是构建输入或占位值。建议记录四类信息：来源 Release/tag、解析后的 commit、最终产物 manifest 版本、内容摘要。更新比较以已选择产品/通道和实际扩展版本为基础，不能拿整个仓库的标签直接与磁盘安装版本比较。

### 本地目录有现成的版本声明标准

可加载扩展根目录的 `manifest.json.version` 是浏览器自己的版本字段；`manifest_version` 表示清单格式，`version_name` 是展示名称。建议本地源采用同一声明，不再强制增加另一份项目专用版本文件。具体规则见 [本地目录版本研究](./local-directory-version.md)；官方依据为 [Chrome Manifest Version](https://developer.chrome.com/docs/extensions/reference/manifest/version)。

### Actions artifact 不宜自动当作正式 Release

GitHub 为 Actions artifact 提供单独接口，记录 workflow run、commit、过期状态及摘要；访问权限也不同于普通公共附件。它需要独立的运行选择和可信来源规则，不能仅因文件名匹配就安装。[GitHub 官方接口](https://docs.github.com/en/rest/actions/artifacts)

本次并未全面统计每个样本的 Actions artifact，因此不据此宣称它的流行程度。

## 对 CEPM 的建议

以下是基于样本的设计建议，仍待用户确认：

1. **Release 附件源**：支持公开及私有 GitHub；保存产品标签规则、正式/预发布通道、目标浏览器及附件选择。候选含糊或作者标为测试用途时要求用户明确选择。解包后校验 MV3、版本、入口资源和目录安全性。
2. **仓库目录源**：支持指定 tag/branch/commit 和扩展子目录，每次操作先解析为固定 commit。只接受该目录中已有完整可加载产物的情况；多 manifest 时让用户选根目录。不执行仓库构建脚本，不把 Git submodule、LFS 指针或源码占位符当作资源。
3. **本地目录源**：要求有效 `manifest.json.version`，读取时形成一致快照并验证资源；如何复制到固定安装目录、同版本内容变化是否允许安装，仍属于待确认的产品策略。
4. **延后单独适配**：源码构建、Actions artifact、CRX 解包、从商店提取包。这些不是增加一个 URL 正则就能可靠覆盖的来源。
5. **私有仓库**：对同一来源适配器增加授权访问；具体凭据交互与最小权限另行决定。本次没有访问用户私有扩展仓库，公开样本不能证明私有项目一般采用哪一种分发方式。

这些建议解决“拿到哪个可加载产物”，并不自动满足“浏览器永久安装”。用户已明确管理器退出、浏览器重启后扩展仍须可用；持久加载与配置保持应作为 Windows/macOS、Chrome/Edge 的独立验收项。
