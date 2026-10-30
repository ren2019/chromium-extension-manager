mod keyboard;
use gpui_kit::base::FocusTrapElement;
mod visual;
use cepm_core::RemoteOrigin;
use cepm_core::bulk::{BulkResult, BulkStatus, CheckedUpdate};
use cepm_core::operations::{OperationRecord, OperationStage};
use cepm_core::profiles::{
    BrowserLocation, BrowserProfile, ProfileBook, ProfileCandidate, discover_profiles,
};
use cepm_core::remote::{CommandGh, ReleaseArtifact, ReleaseCatalog, ReleaseSelection};
use cepm_core::{Browser, Installation, Manager, Preview, UpdateKind, UpdatePlan, Verification};
use gpui_kit::component::{
    button::*,
    checkbox::Checkbox,
    input::{Input, InputState},
    *,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use visual::*;

const GREEN: u32 = 0x2563eb;
const MUTED: u32 = 0x6b6b73;
const BORDER: u32 = 0xd9d9e0;
#[derive(Clone, Copy, PartialEq)]
enum SourceKind {
    Local,
    Release,
    Repository,
}
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Library,
    Attention,
    History,
    About,
}
#[derive(Clone, Copy, PartialEq)]
enum Panel {
    None,
    Bind,
    Legacy,
    Add,
    Detail,
    Guide,
    Update,
    Restore,
    Bulk,
}
struct Desktop {
    keyboard: keyboard::Keyboard,
    manager: Option<Arc<Mutex<Manager>>>,
    profiles: Option<ProfileBook>,
    candidates: Vec<ProfileCandidate>,
    records: Vec<Installation>,
    operations: Vec<OperationRecord>,
    selected: Option<String>,
    preview: Option<Preview>,
    update_plan: Option<UpdatePlan>,
    bulk_checks: Vec<CheckedUpdate>,
    bulk_results: Vec<BulkResult>,
    check_outcomes: std::collections::HashMap<String, Result<Option<String>, String>>,
    page: Page,
    panel: Panel,
    add_step: u8,
    guide_record: Option<Installation>,
    path_copied: bool,
    source_kind: SourceKind,
    source_update_id: Option<String>,
    release_catalog: Option<ReleaseCatalog>,
    release_tag: Option<String>,
    release_asset: Option<u64>,
    release_artifact: Option<ReleaseArtifact>,
    remote_repo: Entity<InputState>,
    remote_reference: Entity<InputState>,
    remote_directory: Entity<InputState>,
    switcher: bool,
    cross_profiles: bool,
    artwork: std::collections::HashMap<String, Artwork>,
    show_technical: bool,
    show_versions: bool,
    checked: bool,
    bind_browser: Browser,
    bind_name: Entity<InputState>,
    bind_app: Entity<InputState>,
    bind_path: Entity<InputState>,
    status: String,
    error: bool,
    busy: bool,
}
enum PreparedRelease {
    Ready(Box<PreparedSource>),
    ChooseRoot(ReleaseArtifact),
}
enum PreparedSource {
    New(Preview),
    Update(UpdatePlan),
}
fn prepare_source(
    manager: &Manager,
    preview: Preview,
    update_id: Option<String>,
) -> cepm_core::Result<PreparedSource> {
    match update_id {
        Some(id) => manager
            .prepare_update(&id, preview)
            .map(PreparedSource::Update),
        None => Ok(PreparedSource::New(preview)),
    }
}
fn accept_source(view: &mut Desktop, prepared: PreparedSource) {
    view.status.clear();
    view.checked = false;
    match prepared {
        PreparedSource::New(preview) => {
            view.preview = Some(preview);
            view.add_step = 2;
        }
        PreparedSource::Update(plan) => {
            view.update_plan = Some(plan);
            view.panel = Panel::Update;
        }
    }
}
fn origin_description(origin: Option<&RemoteOrigin>) -> String {
    match origin {
        Some(RemoteOrigin::GitHubRelease {
            tag,
            asset_name,
            root,
            ..
        }) => format!(
            "Release {tag} · {asset_name} · {}",
            if root.is_empty() {
                "ZIP 根目录"
            } else {
                root
            }
        ),
        Some(RemoteOrigin::GitHubDirectory {
            reference,
            commit,
            subdirectory,
            ..
        }) => format!(
            "引用 {reference} · 提交 {} · {}",
            &commit[..commit.len().min(12)],
            if subdirectory.is_empty() {
                "仓库根目录"
            } else {
                subdirectory
            }
        ),
        None => "本地目录".into(),
    }
}
fn caption(text: impl Into<SharedString>) -> Div {
    div().text_sm().text_color(rgb(MUTED)).child(text.into())
}
fn heading(text: impl Into<SharedString>) -> Div {
    div()
        .text_xl()
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}
fn badge(text: impl Into<SharedString>, pending: bool) -> Div {
    div()
        .px_2()
        .py_1()
        .rounded_md()
        .text_xs()
        .bg(rgb(if pending { 0xfcf3e3 } else { 0xeff3ff }))
        .text_color(rgb(if pending { 0x94631c } else { GREEN }))
        .child(text.into())
}
fn input(window: &mut Window, cx: &mut Context<Desktop>, placeholder: &str) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
}
impl Desktop {
    fn new(root: cepm_core::Result<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut status = String::new();
        let mut records = Vec::new();
        let (manager, mut profiles) = match root {
            Ok(root) => {
                let manager = match Manager::open(&root) {
                    Ok(m) => {
                        match m.list() {
                            Ok(r) => records = r,
                            Err(e) => status = e,
                        };
                        Some(Arc::new(Mutex::new(m)))
                    }
                    Err(e) => {
                        status = e;
                        None
                    }
                };
                let profiles = match ProfileBook::open(&root) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        status = e;
                        None
                    }
                };
                (manager, profiles)
            }
            Err(e) => {
                status = e;
                (None, None)
            }
        };
        let candidates = match discover_profiles(&browser_locations()) {
            Ok(p) => p,
            Err(e) => {
                if status.is_empty() {
                    status = e
                };
                Vec::new()
            }
        };
        if let Some(book) = profiles.as_mut()
            && book.selected().is_none()
        {
            let preferred = book
                .profiles()
                .iter()
                .filter(|p| p.validate_location().is_ok())
                .min_by_key(|p| p.browser != Browser::Edge)
                .map(|p| p.id.clone());
            if let Some(id) = preferred {
                let _ = book.select(&id);
            } else if book.profiles().is_empty()
                && let Some(candidate) =
                    candidates.iter().min_by_key(|p| p.browser != Browser::Edge)
                && let Err(error) = book.bind(
                    candidate.browser.clone(),
                    &candidate.name,
                    &candidate.application,
                    &candidate.profile_path,
                )
            {
                status = error;
            }
        }
        let operations = profiles
            .as_ref()
            .and_then(|p| p.selected())
            .and_then(|p| {
                manager.as_ref().map(|m| {
                    m.lock()
                        .map_err(|_| "操作记录不可用".to_string())
                        .and_then(|m| m.operation_history(&p.target()))
                })
            })
            .map(|result| match result {
                Ok(history) => history,
                Err(error) => {
                    status = error;
                    Vec::new()
                }
            })
            .unwrap_or_default();
        let artwork = load_artwork(manager.as_ref(), &records);
        let error = !status.is_empty();
        Self {
            keyboard: keyboard::Keyboard::new(window, cx),
            manager,
            profiles,
            candidates,
            records,
            operations,
            selected: None,
            preview: None,
            update_plan: None,
            bulk_checks: Vec::new(),
            bulk_results: Vec::new(),
            check_outcomes: Default::default(),
            page: Page::Library,
            panel: Panel::None,
            add_step: 1,
            guide_record: None,
            path_copied: false,
            source_kind: SourceKind::Local,
            source_update_id: None,
            release_catalog: None,
            release_tag: None,
            release_asset: None,
            release_artifact: None,
            remote_repo: input(window, cx, "owner/repository 或 GitHub 地址"),
            remote_reference: input(window, cx, "分支、标签或完整提交，例如 main"),
            remote_directory: input(window, cx, "仓库内预构建扩展目录，例如 dist/chrome"),
            switcher: false,
            cross_profiles: false,
            artwork,
            show_technical: false,
            show_versions: false,
            checked: false,
            bind_browser: Browser::Edge,
            bind_name: input(window, cx, "例如：个人、工作"),
            bind_app: input(window, cx, "浏览器应用或可执行文件的完整路径"),
            bind_path: input(window, cx, "从浏览器 version 页复制 Profile Path"),
            status,
            error,
            busy: false,
        }
    }
    fn target(&self) -> Option<BrowserProfile> {
        self.profiles.as_ref().and_then(|p| p.selected()).cloned()
    }
    fn target_label(&self) -> String {
        self.target()
            .map(|p| p.label())
            .unwrap_or("选择浏览器配置".into())
    }
    fn target_ready(&self) -> bool {
        self.target().is_some_and(|p| p.validate_location().is_ok())
    }
    fn scoped(&self) -> Vec<Installation> {
        let target = self.target();
        self.records
            .iter()
            .filter(|r| target.as_ref().is_some_and(|p| r.target == p.target()))
            .cloned()
            .collect()
    }
    fn current_record(&self) -> Option<Installation> {
        self.scoped()
            .into_iter()
            .find(|r| Some(&r.id) == self.selected.as_ref())
    }
    fn message(&mut self, result: cepm_core::Result<()>, success: &str, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.status = success.into();
                self.error = false
            }
            Err(e) => {
                self.status = e;
                self.error = true
            }
        }
        cx.notify();
    }
    fn open_panel(&mut self, panel: Panel, _window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.panel = panel;
        self.guide_record = if panel == Panel::Guide {
            self.current_record()
        } else {
            None
        };
        self.path_copied = false;
        self.checked = false;
        self.show_technical = false;
        self.cross_profiles = false;
        self.show_versions = false;
        self.status.clear();
        self.error = false;
        cx.notify();
    }
    fn clear_target_work(&mut self) {
        self.selected = None;
        self.guide_record = None;
        self.path_copied = false;
        self.preview = None;
        self.update_plan = None;
        self.bulk_checks.clear();
        self.bulk_results.clear();
        self.source_update_id = None;
        self.release_catalog = None;
        self.release_tag = None;
        self.release_asset = None;
        self.release_artifact = None;
        self.operations.clear();
        self.panel = Panel::None;
        self.switcher = false;
        self.page = Page::Library;
        self.checked = false;
    }
    fn refresh_history(&mut self) -> cepm_core::Result<()> {
        if let (Some(manager), Some(profile)) = (&self.manager, self.target()) {
            self.operations = manager
                .lock()
                .map_err(|_| "操作记录不可用")?
                .operation_history(&profile.target())?;
        }
        Ok(())
    }
    fn select_profile(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let mut result = self
            .profiles
            .as_mut()
            .ok_or("配置偏好不可用".into())
            .and_then(|p| p.select(id));
        if result.is_ok() {
            self.clear_target_work();
            result = self.refresh_history();
        }
        self.message(result, "", cx);
    }
    fn pin(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let result = self
            .profiles
            .as_mut()
            .ok_or("配置偏好不可用".into())
            .and_then(|p| {
                let pinned = p.pinned_ids().iter().any(|p| p == id);
                p.set_pinned(id, !pinned)
            });
        self.message(result, "", cx);
    }
    fn bind_candidate(&mut self, candidate: ProfileCandidate, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let mut result = self
            .profiles
            .as_mut()
            .ok_or("配置偏好不可用".into())
            .and_then(|book| {
                let p = book.bind(
                    candidate.browser,
                    &candidate.name,
                    &candidate.application,
                    &candidate.profile_path,
                )?;
                book.select(&p.id)
            });
        if result.is_ok() {
            self.clear_target_work();
            result = self.refresh_history();
        }
        self.message(result, "已绑定具体浏览器配置。首次加载请核对目标窗口", cx);
    }
    fn bind_manual(&mut self, cx: &mut Context<Self>) {
        if !self.checked {
            self.message(
                Err("请先核对浏览器中的 Profile Path，再勾选确认".into()),
                "",
                cx,
            );
            return;
        }
        let name = self.bind_name.read(cx).value().trim().to_string();
        let app = application_binary(
            PathBuf::from(self.bind_app.read(cx).value().trim()),
            &self.bind_browser,
        );
        let path = PathBuf::from(self.bind_path.read(cx).value().trim());
        self.bind_candidate(
            ProfileCandidate {
                browser: self.bind_browser.clone(),
                name,
                application: app,
                profile_path: path,
            },
            cx,
        );
    }
    fn open_native(&mut self, cx: &mut Context<Self>) {
        let result = self
            .target()
            .ok_or("请先选择浏览器配置".into())
            .and_then(|p| p.management_request())
            .and_then(|r| r.launch());
        self.message(
            result,
            "已请求打开目标配置；若出现新标签页，请复制管理页地址并在该窗口粘贴",
            cx,
        );
    }
    fn copy_management_url(&mut self, cx: &mut Context<Self>) {
        if let Some(profile) = self.target() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                profile.management_url().to_owned(),
            ));
            self.message(
                Ok(()),
                &format!("已复制。请在 {} 窗口的地址栏粘贴并打开", profile.label()),
                cx,
            );
        }
    }
    fn work<T: Send + 'static>(
        &mut self,
        label: &str,
        stage: OperationStage,
        cx: &mut Context<Self>,
        operation: impl FnOnce(&mut Manager) -> cepm_core::Result<T> + Send + 'static,
        apply: impl FnOnce(&mut Self, T) + 'static,
    ) {
        let Some(manager) = self.manager.clone() else {
            return;
        };
        if self.busy {
            return;
        }
        let target = self.target().map(|p| p.target());
        let operation_target = target.clone();
        let installation_id = if self.panel == Panel::Add {
            self.source_update_id.clone()
        } else {
            self.selected.clone()
        };
        self.busy = true;
        self.switcher = false;
        self.error = false;
        self.status = label.into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut m = manager
                        .lock()
                        .map_err(|_| "操作异常，请重启后检查记录".to_string())?;
                    let mut result = operation(&mut m);
                    if let (Some(target), Err(error)) = (&operation_target, &result)
                        && !(stage == OperationStage::Check && installation_id.is_some())
                        && let Err(log_error) = m.record_operation(
                            target,
                            installation_id.as_deref(),
                            stage,
                            Err(error),
                        )
                    {
                        result = Err(format!("{error}；{log_error}"));
                    }
                    let history = operation_target
                        .as_ref()
                        .map(|t| m.operation_history(t))
                        .transpose()?;
                    let refreshed = if result.is_err() { m.list().ok() } else { None };
                    Ok::<_, String>((result, refreshed, history))
                })
                .await;
            this.update(cx, |v, cx| {
                if v.target().map(|p| p.target()) != target {
                    v.busy = false;
                    cx.notify();
                    return;
                }
                let result = match result {
                    Ok((result, refreshed, history)) => {
                        if let Some(history) = history {
                            v.operations = history;
                        }
                        if let Some(records) = refreshed {
                            v.records = records;
                        }
                        result
                    }
                    Err(error) => Err(error),
                };
                v.busy = false;
                match result {
                    Ok(value) => apply(v, value),
                    Err(e) => {
                        v.error = true;
                        v.status = e
                    }
                }
                v.artwork = load_artwork(v.manager.as_ref(), &v.records);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn choose_source(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.preview = None;
        self.status = "请选择完整的本地扩展目录".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let folder = rfd::AsyncFileDialog::new()
                .set_title("选择包含 manifest.json 的扩展目录")
                .pick_folder()
                .await;
            this.update(cx, |v, cx| {
                v.busy = false;
                if let Some(folder) = folder {
                    let path = folder.path().to_path_buf();
                    v.work(
                        "正在复制与校验扩展产物…",
                        OperationStage::Source,
                        cx,
                        move |m| m.preview_local(&path),
                        |v, p| {
                            v.preview = Some(p);
                            v.add_step = 2;
                            v.status.clear();
                        },
                    );
                } else {
                    v.status = "已取消目录选择".into();
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn choose_local_version(&mut self, cx: &mut Context<Self>) {
        let Some(record) = self.current_record() else {
            return;
        };
        self.busy = true;
        self.update_plan = None;
        self.checked = false;
        cx.spawn(async move |this, cx| {
            let folder = rfd::AsyncFileDialog::new()
                .set_title("选择要切换到的完整扩展产物")
                .pick_folder()
                .await;
            this.update(cx, |v, cx| {
                v.busy = false;
                if let Some(folder) = folder {
                    let path = folder.path().to_path_buf();
                    v.work(
                        "正在检查所选版本…",
                        OperationStage::Source,
                        cx,
                        move |m| {
                            let preview = m.preview_local(&path)?;
                            m.prepare_update(&record.id, preview)
                        },
                        |v, plan| {
                            v.update_plan = Some(plan);
                            v.panel = Panel::Update;
                            v.status.clear();
                        },
                    );
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn install(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.target() else { return };
        if let Err(e) = target.validate_location() {
            self.message(Err(e), "", cx);
            return;
        }
        let Some(preview) = self.preview.take() else {
            return;
        };
        self.checked = false;
        self.work(
            "正在准备固定安装目录…",
            OperationStage::Deploy,
            cx,
            move |m| {
                target.validate_location()?;
                let record = m.install_local(preview, target.target())?;
                Ok((record, m.list()?))
            },
            |v, (record, records)| {
                v.selected = Some(record.id);
                v.records = records;
                v.panel = Panel::Guide;
                v.guide_record = v.current_record();
                v.path_copied = false;
                v.show_technical = false;
                v.status.clear();
            },
        );
    }
    fn bind_legacy(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.checked {
            self.message(Err("请确认旧安装原本就在当前配置中，再绑定".into()), "", cx);
            return;
        }
        let Some(profile) = self.target() else { return };
        self.work(
            "正在定位旧安装…",
            OperationStage::Bind,
            cx,
            move |m| {
                m.bind_legacy_target(&id, &profile)?;
                m.list()
            },
            |v, records| {
                v.records = records;
                v.panel = Panel::None;
                v.status = "已定位旧安装，请重新核对浏览器 ID 和版本".into();
            },
        );
    }
    fn copy_installation_path(&mut self, cx: &mut Context<Self>) {
        self.path_copied = false;
        let record = if self.panel == Panel::Guide {
            self.guide_record.clone()
        } else {
            self.current_record()
        };
        let result = (|| {
            let record = record.ok_or("请重新选择安装")?;
            let target = self.target().ok_or("请重新选择配置")?.target();
            let manager = self.manager.as_ref().ok_or("安装记录不可用")?;
            let path = manager
                .lock()
                .map_err(|_| "安装记录不可用")?
                .installation_path(&record.id, &target, record.generation)?;
            cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
            if cx
                .read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref()
                != Some(path.as_str())
            {
                return Err("未能确认路径已复制，请在下方完整路径中选中并手动复制".into());
            }
            Ok(())
        })();
        self.path_copied = result.is_ok();
        self.message(result, "安装路径已复制", cx);
    }
    fn confirm(&mut self, cx: &mut Context<Self>) {
        if self.panel != Panel::Guide {
            return;
        }
        let Some(record) = self.guide_record.clone() else {
            return;
        };
        let Some(profile) = self.target() else {
            return;
        };
        if self
            .current_record()
            .is_none_or(|r| r.id != record.id || r.generation != record.generation)
        {
            self.message(Err("此加载说明已过期，请重新打开详情".into()), "", cx);
            return;
        }
        self.work(
            "正在保存用户确认…",
            OperationStage::Confirm,
            cx,
            move |m| {
                profile.validate_location()?;
                m.confirm_manual(
                    &record.id,
                    &profile.target(),
                    record.generation,
                    &record.installed_version,
                )?;
                m.list()
            },
            |v, records| {
                v.records = records;
                v.panel = Panel::Detail;
                v.guide_record = None;
                v.status = "已记录用户确认".into();
            },
        );
    }
    fn report_browser_failure(&mut self, cx: &mut Context<Self>) {
        let Some(record) = self.guide_record.clone() else {
            return;
        };
        let Some(profile) = self.target() else {
            return;
        };
        self.work(
            "正在记录更新结果…",
            OperationStage::Confirm,
            cx,
            move |m| {
                m.report_browser_failure(&record.id, &profile.target(), record.generation)?;
                m.record_operation(
                    &profile.target(),
                    Some(&record.id),
                    OperationStage::Confirm,
                    Err("用户反馈浏览器加载失败"),
                )?;
                m.list()
            },
            |v, records| {
                v.records = records;
                v.panel = Panel::Detail;
                v.guide_record = None;
                v.status = "已记录更新失败，当前文件与恢复备份保留".into();
            },
        );
    }
    fn reinstall(&mut self, cx: &mut Context<Self>) {
        let Some(record) = self.current_record() else {
            return;
        };
        let Some(profile) = self.target() else {
            return;
        };
        self.work(
            "正在重新获取已安装版本…",
            OperationStage::Source,
            cx,
            move |m| {
                m.prepare_reinstall(&CommandGh, &record.id, &profile.target(), record.generation)
            },
            |v, plan| {
                v.update_plan = Some(plan);
                v.checked = false;
                v.panel = Panel::Update;
                v.status = "将在原目录重新部署此版本，保留原有恢复备份".into();
            },
        );
    }
    fn check_update(&mut self, cx: &mut Context<Self>) {
        let Some(record) = self.current_record() else {
            return;
        };
        self.update_plan = None;
        self.checked = false;
        self.work(
            "正在读取来源并检查版本…",
            OperationStage::Check,
            cx,
            move |m| Ok((record.id.clone(), m.check_update(&CommandGh, &record.id))),
            |v, (id, result)| {
                let plan = match result {
                    Ok(plan) => plan,
                    Err(error) => {
                        v.check_outcomes.insert(id, Err(error.clone()));
                        v.error = true;
                        v.status = error;
                        return;
                    }
                };
                v.check_outcomes.insert(
                    id,
                    Ok((plan.kind() == UpdateKind::Upgrade).then(|| plan.version().to_owned())),
                );
                v.status = if plan.kind() == UpdateKind::Unchanged {
                    "没有发现新版本".into()
                } else {
                    "发现可用版本".into()
                };
                v.panel = if plan.kind() == UpdateKind::Unchanged {
                    Panel::None
                } else {
                    Panel::Update
                };
                v.update_plan = Some(plan);
            },
        );
    }
    fn apply_update(&mut self, cx: &mut Context<Self>) {
        let Some(plan) = self.update_plan.take() else {
            return;
        };
        if matches!(plan.kind(), UpdateKind::Downgrade | UpdateKind::Redeploy) && !self.checked {
            self.update_plan = Some(plan);
            self.message(Err("请明确确认降级或同版本重新部署".into()), "", cx);
            return;
        }
        let Some(profile) = self.target() else { return };
        self.work(
            "正在准备 → 备份旧文件 → 部署文件…",
            OperationStage::Deploy,
            cx,
            move |m| {
                profile.validate_location()?;
                let kind = plan.kind();
                let record = m.apply_update(plan, kind)?;
                m.record_operation(
                    &record.target,
                    Some(&record.id),
                    OperationStage::Deploy,
                    Ok(()),
                )?;
                m.list()
            },
            |v, records| {
                v.records = records;
                v.panel = Panel::Guide;
                v.guide_record = v.current_record();
                if let Some(id) = &v.selected {
                    v.check_outcomes.remove(id);
                }
                v.path_copied = false;
                v.show_technical = false;
                v.checked = false;
                v.status.clear();
            },
        );
    }
    fn restore(&mut self, cx: &mut Context<Self>) {
        if !self.checked {
            return;
        }
        let Some(record) = self.current_record() else {
            return;
        };
        self.work(
            "正在恢复上一份文件…",
            OperationStage::Recover,
            cx,
            move |m| {
                m.restore_previous_files(&record.id, &record.target, record.generation)?;
                m.record_operation(
                    &record.target,
                    Some(&record.id),
                    OperationStage::Recover,
                    Ok(()),
                )?;
                m.list()
            },
            |v, records| {
                v.records = records;
                v.panel = Panel::Guide;
                v.guide_record = v.current_record();
                v.checked = false;
                v.status.clear();
            },
        );
    }
    fn load_releases(&mut self, cx: &mut Context<Self>) {
        let source = self.remote_repo.read(cx).value().trim().to_owned();
        self.release_catalog = None;
        self.release_artifact = None;
        self.work(
            "正在通过 gh 获取发布版本…",
            OperationStage::Source,
            cx,
            move |m| m.list_releases(&CommandGh, &source),
            |v, catalog| {
                v.release_tag = catalog
                    .requested_tag
                    .clone()
                    .or(catalog.default_tag.clone());
                v.release_asset = v
                    .release_tag
                    .as_deref()
                    .and_then(|tag| catalog.preselected_asset(tag));
                v.release_catalog = Some(catalog);
                v.status.clear();
            },
        );
    }
    fn fetch_release(&mut self, cx: &mut Context<Self>) {
        let source = self.remote_repo.read(cx).value().trim().to_owned();
        let selection = ReleaseSelection {
            tag: self.release_tag.clone(),
            asset_id: self.release_asset,
        };
        let update_id = self.source_update_id.clone();
        self.work(
            "正在下载并检查扩展…",
            OperationStage::Source,
            cx,
            move |m| {
                let artifact = m.preview_release(&CommandGh, &source, &selection)?;
                if artifact.roots().len() == 1 {
                    let preview = artifact.select_root(None)?;
                    Ok(PreparedRelease::Ready(Box::new(prepare_source(
                        m, preview, update_id,
                    )?)))
                } else {
                    Ok(PreparedRelease::ChooseRoot(artifact))
                }
            },
            |v, result| match result {
                PreparedRelease::Ready(prepared) => accept_source(v, *prepared),
                PreparedRelease::ChooseRoot(artifact) => {
                    v.release_artifact = Some(artifact);
                    v.status = "发现多个扩展目录，请选择要安装的扩展".into();
                }
            },
        );
    }
    fn select_release_root(&mut self, root: String, cx: &mut Context<Self>) {
        let Some(artifact) = self.release_artifact.take() else {
            return;
        };
        let update_id = self.source_update_id.clone();
        self.work(
            "正在冻结与校验扩展产物…",
            OperationStage::Source,
            cx,
            move |m| {
                let preview = artifact.select_root(Some(&root))?;
                prepare_source(m, preview, update_id)
            },
            accept_source,
        );
    }
    fn fetch_repository(&mut self, cx: &mut Context<Self>) {
        let repository = self.remote_repo.read(cx).value().trim().to_owned();
        let reference = self.remote_reference.read(cx).value().trim().to_owned();
        let directory = self.remote_directory.read(cx).value().trim().to_owned();
        let update_id = self.source_update_id.clone();
        self.work(
            "正在解析引用并获取完整扩展目录…",
            OperationStage::Source,
            cx,
            move |m| {
                let preview =
                    m.preview_repository(&CommandGh, &repository, &reference, &directory)?;
                prepare_source(m, preview, update_id)
            },
            accept_source,
        );
    }
    fn choose_remote_version(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(record) = self.current_record() else {
            return;
        };
        let Some(origin) = record.origin else { return };
        self.open_panel(Panel::Add, window, cx);
        self.source_update_id = Some(record.id);
        self.add_step = 1;
        self.release_catalog = None;
        self.release_artifact = None;
        match origin {
            RemoteOrigin::GitHubRelease { repo, .. } => {
                self.source_kind = SourceKind::Release;
                self.remote_repo
                    .update(cx, |s, cx| s.set_value(repo.slug(), window, cx));
            }
            RemoteOrigin::GitHubDirectory {
                repo,
                reference,
                subdirectory,
                ..
            } => {
                self.source_kind = SourceKind::Repository;
                self.remote_repo
                    .update(cx, |s, cx| s.set_value(repo.slug(), window, cx));
                self.remote_reference
                    .update(cx, |s, cx| s.set_value(reference, window, cx));
                self.remote_directory
                    .update(cx, |s, cx| s.set_value(subdirectory, window, cx));
            }
        }
    }
    fn source_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.busy;
        let mut content = v_flex().gap_4();
        if self.source_update_id.is_none() {
            let mut kinds = h_flex().gap_2();
            for (kind, id, label) in [
                (SourceKind::Local, "local-source", "本地文件夹"),
                (SourceKind::Release, "release-source", "GitHub Release"),
                (SourceKind::Repository, "repository-source", "仓库子目录"),
            ] {
                kinds = kinds.child(
                    Button::new(id)
                        .label(label)
                        .selected(self.source_kind == kind)
                        .disabled(disabled)
                        .on_click(cx.listener(move |v, _, _, cx| {
                            v.source_kind = kind;
                            v.release_catalog = None;
                            v.release_artifact = None;
                            cx.notify();
                        })),
                );
            }
            content = content.child(kinds);
        }
        match self.source_kind {
            SourceKind::Local => {
                content = content
                    .child(heading("选择完整的扩展文件夹"))
                    .child(caption("包含 manifest.json 和所需文件，不运行构建脚本。"))
                    .child(
                        Button::new("choose-source")
                            .label("▱ 选择本地扩展文件夹")
                            .disabled(disabled)
                            .on_click(cx.listener(|v, _, _, cx| v.choose_source(cx))),
                    );
            }
            SourceKind::Repository => {
                content = content
                    .child(heading("选择仓库中的扩展产物"))
                    .child(caption(
                        "支持私有仓库，由本机 gh 提供访问。只获取已有产物，不执行仓库脚本。",
                    ))
                    .child(Input::new(&self.remote_repo).disabled(disabled))
                    .child(Input::new(&self.remote_reference).disabled(disabled))
                    .child(Input::new(&self.remote_directory).disabled(disabled))
                    .child(caption(
                        "分支用于跟随更新；标签或提交可选择任意版本。目录留空表示仓库根目录。",
                    ))
                    .child(
                        Button::new("fetch-repository")
                            .primary()
                            .label("预览此引用的产物")
                            .disabled(disabled)
                            .on_click(cx.listener(|v, _, _, cx| v.fetch_repository(cx))),
                    );
            }
            SourceKind::Release => {
                content = content
                    .child(heading("从 GitHub 安装"))
                    .child(caption("输入仓库地址，默认选择最新正式版。"))
                    .child(Input::new(&self.remote_repo).disabled(disabled))
                    .child(
                        Button::new("list-releases")
                            .label("获取发布版本")
                            .disabled(disabled)
                            .on_click(cx.listener(|v, _, _, cx| v.load_releases(cx))),
                    );
                if let Some(catalog) = &self.release_catalog {
                    let mut versions = v_flex()
                        .id("release-versions")
                        .max_h(px(180.))
                        .overflow_y_scroll()
                        .gap_1();
                    for release in &catalog.releases {
                        let tag = release.tag_name.clone();
                        versions = versions.child(
                            Button::new(SharedString::from(format!("tag-{tag}")))
                                .label(format!(
                                    "{}{}",
                                    tag,
                                    if release.prerelease {
                                        " · 预发布"
                                    } else {
                                        ""
                                    }
                                ))
                                .selected(self.release_tag.as_ref() == Some(&tag))
                                .disabled(disabled)
                                .on_click(cx.listener(move |v, _, _, cx| {
                                    v.show_versions = false;
                                    v.release_tag = Some(tag.clone());
                                    v.release_asset = v
                                        .release_catalog
                                        .as_ref()
                                        .and_then(|c| c.preselected_asset(&tag));
                                    v.release_artifact = None;
                                    cx.notify();
                                })),
                        );
                    }
                    content = content
                        .child(
                            Button::new("show-versions")
                                .ghost()
                                .label(format!(
                                    "{} · {}",
                                    self.release_tag.as_deref().unwrap_or("选择版本"),
                                    if self.show_versions {
                                        "收起版本列表"
                                    } else {
                                        "更换版本"
                                    }
                                ))
                                .disabled(disabled)
                                .on_click(cx.listener(|v, _, _, cx| {
                                    v.show_versions = !v.show_versions;
                                    cx.notify();
                                })),
                        )
                        .when(self.show_versions, |v| v.child(versions));
                    if let Some(release) = catalog
                        .releases
                        .iter()
                        .find(|r| Some(&r.tag_name) == self.release_tag.as_ref())
                    {
                        for asset in &release.assets {
                            if release.assets.len() == 1 && self.release_asset == Some(asset.id) {
                                content = content.child(caption(asset.name.clone()));
                                continue;
                            }
                            let id = asset.id;
                            content = content.child(
                                Button::new(SharedString::from(format!("asset-{id}")))
                                    .label(format!("{} · {} KB", asset.name, asset.size / 1024))
                                    .selected(self.release_asset == Some(id))
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |v, _, _, cx| {
                                        v.release_asset = Some(id);
                                        v.release_artifact = None;
                                        cx.notify();
                                    })),
                            );
                        }
                    }
                    content = content.child(
                        Button::new("fetch-release")
                            .primary()
                            .label("继续")
                            .disabled(
                                disabled
                                    || self.release_tag.is_none()
                                    || self.release_asset.is_none(),
                            )
                            .on_click(cx.listener(|v, _, _, cx| v.fetch_release(cx))),
                    );
                }
                if let Some(artifact) = &self.release_artifact {
                    content = content.child(caption("ZIP 中可安装的扩展目录"));
                    for (index, root) in artifact.roots().into_iter().enumerate() {
                        content = content.child(
                            Button::new(SharedString::from(format!("root-{index}")))
                                .label(if root.is_empty() {
                                    "ZIP 根目录".into()
                                } else {
                                    root.clone()
                                })
                                .disabled(disabled)
                                .on_click(cx.listener(move |v, _, _, cx| {
                                    v.select_release_root(root.clone(), cx)
                                })),
                        );
                    }
                }
            }
        }
        content.into_any_element()
    }
    fn check_all(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        let Some(profile) = self.target() else { return };
        self.bulk_checks.clear();
        self.bulk_results.clear();
        self.work(
            "正在逐项检查当前配置的来源…",
            OperationStage::Check,
            cx,
            move |m| {
                profile.validate_location()?;
                m.check_target_updates(&CommandGh, &profile.target())
            },
            |v, checked| {
                let unchanged = checked
                    .iter()
                    .all(|c| c.plan().is_some_and(|p| p.kind() == UpdateKind::Unchanged));
                for c in &checked {
                    v.check_outcomes.insert(
                        c.id().to_owned(),
                        match c.plan() {
                            Some(plan) => Ok((plan.kind() == UpdateKind::Upgrade)
                                .then(|| plan.version().to_owned())),
                            None => Err(c.error().unwrap_or("检查失败").to_owned()),
                        },
                    );
                }
                v.bulk_checks = checked;
                v.panel = if unchanged { Panel::None } else { Panel::Bulk };
                v.status = if unchanged {
                    "没有发现新版本"
                } else {
                    "检查完成"
                }
                .into();
            },
        );
    }
    fn apply_all(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.target() else { return };
        let checked = std::mem::take(&mut self.bulk_checks);
        self.work(
            "正在逐项备份与部署；每项独立保留结果…",
            OperationStage::Deploy,
            cx,
            move |m| {
                profile.validate_location()?;
                let results = m.apply_target_upgrades(&profile.target(), checked);
                Ok((results, m.list()?))
            },
            |v, (results, records)| {
                for result in &results {
                    if matches!(result.status, BulkStatus::Deployed(_)) {
                        v.check_outcomes.remove(&result.id);
                    }
                }
                v.bulk_results = results;
                v.records = records;
                v.status = "批量准备结束。成功项仍需在当前浏览器配置重新加载与核对。".into();
            },
        );
    }
    fn panel_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.busy;
        let mut content = v_flex().gap_4();
        match self.panel {
            Panel::Bind => {
                content =
                    content.child(caption("只读取配置名称和位置，不读取历史记录或登录信息。"));
                for (i, candidate) in self.candidates.iter().cloned().enumerate() {
                    let label = format!("{:?} · {}", candidate.browser, candidate.name);
                    let path = candidate.profile_path.display().to_string();
                    content = content.child(
                        v_flex()
                            .gap_1()
                            .child(
                                Button::new(SharedString::from(format!("candidate-{i}")))
                                    .label(label)
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |v, _, _, cx| {
                                        v.bind_candidate(candidate.clone(), cx)
                                    })),
                            )
                            .child(caption(path)),
                    );
                }
                content = content
                    .child(div().h(px(1.)).bg(rgb(BORDER)))
                    .child(heading("手动定位"))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("bind-chrome")
                                    .label("Chrome")
                                    .selected(self.bind_browser == Browser::Chrome)
                                    .on_click(cx.listener(|v, _, _, cx| {
                                        v.bind_browser = Browser::Chrome;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("bind-edge")
                                    .label("Edge")
                                    .selected(self.bind_browser == Browser::Edge)
                                    .on_click(cx.listener(|v, _, _, cx| {
                                        v.bind_browser = Browser::Edge;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(caption("配置显示名称"))
                    .child(Input::new(&self.bind_name))
                    .child(caption("浏览器应用位置"))
                    .child(Input::new(&self.bind_app))
                    .child(caption(
                        "Profile Path（在 chrome://version 或 edge://version 查看）",
                    ))
                    .child(Input::new(&self.bind_path))
                    .child(
                        Checkbox::new("bind-check")
                            .label("我已在目标浏览器配置核对 Profile Path")
                            .checked(self.checked)
                            .on_click(cx.listener(|v, value, _, cx| {
                                v.checked = *value;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("bind-manual")
                            .primary()
                            .label("绑定这个配置")
                            .disabled(disabled)
                            .on_click(cx.listener(|v, _, _, cx| v.bind_manual(cx))),
                    );
            }
            Panel::Legacy => {
                content = content
                    .child(caption(format!(
                        "将旧安装定位到 {}。仅用于识别原来使用的配置，不迁移浏览器数据。",
                        self.target_label()
                    )))
                    .child(
                        Checkbox::new("legacy-check")
                            .label("我确认原配置一致，并接受当前安装文件作为后续更新基线")
                            .checked(self.checked)
                            .on_click(cx.listener(|v, value, _, cx| {
                                v.checked = *value;
                                cx.notify();
                            })),
                    );
                for r in self
                    .records
                    .iter()
                    .filter(|r| uuid_is_legacy(&r.target.profile))
                {
                    let id = r.id.clone();
                    content = content.child(
                        v_flex()
                            .gap_1()
                            .child(heading(r.name.clone()))
                            .child(caption(format!(
                                "旧标签：{:?} / {} · 文件 {}",
                                r.target.browser, r.target.profile, r.installed_version
                            )))
                            .child(
                                Button::new(SharedString::from(format!("bind-old-{}", r.id)))
                                    .label("绑定到当前配置并重新核对")
                                    .disabled(disabled || !self.checked)
                                    .on_click(cx.listener(move |v, _, _, cx| {
                                        v.bind_legacy(id.clone(), cx)
                                    })),
                            ),
                    );
                }
            }
            Panel::Add => {
                content = content
                    .child(caption(format!(
                        "{} {}",
                        if self.source_update_id.is_some() {
                            "为当前安装选择版本 ·"
                        } else {
                            "添加到"
                        },
                        self.target_label()
                    )))
                    .child(
                        h_flex()
                            .gap_4()
                            .child(badge("1 选择来源", self.add_step == 1))
                            .child(badge("2 确认安装", self.add_step == 2)),
                    );
                match self.add_step {
                    1 => {
                        content = content.child(self.source_picker(cx));
                    }
                    2 => {
                        if let Some(p) = &self.preview {
                            content = content
                                .child(
                                    div().p_4().rounded_lg().bg(rgb(0xeff3ff)).child(
                                        v_flex()
                                            .gap_2()
                                            .child(heading(p.name().to_string()))
                                            .child(format!("扩展版本 {}", p.version()))
                                            .child(caption(p.source().display().to_string()))
                                            .child(caption(origin_description(p.origin()))),
                                    ),
                                )
                                .child(caption("安装后需要在浏览器中完成首次加载。"))
                                .child(
                                    Button::new("install")
                                        .primary()
                                        .label("安装")
                                        .disabled(disabled)
                                        .on_click(cx.listener(|v, _, w, cx| v.install(w, cx))),
                                );
                        }
                    }
                    _ => {}
                }
                if self.add_step > 1 {
                    content = content.child(
                        Button::new("add-back")
                            .ghost()
                            .icon(gpui_kit::assets::IconName::ArrowLeft)
                            .label("上一步")
                            .disabled(disabled)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.add_step -= 1;
                                cx.notify();
                            })),
                    );
                }
            }
            Panel::Guide => {
                if let Some(r) = self.guide_record.as_ref() {
                    let profile = self.target().unwrap();
                    let disabled = disabled || r.recovery_required || r.target != profile.target();
                    let first_load = r.generation <= 1;
                    let path = r.install_dir.to_string_lossy().into_owned();
                    let escaped_path = path
                        .replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;")
                        .replace('"', "&quot;");
                    content =
                        content
                            .gap_3()
                            .child(
                                h_flex().gap_3().child(self.extension_icon(r)).child(
                                    v_flex()
                                        .gap_1()
                                        .child(heading(r.name.clone()))
                                        .child(caption(format!(
                                            "{} · 版本 {}",
                                            profile.label(),
                                            r.installed_version
                                        ))),
                                ),
                            )
                            .child(caption(if first_load {
                                "开启「开发者模式」，点击「加载已解压的扩展」，选择下方文件夹。"
                            } else {
                                "请在浏览器中重新加载扩展，确认显示上方版本后完成更新。"
                            }))
                            .child(
                                h_flex().child(
                                    Button::new("guide-native")
                                        .ghost()
                                        .icon(gpui_kit::assets::IconName::ExternalLink)
                                        .label("浏览器扩展管理")
                                        .disabled(disabled || !self.target_ready())
                                        .on_click(cx.listener(|v, _, _, cx| v.open_native(cx))),
                                ),
                            )
                            .when(first_load, |v| {
                                v.child(
                                    gpui_kit::component::text::TextView::html(
                                        "installation-path",
                                        format!("<p>{escaped_path}</p>"),
                                    )
                                    .selectable(true)
                                    .selection_format(
                                        gpui_kit::component::text::SelectionFormat::Plain,
                                    ),
                                )
                                .child(
                                    h_flex().child(
                                        Button::new("copy-path")
                                            .icon(gpui_kit::assets::IconName::Copy)
                                            .label(if self.path_copied {
                                                "已复制"
                                            } else {
                                                "复制安装路径"
                                            })
                                            .disabled(disabled)
                                            .on_click(cx.listener(|v, _, _, cx| {
                                                v.copy_installation_path(cx)
                                            })),
                                    ),
                                )
                                .child(caption(
                                    if cfg!(target_os = "macos") {
                                        "在文件夹选择窗口按 ⌘⇧G，粘贴路径，回车，再点「选择」。"
                                    } else if cfg!(target_os = "windows") {
                                        "在文件夹窗口地址栏粘贴路径，回车，再点「选择文件夹」。"
                                    } else {
                                        "在文件夹选择窗口粘贴完整路径，再选择该文件夹。"
                                    },
                                ))
                            })
                            .child(
                                h_flex()
                                    .gap_2()
                                    .justify_between()
                                    .child(if first_load {
                                        Button::new("guide-later")
                                            .ghost()
                                            .label("稍后继续")
                                            .disabled(disabled)
                                            .on_click(cx.listener(|v, _, _, cx| {
                                                v.panel = Panel::Detail;
                                                v.guide_record = None;
                                                cx.notify();
                                            }))
                                    } else {
                                        Button::new("update-failed")
                                            .label("更新失败")
                                            .disabled(disabled)
                                            .on_click(cx.listener(|v, _, _, cx| {
                                                v.report_browser_failure(cx)
                                            }))
                                    })
                                    .child(
                                        Button::new("confirm")
                                            .primary()
                                            .label(if first_load {
                                                "已加载，确认完成"
                                            } else {
                                                "确认更新完成"
                                            })
                                            .disabled(disabled || !self.target_ready())
                                            .on_click(cx.listener(|v, _, _, cx| v.confirm(cx))),
                                    ),
                            )
                            .child(
                                h_flex().child(
                                    Button::new("guide-help")
                                        .ghost()
                                        .small()
                                        .label(if self.show_technical {
                                            "收起帮助"
                                        } else {
                                            "没有打开管理页？"
                                        })
                                        .on_click(cx.listener(|v, _, _, cx| {
                                            v.show_technical = !v.show_technical;
                                            cx.notify();
                                        })),
                                ),
                            )
                            .when(self.show_technical || self.error, |v| {
                                v.child(caption(format!(
                                    "在 {} 的地址栏打开 {}，完成后回来确认。",
                                    profile.label(),
                                    profile.management_url()
                                )))
                                .child(h_flex().child(
                                    Button::new("copy-url").label("复制管理页地址").on_click(
                                        cx.listener(|v, _, _, cx| v.copy_management_url(cx)),
                                    ),
                                ))
                            });
                }
            }
            Panel::Update => {
                if let (Some(r), Some(plan)) = (self.current_record(), &self.update_plan) {
                    let kind = match plan.kind() {
                        UpdateKind::Upgrade => "升级",
                        UpdateKind::Downgrade => "降级",
                        UpdateKind::Redeploy => "同版本重新部署",
                        UpdateKind::Unchanged => "已经是当前版本",
                    };
                    content = content
                        .child(heading(r.name))
                        .child(badge(self.target_label(), false))
                        .child(heading(format!(
                            "{} → {}",
                            r.installed_version,
                            plan.version()
                        )))
                        .child(caption(format!("{kind} · {}", plan.source().display())))
                        .child(caption("更新后需要在浏览器重新加载。"));
                    if matches!(plan.kind(), UpdateKind::Downgrade | UpdateKind::Redeploy) {
                        content = content.child(
                            Checkbox::new("version-choice")
                                .label(format!("我明确选择{kind}"))
                                .checked(self.checked)
                                .disabled(disabled)
                                .on_click(cx.listener(|v, value, _, cx| {
                                    v.checked = *value;
                                    cx.notify();
                                })),
                        );
                    }
                    content = content.child(
                        Button::new("apply-update")
                            .primary()
                            .label(if plan.kind() == UpdateKind::Upgrade {
                                "更新".to_owned()
                            } else {
                                format!("确认{kind}")
                            })
                            .disabled(
                                disabled
                                    || plan.kind() == UpdateKind::Unchanged
                                    || (matches!(
                                        plan.kind(),
                                        UpdateKind::Downgrade | UpdateKind::Redeploy
                                    ) && !self.checked),
                            )
                            .on_click(cx.listener(|v, _, _, cx| v.apply_update(cx))),
                    );
                }
            }
            Panel::Restore => {
                content = content.child(heading("浏览器无法加载新版本？"))
                    .child(caption("恢复本次更新前保留的文件，然后重新加载与核对。浏览器数据由浏览器保管；恢复文件不会自动恢复浏览器数据。"))
                    .child(Checkbox::new("restore-check").label("新文件加载失败，我要恢复上一份文件").checked(self.checked).disabled(disabled)
                        .on_click(cx.listener(|v, value, _, cx| { v.checked = *value; cx.notify(); })))
                    .child(Button::new("restore-files").primary().label("恢复上一份文件").disabled(disabled || !self.checked)
                        .on_click(cx.listener(|v, _, _, cx| v.restore(cx))));
            }
            Panel::Bulk => {
                content = content.child(heading(self.target_label())).child(caption(
                    "本次只处理当前配置。降级、同版本重新部署与冲突需单独确认或处理。",
                ));
                for check in &self.bulk_checks {
                    let summary = match check.plan() {
                        Some(plan) => format!(
                            "目标 {} · {}",
                            plan.version(),
                            match plan.kind() {
                                UpdateKind::Upgrade => "可升级",
                                UpdateKind::Downgrade => "降级需单独确认",
                                UpdateKind::Redeploy => "同版本变更需单独确认",
                                UpdateKind::Unchanged => "无需更新",
                            }
                        ),
                        None => format!("检查失败：{}", check.error().unwrap_or("请重新检查")),
                    };
                    content = content.child(
                        v_flex()
                            .gap_1()
                            .child(heading(check.name().to_owned()))
                            .child(caption(summary)),
                    );
                }
                let upgrades = self
                    .bulk_checks
                    .iter()
                    .filter(|c| c.plan().is_some_and(|p| p.kind() == UpdateKind::Upgrade))
                    .count();
                if !self.bulk_checks.is_empty() {
                    content = content.child(
                        Button::new("apply-all")
                            .primary()
                            .label(format!("更新 {upgrades} 个扩展"))
                            .disabled(disabled || upgrades == 0)
                            .on_click(cx.listener(|v, _, _, cx| v.apply_all(cx))),
                    );
                }
                for result in &self.bulk_results {
                    let id = result.id.clone();
                    let (label, message) = match &result.status {
                        BulkStatus::Deployed(_) => {
                            ("文件已准备", "等待在浏览器重新加载并核对".to_owned())
                        }
                        BulkStatus::Skipped(reason) => ("已跳过", reason.clone()),
                        BulkStatus::Failed(reason) => ("未完成", reason.clone()),
                    };
                    content = content.child(
                        v_flex()
                            .gap_2()
                            .child(heading(result.name.clone()))
                            .child(badge(
                                label,
                                !matches!(result.status, BulkStatus::Deployed(_)),
                            ))
                            .child(caption(message))
                            .child(
                                Button::new(SharedString::from(format!("bulk-result-{id}")))
                                    .label("查看此扩展")
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |v, _, w, cx| {
                                        v.selected = Some(id.clone());
                                        v.open_panel(Panel::Detail, w, cx);
                                    })),
                            ),
                    );
                }
            }
            Panel::None | Panel::Detail => {}
        }
        content.into_any_element()
    }
}
impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = self.panel;
        self.keyboard.sync(
            if self.switcher {
                1
            } else if panel != Panel::None && panel != Panel::Detail {
                2
            } else {
                0
            },
            window,
            cx,
        );
        let title = match panel {
            Panel::Bind => "浏览器配置",
            Panel::Legacy => "定位旧安装",
            Panel::Add => "添加扩展",
            Panel::Detail => "扩展详情",
            Panel::Guide => {
                if self.guide_record.as_ref().is_some_and(|r| r.generation > 1) {
                    "确认更新结果"
                } else {
                    "完成首次加载"
                }
            }
            Panel::Update => "确认版本变更",
            Panel::Restore => "恢复旧文件",
            Panel::Bulk => "当前配置的更新",
            Panel::None => "",
        };
        v_flex()
            .id("desktop-focus")
            .tab_group()
            .track_focus(&self.keyboard.root)
            .tab_stop(false)
            .on_key_down(cx.listener(|v, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && !v.busy {
                    v.switcher = false;
                    if v.panel != Panel::Detail {
                        v.panel = Panel::None;
                    }
                    cx.notify();
                }
            }))
            .relative()
            .size_full()
            .text_color(rgb(0x1a1a1d))
            .bg(rgb(0xf4f4f6))
            .child(
                TitleBar::new()
                    .bg(rgb(0xf8f8fa))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        h_flex()
                            .w_full()
                            .justify_center()
                            .gap_2()
                            .child(img("cepm/logo.png").size(px(22.)))
                            .child(caption("Chromium Extension Package Manager")),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.sidebar(cx))
                    .child(if panel == Panel::Detail {
                        self.detail_page(cx)
                    } else {
                        self.library(cx)
                    }),
            )
            .when(self.switcher, |root| root.child(self.profile_menu(cx)))
            .when(panel != Panel::None && panel != Panel::Detail, |root| {
                root.child(
                    div()
                        .id("task-modal-focus")
                        .track_focus(&self.keyboard.overlay)
                        .tab_stop(false)
                        .absolute()
                        .top(px(36.))
                        .bottom_0()
                        .right_0()
                        .left(if panel == Panel::Guide {
                            px(220.)
                        } else {
                            px(0.)
                        })
                        .bg(rgba(0x202d3655))
                        .flex()
                        .items_center()
                        .justify_center()
                        .p_6()
                        .child(
                            v_flex()
                                .w(px(640.))
                                .max_h_full()
                                .bg(rgb(0xffffff))
                                .rounded_xl()
                                .shadow_lg()
                                .overflow_hidden()
                                .child(
                                    h_flex()
                                        .p_5()
                                        .justify_between()
                                        .child(heading(title))
                                        .child(
                                            Button::new("close-panel")
                                                .ghost()
                                                .icon(gpui_kit::assets::IconName::X)
                                                .tooltip("关闭")
                                                .accessibility_label("关闭")
                                                .disabled(self.busy)
                                                .on_click(cx.listener(|v, _, _, cx| {
                                                    v.panel = Panel::None;
                                                    v.preview = None;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("panel-scroll")
                                        .flex_1()
                                        .min_h_0()
                                        .overflow_y_scroll()
                                        .p_5()
                                        .child(self.panel_content(cx)),
                                )
                                .when(!self.status.is_empty(), |v| {
                                    v.child(
                                        div()
                                            .px_5()
                                            .py_3()
                                            .text_sm()
                                            .bg(rgb(if self.error { 0xfaeeec } else { 0xeff3ff }))
                                            .child(self.status.clone()),
                                    )
                                })
                                .when(panel != Panel::Guide, |d| {
                                    d.child(
                                        h_flex()
                                            .justify_between()
                                            .p_4()
                                            .border_t_1()
                                            .border_color(rgb(BORDER))
                                            .child(div())
                                            .child(
                                                Button::new("later")
                                                    .label("关闭")
                                                    .disabled(self.busy)
                                                    .on_click(cx.listener(|v, _, _, cx| {
                                                        v.panel = Panel::None;
                                                        cx.notify();
                                                    })),
                                            ),
                                    )
                                }),
                        )
                        .focus_trap("task-modal-trap", &self.keyboard.overlay),
                )
            })
    }
}
fn uuid_is_legacy(value: &str) -> bool {
    !(value.len() == 36
        && value.chars().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        }))
}
fn application_binary(path: PathBuf, browser: &Browser) -> PathBuf {
    if path.extension().is_some_and(|e| e == "app") {
        path.join("Contents/MacOS").join(match browser {
            Browser::Chrome => "Google Chrome",
            Browser::Edge => "Microsoft Edge",
        })
    } else {
        path
    }
}
fn browser_locations() -> Vec<BrowserLocation> {
    let Some(home) = directories::BaseDirs::new() else {
        return Vec::new();
    };
    #[cfg(target_os = "macos")]
    {
        vec![
            BrowserLocation {
                browser: Browser::Chrome,
                application: PathBuf::from(
                    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                ),
                user_data_dir: home
                    .home_dir()
                    .join("Library/Application Support/Google/Chrome"),
            },
            BrowserLocation {
                browser: Browser::Edge,
                application: PathBuf::from(
                    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
                ),
                user_data_dir: home
                    .home_dir()
                    .join("Library/Application Support/Microsoft Edge"),
            },
        ]
    }
    #[cfg(target_os = "windows")]
    {
        let programs = std::env::var_os("PROGRAMFILES")
            .map(PathBuf::from)
            .unwrap_or_default();
        let local = home.data_local_dir();
        vec![
            BrowserLocation {
                browser: Browser::Chrome,
                application: programs.join("Google/Chrome/Application/chrome.exe"),
                user_data_dir: local.join("Google/Chrome/User Data"),
            },
            BrowserLocation {
                browser: Browser::Edge,
                application: programs.join("Microsoft/Edge/Application/msedge.exe"),
                user_data_dir: local.join("Microsoft/Edge/User Data"),
            },
        ]
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = home;
        Vec::new()
    }
}
fn data_root() -> cepm_core::Result<PathBuf> {
    if let Some(path) = std::env::var_os("CEPM_DATA_DIR") {
        return Ok(path.into());
    }
    directories::ProjectDirs::from("dev", "CEPM", "CEPM")
        .map(|d| d.data_local_dir().to_path_buf())
        .ok_or("无法定位应用数据目录".into())
}
fn main() {
    let root = data_root();
    gpui_kit::application()
        .with_assets(AppAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            apply_theme(cx);
            cx.activate(true);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let options = WindowOptions {
                titlebar: Some(TitleBar::title_bar_options()),
                app_owns_titlebar_drag: true,
                window_bounds: Some(WindowBounds::centered(size(px(1120.), px(780.)), cx)),
                ..Default::default()
            };
            if let Err(e) = gpui_kit::open_window(options, cx, |w, cx| {
                w.set_window_title("Chromium Extension Package Manager");
                cx.new(|cx| Desktop::new(root, w, cx))
            }) {
                eprintln!("无法打开 CEPM：{e}");
                cx.quit();
            }
        });
}
