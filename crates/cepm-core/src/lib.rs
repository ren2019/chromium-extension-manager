//! Installation use cases, independent of the desktop UI.
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::{NamedTempFile, TempDir};

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Browser {
    Chrome,
    Edge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub browser: Browser,
    pub profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verification {
    Pending,
    UserConfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installation {
    pub id: String,
    pub name: String,
    pub source: PathBuf,
    pub install_dir: PathBuf,
    pub target: Target,
    pub installed_version: String,
    pub extension_id: Option<String>,
    pub effective_version: Option<String>,
    pub verification: Verification,
}

pub struct Preview {
    source: PathBuf,
    name: String,
    version: String,
    snapshot: TempDir,
}
impl Preview {
    pub fn source(&self) -> &Path {
        &self.source
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn version(&self) -> &str {
        &self.version
    }
}

pub struct Manager {
    root: PathBuf,
    _lock: File,
}

impl Manager {
    pub fn open(root: &Path) -> Result<Self> {
        fs::create_dir_all(root).map_err(io_error)?;
        let root = root.canonicalize().map_err(io_error)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("manager.lock"))
            .map_err(io_error)?;
        lock.try_lock_exclusive()
            .map_err(|_| "安装记录正在使用，请关闭另一个 CEPM 窗口后重试".to_string())?;
        fs::create_dir_all(root.join("installations")).map_err(io_error)?;
        fs::create_dir_all(root.join("records")).map_err(io_error)?;
        Ok(Self { root, _lock: lock })
    }

    pub fn preview_local(&self, source: &Path) -> Result<Preview> {
        let source = source.canonicalize().map_err(io_error)?;
        if source.starts_with(&self.root) || self.root.starts_with(&source) {
            return Err("来源目录与受管数据目录不能互相包含，请选择独立的扩展来源".into());
        }
        let snapshot = tempfile::tempdir_in(&self.root).map_err(io_error)?;
        copy_directory(&source, snapshot.path())?;
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(snapshot.path().join("manifest.json")).map_err(io_error)?,
        )
        .map_err(|e| format!("无法读取扩展清单：{e}"))?;
        validate_manifest(&manifest, snapshot.path())?;
        let name = manifest["name"].as_str().ok_or("清单缺少名称")?.to_string();
        let version = manifest["version"]
            .as_str()
            .ok_or("清单缺少版本")?
            .to_string();
        Ok(Preview {
            source,
            name,
            version,
            snapshot,
        })
    }

    pub fn install_local(&mut self, preview: Preview, target: Target) -> Result<Installation> {
        if target.profile.trim().is_empty() {
            return Err("请填写目标浏览器配置名称或路径，避免在错误配置确认".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let install_dir = self.root.join("installations").join(&id);
        fs::rename(preview.snapshot.path(), &install_dir).map_err(io_error)?;
        let record = Installation {
            id,
            name: preview.name,
            source: preview.source,
            install_dir,
            target,
            installed_version: preview.version,
            extension_id: None,
            effective_version: None,
            verification: Verification::Pending,
        };
        if let Err(error) = self.save(&record) {
            fs::remove_dir_all(&record.install_dir).map_err(|cleanup| {
                format!(
                    "{error}；清理未登记文件失败：{cleanup}，目录：{}",
                    record.install_dir.display()
                )
            })?;
            return Err(error);
        }
        Ok(record)
    }

    pub fn list(&self) -> Result<Vec<Installation>> {
        let mut records = Vec::new();
        for entry in fs::read_dir(self.root.join("records")).map_err(io_error)? {
            let path = entry.map_err(io_error)?.path();
            if path.extension().is_some_and(|e| e == "json") {
                let record: Installation =
                    serde_json::from_slice(&fs::read(&path).map_err(io_error)?)
                        .map_err(|e| format!("安装记录损坏，请保留数据目录并检查：{e}"))?;
                let valid_id =
                    uuid::Uuid::parse_str(&record.id).is_ok_and(|id| id.to_string() == record.id);
                if !valid_id
                    || path.file_stem().and_then(|s| s.to_str()) != Some(&record.id)
                    || record.install_dir != self.root.join("installations").join(&record.id)
                {
                    return Err("安装记录身份或路径无效，请保留数据目录并检查".into());
                }
                records.push(record);
            }
        }
        records.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(records)
    }

    pub fn confirm(&mut self, id: &str, extension_id: &str, version: &str) -> Result<Installation> {
        let mut record = self
            .list()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or("找不到该安装记录，请重新选择")?;
        if version != record.installed_version {
            return Err("核对版本与磁盘安装版本不一致，请在目标浏览器配置重载后再次核对".into());
        }
        if extension_id.len() != 32 || !extension_id.bytes().all(|b| (b'a'..=b'p').contains(&b)) {
            return Err("扩展 ID 应为 32 个 a–p 小写字母，请从目标浏览器扩展管理页复制".into());
        }
        if record
            .extension_id
            .as_deref()
            .is_some_and(|old| old != extension_id)
        {
            return Err("扩展 ID 与已有记录不一致，请检查浏览器配置和安装目录".into());
        }
        record.extension_id = Some(extension_id.into());
        record.effective_version = Some(version.into());
        record.verification = Verification::UserConfirmed;
        self.save(&record)?;
        Ok(record)
    }

    fn save(&self, record: &Installation) -> Result<()> {
        let mut file = NamedTempFile::new_in(self.root.join("records")).map_err(io_error)?;
        file.write_all(&serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?)
            .map_err(io_error)?;
        file.as_file().sync_all().map_err(io_error)?;
        file.persist(
            self.root
                .join("records")
                .join(format!("{}.json", record.id)),
        )
        .map_err(|e| io_error(e.error))?;
        Ok(())
    }
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(source).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let output = destination.join(entry.file_name());
        if entry.file_type().map_err(io_error)?.is_dir() {
            fs::create_dir(&output).map_err(io_error)?;
            copy_directory(&entry.path(), &output)?;
        } else if entry.file_type().map_err(io_error)?.is_file() {
            fs::copy(entry.path(), output).map_err(io_error)?;
        } else {
            return Err(format!(
                "不支持链接或特殊文件：{}。请提供普通文件构成的完整产物",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn io_error(error: std::io::Error) -> String {
    format!("文件操作失败：{error}。请检查路径和读写权限后重试")
}

// Chromium's manifest version format, not SemVer:
// https://developer.chrome.com/docs/extensions/reference/manifest/version
fn valid_version(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    (1..=4).contains(&parts.len())
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
                && part.parse::<u16>().is_ok()
        })
}

fn validate_manifest(manifest: &serde_json::Value, root: &Path) -> Result<()> {
    if manifest["manifest_version"].as_u64() != Some(3) {
        return Err("需要完整的 Manifest V3 扩展产物".into());
    }
    if manifest["name"]
        .as_str()
        .is_none_or(|s| s.trim().is_empty())
    {
        return Err("清单名称不能为空".into());
    }
    if !manifest["version"].as_str().is_some_and(valid_version) {
        return Err("扩展版本应为 1–4 段 0–65535 整数，不能有前导零".into());
    }
    let resource = |value: &serde_json::Value| -> Result<()> {
        let path = value.as_str().ok_or("清单资源路径必须为文本")?;
        if path.is_empty()
            || path.contains('\\')
            || path.contains(':')
            || path.starts_with('/')
            || path
                .split('/')
                .any(|p| p.is_empty() || p == ".." || p == ".")
        {
            return Err(format!("不安全的扩展资源路径：{path}"));
        }
        if !root.join(path).is_file() {
            return Err(format!("扩展缺少资源：{path}。请选择完整的预构建产物"));
        }
        Ok(())
    };
    for pointer in [
        "/background/service_worker",
        "/action/default_popup",
        "/options_ui/page",
        "/options_page",
        "/devtools_page",
        "/side_panel/default_path",
    ] {
        if let Some(value) = manifest.pointer(pointer) {
            resource(value)?;
        }
    }
    for pointer in ["/icons", "/chrome_url_overrides"] {
        if let Some(value) = manifest.pointer(pointer) {
            for value in value.as_object().ok_or("清单资源映射格式无效")?.values() {
                resource(value)?;
            }
        }
    }
    if let Some(value) = manifest.pointer("/action/default_icon") {
        if let Some(map) = value.as_object() {
            for value in map.values() {
                resource(value)?;
            }
        } else {
            resource(value)?;
        }
    }
    if let Some(scripts) = manifest.get("content_scripts") {
        for script in scripts.as_array().ok_or("content_scripts 格式无效")? {
            for kind in ["js", "css"] {
                if let Some(files) = script.get(kind) {
                    for file in files.as_array().ok_or("内容脚本资源列表格式无效")? {
                        resource(file)?;
                    }
                }
            }
        }
    }
    if let Some(rules) = manifest.pointer("/declarative_net_request/rule_resources") {
        for rule in rules.as_array().ok_or("声明式规则资源列表格式无效")? {
            resource(rule.get("path").ok_or("声明式规则缺少资源路径")?)?;
        }
    }
    if let Some(pages) = manifest.pointer("/sandbox/pages") {
        for page in pages.as_array().ok_or("沙盒页面列表格式无效")? {
            resource(page)?;
        }
    }
    if let Some(locale) = manifest.get("default_locale") {
        let locale = locale.as_str().ok_or("默认语言格式无效")?;
        if locale.is_empty()
            || !locale
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("默认语言名称无效".into());
        }
        resource(&serde_json::Value::String(format!(
            "_locales/{locale}/messages.json"
        )))?;
    }
    Ok(())
}
