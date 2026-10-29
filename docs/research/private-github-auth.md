# 私有 GitHub 来源认证方案

核查日期：2026-10-29。仅调查官方资料；未注册应用、访问私有仓库、读取凭据或执行授权。

**后续决定已替代本文建议**：用户明确将认证及来源获取交给 `gh`、`glab` 等外部 CLI，由 CLI 管理 Token。本项目不提供 PAT 输入、系统凭据库存储或 GitHub App 登录。下文作为历史调研保留，不是实现要求；当前以 [ADR-0004](../adr/0004-delegate-remote-access-to-cli.md) 为准。

## 首版建议

**最小可交付方案：用户提供 fine-grained PAT，限定仓库和 `Contents: read`，由系统凭据库保存。** 这不需要项目方注册并维护 GitHub App，适合先完成私有来源验收；代价是用户需要手工创建、粘贴及到期更换 token，且不是所有组织协作者场景都适用。

如果首版必须做到“点击连接 GitHub、在浏览器中授权”，建议采用 **GitHub App user access token**，而不是 OAuth App 的宽泛 `repo` scope。Device flow 是官方明确支持的无 `client_secret` 取 token 路径，但应把安装授权及组织审批成本纳入产品流程；它并非所有桌面场景的官方默认优选，见下文。

## 三种方案的区别

| 方案 | 私有仓库权限范围 | 用户操作及限制 | 桌面是否需要携带应用 secret |
| --- | --- | --- | --- |
| Fine-grained PAT | 单一资源所有者、选定仓库、细粒度权限 | 手工生成；组织可禁止或要求审批；多所有者需多 token；到期需更新 | 不需要应用注册或 client secret，token 本身是用户秘密 |
| GitHub App + device flow | App 安装范围、App 权限及用户权限的交集 | 项目方注册 App 并启用 device flow；用户安装到选定仓库并授权；组织可能需管理员批准 | 初次 token 交换及 device token 的刷新均不要求 client secret |
| OAuth App + device flow | 私有代码通常使用 `repo` scope，包含读写能力，不能等价于选定仓库只读 | 项目方注册 OAuth App；用户授权；组织 OAuth 限制仍可能要求审批 | Device flow 不需要 client secret，但权限过宽，不建议作为默认方案 |

表中依据：[PAT 限制与创建](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens)、[GitHub App 用户权限交集](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/authenticating-with-a-github-app-on-behalf-of-a-user)、[device flow](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)、[OAuth scopes](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps)、[OAuth 授权流程](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps)。

## 最小 API 权限与下载行为

- 私有仓库的 Contents、Git 树/引用读取、Release 及附件读取、指定 ref 的仓库 ZIP 归档均有 `Contents: read` 的细粒度授权路径；无需申请写代码、Issues、管理仓库或 Actions 权限。[权限表](https://docs.github.com/en/rest/authentication/permissions-required-for-fine-grained-personal-access-tokens)
- 私有归档通过 `GET /repos/{owner}/{repo}/zipball/{ref}` 获取跳转地址，地址临时有效约五分钟；它不是应永久写入安装记录的来源地址。[Contents/Archive API](https://docs.github.com/en/rest/repos/contents#download-a-repository-archive-zip)
- Release 附件二进制可用资产 API 加 `Accept: application/octet-stream` 获取；客户端必须处理直接返回内容的 `200` 和跳转的 `302`。[Release Assets API](https://docs.github.com/en/rest/releases/assets#get-a-release-asset)
- 产品应保存 owner/repo、选定引用/子目录或资产选择规则，下载时重新鉴权。跨主机跳转不应盲目转发 `Authorization`；这是实现建议，不是扩大 token 权限的理由。

## 授权流程及账号边界

- GitHub App 的“安装”授予选定仓库访问，“用户授权”允许以该用户身份请求，两者不同；仅完成登录授权不代表 App 已获该私有仓库访问权。组织管理员可限制安装，普通成员可能只能请求安装。[安装规则](https://docs.github.com/en/apps/using-github-apps/installing-a-github-app-from-a-third-party)
- Fine-grained PAT 只针对一个个人或组织所有者；等待组织审批时只能访问公共资源。官方目前仍列出 outside collaborator 使用组织仓库的限制，因此不能承诺它覆盖用户能在网页打开的所有私有仓库。[PAT 官方说明](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens)
- OAuth App 在组织开启访问限制时也要经组织批准，不能绕过 GitHub App/PAT 的审批问题。[OAuth 组织审批](https://docs.github.com/en/organizations/managing-oauth-access-to-your-organizations-data/approving-oauth-apps-for-your-organization)
- GitHub App device flow 只需公开的 `client_id`、device code 和 grant type；使用过期用户令牌时，access token 八小时、refresh token 六个月。刷新会替换旧 token 对；device flow 取得的 token 刷新无需 client secret。[生成流程](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)、[刷新规则](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens)
- 官方生成文档将桌面应用列为 device flow 示例；同时最佳实践建议有条件时使用授权码 + PKCE，避免无回调绑定的 device flow 被用于钓鱼。当前授权码交换文档仍要求 client secret，不能宣称“加 PKCE 就完全不需要 secret”。原生客户端也无法保密嵌入的应用 secret。若采用这条路线，应单独决定回调及应用身份模型。[GitHub App 最佳实践](https://docs.github.com/en/apps/creating-github-apps/about-creating-github-apps/best-practices-for-creating-a-github-app#client-secrets)
- 不在桌面端分发 GitHub App 私钥生成 installation token；官方对原生客户端推荐 user access token。[原生客户端令牌建议](https://docs.github.com/en/apps/creating-github-apps/about-creating-github-apps/best-practices-for-creating-a-github-app#installation-access-tokens-user-access-tokens-and-refresh-tokens)

## Windows/macOS 凭据保存

- Windows 原生 Credential Manager 提供 `CredWriteW`，可向当前用户凭据集中写入应用的通用凭据。[Microsoft API](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew)、[凭据类型](https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw)
- macOS Keychain Services 提供加密保存秘密及控制应用访问的系统机制。[Apple Keychain Services](https://developer.apple.com/documentation/security/keychain-services/)
- 建议 Rust 核心持有并通过原生适配器存取 token；安装记录只存凭据引用，Web GUI 不读取 token。系统凭据库不可用时显示可恢复错误或允许仅本次会话，不静默退化为明文文件。这里是产品实现建议；系统凭据库不等于能抵御同机已被控制的用户会话。

## 下一轮只需决定的产品取舍

1. 首版接受“手工配置只读 PAT”，还是必须交付“GitHub 登录授权”？前者减少项目方运维与注册前置条件，后者减少普通用户创建 token 的负担。
2. 是否要求首版覆盖组织外部协作者的私有仓库？若要求，不能仅承诺 fine-grained PAT；应考虑经组织安装的 GitHub App。组织禁止所有可用授权方式时只提供清晰提示及本地目录入口。
3. 凭据是否默认记住、是否允许多个资源所有者凭据并存？建议默认系统凭据库保存，并按 owner/仓库明确绑定；不自动读取用户其他工具中的凭据。

无需用户决定 HTTP 头、轮询间隔和刷新算法；这些由实现遵循官方协议处理。以上权限与流程仍需在明确授权的测试仓库中验收，本次未作成功登录或下载的实测承诺。
