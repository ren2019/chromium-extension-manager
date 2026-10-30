//! Short-lived Edge tasks. The root Extensions domain has no Profile selector;
//! shared Profile roots are deliberately unsupported, rather than guessed.
use crate::{Browser, Result, profiles::BrowserProfile};
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Default)]
pub struct BrowserCancellation(Arc<AtomicBool>);
impl BrowserCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err("已取消浏览器连接".into())
        } else {
            Ok(())
        }
    }
}
/// Capabilities are separate: a metadata connection never implies persistent
/// loading, reload, or an enabled destructive operation.
#[derive(Debug, Clone, Copy)]
pub struct BrowserCapabilities {
    pub read_metadata: bool,
    pub uninstall: bool,
    pub persistent_load: bool,
    pub reload: bool,
}

pub struct BrowserObservation {
    profile: BrowserProfile,
    installation: crate::Installation,
    extension_id: String,
    cancel: BrowserCancellation,
    observed_at: std::time::Instant,
}

/// Explicit confirmation shown with the exact installation and browser target.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AutomaticRemovalConfirmation {
    NotConfirmed,
    Confirmed,
}

/// Off by default until controlled UI-persistent removal acceptance is recorded.
pub fn automatic_uninstall_enabled() -> bool {
    cfg!(all(feature = "edge-uninstall", target_os = "macos"))
}

