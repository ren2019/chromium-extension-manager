//! Optional local display metadata. Missing or unsafe artwork never blocks installation.
use crate::{Manager, RemoteOrigin, Result};
use std::{fs, io::Cursor, path::Component};

#[derive(Default, Clone)]
pub struct Presentation {
    pub description: Option<String>,
    /// Decoded, bounded local artwork, normalized to PNG for the desktop renderer.
    pub icon_png: Option<Vec<u8>>,
}
impl Manager {
    pub fn presentation(&self, id: &str) -> Result<Presentation> {
        let record = self.record(id)?;
        let read = || -> Option<Presentation> {
            if record.install_dir.canonicalize().ok().as_ref() != Some(&record.install_dir) {
                return None;
            }
            let path = record.install_dir.join("manifest.json");
            if fs::symlink_metadata(&path).ok()?.file_type().is_symlink()
                || fs::metadata(&path).ok()?.len() > 1024 * 1024
            {
                return None;
            }
            let manifest: serde_json::Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
            let description = manifest["description"]
                .as_str()
                .filter(|s| !s.trim().is_empty() && !s.contains("__MSG_"))
                .map(str::to_owned);
            let icon_png = (|| {
                let mut icons: Vec<_> = manifest["icons"].as_object()?.iter().collect();
                icons.sort_by_key(|(size, _)| std::cmp::Reverse(size.parse::<u32>().unwrap_or(0)));
                for (_, value) in icons {
                    let Some(name) = value.as_str() else { continue };
                    let relative = std::path::Path::new(name);
                    if relative
                        .components()
                        .any(|p| !matches!(p, Component::Normal(_)))
                        || name.contains(['\\', ':'])
                    {
                        continue;
                    }
                    let path = record.install_dir.join(relative);
                    let Ok(canonical) = path.canonicalize() else {
                        continue;
                    };
                    if !canonical.starts_with(&record.install_dir) {
                        continue;
                    }
                    let Ok(info) = fs::metadata(&canonical) else {
                        continue;
                    };
                    if !info.is_file() || info.len() > 4 * 1024 * 1024 {
                        continue;
                    }
                    let Ok(bytes) = fs::read(&canonical) else {
                        continue;
                    };
                    let Ok(mut reader) =
                        image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()
                    else {
                        continue;
                    };
                    let mut limits = image::Limits::default();
                    limits.max_image_width = Some(2048);
                    limits.max_image_height = Some(2048);
                    limits.max_alloc = Some(32 * 1024 * 1024);
                    reader.limits(limits);
                    let Ok(image) = reader.decode() else { continue };
                    let mut png = Cursor::new(Vec::new());
                    if image.write_to(&mut png, image::ImageFormat::Png).is_ok() {
                        return Some(png.into_inner());
                    }
                }
                None
            })();
            Some(Presentation {
                description,
                icon_png,
            })
        };
        Ok(read().unwrap_or_default())
    }

    /// Only explicit source/artifact evidence joins installations across targets.
    pub fn related_installations(&self, id: &str) -> Result<Vec<crate::Installation>> {
        let current = self.record(id)?;
        Ok(self
            .list()?
            .into_iter()
            .filter(|other| {
                if other.id == current.id {
                    return true;
                }
                match (&current.origin, &other.origin) {
                    (None, None) => current.source == other.source,
                    (
                        Some(RemoteOrigin::GitHubDirectory {
                            repo: a,
                            subdirectory: x,
                            ..
                        }),
                        Some(RemoteOrigin::GitHubDirectory {
                            repo: b,
                            subdirectory: y,
                            ..
                        }),
                    ) => a == b && x == y,
                    (
                        Some(RemoteOrigin::GitHubRelease {
                            repo: a,
                            asset_name: x,
                            root: ar,
                            ..
                        }),
                        Some(RemoteOrigin::GitHubRelease {
                            repo: b,
                            asset_name: y,
                            root: br,
                            ..
                        }),
                    ) => a == b && !x.is_empty() && x == y && ar == br,
                    _ => false,
                }
            })
            .collect())
    }
}
