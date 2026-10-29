use super::*;
use sha2::{Digest, Sha256};
use std::{
    cmp::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateKind {
    Upgrade,
    Downgrade,
    Redeploy,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStage {
    Prepared,
    BackedUp,
    FilesDeployed,
    RecordSaved,
    AwaitingConfirmation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Phase {
    Prepared,
    Deployed,
    Restoring,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    before: Installation,
    after: Installation,
    phase: Phase,
}

pub struct UpdatePlan {
    id: String,
    generation: u64,
    preview: Preview,
    kind: UpdateKind,
}
impl UpdatePlan {
    pub fn origin(&self) -> Option<&RemoteOrigin> {
        self.preview.origin()
    }
    pub fn kind(&self) -> UpdateKind {
        self.kind
    }
    pub fn version(&self) -> &str {
        self.preview.version()
    }
    pub fn name(&self) -> &str {
        self.preview.name()
    }
    pub fn source(&self) -> &Path {
        self.preview.source()
    }
}

impl Manager {
    pub fn check_local(&self, id: &str) -> Result<UpdatePlan> {
        let record = self.record(id)?;
        if record.origin.is_some() {
            return Err("此安装来自远程来源，请使用对应的远程检查入口".into());
        }
        let preview = self
            .preview_local(&record.source)
            .map_err(|e| format!("检查来源失败：{e}。请恢复来源目录或选择其他完整产物"))?;
        self.prepare_update(id, preview)
    }

    pub fn prepare_update(&self, id: &str, preview: Preview) -> Result<UpdatePlan> {
        let record = self.record(id)?;
        self.ensure_unmodified(&record)?;
        let kind =
            match version_parts(preview.version()).cmp(&version_parts(&record.installed_version)) {
                Ordering::Greater => UpdateKind::Upgrade,
                Ordering::Less => UpdateKind::Downgrade,
                Ordering::Equal
                    if directory_digest(preview.snapshot.path())? != record.content_digest =>
                {
                    UpdateKind::Redeploy
                }
                Ordering::Equal => UpdateKind::Unchanged,
            };
        Ok(UpdatePlan {
            id: record.id,
            generation: record.generation,
            preview,
            kind,
        })
    }

    pub fn apply_update(&mut self, plan: UpdatePlan, choice: UpdateKind) -> Result<Installation> {
        self.apply_update_with_progress(plan, choice, |_| {})
    }

    /// Reports durable operation boundaries. Work must stay on a background thread.
    pub fn apply_update_with_progress(
        &mut self,
        plan: UpdatePlan,
        choice: UpdateKind,
        mut progress: impl FnMut(UpdateStage),
    ) -> Result<Installation> {
        let before = self.record(&plan.id)?;
        if before.generation != plan.generation {
            return Err("更新预览已过期，请重新检查".into());
        }
        if choice != plan.kind || choice == UpdateKind::Unchanged {
            return Err("请明确选择此版本的升级、降级或同版本重新部署".into());
        }
        self.ensure_unmodified(&before)?;
        let transaction = self.transaction_path(&before.id);
        if transaction.exists() {
            return Err("上次更新仍等待核对。请先确认浏览器版本，或报告加载失败恢复旧文件".into());
        }
        let mut after = before.clone();
        after.name = plan.preview.name;
        after.source = plan.preview.source;
        after.origin = plan.preview.origin;
        after.installed_version = plan.preview.version;
        after.content_digest = directory_digest(plan.preview.snapshot.path())?;
        after.generation = before
            .generation
            .checked_add(1)
            .ok_or("安装代数溢出，请检查记录")?;
        after.historical_version = after.effective_version.take().or(after.historical_version);
        after.verification = Verification::Pending;
        after.updated_at = now();
        after.last_operation = "文件已更新，等待浏览器重载与核对".into();
        let mut journal = Journal {
            before,
            after: after.clone(),
            phase: Phase::Prepared,
        };
        fs::create_dir(&transaction).map_err(io_error)?;
        if let Err(error) = self.save_journal(&journal) {
            fs::remove_dir(&transaction).ok();
            return Err(format!("准备更新记录失败：{error}。旧文件未改动"));
        }
        progress(UpdateStage::Prepared);
        let result: Result<Installation> = (|| {
            fs::rename(&journal.before.install_dir, transaction.join("old")).map_err(io_error)?;
            progress(UpdateStage::BackedUp);
            fs::rename(plan.preview.snapshot.path(), &after.install_dir).map_err(io_error)?;
            progress(UpdateStage::FilesDeployed);
            self.save(&after)?;
            progress(UpdateStage::RecordSaved);
            journal.phase = Phase::Deployed;
            self.save_journal(&journal)?;
            progress(UpdateStage::AwaitingConfirmation);
            Ok(after)
        })();
        match result {
            Ok(record) => Ok(record),
            Err(error) => {
                journal.phase = Phase::Restoring;
                match self
                    .save_journal(&journal)
                    .and_then(|_| self.restore_transaction(&journal))
                {
                    Ok(()) => Err(format!(
                        "部署失败：{error}。已恢复旧文件，请在浏览器重新核对后重试"
                    )),
                    Err(recovery) => Err(format!(
                        "部署失败：{error}；恢复需要处理：{recovery}。请保留数据目录，修复权限或磁盘问题后重新打开应用"
                    )),
                }
            }
        }
    }

    pub fn report_load_failure(&mut self, id: &str, generation: u64) -> Result<Installation> {
        let current = self.record(id)?;
        if current.generation != generation {
            return Err("加载失败报告已过期，请重新检查当前安装".into());
        }
        self.ensure_unmodified(&current)?;
        let mut journal = self.read_journal(&self.transaction_path(id))?;
        if journal.phase != Phase::Deployed || journal.after.generation != generation {
            return Err("没有与当前部署匹配的备份，请保留文件并检查安装记录".into());
        }
        journal.phase = Phase::Restoring;
        self.save_journal(&journal)?;
        self.restore_transaction(&journal)
            .map_err(|e| format!("恢复需要处理：{e}。请保留数据目录并修复权限后重开应用"))?;
        self.record(id)
    }

    pub(super) fn recover_updates(&mut self) -> Result<()> {
        for transaction in fs::read_dir(self.root.join("transactions")).map_err(io_error)? {
            let transaction = transaction.map_err(io_error)?.path();
            let id = transaction
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("更新日志目录名称无效")?
                .to_owned();
            if !uuid::Uuid::parse_str(&id).is_ok_and(|parsed| parsed.to_string() == id) {
                return Err("更新日志目录身份无效，请保留数据目录并检查".into());
            }
            let result = (|| {
                let mut journal = self.read_journal(&transaction)?;
                if journal.phase == Phase::Deployed {
                    let current = self
                        .read_records()?
                        .into_iter()
                        .find(|r| r.id == journal.after.id)
                        .ok_or("更新记录缺失，请保留数据目录并检查")?;
                    if current.generation != journal.after.generation
                        || current.content_digest != journal.after.content_digest
                    {
                        return Err("更新记录与部署日志不一致，请保留数据目录并检查".into());
                    }
                    self.ensure_unmodified(&current)?;
                    if current.verification == Verification::UserConfirmed {
                        fs::remove_dir_all(&transaction).map_err(io_error)?;
                    }
                } else {
                    journal.phase = Phase::Restoring;
                    self.save_journal(&journal)?;
                    self.restore_transaction(&journal)?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                self.recovery_errors.insert(id, error);
            }
        }
        Ok(())
    }

    pub(super) fn mark_recovery_state(&self, record: &mut Installation) {
        let transaction = self.transaction_path(&record.id);
        let error = self.recovery_errors.get(&record.id).cloned().or_else(|| {
            if !transaction.exists() {
                return None;
            }
            match self.read_journal(&transaction) {
                Ok(journal) if journal.phase == Phase::Deployed => None,
                Ok(_) => Some("存在未完成的文件替换或恢复".into()),
                Err(error) => Some(error),
            }
        });
        if let Some(error) = error {
            record.recovery_required = true;
            record.verification = Verification::Pending;
            record.effective_version = None;
            record.last_operation =
                format!("恢复需要处理：{error}。请保留数据目录，修复权限或磁盘问题后重开应用");
        }
    }

    fn restore_transaction(&self, journal: &Journal) -> Result<()> {
        let transaction = self.transaction_path(&journal.before.id);
        let installed = &journal.before.install_dir;
        if installed.exists() {
            let digest = directory_digest(installed)?;
            if digest != journal.before.content_digest {
                if digest != journal.after.content_digest {
                    return Err("固定目录有无法识别的改动，恢复已暂停，未覆盖文件".into());
                }
                if directory_digest(&transaction.join("old"))? != journal.before.content_digest {
                    return Err("旧版备份损坏，恢复已暂停，未替换当前文件".into());
                }
                fs::rename(installed, transaction.join("failed")).map_err(io_error)?;
            }
        }
        if !installed.exists() {
            if directory_digest(&transaction.join("old"))? != journal.before.content_digest {
                return Err("旧版备份损坏，恢复已暂停，请保留数据目录".into());
            }
            fs::rename(transaction.join("old"), installed).map_err(io_error)?;
        }
        let mut restored = journal.before.clone();
        restored.generation = journal
            .after
            .generation
            .checked_add(1)
            .ok_or("安装代数溢出")?;
        restored.historical_version = restored
            .effective_version
            .take()
            .or(restored.historical_version);
        restored.verification = Verification::Pending;
        restored.updated_at = now();
        restored.last_operation = "已恢复旧文件，等待浏览器重载与核对；不撤销扩展数据迁移".into();
        self.save(&restored)?;
        fs::remove_dir_all(transaction).map_err(io_error)?;
        Ok(())
    }

    pub(super) fn transaction_path(&self, id: &str) -> PathBuf {
        self.root.join("transactions").join(id)
    }

    fn save_journal(&self, journal: &Journal) -> Result<()> {
        let path = self.transaction_path(&journal.before.id);
        let mut file = NamedTempFile::new_in(&path).map_err(io_error)?;
        file.write_all(&serde_json::to_vec(journal).map_err(|e| e.to_string())?)
            .map_err(io_error)?;
        file.as_file().sync_all().map_err(io_error)?;
        file.persist(path.join("journal.json"))
            .map_err(|e| io_error(e.error))?;
        Ok(())
    }

    fn read_journal(&self, path: &Path) -> Result<Journal> {
        if fs::symlink_metadata(path)
            .map_err(io_error)?
            .file_type()
            .is_symlink()
        {
            return Err("更新日志目录不能是链接，请保留数据目录并检查".into());
        }
        let journal: Journal =
            serde_json::from_slice(&fs::read(path.join("journal.json")).map_err(io_error)?)
                .map_err(|e| format!("更新日志损坏，请保留数据目录并检查：{e}"))?;
        let id = &journal.before.id;
        if !uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == *id)
            || path != self.transaction_path(id)
            || journal.after.id != *id
            || journal.before.install_dir != self.root.join("installations").join(id)
            || journal.after.install_dir != journal.before.install_dir
            || journal.after.generation
                != journal
                    .before
                    .generation
                    .checked_add(1)
                    .ok_or("无效安装代数")?
            || journal.before.content_digest.is_empty()
        {
            return Err("更新日志身份或路径无效，请保留数据目录并检查".into());
        }
        Ok(journal)
    }

    pub(super) fn ensure_unmodified(&self, record: &Installation) -> Result<()> {
        if record.content_digest.is_empty() {
            return Err("旧安装记录缺少文件基线，请保留旧目录并重新安装后再更新或核对".into());
        }
        if directory_digest(&record.install_dir)? != record.content_digest {
            return Err(
                "固定安装目录存在手动改动冲突，已暂停。请先另存改动并恢复原文件后重试".into(),
            );
        }
        Ok(())
    }

    pub(super) fn record(&self, id: &str) -> Result<Installation> {
        let record = self
            .list()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or("找不到该安装记录，请重新选择")?;
        if record.recovery_required {
            return Err(record.last_operation);
        }
        Ok(record)
    }
}

fn version_parts(version: &str) -> [u16; 4] {
    let mut result = [0; 4];
    for (index, part) in version.split('.').take(4).enumerate() {
        result[index] = part.parse().unwrap_or(0);
    }
    result
}

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(super) fn directory_digest(root: &Path) -> Result<String> {
    if !fs::symlink_metadata(root)
        .map_err(io_error)?
        .file_type()
        .is_dir()
    {
        return Err("目录存在链接或非目录冲突，请保留改动并检查".into());
    }
    fn walk(root: &Path, relative: &Path, hash: &mut Sha256) -> Result<()> {
        let mut entries = fs::read_dir(root.join(relative))
            .map_err(io_error)?
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(io_error)?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let relative = relative.join(entry.file_name());
            let kind = entry.file_type().map_err(io_error)?;
            let path = relative.to_str().ok_or("文件名必须为有效 Unicode")?;
            hash.update((path.len() as u64).to_le_bytes());
            hash.update(path.as_bytes());
            if kind.is_dir() {
                hash.update([0]);
                walk(root, &relative, hash)?;
            } else if kind.is_file() {
                hash.update([1]);
                let bytes = fs::read(entry.path()).map_err(io_error)?;
                hash.update((bytes.len() as u64).to_le_bytes());
                hash.update(bytes);
            } else {
                return Err("安装目录包含链接或特殊文件，请保留改动并检查".into());
            }
        }
        Ok(())
    }
    let mut hash = Sha256::new();
    walk(root, Path::new(""), &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}