pub struct EdgeSession {
    profile: BrowserProfile,
    edge_version: String,
    socket: tungstenite::WebSocket<std::net::TcpStream>,
    cancel: BrowserCancellation,
    deadline: std::time::Instant,
    sequence: u64,
    version_target: Option<String>,
    version_session: Option<String>,
}
impl EdgeSession {
    pub fn connect(profile: &BrowserProfile, cancel: BrowserCancellation) -> Result<Self> {
        cancel.check()?;
        validate_single_profile(profile)?;
        let endpoint = fs::read_to_string(profile.user_data_dir.join("DevToolsActivePort"))
            .map_err(|_| "请在 Edge 的远程调试页面允许本次连接，然后重试；也可以继续手动确认")?;
        let mut lines = endpoint.lines();
        let port = lines
            .next()
            .and_then(|p| p.parse::<u16>().ok())
            .filter(|p| *p != 0)
            .ok_or("浏览器连接信息无效，请重新开启本次连接")?;
        let path = lines
            .next()
            .filter(|p| {
                p.starts_with("/devtools/browser/")
                    && p.len() > 18
                    && p.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'/' || b == b'-')
            })
            .ok_or("浏览器连接信息无效，请重新开启本次连接")?;
        if lines.next().is_some() {
            return Err("浏览器连接信息无效".into());
        }
        let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let stream =
            std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_secs(2))
                .map_err(|_| "Edge 尚未允许连接，请在浏览器批准后重试，或继续手动确认")?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .map_err(|_| "无法建立浏览器连接")?;
        stream
            .set_write_timeout(Some(std::time::Duration::from_secs(2)))
            .map_err(|_| "无法建立浏览器连接")?;
        stream
            .set_nonblocking(true)
            .map_err(|_| "无法建立浏览器连接")?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut handshake = tungstenite::client(format!("ws://127.0.0.1:{port}{path}"), stream);
        let socket = loop {
            cancel.check()?;
            if std::time::Instant::now() >= deadline {
                return Err("等待 Edge 授权超时，请批准后重试或手动确认".into());
            }
            match handshake {
                Ok((socket, _)) => break socket,
                Err(tungstenite::HandshakeError::Interrupted(pending)) => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    handshake = pending.handshake();
                }
                Err(_) => return Err("Edge 连接被拒绝或已失效，请在浏览器批准后重试".into()),
            }
        };
        socket
            .get_ref()
            .set_nonblocking(false)
            .map_err(|_| "无法建立浏览器连接")?;
        socket
            .get_ref()
            .set_read_timeout(Some(std::time::Duration::from_millis(100)))
            .map_err(|_| "无法建立浏览器连接")?;
        let mut session = Self {
            profile: profile.clone(),
            edge_version: String::new(),
            socket,
            cancel,
            deadline,
            sequence: 0,
            version_target: None,
            version_session: None,
        };
        let version = session.call("Browser.getVersion", serde_json::json!({}), None)?;
        let product = version["product"].as_str().unwrap_or("");
        let agent = version["userAgent"].as_str().unwrap_or("");
        if !product.starts_with("Edg/")
            && !product.starts_with("Microsoft Edge/")
            && !agent.contains("Edg/")
        {
            return Err("连接的浏览器不是 Edge，已停止自动核对".into());
        }
        session.edge_version = if product.starts_with("Edg/") {
            product.trim_start_matches("Edg/").to_string()
        } else if product.starts_with("Microsoft Edge/") {
            product.trim_start_matches("Microsoft Edge/").to_string()
        } else {
            agent
                .split("Edg/")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .unwrap_or("")
                .to_string()
        };
        let target = session.call(
            "Target.createTarget",
            serde_json::json!({"url":"edge://version"}),
            None,
        )?;
        let target = target["targetId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("无法核对 Edge 配置")?
            .to_string();
        session.version_target = Some(target.clone());
        let attached = session.call(
            "Target.attachToTarget",
            serde_json::json!({"targetId":target,"flatten":true}),
            None,
        )?;
        session.version_session = Some(
            attached["sessionId"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("无法核对 Edge 配置")?
                .into(),
        );
        session.validate_binding()?;
        let extensions = session.call("Extensions.getExtensions", serde_json::json!({}), None)?;
        if !extensions["extensions"].is_array() {
            return Err("浏览器不支持扩展读取，请使用手动确认".into());
        }
        Ok(session)
    }
    pub fn capabilities(&self) -> BrowserCapabilities {
        BrowserCapabilities {
            read_metadata: true,
            uninstall: automatic_uninstall_enabled() && self.edge_version == "156.0.4314.50",
            persistent_load: false,
            reload: false,
        }
    }
    fn check(&self) -> Result<()> {
        self.cancel.check()?;
        if std::time::Instant::now() >= self.deadline {
            return Err("浏览器核对已超时，请重试或手动确认".into());
        }
        Ok(())
    }
    fn call(
        &mut self,
        method: &str,
        params: serde_json::Value,
        session: Option<String>,
    ) -> Result<serde_json::Value> {
        self.check()?;
        self.sequence += 1;
        let id = self.sequence;
        let mut request = serde_json::json!({"id":id,"method":method,"params":params});
        if let Some(session) = &session {
            request["sessionId"] = session.clone().into();
        }
        self.socket
            .send(tungstenite::Message::Text(request.to_string().into()))
            .map_err(|_| "浏览器连接已断开，请重新连接或手动确认")?;
        loop {
            self.check()?;
            let message = match self.socket.read() {
                Ok(message) => message,
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(_) => return Err("浏览器连接已断开或授权已撤销，请重新连接或手动确认".into()),
            };
            if message.is_close() {
                return Err("浏览器已结束本次连接，请重试或手动确认".into());
            }
            if !message.is_text() {
                continue;
            }
            let response: serde_json::Value =
                serde_json::from_str(message.to_text().map_err(|_| "浏览器响应无效")?)
                    .map_err(|_| "浏览器响应无效")?;
            if response["id"].as_u64() != Some(id) {
                continue;
            }
            if response.get("sessionId").and_then(|v| v.as_str()) != session.as_deref() {
                return Err("浏览器响应目标不一致，已停止核对".into());
            }
            if response.get("error").is_some() {
                return Err("浏览器不支持本次操作或拒绝了请求，请使用手动流程".into());
            }
            self.check()?;
            return response
                .get("result")
                .cloned()
                .ok_or("浏览器响应缺少结果".into());
        }
    }
    fn validate_binding(&mut self) -> Result<()> {
        validate_single_profile(&self.profile)?;
        let expression = "({url:location.href,profilePath:document.getElementById('profile_path')?.textContent?.trim()})";
        let value = loop {
            let response = self.call(
                "Runtime.evaluate",
                serde_json::json!({"expression":expression,"returnByValue":true}),
                self.version_session.clone(),
            )?;
            let value = response["result"]["value"].clone();
            match value["url"].as_str() {
                Some("edge://version") | Some("edge://version/")
                    if value["profilePath"].as_str().is_some_and(|p| !p.is_empty()) =>
                {
                    break value;
                }
                Some("about:blank") | Some("edge://version") | Some("edge://version/") | None => {
                    self.check()?;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                _ => return Err("无法核对浏览器实际配置，已停止自动操作".into()),
            }
        };
        let path = value["profilePath"]
            .as_str()
            .ok_or("Edge 配置路径尚未就绪，请稍后重试")?;
        if std::path::Path::new(path).canonicalize().ok().as_ref()
            != Some(&self.profile.profile_path())
        {
            return Err("Edge 实际配置与所选目标不一致，已停止自动操作".into());
        }
        self.check()?;
        Ok(())
    }
    fn remove_and_verify_absence(&mut self, record: &crate::Installation) -> Result<()> {
        if !self.capabilities().uninstall {
            return Err("此 Edge 版本暂不支持自动卸载，请使用手动移除".into());
        }
        if record.target != self.profile.target() {
            return Err("卸载浏览器目标不一致".into());
        }
        let id = record
            .extension_id
            .as_deref()
            .filter(|id| id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b)))
            .ok_or("尚未识别扩展身份，请先识别或手动移除")?;
        if record.install_dir.canonicalize().ok().as_ref() != Some(&record.install_dir) {
            return Err("固定安装目录位置变化，已停止卸载".into());
        }
        self.validate_binding()?;
        let entries = self.call("Extensions.getExtensions", serde_json::json!({}), None)?;
        let entries = complete_extension_list(&entries)?;
        let by_id: Vec<_> = entries
            .iter()
            .filter(|e| e["id"].as_str() == Some(id))
            .collect();
        let by_path: Vec<_> = entries
            .iter()
            .filter(|e| {
                e["path"].as_str().is_some_and(|path| {
                    std::path::Path::new(path).canonicalize().ok().as_ref()
                        == Some(&record.install_dir)
                })
            })
            .collect();
        if by_id.len() != 1 || by_path.len() != 1 || by_path[0]["id"].as_str() != Some(id) {
            return Err("未找到唯一匹配的受管扩展，未执行卸载；请手动检查目标配置".into());
        }
        self.validate_binding()?;
        self.call("Extensions.uninstall", serde_json::json!({"id":id}), None)?;
        // A successful request is not evidence of disappearance. Read failure,
        // a reused ID or an unchanged path all preserve local state for retry.
        self.validate_binding()?;
        let response = self.call("Extensions.getExtensions", serde_json::json!({}), None)?;
        let remaining = complete_extension_list(&response)?;
        if remaining.iter().any(|entry| {
            entry["id"].as_str() == Some(id)
                || entry["path"].as_str().is_some_and(|path| {
                    std::path::Path::new(path).canonicalize().ok().as_ref()
                        == Some(&record.install_dir)
                })
        }) {
            return Err("浏览器仍显示这个扩展，未清理文件；请重试或手动移除".into());
        }
        self.validate_binding()?;
        Ok(())
    }
    /// Read and bind browser evidence to this precise deployment. This does not
    /// write an installation record; callers commit only if their task is current.
    pub fn verify(&mut self, record: &crate::Installation) -> Result<BrowserObservation> {
        if record.target != self.profile.target() {
            return Err("安装与浏览器目标不一致".into());
        }
        self.validate_binding()?;
        let response = self.call("Extensions.getExtensions", serde_json::json!({}), None)?;
        let entries = response["extensions"]
            .as_array()
            .ok_or("浏览器未返回可验证的扩展列表")?;
        let matches: Vec<_> = entries
            .iter()
            .filter(|entry| {
                entry["path"].as_str().is_some_and(|path| {
                    std::path::Path::new(path).canonicalize().ok().as_ref()
                        == Some(&record.install_dir)
                })
            })
            .collect();
        if matches.len() != 1 {
            return Err("没有找到唯一匹配的扩展，请在目标 Edge 配置加载后重试".into());
        }
        let found = matches[0];
        let id = found["id"].as_str().ok_or("浏览器未返回扩展身份")?;
        if id.len() != 32
            || !id.bytes().all(|b| (b'a'..=b'p').contains(&b))
            || record.extension_id.as_deref().is_some_and(|old| old != id)
        {
            return Err("浏览器扩展身份与安装记录不一致".into());
        }
        if found["version"].as_str() != Some(record.installed_version.as_str()) {
            return Err("浏览器版本与本次部署不一致，请重新加载后重试".into());
        }
        if found["enabled"].as_bool() != Some(true) {
            return Err("扩展尚未启用，请在浏览器启用后重试".into());
        }
        self.validate_binding()?;
        Ok(BrowserObservation {
            profile: self.profile.clone(),
            installation: record.clone(),
            extension_id: id.into(),
            cancel: self.cancel.clone(),
            observed_at: std::time::Instant::now(),
        })
    }
}
impl Drop for EdgeSession {
    fn drop(&mut self) {
        // Best-effort cleanup of only our own native version tab, even after cancel.
        if let Some(target) = self.version_target.take() {
            self.sequence += 1;
            let message = serde_json::json!({"id":self.sequence,"method":"Target.closeTarget","params":{"targetId":target}});
            let _ = self
                .socket
                .send(tungstenite::Message::Text(message.to_string().into()));
        }
        let _ = self.socket.close(None);
        let _ = self.socket.get_ref().shutdown(std::net::Shutdown::Both);
    }
}
impl crate::Manager {
    /// The browser operation and its result verification are inseparable from
    /// cleanup authorization; callers cannot fabricate a successful removal.
    pub fn uninstall_edge(
        &mut self,
        id: &str,
        target: &crate::Target,
        generation: u64,
        session: EdgeSession,
        confirmation: AutomaticRemovalConfirmation,
    ) -> Result<()> {
        self.uninstall_edge_with_progress(id, target, generation, session, confirmation, |_| {})
    }
    pub fn uninstall_edge_with_progress(
        &mut self,
        id: &str,
        target: &crate::Target,
        generation: u64,
        mut session: EdgeSession,
        confirmation: AutomaticRemovalConfirmation,
        progress: impl FnMut(crate::RemovalPhase),
    ) -> Result<()> {
        if confirmation != AutomaticRemovalConfirmation::Confirmed {
            return Err("尚未确认卸载，未操作浏览器或文件".into());
        }
        self.validate_storage_directories()?;
        let record = self.record(id)?;
        if record.target != *target || record.generation != generation {
            return Err("卸载目标或部署已变化，请重新打开详情".into());
        }
        session.remove_and_verify_absence(&record)?;
        session.check()?;
        self.begin_removal(
            id,
            target,
            generation,
            crate::removal::RemovalMethod::Automatic,
        )?;
        drop(session);
        self.retry_removal_with_progress(id, target, generation, progress)
    }

