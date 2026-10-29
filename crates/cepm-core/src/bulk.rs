//! Profile-scoped orchestration; each installation keeps its own transaction and result.
use crate::{
    Installation, Manager, RemoteOrigin, Result, Target, UpdateKind, UpdatePlan, remote::Gh,
};

pub struct CheckedUpdate {
    id: String,
    name: String,
    result: Result<UpdatePlan>,
}
impl CheckedUpdate {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn plan(&self) -> Option<&UpdatePlan> {
        self.result.as_ref().ok()
    }
    pub fn error(&self) -> Option<&str> {
        self.result.as_ref().err().map(String::as_str)
    }
}
#[derive(Debug)]
pub enum BulkStatus {
    Deployed(Box<Installation>),
    Skipped(String),
    Failed(String),
}
#[derive(Debug)]
pub struct BulkResult {
    pub id: String,
    pub name: String,
    pub status: BulkStatus,
}
impl Manager {
    /// Fetch each source independently and freeze its candidate. Never checks another Profile.
    pub fn check_target_updates(
        &self,
        gh: &impl Gh,
        target: &Target,
    ) -> Result<Vec<CheckedUpdate>> {
        self.list()?
            .into_iter()
            .filter(|r| &r.target == target)
            .map(|record| {
                let result = match record.origin {
                    Some(RemoteOrigin::GitHubRelease { .. }) => self.check_release(gh, &record.id),
                    Some(RemoteOrigin::GitHubDirectory { .. }) => {
                        self.check_repository(gh, &record.id)
                    }
                    None => self.check_local(&record.id),
                };
                Ok(CheckedUpdate {
                    id: record.id,
                    name: record.name,
                    result,
                })
            })
            .collect()
    }
    /// Only ordinary upgrades are eligible. A conflict/error never rolls back another item.
    pub fn apply_target_upgrades(
        &mut self,
        target: &Target,
        checked: Vec<CheckedUpdate>,
    ) -> Vec<BulkResult> {
        checked
            .into_iter()
            .map(|check| {
                let result = self.record(&check.id).and_then(|record| {
                    if &record.target != target {
                        return Err("目标配置已变化，请重新检查当前配置".into());
                    }
                    match check.result {
                        Err(error) => Err(error),
                        Ok(plan) if plan.kind() == UpdateKind::Upgrade => self
                            .apply_update(plan, UpdateKind::Upgrade)
                            .map(Box::new)
                            .map(BulkStatus::Deployed),
                        Ok(plan) => Ok(BulkStatus::Skipped(
                            match plan.kind() {
                                UpdateKind::Downgrade => "降级需要单独选择版本并确认",
                                UpdateKind::Redeploy => "同版本重新部署需要单独确认",
                                _ => "来源与当前安装一致",
                            }
                            .into(),
                        )),
                    }
                });
                BulkResult {
                    id: check.id,
                    name: check.name,
                    status: result.unwrap_or_else(BulkStatus::Failed),
                }
            })
            .collect()
    }
}
