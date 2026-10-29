//! Browser/Profile bindings and local navigation preferences. No browser data is imported.
use crate::{Browser, Result, Target};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserProfile {
    pub id: String,
    pub browser: Browser,
    pub name: String,
    pub application: PathBuf,
    pub user_data_dir: PathBuf,
    pub profile_directory: String,
}
impl BrowserProfile {
    pub fn profile_path(&self) -> PathBuf {
        self.user_data_dir.join(&self.profile_directory)
    }
    pub fn target(&self) -> Target {
        Target {
            browser: self.browser.clone(),
            profile: self.id.clone(),
        }
    }
    pub fn label(&self) -> String {
        format!("{:?} · {}", self.browser, self.name)
    }
    pub fn management_url(&self) -> &'static str {
        match self.browser {
            Browser::Chrome => "chrome://extensions",
            Browser::Edge => "edge://extensions",
        }
    }
    pub fn validate_location(&self) -> Result<()> {
        if !self.application.is_file()
            || !self.profile_path().join("Preferences").is_file()
            || self.profile_directory.is_empty()
            || self.profile_directory == "."
            || self.profile_directory == ".."
            || self.profile_directory.contains(['/', '\\', ':'])
            || self.profile_path().canonicalize().ok().as_ref() != Some(&self.profile_path())
        {
            return Err(format!(
                "找不到 {} 的应用或已有配置（Preferences）。请从浏览器 version 页核对 Profile Path；不会创建或切换配置",
                self.label()
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Settings {
    profiles: Vec<BrowserProfile>,
    pinned: Vec<String>,
    selected: Option<String>,
}
pub struct ProfileBook {
    root: PathBuf,
    settings: Settings,
    _lock: File,
}
impl ProfileBook {
    pub fn open(root: &Path) -> Result<Self> {
        fs::create_dir_all(root).map_err(error)?;
        let root = root.canonicalize().map_err(error)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("profiles.lock"))
            .map_err(error)?;
        lock.try_lock_exclusive()
            .map_err(|_| "配置偏好正在使用，请关闭另一个 CEPM 窗口".to_string())?;
        let path = root.join("profiles.json");
        let settings: Settings = if path.exists() {
            serde_json::from_slice(&fs::read(path).map_err(error)?)
                .map_err(|e| format!("配置偏好损坏，请保留文件后检查：{e}"))?
        } else {
            Settings::default()
        };
        let known = |id: &str| settings.profiles.iter().any(|p| p.id == id);
        let valid = settings.selected.as_deref().is_none_or(known)
            && settings.pinned.iter().all(|id| known(id))
            && settings.profiles.iter().all(|p| {
                uuid::Uuid::parse_str(&p.id).is_ok_and(|id| id.to_string() == p.id)
                    && p.application.is_absolute()
                    && p.user_data_dir.is_absolute()
                    && !p.profile_directory.is_empty()
                    && p.profile_directory != "."
                    && p.profile_directory != ".."
                    && !p.profile_directory.contains(['/', '\\', ':'])
            });
        let unique: std::collections::HashSet<_> =
            settings.profiles.iter().map(|p| &p.id).collect();
        if !valid || unique.len() != settings.profiles.len() {
            return Err(
                "配置偏好的身份或位置损坏，请保留数据文件并检查；不会切换到其他配置".into(),
            );
        }
        Ok(Self {
            root,
            settings,
            _lock: lock,
        })
    }
    pub fn profiles(&self) -> &[BrowserProfile] {
        &self.settings.profiles
    }
    pub fn pinned_ids(&self) -> &[String] {
        &self.settings.pinned
    }
    pub fn selected(&self) -> Option<&BrowserProfile> {
        self.settings
            .selected
            .as_ref()
            .and_then(|id| self.settings.profiles.iter().find(|p| &p.id == id))
    }
    pub fn bind(
        &mut self,
        browser: Browser,
        name: &str,
        application: &Path,
        profile_path: &Path,
    ) -> Result<BrowserProfile> {
        if name.trim().is_empty() {
            return Err("请填写便于识别的配置名称".into());
        }
        let application = application.canonicalize().map_err(error)?;
        let profile_path = profile_path.canonicalize().map_err(error)?;
        let parent = profile_path
            .parent()
            .ok_or("请选择具体 Profile 目录，不是磁盘根目录")?;
        let directory = profile_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("配置目录名称无法读取")?;
        let existing = self.settings.profiles.iter().find(|p| {
            p.browser == browser && p.application == application && p.profile_path() == profile_path
        });
        if let Some(p) = existing {
            return Ok(p.clone());
        }
        let profile = BrowserProfile {
            id: uuid::Uuid::new_v4().to_string(),
            browser,
            name: name.trim().into(),
            application,
            user_data_dir: parent.into(),
            profile_directory: directory.into(),
        };
        profile.validate_location()?;
        let mut next = self.settings.clone();
        if next.selected.is_none() {
            next.selected = Some(profile.id.clone());
        }
        next.profiles.push(profile.clone());
        self.save(next)?;
        Ok(profile)
    }
    pub fn set_pinned(&mut self, id: &str, pinned: bool) -> Result<()> {
        self.get(id)?;
        let mut next = self.settings.clone();
        if pinned && !next.pinned.iter().any(|p| p == id) {
            next.pinned.push(id.into());
        }
        if !pinned {
            next.pinned.retain(|p| p != id);
        }
        self.save(next)
    }
    pub fn select(&mut self, id: &str) -> Result<()> {
        self.get(id)?;
        let mut next = self.settings.clone();
        next.selected = Some(id.into());
        self.save(next)
    }
    pub fn get(&self, id: &str) -> Result<&BrowserProfile> {
        self.settings
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or("找不到绑定的配置，请先定位".into())
    }
    fn save(&mut self, settings: Settings) -> Result<()> {
        let mut file = NamedTempFile::new_in(&self.root).map_err(error)?;
        file.write_all(&serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?)
            .map_err(error)?;
        file.as_file().sync_all().map_err(error)?;
        file.persist(self.root.join("profiles.json"))
            .map_err(|e| error(e.error))?;
        self.settings = settings;
        Ok(())
    }
}
fn error(e: std::io::Error) -> String {
    format!("配置文件操作失败：{e}。请检查路径和权限后重试")
}

#[derive(Debug, Clone)]
pub struct BrowserLocation {
    pub browser: Browser,
    pub application: PathBuf,
    pub user_data_dir: PathBuf,
}
#[derive(Debug, Clone)]
pub struct ProfileCandidate {
    pub browser: Browser,
    pub name: String,
    pub application: PathBuf,
    pub profile_path: PathBuf,
}
/// Reads only the profile name/directory index, never history, cookies or account data.
pub fn discover_profiles(locations: &[BrowserLocation]) -> Result<Vec<ProfileCandidate>> {
    let mut found = Vec::new();
    for location in locations {
        if !location.application.is_file() {
            continue;
        }
        let file = location.user_data_dir.join("Local State");
        if !file.exists() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_slice(&fs::read(file).map_err(error)?)
            .map_err(|_| "无法读取浏览器配置列表，请使用手动定位".to_string())?;
        if let Some(profiles) = value
            .pointer("/profile/info_cache")
            .and_then(|v| v.as_object())
        {
            for (directory, metadata) in profiles {
                if directory.is_empty()
                    || directory == "."
                    || directory == ".."
                    || directory.contains(['/', '\\', ':'])
                {
                    continue;
                }
                let path = location.user_data_dir.join(directory);
                if !path.is_dir() {
                    continue;
                }
                let name = metadata
                    .get("name")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(directory);
                found.push(ProfileCandidate {
                    browser: location.browser.clone(),
                    name: name.into(),
                    application: location.application.clone(),
                    profile_path: path,
                });
            }
        }
    }
    Ok(found)
}

#[derive(Debug, Clone)]
pub struct ManagementRequest {
    pub application: PathBuf,
    pub arguments: Vec<std::ffi::OsString>,
}
impl ManagementRequest {
    /// Requests a browser window. Success does not assert which page loaded or extension health.
    pub fn launch(&self) -> Result<()> {
        use std::process::{Command, Stdio};
        let mut child = Command::new(&self.application)
            .args(&self.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                format!("无法打开浏览器扩展页：{e}。请检查浏览器位置，或复制管理页地址手动打开")
            })?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}
impl BrowserProfile {
    pub fn management_request(&self) -> Result<ManagementRequest> {
        self.validate_location()?;
        let mut data = std::ffi::OsString::from("--user-data-dir=");
        data.push(&self.user_data_dir);
        let arguments = vec![
            data,
            format!("--profile-directory={}", self.profile_directory).into(),
            "--new-window".into(),
            self.management_url().into(),
        ];
        // Let LaunchServices own a macOS app's lifecycle and normal application launch context.
        // The explicit bundle and --args retain the exact browser/Profile binding; no default URL handler.
        #[cfg(target_os = "macos")]
        if let Some(macos) = self.application.parent()
            && macos.file_name().is_some_and(|name| name == "MacOS")
            && let Some(contents) = macos.parent()
            && contents.file_name().is_some_and(|name| name == "Contents")
            && let Some(bundle) = contents.parent()
            && bundle
                .extension()
                .is_some_and(|extension| extension == "app")
        {
            let mut launch_arguments = vec![
                "-n".into(),
                "-a".into(),
                bundle.as_os_str().to_owned(),
                "--args".into(),
            ];
            launch_arguments.extend(arguments);
            return Ok(ManagementRequest {
                application: "/usr/bin/open".into(),
                arguments: launch_arguments,
            });
        }
        Ok(ManagementRequest {
            application: self.application.clone(),
            arguments,
        })
    }
}

impl crate::Manager {
    /// Explicitly identifies a pre-binding record; this never migrates an already-bound installation.
    pub fn bind_legacy_target(
        &mut self,
        id: &str,
        profile: &BrowserProfile,
    ) -> Result<crate::Installation> {
        profile.validate_location()?;
        let mut record = self
            .list()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or("找不到旧安装记录")?;
        if uuid::Uuid::parse_str(&record.target.profile).is_ok() {
            return Err("此安装已绑定具体配置，不支持跨配置迁移".into());
        }
        if record.target.browser != profile.browser {
            return Err("请选择与旧安装一致的浏览器；不支持迁移浏览器数据".into());
        }
        if record.recovery_required {
            return Err("此安装需要先恢复，不能重新绑定".into());
        }
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(record.install_dir.join("manifest.json")).map_err(error)?,
        )
        .map_err(|e| e.to_string())?;
        super::validate_manifest(&manifest, &record.install_dir)?;
        if manifest["version"].as_str() != Some(&record.installed_version) {
            return Err("当前文件与记录版本不一致，请保留数据并检查".into());
        }
        record.content_digest = super::updates::directory_digest(&record.install_dir)?;
        record.generation = record.generation.checked_add(1).ok_or("安装代数溢出")?;
        record.historical_version = record
            .effective_version
            .take()
            .or(record.historical_version);
        record.last_operation = "用户接受当前文件基线并定位配置，等待重新核对".into();
        record.target = profile.target();
        record.verification = crate::Verification::Pending;
        record.effective_version = None;
        record.extension_id = None;
        self.save(&record)?;
        Ok(record)
    }
}
