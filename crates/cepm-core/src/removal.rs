//! Browser removal is confirmed before any owned files are removed. Local cleanup is resumable.
use crate::{Installation, Manager, Result, Target, io_error};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualRemovalConfirmation {
    NotRemoved,
    Removed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemovalPhase {
    BrowserRemoved,
    FilesRemoved,
    BackupsRemoved,
    HistoryRecorded,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum RemovalMethod {
    UserConfirmed,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovalState {
    pub(crate) operation_id: String,
    pub(crate) method: RemovalMethod,
    phase: RemovalPhase,
    error: Option<String>,
}
impl RemovalState {
    pub fn phase(&self) -> RemovalPhase {
        self.phase
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
impl Manager {
    pub fn confirm_manual_removal(
        &mut self,
        id: &str,
        target: &Target,
        generation: u64,
        confirmation: ManualRemovalConfirmation,
    ) -> Result<()> {
        self.confirm_manual_removal_with_progress(id, target, generation, confirmation, |_| {})
    }
    pub fn confirm_manual_removal_with_progress(
        &mut self,
        id: &str,
        target: &Target,
        generation: u64,
        confirmation: ManualRemovalConfirmation,
        progress: impl FnMut(RemovalPhase),
    ) -> Result<()> {
        if confirmation != ManualRemovalConfirmation::Removed {
            return Err("请先在目标浏览器移除扩展，再明确确认；尚未清理任何文件".into());
        }
        self.begin_removal(id, target, generation, RemovalMethod::UserConfirmed)?;
        self.retry_removal_with_progress(id, target, generation, progress)
    }
    /// Only a browser adapter holding validated removal evidence may add a non-manual method.
    pub(crate) fn begin_removal(
        &mut self,
        id: &str,
        target: &Target,
        generation: u64,
        method: RemovalMethod,
    ) -> Result<()> {
        let mut record = self.removal_record(id, target, generation)?;
        if record.removal.is_none() {
            record.removal = Some(RemovalState {
                operation_id: uuid::Uuid::new_v4().to_string(),
                method,
                phase: RemovalPhase::BrowserRemoved,
                error: None,
            });
            record.historical_version = record
                .effective_version
                .take()
                .or(record.historical_version);
            record.verification = crate::Verification::Pending;
            record.last_operation = "已确认浏览器移除，等待清理受管文件".into();
            self.save(&record)?;
        }
        self.recovery_errors.remove(id);
        Ok(())
    }
    pub fn retry_removal(&mut self, id: &str, target: &Target, generation: u64) -> Result<()> {
        self.retry_removal_with_progress(id, target, generation, |_| {})
    }
    pub fn retry_removal_with_progress(
        &mut self,
        id: &str,
        target: &Target,
        generation: u64,
        mut progress: impl FnMut(RemovalPhase),
    ) -> Result<()> {
        let mut record = self.removal_record(id, target, generation)?;
        if record.removal.is_none() {
            return Err("尚未确认浏览器已移除，不能清理受管文件".into());
        }
        let result: Result<()> = (|| {
            loop {
                let phase = record.removal.as_ref().unwrap().phase;
                progress(phase);
                self.validate_storage_directories()?;
                let next = match phase {
                    RemovalPhase::BrowserRemoved => {
                        remove_owned_directory(&record.install_dir)?;
                        RemovalPhase::FilesRemoved
                    }
                    RemovalPhase::FilesRemoved => {
                        remove_owned_directory(&self.transaction_path(id))?;
                        RemovalPhase::BackupsRemoved
                    }
                    RemovalPhase::BackupsRemoved => {
                        self.record_removal_cleanup(&record)?;
                        RemovalPhase::HistoryRecorded
                    }
                    RemovalPhase::HistoryRecorded => {
                        for path in [&record.install_dir, &self.transaction_path(id)] {
                            match fs::symlink_metadata(path) {
                                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                                _ => {
                                    return Err(
                                        "已清理目录再次出现或不可读取，请保留文件并检查".into()
                                    );
                                }
                            }
                        }
                        fs::remove_file(self.root.join("records").join(format!("{id}.json")))
                            .map_err(io_error)?;
                        self.recovery_errors.remove(id);
                        return Ok(());
                    }
                };
                let state = record.removal.as_mut().unwrap();
                state.phase = next;
                state.error = None;
                record.last_operation = "浏览器移除已确认，正在清理受管文件与记录".into();
                self.save(&record)?;
            }
        })();
        if let Err(error) = &result {
            self.validate_storage_directories()?;
            record.removal.as_mut().unwrap().error = Some(error.clone());
            record.last_operation = format!("浏览器移除已确认，清理尚未完成：{error}。可重试清理");
            if let Err(save_error) = self.save(&record) {
                return Err(format!(
                    "{error}；无法保存清理错误：{save_error}。原清理阶段仍保留，可重开后重试"
                ));
            }
        }
        result
    }
    pub(super) fn validate_storage_directories(&self) -> Result<()> {
        for path in [
            self.root.clone(),
            self.root.join("installations"),
            self.root.join("records"),
            self.root.join("transactions"),
        ] {
            let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
            if !metadata.file_type().is_dir() || path.canonicalize().map_err(io_error)? != path {
                return Err("受管数据目录存在链接或位置变化，已停止文件操作".into());
            }
        }
        Ok(())
    }
    fn removal_record(&self, id: &str, target: &Target, generation: u64) -> Result<Installation> {
        self.validate_storage_directories()?;
        let record = self
            .read_records()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or("找不到待卸载安装")?;
        if record.target != *target || record.generation != generation {
            return Err("卸载目标或部署已变化，请重新打开详情".into());
        }
        Ok(record)
    }
}
fn remove_owned_directory(path: &std::path::Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_dir() => fs::remove_dir_all(path).map_err(io_error),
        Ok(_) => Err("受管目录存在链接或非目录冲突，未清理，请保留文件并检查".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}