    /// Commits only evidence minted by an authorized live browser task. A stale
    /// UI target, deployment, cancellation or modified directory cannot confirm.
    pub fn confirm_browser(
        &mut self,
        evidence: BrowserObservation,
        current_target: &crate::Target,
    ) -> Result<crate::Installation> {
        evidence.cancel.check()?;
        validate_single_profile(&evidence.profile)?;
        if evidence.observed_at.elapsed() > std::time::Duration::from_secs(5) {
            return Err("浏览器核对结果已过期，请重新连接".into());
        }
        let mut record = self.record(&evidence.installation.id)?;
        if &record.target != current_target
            || record.target != evidence.installation.target
            || record.generation != evidence.installation.generation
            || record.installed_version != evidence.installation.installed_version
            || record.install_dir != evidence.installation.install_dir
        {
            return Err("核对目标或部署已变化，请重新核对".into());
        }
        self.ensure_unmodified(&record)?;
        if record
            .extension_id
            .as_ref()
            .is_some_and(|id| id != &evidence.extension_id)
        {
            return Err("扩展身份已变化，请重新核对".into());
        }
        record.extension_id = Some(evidence.extension_id);
        record.effective_version = Some(record.installed_version.clone());
        record.verification = crate::Verification::Automatic;
        record.load_failed = false;
        record.last_operation =
            "浏览器已自动核对身份、版本与启用状态（不代表全部功能健康检查）".into();
        self.commit_confirmation(record)
    }
}
fn complete_extension_list(response: &serde_json::Value) -> Result<&Vec<serde_json::Value>> {
    let entries = response["extensions"]
        .as_array()
        .ok_or("浏览器结果不完整，未清理文件，请手动核对")?;
    if entries.iter().any(|entry| {
        !entry["id"]
            .as_str()
            .is_some_and(|id| id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b)))
            || !entry["path"].as_str().is_some_and(|path| {
                std::path::Path::new(path).is_absolute() && !path.chars().any(char::is_control)
            })
            || !entry["version"].as_str().is_some_and(crate::valid_version)
            || entry["enabled"].as_bool().is_none()
    }) {
        return Err("浏览器结果不完整，未清理文件，请手动核对".into());
    }
    Ok(entries)
}

