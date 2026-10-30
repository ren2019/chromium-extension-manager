//! Bounded, sanitized outcomes for the user's operation history, separate from recovery journals.
use crate::{Manager, RemoteOrigin, Result, Target, UpdatePlan, remote::Gh};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationStage {
    Source,
    Check,
    Deploy,
    Confirm,
    Recover,
    Bind,
    Uninstall,
}
impl OperationStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "获取与校验",
            Self::Check => "检查更新",
            Self::Deploy => "部署文件",
            Self::Confirm => "核对生效",
            Self::Recover => "恢复文件",
            Self::Bind => "定位配置",
            Self::Uninstall => "卸载清理",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationStatus {
    Succeeded,
    Failed,
    Skipped,
    PendingCleanup,
}
impl OperationStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Succeeded => "已完成",
            Self::Failed => "失败",
            Self::Skipped => "未执行",
            Self::PendingCleanup => "清理未完成",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRecord {
    pub target: Target,
    pub installation_id: Option<String>,
    #[serde(default)]
    pub installation_name: Option<String>,
    pub at: u64,
    pub stage: OperationStage,
    pub status: OperationStatus,
    pub reason: String,
    pub next_step: String,
    #[serde(default)]
    pub operation_id: Option<String>,
}
impl Manager {
    /// One source dispatch for single and bulk checks; failure never changes installation state.
    pub fn check_update(&self, gh: &impl Gh, id: &str) -> Result<UpdatePlan> {
        let record = self.record(id)?;
        let result = match record.origin {
            Some(RemoteOrigin::GitHubRelease { .. }) => self.check_release(gh, id),
            Some(RemoteOrigin::GitHubDirectory { .. }) => self.check_repository(gh, id),
            None => self.check_local(id),
        };
        self.record_operation(
            &record.target,
            Some(id),
            OperationStage::Check,
            result.as_ref().map(|_| ()).map_err(String::as_str),
        )?;
        result
    }
    /// Record only a controlled diagnosis, never raw CLI output, file paths or source input.
    pub fn record_operation(
        &self,
        target: &Target,
        id: Option<&str>,
        stage: OperationStage,
        result: std::result::Result<(), &str>,
    ) -> Result<()> {
        let (status, reason, next_step) = match result {
            Ok(()) => (
                OperationStatus::Succeeded,
                "操作已完成",
                match stage {
                    OperationStage::Confirm => "浏览器生效版本已确认",
                    OperationStage::Check => "可在扩展详情中查看检查结果",
                    _ => "磁盘变更仍需在目标浏览器重载并核对",
                },
            ),
            Err(error) => {
                let (reason, next_step) = diagnosis(error);
                (OperationStatus::Failed, reason, next_step)
            }
        };
        self.save_operation(target, id, stage, status, reason, next_step)
    }
    pub(crate) fn record_skipped(&self, target: &Target, id: &str) -> Result<()> {
        self.save_operation(
            target,
            Some(id),
            OperationStage::Deploy,
            OperationStatus::Skipped,
            "没有普通升级可执行",
            "如需降级或同版本重新部署，请在详情中单独选择并确认",
        )
    }
    fn save_operation(
        &self,
        target: &Target,
        id: Option<&str>,
        stage: OperationStage,
        status: OperationStatus,
        reason: &str,
        next_step: &str,
    ) -> Result<()> {
        let installation_name = if let Some(id) = id {
            let record = self.record(id)?;
            if record.target != *target {
                return Err("操作记录目标不一致，请重新选择配置".into());
            }
            Some(record.name)
        } else {
            None
        };
        let mut history = self.read_operations()?;
        history.push(OperationRecord {
            target: target.clone(),
            installation_id: id.map(str::to_owned),
            installation_name,
            at: crate::updates::now(),
            stage,
            status,
            reason: reason.into(),
            next_step: next_step.into(),
            operation_id: None,
        });
        self.save_history(history)
    }
    pub(crate) fn record_removal_cleanup(&self, record: &crate::Installation) -> Result<()> {
        let state = record.removal.as_ref().ok_or("浏览器移除尚未确认")?;
        let mut history = self.read_operations()?;
        for event in &mut history {
            if event.installation_id.as_deref() == Some(&record.id)
                && event.installation_name.is_none()
            {
                event.installation_name = Some(record.name.clone());
            }
        }
        if history
            .iter()
            .any(|event| event.operation_id.as_deref() == Some(&state.operation_id))
        {
            return self.save_history(history);
        }
        history.push(OperationRecord {
            target: record.target.clone(),
            installation_id: Some(record.id.clone()),
            installation_name: Some(record.name.clone()),
            at: crate::updates::now(),
            stage: OperationStage::Uninstall,
            status: OperationStatus::PendingCleanup,
            reason: match state.method {
                crate::removal::RemovalMethod::UserConfirmed => {
                    "用户确认浏览器已移除；受管文件已清理"
                }
                crate::removal::RemovalMethod::Automatic => {
                    "浏览器自动卸载并回读确认移除；受管文件已清理"
                }
            }
            .into(),
            next_step: "管理记录移除失败时可重试清理，无需重复浏览器操作".into(),
            operation_id: Some(state.operation_id.clone()),
        });
        self.save_history(history)
    }
    fn save_history(&self, mut history: Vec<OperationRecord>) -> Result<()> {
        if history.len() > 100 {
            history.drain(..history.len() - 100);
        }
        let mut file = tempfile::NamedTempFile::new_in(&self.root).map_err(crate::io_error)?;
        file.write_all(&serde_json::to_vec_pretty(&history).map_err(|e| e.to_string())?)
            .map_err(crate::io_error)?;
        file.as_file().sync_all().map_err(crate::io_error)?;
        file.persist(self.root.join("operations.json"))
            .map_err(|_| "无法保存操作记录，请检查数据目录权限和可用空间".to_string())?;
        Ok(())
    }
    fn read_operations(&self) -> Result<Vec<OperationRecord>> {
        match fs::read(self.root.join("operations.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "操作记录损坏，请保留数据目录并检查".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(_) => Err("无法读取操作记录，请检查数据目录权限".into()),
        }
    }
    pub fn operation_history(&self, target: &Target) -> Result<Vec<OperationRecord>> {
        Ok(self
            .read_operations()?
            .into_iter()
            .rev()
            .filter(|r| &r.target == target)
            .map(|mut event| {
                // Deleting the final record is the commit point. Project completion
                // from absence so a crash cannot lose an audit update after deletion.
                if event.stage == OperationStage::Uninstall
                    && matches!(event.status, OperationStatus::PendingCleanup | OperationStatus::Succeeded)
                {
                    let complete = event.installation_id.as_ref().is_some_and(|id| {
                        uuid::Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == *id)
                            && self.validate_storage_directories().is_ok()
                            && [self.root.join("records").join(format!("{id}.json")),
                                self.root.join("installations").join(id), self.transaction_path(id)]
                                .iter().all(|path| matches!(fs::symlink_metadata(path), Err(error) if error.kind() == std::io::ErrorKind::NotFound))
                    });
                    event.status = if complete { OperationStatus::Succeeded } else { OperationStatus::PendingCleanup };
                    event.next_step = if complete { "本安装的受管文件、备份及管理记录已清理" } else { "清理尚未完成，可重试；无需重复浏览器移除" }.into();
                }
                event
            })
            .collect())
    }
}
fn diagnosis(error: &str) -> (&'static str, &'static str) {
    if error.contains("用户反馈浏览器加载失败") {
        (
            "用户反馈更新失败",
            "重新安装当前版本、选择其他版本或显式恢复旧文件",
        )
    } else if error.contains("找不到 gh") {
        (
            "GitHub CLI 未安装",
            "安装 gh 并完成 gh auth login；本地目录仍可用",
        )
    } else if error.contains("gh") || error.contains("权限") || error.contains("网络") {
        (
            "来源获取或访问失败",
            "检查网络、gh auth status 和来源访问权限后重试",
        )
    } else if error.contains("摘要") || error.contains("SHA") {
        ("产物摘要不一致", "停止使用此产物，核对作者发布的附件后重试")
    } else if error.contains("ZIP")
        || error.contains("归档")
        || error.contains("链接")
        || error.contains("穿越")
    {
        ("产物安全校验失败", "选择可信来源的完整普通文件产物后重试")
    } else if error.contains("清单")
        || error.contains("资源")
        || error.to_ascii_lowercase().contains("manifest")
        || error.contains("扩展版本应为")
        || error.contains("content_scripts")
        || error.contains("沙盒页面")
        || error.contains("默认语言")
    {
        (
            "扩展清单或必要资源无效",
            "选择完整的 Manifest V3 预构建产物后重试",
        )
    } else if error.contains("改动") || error.contains("冲突") || error.contains("修改") {
        ("固定安装目录存在改动", "保留手动修改，处理冲突后重新检查")
    } else if error.contains("过期") || error.contains("版本") || error.contains("ID") {
        (
            "版本或身份核对不一致",
            "重新打开详情，在当前目标配置重载后核对 ID 和版本",
        )
    } else if error.contains("配置") {
        (
            "目标浏览器配置不可用",
            "核对浏览器应用与 Profile 原位置后重试",
        )
    } else {
        (
            "文件或操作不可用",
            "检查来源、文件权限和可用空间后重试；保留现有安装文件",
        )
    }
}
