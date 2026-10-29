use crate::{Manager, Preview, Result, UpdatePlan, remote::*};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub digest: Option<String>,
    #[serde(default)]
    pub state: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}
pub struct ReleaseCatalog {
    pub repo: GitHubRepo,
    pub releases: Vec<Release>,
    pub default_tag: Option<String>,
    pub requested_tag: Option<String>,
}
impl ReleaseCatalog {
    pub fn preselected_asset(&self, tag: &str) -> Option<u64> {
        let release = self.releases.iter().find(|r| r.tag_name == tag)?;
        let mut recognizable = release.assets.iter().filter(|a| {
            a.name
                .to_ascii_lowercase()
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|part| part == "extension")
        });
        let first = recognizable.next()?;
        if recognizable.next().is_some() {
            None
        } else {
            Some(first.id)
        }
    }
}
#[derive(Default, Debug, Clone)]
pub struct ReleaseSelection {
    pub tag: Option<String>,
    pub asset_id: Option<u64>,
}
pub struct ReleaseArtifact {
    candidates: Vec<(String, Preview)>,
}
impl ReleaseArtifact {
    pub fn roots(&self) -> Vec<String> {
        self.candidates
            .iter()
            .map(|(path, _)| path.clone())
            .collect()
    }
    pub fn select_root(self, root: Option<&str>) -> Result<Preview> {
        if root.is_none() && self.candidates.len() != 1 {
            return Err(format!("请选择扩展根目录：{}", self.roots().join("、")));
        }
        self.candidates
            .into_iter()
            .find(|(p, _)| root.is_none_or(|r| r == p))
            .map(|(_, p)| p)
            .ok_or_else(|| "所选扩展根目录已不存在，请重新选择".into())
    }
}
impl Manager {
    pub fn list_releases(&self, gh: &impl Gh, source: &str) -> Result<ReleaseCatalog> {
        let (repo, requested_tag) = parse_release_source(source)?;
        let mut args = api_args(repo.endpoint("releases?per_page=100"));
        args.extend(["--paginate".into(), "--slurp".into()]);
        let bytes = gh
            .run(&args, MAX_JSON_BYTES)
            .map_err(|e| format!("查询版本失败：{e}"))?;
        if bytes.len() > MAX_JSON_BYTES {
            return Err("查询版本失败：元数据过大".into());
        }
        let pages: Vec<Vec<Release>> = serde_json::from_slice(&bytes)
            .map_err(|e| format!("查询版本失败：gh 返回无效版本清单（{e}）"))?;
        let mut releases: Vec<_> = pages.into_iter().flatten().filter(|r| !r.draft).collect();
        for release in &mut releases {
            release
                .assets
                .retain(|a| a.state == "uploaded" && a.name.to_ascii_lowercase().ends_with(".zip"));
        }
        let default_tag = if releases.iter().any(|r| !r.prerelease) {
            let bytes = gh
                .run(&api_args(repo.endpoint("releases/latest")), MAX_JSON_BYTES)
                .map_err(|e| format!("查询默认正式版本失败：{e}"))?;
            let latest: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|e| format!("默认版本响应无效：{e}"))?;
            let tag = latest["tag_name"].as_str().ok_or("默认版本缺少标签")?;
            if !releases.iter().any(|r| r.tag_name == tag && !r.prerelease) {
                return Err("正式版本清单已变化，请重新查询".into());
            }
            Some(tag.to_string())
        } else {
            None
        };
        Ok(ReleaseCatalog {
            repo,
            releases,
            default_tag,
            requested_tag,
        })
    }
    pub fn preview_release(
        &self,
        gh: &impl Gh,
        source: &str,
        selection: &ReleaseSelection,
    ) -> Result<ReleaseArtifact> {
        let catalog = self.list_releases(gh, source)?;
        let tag = selection
            .tag
            .as_ref()
            .or(catalog.requested_tag.as_ref())
            .or(catalog.default_tag.as_ref())
            .ok_or("没有正式版本，请明确选择可获取的预发布版本")?;
        let release = catalog
            .releases
            .iter()
            .find(|r| &r.tag_name == tag)
            .ok_or("所选 Release 已不可获取，请重新选择版本")?;
        let asset_id = selection
            .asset_id
            .or_else(|| catalog.preselected_asset(tag))
            .ok_or_else(|| {
                format!(
                    "请选择 ZIP 附件：{}",
                    release
                        .assets
                        .iter()
                        .map(|a| a.name.as_str())
                        .collect::<Vec<_>>()
                        .join("、")
                )
            })?;
        let asset = release
            .assets
            .iter()
            .find(|a| a.id == asset_id)
            .ok_or("所选附件已不可获取，请重新选择")?;
        if asset.size > MAX_ARCHIVE_BYTES as u64 {
            return Err("下载失败：附件超过 100 MiB，请选择较小产物".into());
        }
        let mut args = api_args(
            catalog
                .repo
                .endpoint(&format!("releases/assets/{}", asset.id)),
        );
        args.extend(["-H".into(), "Accept: application/octet-stream".into()]);
        let bytes = gh
            .run(&args, MAX_ARCHIVE_BYTES)
            .map_err(|e| format!("下载附件失败：{e}"))?;
        if let Some(digest) = &asset.digest {
            let expected = digest
                .strip_prefix("sha256:")
                .ok_or("校验失败：不支持的摘要算法，请检查来源摘要")?;
            if expected.len() != 64
                || !expected.bytes().all(|b| b.is_ascii_hexdigit())
                || format!("{:x}", Sha256::digest(&bytes)) != expected.to_ascii_lowercase()
            {
                return Err("校验失败：附件 SHA-256 与来源声明不一致，请重新下载或核对发布产物；已有安装未改动".into());
            }
        }
        let extracted = tempfile::tempdir().map_err(crate::io_error)?;
        extract_zip(&bytes, extracted.path())?;
        let mut roots = Vec::new();
        find_roots(extracted.path(), extracted.path(), &mut roots)?;
        let mut candidates = Vec::new();
        for root in roots {
            if let Ok(preview) = self.preview_local(&extracted.path().join(&root)) {
                if declared_version(&asset.name)
                    .is_some_and(|(_, _, version)| version != preview.version())
                {
                    return Err(
                        "校验失败：附件声明的扩展版本与 manifest.json 不一致，请核对发布产物"
                            .into(),
                    );
                }
                let origin = RemoteOrigin::GitHubRelease {
                    repo: catalog.repo.clone(),
                    tag: tag.clone(),
                    asset_name: asset.name.clone(),
                    asset_selector: selector(&asset.name),
                    root: root.clone(),
                };
                candidates.push((root, preview.with_origin(origin)));
            }
        }
        if candidates.is_empty() {
            return Err(
                "验证扩展失败：ZIP 中没有完整的 Manifest V3 扩展产物；请选择已构建产物".into(),
            );
        }
        Ok(ReleaseArtifact { candidates })
    }
    pub fn check_release(&self, gh: &impl Gh, id: &str) -> Result<UpdatePlan> {
        let record = self.record(id)?;
        let Some(RemoteOrigin::GitHubRelease {
            repo,
            asset_selector,
            root,
            ..
        }) = record.origin
        else {
            return Err("此安装不是 GitHub Release 来源".into());
        };
        let catalog = self.list_releases(gh, &repo.slug())?;
        let tag = catalog
            .default_tag
            .ok_or("没有可获取的正式版本，请重新选择")?;
        let release = catalog
            .releases
            .iter()
            .find(|r| r.tag_name == tag)
            .ok_or("版本已不可用")?;
        let matching: Vec<_> = release
            .assets
            .iter()
            .filter(|a| selector(&a.name) == asset_selector)
            .collect();
        if matching.len() != 1 {
            return Err("保存的附件选择不再唯一匹配，请重新选择版本和附件".into());
        }
        let artifact = self.preview_release(
            gh,
            &repo.slug(),
            &ReleaseSelection {
                tag: Some(tag),
                asset_id: Some(matching[0].id),
            },
        )?;
        self.prepare_update(id, artifact.select_root(Some(&root))?)
    }
}
fn parse_release_source(source: &str) -> Result<(GitHubRepo, Option<String>)> {
    if let Some((repo, tag)) = source.split_once("/releases/tag/") {
        if tag.is_empty() {
            return Err("Release 地址缺少标签".into());
        }
        return Ok((GitHubRepo::parse(repo)?, Some(tag.into())));
    }
    Ok((
        GitHubRepo::parse(source.trim_end_matches("/releases"))?,
        None,
    ))
}
fn find_roots(
    base: &std::path::Path,
    current: &std::path::Path,
    roots: &mut Vec<String>,
) -> Result<()> {
    if current.join("manifest.json").is_file() {
        roots.push(
            current
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/"),
        );
    }
    for entry in fs::read_dir(current).map_err(crate::io_error)? {
        let path = entry.map_err(crate::io_error)?.path();
        if path.is_dir() {
            find_roots(base, &path, roots)?;
        }
    }
    roots.sort();
    Ok(())
}

fn declared_version(name: &str) -> Option<(usize, usize, &str)> {
    let lower = name.to_ascii_lowercase();
    let start = lower.find("extension-v").map(|p| p + "extension-v".len())?;
    let tail = &name[start..];
    let end = start
        + tail
            .bytes()
            .take_while(|b| b.is_ascii_digit() || *b == b'.')
            .count();
    let end = if name[..end].ends_with('.') {
        end - 1
    } else {
        end
    };
    let version = &name[start..end];
    if version.is_empty() || !version.bytes().any(|b| b.is_ascii_digit()) {
        None
    } else {
        Some((start, end, version))
    }
}
fn selector(name: &str) -> String {
    if let Some((start, end, _)) = declared_version(name) {
        format!("{}{{version}}{}", &name[..start], &name[end..])
    } else {
        name.to_string()
    }
}