fn validate_single_profile(profile: &BrowserProfile) -> Result<()> {
    if profile.browser != Browser::Edge {
        return Err("当前只支持 Edge 自动核对，请使用手动确认".into());
    }
    profile.validate_location()?;
    let state: serde_json::Value = serde_json::from_slice(
        &fs::read(profile.user_data_dir.join("Local State"))
            .map_err(|_| "无法核对浏览器配置，请使用手动确认")?,
    )
    .map_err(|_| "浏览器配置列表无法读取，请使用手动确认")?;
    let index = state
        .pointer("/profile/info_cache")
        .and_then(|v| v.as_object())
        .ok_or("无法核对浏览器配置，请使用手动确认")?;
    if index.len() != 1 || !index.contains_key(&profile.profile_directory) {
        return Err("此浏览器目录包含多个配置或目标不明确，请使用手动确认".into());
    }
    // Fresh Default profiles may omit the persisted preference. Only that
    // sole-profile case can proceed to the live edge://version path proof.
    let last_used_matches = match state.pointer("/profile/last_used") {
        None => profile.profile_directory == "Default",
        Some(value) => value.as_str() == Some(profile.profile_directory.as_str()),
    };
    if !last_used_matches {
        return Err("浏览器实际配置与所选目标不一致，请使用手动确认".into());
    }
    if ["Guest Profile", "System Profile"].contains(&profile.profile_directory.as_str()) {
        return Err("不能自动操作访客或系统配置".into());
    }
    for entry in fs::read_dir(&profile.user_data_dir).map_err(|_| "无法核对浏览器目录")? {
        let entry = entry.map_err(|_| "无法核对浏览器目录")?;
        let name = entry.file_name();
        if name != profile.profile_directory.as_str()
            && name != "Guest Profile"
            && name != "System Profile"
            && entry.path().join("Preferences").exists()
        {
            return Err("此浏览器目录包含多个配置，请使用手动确认".into());
        }
    }
    Ok(())
}
