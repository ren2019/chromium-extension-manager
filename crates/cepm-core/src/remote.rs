//! Shared GitHub CLI and archive boundary. Authentication remains entirely in gh.
use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubRepo {
    pub owner: String,
    pub name: String,
}
impl GitHubRepo {
    pub fn parse(input: &str) -> Result<Self> {
        let slug = input
            .trim()
            .strip_prefix("https://github.com/")
            .unwrap_or(input.trim())
            .trim_end_matches('/');
        let parts: Vec<_> = slug.split('/').collect();
        if parts.len() != 2 {
            return Err("来源地址无效：请提供 github.com 的 owner/repo 或仓库地址".into());
        }
        let name = parts[1].strip_suffix(".git").unwrap_or(parts[1]);
        if [parts[0], name].iter().any(|p| {
            p.is_empty()
                || *p == "."
                || *p == ".."
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        }) {
            return Err("来源地址无效：仓库名称包含不支持的字符".into());
        }
        Ok(Self {
            owner: parts[0].into(),
            name: name.into(),
        })
    }
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
    pub fn endpoint(&self, suffix: &str) -> String {
        format!("repos/{}/{}", self.slug(), suffix)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteOrigin {
    GitHubRelease {
        repo: GitHubRepo,
        tag: String,
        asset_name: String,
        asset_selector: String,
        root: String,
    },
    GitHubDirectory {
        repo: GitHubRepo,
        reference: String,
        commit: String,
        subdirectory: String,
    },
}
impl RemoteOrigin {
    pub fn location(&self) -> String {
        match self {
            Self::GitHubRelease { repo, tag, .. } => format!(
                "https://github.com/{}/releases/tag/{}",
                repo.slug(),
                encode_segment(tag)
            ),
            Self::GitHubDirectory {
                repo,
                reference,
                subdirectory,
                ..
            } => format!(
                "https://github.com/{}/tree/{}/{}",
                repo.slug(),
                encode_segment(reference),
                subdirectory
            ),
        }
    }
}
pub fn encode_segment(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

use std::{
    fs,
    io::{Cursor, Read},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use unicode_normalization::UnicodeNormalization;
pub const MAX_ARCHIVE_BYTES: usize = 100 * 1024 * 1024;
pub const MAX_JSON_BYTES: usize = 16 * 1024 * 1024;
pub trait Gh {
    fn run(&self, args: &[String], max_bytes: usize) -> Result<Vec<u8>>;
}
#[derive(Default)]
pub struct CommandGh;
impl Gh for CommandGh {
    fn run(&self, args: &[String], max_bytes: usize) -> Result<Vec<u8>> {
        let mut child = Command::new("gh")
            .args(args)
            .env_remove("GH_DEBUG")
            .env_remove("GH_FORCE_TTY")
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_PAGER", "cat")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    "获取失败：找不到 gh。请安装 GitHub CLI 并完成 gh auth login；本地目录仍可用"
                        .into()
                } else {
                    format!("获取失败：无法启动 gh（{e}）。请检查 GitHub CLI 安装")
                }
            })?;
        let stdout = child.stdout.take().ok_or("无法读取 gh 输出")?;
        let stderr = child.stderr.take().ok_or("无法读取 gh 错误")?;
        let out =
            std::thread::spawn(move || bounded_read(stdout, max_bytes.min(MAX_ARCHIVE_BYTES)));
        let err = std::thread::spawn(move || bounded_read(stderr, 64 * 1024));
        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {}
                Err(_) => break Err("获取失败：无法等待 gh；请重试".to_string()),
            }
            if start.elapsed() > Duration::from_secs(120) {
                break Err(
                    "获取失败：gh 请求超过两分钟。请检查网络和 gh auth status 后重试".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        if status.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let output = out.join().map_err(|_| "读取 gh 输出失败")?;
        let error = err.join().map_err(|_| "读取 gh 错误失败")?;
        let status = status?;
        if !status.success() {
            let stderr = String::from_utf8_lossy(error.as_deref().unwrap_or_default());
            let reason = if stderr.contains("401") || stderr.contains("auth login") {
                "授权不可用，请在终端运行 gh auth login / gh auth status"
            } else if stderr.contains("403") {
                "GitHub 拒绝访问，请检查权限、SSO 或请求额度后重试"
            } else if stderr.contains("404") {
                "来源不存在或当前账号无权访问，请核对地址与 gh 的仓库访问权限"
            } else {
                "gh 请求失败，请检查网络以及 gh auth status 后重试"
            };
            return Err(format!("获取失败：{reason}；已有安装未改动"));
        }
        output
    }
}
fn bounded_read(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("读取产物失败：{e}。请重试"))?;
    if bytes.len() > limit {
        return Err("来源超过大小限制，请选择较小的预构建扩展产物".into());
    }
    Ok(bytes)
}
pub(crate) fn api_args(endpoint: String) -> Vec<String> {
    vec![
        "api".into(),
        "--hostname".into(),
        "github.com".into(),
        endpoint,
    ]
}

/// Extract only into a newly created, empty private temporary directory.
pub fn extract_zip(bytes: &[u8], destination: &Path) -> Result<()> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err("解压失败：压缩包超过 100 MiB，请选择较小产物".into());
    }
    if !destination.is_dir()
        || destination
            .symlink_metadata()
            .map_err(crate::io_error)?
            .file_type()
            .is_symlink()
        || fs::read_dir(destination)
            .map_err(crate::io_error)?
            .next()
            .is_some()
    {
        return Err("解压目标必须是空的临时目录".into());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("解压失败：无效 ZIP（{e}），请选择完整产物"))?;
    if archive.is_empty() || archive.len() > 10000 {
        return Err("解压失败：ZIP 为空或文件数量超过 10000".into());
    }
    let mut paths = std::collections::BTreeMap::<String, bool>::new();
    let mut spellings = std::collections::BTreeMap::<String, String>::new();
    let mut entries = Vec::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| format!("解压失败：{e}"))?;
        let name =
            std::str::from_utf8(file.name_raw()).map_err(|_| "解压失败：文件名不是 UTF-8")?;
        let clean = name.strip_suffix('/').unwrap_or(name);
        let parts: Vec<_> = clean.split('/').collect();
        if clean.is_empty()
            || clean.len() > 512
            || parts.len() > 32
            || name.contains(['\\', ':', '\0'])
            || parts.iter().any(|p| {
                p.is_empty()
                    || *p == "."
                    || *p == ".."
                    || p.ends_with(['.', ' '])
                    || reserved_name(p)
            })
        {
            return Err(format!("解压失败：不安全的 ZIP 路径 {name:?}"));
        }
        let mode = file.unix_mode().unwrap_or(0) & 0o170000;
        if ![0, 0o040000, 0o100000].contains(&mode) || (mode == 0o040000 && !file.is_dir()) {
            return Err("解压失败：ZIP 包含符号链接或特殊文件".into());
        }
        let key = clean.nfkc().collect::<String>().to_lowercase();
        let mut spelling = String::new();
        for part in &parts {
            if !spelling.is_empty() {
                spelling.push('/');
            }
            spelling.push_str(part);
            let normalized = spelling.nfkc().collect::<String>().to_lowercase();
            if spellings
                .insert(normalized, spelling.clone())
                .is_some_and(|old| old != spelling)
            {
                return Err("解压失败：ZIP 目录存在大小写或 Unicode 冲突".into());
            }
        }
        if paths.insert(key.clone(), file.is_dir()).is_some() {
            return Err("解压失败：ZIP 包含重复或大小写冲突路径".into());
        }
        if file.size() > 64 * 1024 * 1024 {
            return Err("解压失败：单个文件超过 64 MiB".into());
        }
        total = total.checked_add(file.size()).ok_or("解压大小溢出")?;
        if total > 256 * 1024 * 1024 {
            return Err("解压失败：展开大小超过 256 MiB".into());
        }
        entries.push((clean.to_string(), key, file.is_dir()));
    }
    for (_, key, _) in &entries {
        let mut parent = key.as_str();
        while let Some((p, _)) = parent.rsplit_once('/') {
            if paths.get(p) == Some(&false) {
                return Err("解压失败：ZIP 文件与目录路径冲突".into());
            }
            parent = p;
        }
    }
    total = 0;
    for (i, (name, _, is_dir)) in entries.into_iter().enumerate() {
        let path = destination.join(name);
        if is_dir {
            fs::create_dir_all(path).map_err(crate::io_error)?;
            continue;
        }
        fs::create_dir_all(path.parent().ok_or("无效 ZIP 目录")?).map_err(crate::io_error)?;
        let file = archive.by_index(i).map_err(|e| format!("解压失败：{e}"))?;
        let data = bounded_read(file, 64 * 1024 * 1024).map_err(|e| format!("解压失败：{e}"))?;
        total += data.len() as u64;
        if total > 256 * 1024 * 1024 {
            return Err("解压失败：实际展开大小超过 256 MiB".into());
        }
        fs::write(path, data).map_err(crate::io_error)?;
    }
    Ok(())
}
fn reserved_name(name: &str) -> bool {
    let normalized = name.nfkc().collect::<String>();
    let base = normalized
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
}

pub use crate::releases::{
    Release, ReleaseArtifact, ReleaseAsset, ReleaseCatalog, ReleaseSelection,
};
