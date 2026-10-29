use cepm_core::profiles::{
    BrowserLocation, BrowserProfile, ProfileBook, ProfileCandidate, discover_profiles,
};
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

const GREEN: u32 = 0x23735a;
const MUTED: u32 = 0x7b838a;
const BORDER: u32 = 0xe7eaed;
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Library,
    Attention,
    History,
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
}
struct Desktop {
    manager: Option<Arc<Mutex<Manager>>>,
    profiles: Option<ProfileBook>,
    candidates: Vec<ProfileCandidate>,
    records: Vec<Installation>,
    selected: Option<String>,
    preview: Option<Preview>,
    update_plan: Option<UpdatePlan>,
    page: Page,
    panel: Panel,
    add_step: u8,
    switcher: bool,
    show_technical: bool,
    checked: bool,
    bind_browser: Browser,
    bind_name: Entity<InputState>,
    bind_app: Entity<InputState>,
    bind_path: Entity<InputState>,
    extension_id: Entity<InputState>,
    version: Entity<InputState>,
    status: String,
    error: bool,
    busy: bool,
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
        .bg(rgb(if pending { 0xfcf3e3 } else { 0xeaf4ef }))
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
        let (manager, profiles) = match root {
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
        let error = !status.is_empty();
        Self {
            manager,
            profiles,
            candidates,
            records,
            selected: None,
            preview: None,
            update_plan: None,
            page: Page::Library,
            panel: Panel::None,
            add_step: 1,
            switcher: false,
            show_technical: false,
            checked: false,
            bind_browser: Browser::Chrome,
            bind_name: input(window, cx, "例如：个人、工作"),
            bind_app: input(window, cx, "浏览器应用或可执行文件的完整路径"),
            bind_path: input(window, cx, "从浏览器 version 页复制 Profile Path"),
            extension_id: input(window, cx, "从目标浏览器复制 32 位扩展 ID"),
            version: input(window, cx, "填写浏览器中实际显示的版本"),
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
    fn open_panel(&mut self, panel: Panel, window: &mut Window, cx: &mut Context<Self>) {
        self.panel = panel;
        self.checked = false;
        self.show_technical = false;
        self.status.clear();
        self.error = false;
        self.extension_id
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.version.update(cx, |s, cx| s.set_value("", window, cx));
        if panel == Panel::Guide
            && let Some(id) = self.current_record().and_then(|r| r.extension_id)
        {
            self.extension_id
                .update(cx, |s, cx| s.set_value(id, window, cx));
        }
        cx.notify();
    }
    fn select_profile(&mut self, id: &str, cx: &mut Context<Self>) {
        let result = self
            .profiles
            .as_mut()
            .ok_or("配置偏好不可用".into())
            .and_then(|p| p.select(id));
        if result.is_ok() {
            self.selected = None;
            self.panel = Panel::None;
            self.switcher = false;
            self.page = Page::Library;
        }
        self.message(result, "", cx);
    }
    fn pin(&mut self, id: &str, cx: &mut Context<Self>) {
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
        let result = self
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
            self.panel = Panel::None;
            self.switcher = false;
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
            "已请求打开目标配置的原生扩展页；完成后请回来核对",
            cx,
        );
    }
    fn work<T: Send + 'static>(
        &mut self,
        label: &str,
        cx: &mut Context<Self>,
        operation: impl FnOnce(&mut Manager) -> cepm_core::Result<T> + Send + 'static,
        apply: impl FnOnce(&mut Self, T) + 'static,
    ) {
        let Some(manager) = self.manager.clone() else {
            return;
        };
        self.busy = true;
        self.error = false;
        self.status = label.into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut m = manager
                        .lock()
                        .map_err(|_| "操作异常，请重启后检查记录".to_string())?;
                    let result = operation(&mut m);
                    let refreshed = if result.is_err() { m.list().ok() } else { None };
                    Ok::<_, String>((result, refreshed))
                })
                .await;
            this.update(cx, |v, cx| {
                let result = match result {
                    Ok((result, refreshed)) => {
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
    fn install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.target() else { return };
        if let Err(e) = target.validate_location() {
            self.message(Err(e), "", cx);
            return;
        }
        let Some(preview) = self.preview.take() else {
            return;
        };
        self.extension_id
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.version.update(cx, |s, cx| s.set_value("", window, cx));
        self.checked = false;
        self.work(
            "正在准备固定安装目录…",
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
                v.status = "文件已准备好，接下来在浏览器完成首次加载".into();
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
    fn confirm(&mut self, cx: &mut Context<Self>) {
        if !self.checked {
            self.message(Err("请先在目标配置核对，再勾选确认".into()), "", cx);
            return;
        }
        let Some(record) = self.current_record() else {
            return;
        };
        let Some(profile) = self.target() else { return };
        let id = self.extension_id.read(cx).value().trim().to_owned();
        let version = self.version.read(cx).value().trim().to_owned();
        self.work(
            "正在保存用户确认…",
            cx,
            move |m| {
                profile.validate_location()?;
                m.confirm(&record.id, record.generation, &id, &version)?;
                m.list()
            },
            |v, records| {
                v.records = records;
                v.panel = Panel::None;
                v.status = "已记录用户确认".into();
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
            cx,
            move |m| m.check_local(&record.id),
            |v, plan| {
                v.status = if plan.kind() == UpdateKind::Unchanged {
                    "来源文件与安装版本一致".into()
                } else {
                    "请确认版本变更，再准备文件".into()
                };
                v.update_plan = Some(plan);
                v.panel = Panel::Update;
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
            cx,
            move |m| {
                profile.validate_location()?;
                let kind = plan.kind();
                m.apply_update(plan, kind)?;
                m.list()
            },
            |v, records| {
                v.records = records;
                v.panel = Panel::Detail;
                v.checked = false;
                v.status = "文件已部署，等待你在浏览器重新加载与核对".into();
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
            cx,
            move |m| {
                m.report_load_failure(&record.id, record.generation)?;
                m.list()
            },
            |v, records| {
                v.records = records;
                v.panel = Panel::Detail;
                v.checked = false;
                v.status = "上一份文件已恢复；仍需重新加载并核对，才会记录生效".into();
            },
        );
    }
    fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let scoped = self.scoped();
        let pending = scoped
            .iter()
            .filter(|r| r.verification == Verification::Pending)
            .count();
        let mut side = v_flex()
            .w(px(228.))
            .h_full()
            .flex_shrink_0()
            .p_4()
            .gap_2()
            .bg(rgb(0xf0f2f4))
            .border_r_1()
            .border_color(rgb(BORDER))
            .child(
                h_flex()
                    .gap_2()
                    .px_2()
                    .py_4()
                    .child(
                        div()
                            .rounded_lg()
                            .px_2()
                            .py_1()
                            .bg(rgb(GREEN))
                            .text_color(rgb(0xffffff))
                            .child("↗"),
                    )
                    .child(heading("CEPM")),
            )
            .child(
                Button::new("profile-switcher")
                    .label(format!("{}  ⌄", self.target_label()))
                    .w_full()
                    .disabled(self.busy)
                    .on_click(cx.listener(|v, _, _, cx| {
                        v.switcher = !v.switcher;
                        cx.notify();
                    })),
            );
        if self.switcher {
            let mut menu = v_flex()
                .id("profile-menu")
                .max_h(px(340.))
                .overflow_y_scroll()
                .p_2()
                .gap_1()
                .rounded_lg()
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(rgb(BORDER));
            if let Some(book) = &self.profiles {
                let mut profiles = book.profiles().to_vec();
                profiles.sort_by_key(|p| {
                    (
                        !book.pinned_ids().contains(&p.id),
                        format!("{:?}", p.browser),
                        p.name.clone(),
                    )
                });
                let mut group = String::new();
                for p in profiles {
                    let next_group = if book.pinned_ids().contains(&p.id) {
                        "置顶".to_owned()
                    } else {
                        format!("{:?}", p.browser)
                    };
                    if group != next_group {
                        menu = menu.child(div().pt_2().child(caption(next_group.clone())));
                        group = next_group;
                    }
                    let id = p.id.clone();
                    let pin_id = p.id.clone();
                    let pinned = book.pinned_ids().contains(&p.id);
                    let active = self.target().is_some_and(|t| t.id == p.id);
                    menu = menu.child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new(SharedString::from(format!("target-{}", p.id)))
                                    .label(format!(
                                        "{}{}",
                                        if active { "✓ " } else { "" },
                                        p.label()
                                    ))
                                    .small()
                                    .flex_1()
                                    .on_click(
                                        cx.listener(move |v, _, _, cx| v.select_profile(&id, cx)),
                                    ),
                            )
                            .child(
                                Button::new(SharedString::from(format!("pin-{}", p.id)))
                                    .label(if pinned { "★" } else { "☆" })
                                    .accessibility_label(format!(
                                        "{}{}",
                                        if pinned { "取消置顶 " } else { "置顶 " },
                                        p.label()
                                    ))
                                    .small()
                                    .on_click(cx.listener(move |v, _, _, cx| v.pin(&pin_id, cx))),
                            ),
                    );
                }
            }
            menu = menu
                .child(
                    Button::new("bind-profile")
                        .label("＋ 定位浏览器配置")
                        .small()
                        .on_click(cx.listener(|v, _, w, cx| v.open_panel(Panel::Bind, w, cx))),
                )
                .child(caption("记住上次选择 · ☆ 可置顶多个"));
            side = side.child(menu);
        }
        for (page, key, label) in [
            (
                Page::Library,
                "library",
                format!("▦  我的扩展    {}", scoped.len()),
            ),
            (
                Page::Attention,
                "attention",
                format!("◷  待处理       {pending}"),
            ),
            (Page::History, "history", "↶  操作记录".into()),
        ] {
            side = side.child(
                Button::new(key)
                    .label(label)
                    .w_full()
                    .when(self.page == page, |b| b.selected(true))
                    .disabled(self.busy)
                    .on_click(cx.listener(move |v, _, _, cx| {
                        v.page = page;
                        v.selected = None;
                        cx.notify();
                    })),
            );
        }
        side.child(div().h(px(16.)))
            .child(
                Button::new("native-manager")
                    .label("↗ 打开浏览器扩展管理页")
                    .small()
                    .w_full()
                    .disabled(!self.target_ready())
                    .on_click(cx.listener(|v, _, _, cx| v.open_native(cx))),
            )
            .child(caption(format!(
                "在 {} 中管理启停与权限",
                self.target_label()
            )))
            .child(div().flex_1())
            .child(caption("文件保存在这台电脑\n退出 CEPM 后扩展仍可使用"))
            .into_any_element()
    }
    fn library(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.busy || !self.target_ready() || self.manager.is_none();
        let mut body = v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .p_8()
            .gap_5()
            .bg(rgb(0xf6f7f9))
            .child(
                h_flex()
                    .justify_between()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(caption("你的浏览器工具箱"))
                            .child(div().text_3xl().font_weight(FontWeight::SEMIBOLD).child(
                                match self.page {
                                    Page::Library => "我的扩展",
                                    Page::Attention => "待处理",
                                    Page::History => "操作记录",
                                },
                            ))
                            .child(caption(format!(
                                "{} · 安装、版本与待办独立管理",
                                self.target_label()
                            ))),
                    )
                    .child(
                        h_flex().gap_2().child(
                            Button::new("add-extension")
                                .primary()
                                .label("＋ 添加扩展")
                                .disabled(disabled)
                                .on_click(cx.listener(|v, _, w, cx| {
                                    v.add_step = 1;
                                    v.preview = None;
                                    v.open_panel(Panel::Add, w, cx);
                                })),
                        ),
                    ),
            );
        if !self.status.is_empty() {
            body = body.child(
                div()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(if self.error { 0xfaeeec } else { 0xeaf4ef }))
                    .text_color(rgb(if self.error { 0xab4c43 } else { GREEN }))
                    .text_sm()
                    .child(self.status.clone()),
            );
        }
        if self.target().is_none() {
            return body
                .child(
                    v_flex()
                        .flex_1()
                        .justify_center()
                        .items_center()
                        .gap_4()
                        .child(heading("先选择要维护的浏览器配置"))
                        .child(caption("每个配置里的扩展、版本与待办分别维护。"))
                        .child(
                            Button::new("first-profile")
                                .primary()
                                .label("选择或定位配置")
                                .on_click(
                                    cx.listener(|v, _, w, cx| v.open_panel(Panel::Bind, w, cx)),
                                ),
                        ),
                )
                .into_any_element();
        }
        if !self.target_ready() {
            return body
                .child(
                    v_flex()
                        .p_6()
                        .gap_3()
                        .bg(rgb(0xfff5f3))
                        .rounded_lg()
                        .child(heading("找不到这个浏览器配置"))
                        .child(caption(
                            "安装记录和文件仍保留。请恢复原位置，不会自动切换到其他配置。",
                        ))
                        .child(caption(
                            self.target().unwrap().profile_path().display().to_string(),
                        ))
                        .child(
                            Button::new("locate-missing")
                                .label("检查配置位置")
                                .on_click(
                                    cx.listener(|v, _, w, cx| v.open_panel(Panel::Bind, w, cx)),
                                ),
                        ),
                )
                .into_any_element();
        }
        let records = self.scoped();
        let visible: Vec<_> = records
            .iter()
            .filter(|r| self.page != Page::Attention || r.verification == Verification::Pending)
            .cloned()
            .collect();
        let unbound = self
            .records
            .iter()
            .filter(|r| {
                self.profiles
                    .as_ref()
                    .is_none_or(|p| p.get(&r.target.profile).is_err())
            })
            .count();
        if unbound > 0 {
            body =
                body.child(
                    h_flex()
                        .gap_3()
                        .child(caption(format!(
                            "{unbound} 条旧安装尚未定位，不会混入当前配置。"
                        )))
                        .child(Button::new("legacy").label("定位旧安装").small().on_click(
                            cx.listener(|v, _, w, cx| v.open_panel(Panel::Legacy, w, cx)),
                        )),
                );
        }
        if visible.is_empty() {
            body = body.child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .child(heading(if records.is_empty() {
                        "把第一个扩展交给 CEPM"
                    } else {
                        "这里没有待处理的扩展"
                    }))
                    .child(caption("从本地文件夹开始，CEPM 帮你保管产物和版本。"))
                    .when(records.is_empty(), |v| {
                        v.child(
                            Button::new("first-extension")
                                .primary()
                                .label("添加第一个扩展")
                                .on_click(cx.listener(|v, _, w, cx| {
                                    v.add_step = 1;
                                    v.open_panel(Panel::Add, w, cx);
                                })),
                        )
                    }),
            );
        } else {
            body = body.child(
                div()
                    .id("extension-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .children(visible.into_iter().map(|r| {
                        let id = r.id.clone();
                        let guide_id = r.id.clone();
                        let pending = r.verification == Verification::Pending;
                        if self.page == Page::History {
                            return v_flex()
                                .py_4()
                                .gap_1()
                                .border_b_1()
                                .border_color(rgb(BORDER))
                                .child(div().font_weight(FontWeight::MEDIUM).child(r.name))
                                .child(caption(r.last_operation.clone()))
                                .child(caption(format!(
                                    "文件 {} · {}",
                                    r.installed_version,
                                    if pending {
                                        "等待浏览器核对"
                                    } else {
                                        "用户已确认"
                                    }
                                )))
                                .into_any_element();
                        }
                        h_flex()
                            .gap_4()
                            .py_5()
                            .border_b_1()
                            .border_color(rgb(BORDER))
                            .child(
                                div()
                                    .w(px(42.))
                                    .h(px(42.))
                                    .rounded_lg()
                                    .bg(rgb(0x23382e))
                                    .text_color(rgb(0xffffff))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child("E"),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .child(
                                        Button::new(SharedString::from(format!("record-{}", r.id)))
                                            .label(r.name.clone())
                                            .ghost()
                                            .on_click(cx.listener(move |v, _, w, cx| {
                                                v.selected = Some(id.clone());
                                                v.open_panel(Panel::Detail, w, cx);
                                            })),
                                    )
                                    .child(caption(r.source.display().to_string())),
                            )
                            .child(
                                v_flex()
                                    .w(px(125.))
                                    .child(format!("文件 {}", r.installed_version))
                                    .child(caption(format!(
                                        "{}{}",
                                        if pending {
                                            "上次确认 "
                                        } else {
                                            "浏览器 "
                                        },
                                        if pending {
                                            r.historical_version.as_deref()
                                        } else {
                                            r.effective_version.as_deref()
                                        }
                                        .unwrap_or("尚未确认")
                                    ))),
                            )
                            .child(badge(
                                if r.recovery_required {
                                    "恢复需要处理"
                                } else if pending {
                                    "需要你完成"
                                } else {
                                    "已确认生效"
                                },
                                pending,
                            ))
                            .child(
                                Button::new(SharedString::from(format!("action-{}", r.id)))
                                    .label(if pending {
                                        "加载并核对"
                                    } else {
                                        "查看详情"
                                    })
                                    .small()
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |v, _, w, cx| {
                                        v.selected = Some(guide_id.clone());
                                        v.open_panel(
                                            if pending && !r.recovery_required {
                                                Panel::Guide
                                            } else {
                                                Panel::Detail
                                            },
                                            w,
                                            cx,
                                        );
                                    })),
                            )
                            .into_any_element()
                    })),
            );
        }
        body.child(caption(
            "生效版本由你在目标浏览器中核对。文件准备完成后，仍可稍后继续。",
        ))
        .into_any_element()
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
                    .child(caption(format!("添加到 {}", self.target_label())))
                    .child(
                        h_flex()
                            .gap_4()
                            .child(badge("1 选择来源", self.add_step == 1))
                            .child(badge("2 预览版本", self.add_step == 2))
                            .child(badge("3 确认位置", self.add_step == 3)),
                    );
                match self.add_step {
                    1 => {
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
                    2 => {
                        if let Some(p) = &self.preview {
                            content = content
                                .child(
                                    div().p_4().rounded_lg().bg(rgb(0xeaf4ef)).child(
                                        v_flex()
                                            .gap_2()
                                            .child(heading(p.name().to_string()))
                                            .child(format!("扩展版本 {}", p.version()))
                                            .child(caption(p.source().display().to_string())),
                                    ),
                                )
                                .child(caption("CEPM 会复制到独立固定目录。来源移动不会影响安装。"))
                                .child(
                                    Button::new("use-preview")
                                        .primary()
                                        .label("使用此版本 →")
                                        .on_click(cx.listener(|v, _, _, cx| {
                                            v.add_step = 3;
                                            cx.notify();
                                        })),
                                );
                        }
                    }
                    _ => {
                        content = content
                            .child(heading(self.target_label()))
                            .child(caption(
                                self.target()
                                    .map(|p| p.profile_path().display().to_string())
                                    .unwrap_or_default(),
                            ))
                            .child(caption(
                                "只安装到当前配置。文件准备好后，在浏览器完成一次加载。",
                            ))
                            .child(
                                Button::new("install")
                                    .primary()
                                    .label("准备安装文件")
                                    .disabled(disabled)
                                    .on_click(cx.listener(|v, _, w, cx| v.install(w, cx))),
                            );
                    }
                }
                if self.add_step > 1 {
                    content = content.child(
                        Button::new("add-back")
                            .ghost()
                            .label("← 上一步")
                            .disabled(disabled)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.add_step -= 1;
                                cx.notify();
                            })),
                    );
                }
            }
            Panel::Detail | Panel::Guide => {
                if let Some(r) = self.current_record() {
                    let profile = self.target().unwrap();
                    let pending = r.verification == Verification::Pending;
                    let disabled = disabled || r.recovery_required;
                    if r.recovery_required {
                        content = content
                            .child(badge("恢复需要处理", true))
                            .child(caption(r.last_operation.clone()));
                    }
                    content = content
                        .child(heading(r.name.clone()))
                        .child(badge(profile.label(), false))
                        .child(
                            div()
                                .p_4()
                                .rounded_lg()
                                .bg(rgb(if pending { 0xfcf3e3 } else { 0xeaf4ef }))
                                .child(
                                    v_flex()
                                        .gap_1()
                                        .child(format!("磁盘安装版本 {}", r.installed_version))
                                        .child(caption(format!(
                                            "{}：{}",
                                            if pending {
                                                "历史确认（当前待核对）"
                                            } else {
                                                "生效版本"
                                            },
                                            if pending {
                                                r.historical_version.as_deref()
                                            } else {
                                                r.effective_version.as_deref()
                                            }
                                            .unwrap_or("尚未确认")
                                        ))),
                                ),
                        );
                    if self.panel == Panel::Detail {
                        content = content
                            .child(caption(format!("来源：{}", r.source.display())))
                            .child(
                                Button::new("detail-native")
                                    .label("在浏览器中管理 ↗")
                                    .disabled(!self.target_ready())
                                    .on_click(cx.listener(|v, _, _, cx| v.open_native(cx))),
                            )
                            .child(
                                Button::new("check-update")
                                    .label("检查来源与版本")
                                    .disabled(disabled || !self.target_ready())
                                    .on_click(cx.listener(|v, _, _, cx| v.check_update(cx))),
                            )
                            .child(
                                Button::new("choose-local-version")
                                    .label("选择其他本地版本…")
                                    .disabled(disabled || !self.target_ready())
                                    .on_click(
                                        cx.listener(|v, _, _, cx| v.choose_local_version(cx)),
                                    ),
                            )
                            .when(pending && !r.recovery_required, |v| {
                                v.child(
                                    Button::new("detail-guide")
                                        .disabled(disabled)
                                        .primary()
                                        .label("继续加载与核对")
                                        .on_click(cx.listener(|v, _, w, cx| {
                                            v.open_panel(Panel::Guide, w, cx)
                                        })),
                                )
                            })
                            .child(
                                Button::new("technical")
                                    .ghost()
                                    .label(if self.show_technical {
                                        "收起安装信息"
                                    } else {
                                        "安装位置与技术信息 ⌄"
                                    })
                                    .on_click(cx.listener(|v, _, _, cx| {
                                        v.show_technical = !v.show_technical;
                                        cx.notify();
                                    })),
                            )
                            .when(self.show_technical, |v| {
                                v.child(caption(format!("固定目录：{}", r.install_dir.display())))
                                    .child(caption(format!(
                                        "扩展 ID：{}",
                                        r.extension_id.as_deref().unwrap_or("首次加载后核对")
                                    )))
                                    .child(caption(format!(
                                        "配置目录：{}",
                                        profile.profile_path().display()
                                    )))
                            });
                    } else {
                        if pending && r.generation > 1 && !r.recovery_required {
                            content = content.child(
                                Button::new("load-failed")
                                    .label("加载失败，恢复上一份文件")
                                    .disabled(disabled)
                                    .on_click(cx.listener(|v, _, w, cx| {
                                        v.open_panel(Panel::Restore, w, cx)
                                    })),
                            );
                        }
                        let path = r.install_dir.display().to_string();
                        let url = profile.management_url().to_string();
                        content = content
                            .child(heading("1  打开目标配置的扩展管理页"))
                            .child(caption("请先确认窗口属于上方配置，再进行浏览器操作。"))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("guide-native")
                                            .primary()
                                            .label("打开浏览器扩展管理页 ↗")
                                            .disabled(!self.target_ready())
                                            .on_click(cx.listener(|v, _, _, cx| v.open_native(cx))),
                                    )
                                    .child(Button::new("copy-url").label("复制地址").on_click(
                                        move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                url.clone(),
                                            ))
                                        },
                                    )),
                            )
                            .child(heading(if r.extension_id.is_none() {
                                "2  开启开发者模式，加载已解压的扩展"
                            } else {
                                "2  找到此扩展并点击重新加载"
                            }))
                            .child(caption(if r.extension_id.is_none() {
                                "选择下方固定安装目录；后续更新继续使用它。"
                            } else {
                                "保存正在进行的工作。CEPM 不会关闭或重启浏览器。"
                            }))
                            .child(Button::new("copy-path").label("复制固定安装目录").on_click(
                                move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(path.clone()))
                                },
                            ))
                            .child(heading("3  核对实际显示的版本与身份"))
                            .child(caption(format!(
                                "应显示 {}；核对后记录为用户确认。",
                                r.installed_version
                            )))
                            .child(Input::new(&self.extension_id).disabled(disabled))
                            .child(Input::new(&self.version).disabled(disabled))
                            .child(
                                Checkbox::new("confirm-check")
                                    .label(format!("我已在 {} 完成操作并核对", profile.label()))
                                    .checked(self.checked)
                                    .disabled(disabled)
                                    .on_click(cx.listener(|v, value, _, cx| {
                                        v.checked = *value;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("confirm")
                                    .primary()
                                    .label("确认已生效")
                                    .disabled(disabled)
                                    .on_click(cx.listener(|v, _, _, cx| v.confirm(cx))),
                            );
                    }
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
                    content = content.child(heading(r.name)).child(badge(self.target_label(), false))
                        .child(heading(format!("{} → {}", r.installed_version, plan.version())))
                        .child(caption(format!("{kind} · {}", plan.source().display())))
                        .child(caption("保留固定安装目录和上一份文件。准备完成后，请在浏览器重新加载；未经核对不会显示已生效。"));
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
                            .label(format!("确认{kind}"))
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
            Panel::None => {}
        }
        content.into_any_element()
    }
}
impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = self.panel;
        let title = match panel {
            Panel::Bind => "浏览器配置",
            Panel::Legacy => "定位旧安装",
            Panel::Add => "添加扩展",
            Panel::Detail => "扩展详情",
            Panel::Guide => "完成浏览器加载与核对",
            Panel::Update => "确认版本变更",
            Panel::Restore => "加载失败与恢复",
            Panel::None => "",
        };
        div()
            .relative()
            .size_full()
            .text_color(rgb(0x20272d))
            .bg(rgb(0xf6f7f9))
            .child(
                h_flex()
                    .size_full()
                    .child(self.sidebar(cx))
                    .child(self.library(cx)),
            )
            .when(panel != Panel::None, |root| {
                root.child(
                    div()
                        .absolute()
                        .inset_0()
                        .size_full()
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
                                                .label("✕")
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
                                            .bg(rgb(if self.error { 0xfaeeec } else { 0xeaf4ef }))
                                            .child(self.status.clone()),
                                    )
                                })
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .p_4()
                                        .border_t_1()
                                        .border_color(rgb(BORDER))
                                        .child(caption("未完成的操作可以稍后继续"))
                                        .child(
                                            Button::new("later")
                                                .label("稍后完成")
                                                .disabled(self.busy)
                                                .on_click(cx.listener(|v, _, _, cx| {
                                                    v.panel = Panel::None;
                                                    cx.notify();
                                                })),
                                        ),
                                ),
                        ),
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
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        cx.activate(true);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("CEPM · 扩展管理".into()),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::centered(size(px(1120.), px(800.)), cx)),
            ..Default::default()
        };
        if let Err(e) =
            gpui_kit::open_window(options, cx, |w, cx| cx.new(|cx| Desktop::new(root, w, cx)))
        {
            eprintln!("无法打开 CEPM：{e}");
            cx.quit();
        }
    });
}
