use super::*;
use gpui_kit::assets::IconName as Lucide;
use std::{borrow::Cow, collections::HashMap};

pub(super) struct AppAssets;
impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        match path {
            "cepm/logo.png" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/logo.png")))),
            "cepm/chrome.png" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/chrome.png")))),
            "cepm/edge.png" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/edge.png")))),
            _ => gpui_kit::assets::AllAssets.load(path),
        }
    }
    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        gpui_kit::assets::AllAssets.list(path)
    }
}
pub(super) fn apply_theme(cx: &mut App) {
    cx.set_window_appearance(Some(WindowAppearance::Light));
    Theme::change(ThemeMode::Light, None, cx);
    Theme::update(cx, |theme| {
        theme.font_size = px(15.);
        theme.radius = px(7.);
        theme.radius_lg = px(11.);
        theme.colors.background = rgb(0xf4f4f6).into();
        theme.colors.foreground = rgb(0x1a1a1d).into();
        theme.colors.muted_foreground = rgb(MUTED).into();
        theme.colors.border = rgb(BORDER).into();
        theme.colors.secondary_active = rgb(0xe1e8fb).into();
        theme.colors.primary = rgb(GREEN).into();
        theme.colors.primary_foreground = rgb(0xffffff).into();
        theme.colors.button_primary = rgb(GREEN).into();
        theme.colors.button_primary_foreground = rgb(0xffffff).into();
        theme.colors.button_primary_hover = rgb(0x1d4ed8).into();
        theme.colors.button_primary_active = rgb(0x1e40af).into();
        theme.colors.button = rgb(0xffffff).into();
        theme.colors.button_foreground = rgb(0x1a1a1d).into();
        theme.colors.button_hover = rgb(0xe9e9ee).into();
        theme.colors.button_active = rgb(0xe1e1e8).into();
    });
}
pub(super) struct Artwork {
    pub description: Option<String>,
    pub image: Option<Arc<Image>>,
}
pub(super) fn load_artwork(
    manager: Option<&Arc<Mutex<Manager>>>,
    records: &[Installation],
) -> HashMap<String, Artwork> {
    let Some(manager) = manager.and_then(|m| m.lock().ok()) else {
        return HashMap::new();
    };
    records
        .iter()
        .filter_map(|r| {
            manager.presentation(&r.id).ok().map(|m| {
                (
                    r.id.clone(),
                    Artwork {
                        description: m.description,
                        image: m
                            .icon_png
                            .map(|bytes| Arc::new(Image::from_bytes(ImageFormat::Png, bytes))),
                    },
                )
            })
        })
        .collect()
}
fn icon(name: Lucide) -> Icon {
    Icon::new(name).size(px(16.))
}
fn brand(browser: &Browser) -> AnyElement {
    img(match browser {
        Browser::Chrome => "cepm/chrome.png",
        Browser::Edge => "cepm/edge.png",
    })
    .size(px(24.))
    .into_any_element()
}
fn logo() -> Div {
    div()
        .size(px(36.))
        .child(img("cepm/logo.png").size(px(36.)))
}

fn card() -> Div {
    v_flex()
        .p_4()
        .gap_3()
        .bg(rgb(0xffffff))
        .border_1()
        .border_color(rgb(BORDER))
        .rounded_xl()
        .min_w_0()
}
fn time(at: u64, short: bool) -> String {
    if at == 0 {
        return "未记录".into();
    }
    chrono::DateTime::from_timestamp(at as i64, 0)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format(if short {
                    "%m月%d日"
                } else {
                    "%Y-%m-%d %H:%M"
                })
                .to_string()
        })
        .unwrap_or_else(|| "未记录".into())
}
fn updated(r: &Installation, short: bool) -> String {
    if r.updated_at == 0 {
        "未记录".into()
    } else if r.generation == 1 && r.updated_at == r.installed_at {
        "尚未更新".into()
    } else {
        time(r.updated_at, short)
    }
}
fn field(label: &str, value: impl Into<SharedString>) -> Div {
    h_flex()
        .items_start()
        .gap_4()
        .min_w_0()
        .child(
            div()
                .w(px(100.))
                .flex_shrink_0()
                .text_sm()
                .text_color(rgb(MUTED))
                .child(label.to_owned()),
        )
        .child(div().flex_1().min_w_0().text_sm().child(value.into()))
}
fn status(r: &Installation) -> &'static str {
    if r.recovery_required {
        "恢复需要处理"
    } else if r.verification == Verification::Pending {
        if r.generation > 1 {
            "待核对"
        } else {
            "待加载"
        }
    } else if r.waiting_identification() {
        "已确认 · 待识别"
    } else {
        "已安装"
    }
}
impl Desktop {
    fn extension_icon(&self, r: &Installation) -> AnyElement {
        let artwork = self.artwork.get(&r.id).and_then(|m| m.image.clone());
        div()
            .size(px(42.))
            .flex_shrink_0()
            .rounded_lg()
            .bg(rgb(0xf4f4f8))
            .border_1()
            .border_color(rgb(BORDER))
            .flex()
            .items_center()
            .justify_center()
            .child(match artwork {
                Some(image) => img(image).size(px(40.)).rounded_lg().into_any_element(),
                None => Icon::new(Lucide::Puzzle)
                    .size(px(24.))
                    .text_color(rgb(MUTED))
                    .into_any_element(),
            })
            .into_any_element()
    }
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let records = self.scoped();
        let pending = records
            .iter()
            .filter(|r| {
                r.verification == Verification::Pending
                    || r.waiting_identification()
                    || r.recovery_required
            })
            .count();
        let mut side = v_flex()
            .w(px(220.))
            .h_full()
            .flex_shrink_0()
            .p_3()
            .gap_2()
            .bg(rgb(0xededf2))
            .border_r_1()
            .border_color(rgb(BORDER));
        let mut switch = h_flex().w_full().min_w_0().gap_2();
        if let Some(p) = self.target() {
            switch = switch.child(brand(&p.browser)).child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{:?}", p.browser)),
                    )
                    .child(caption(p.name).truncate()),
            );
        } else {
            switch = switch.child(caption("选择浏览器配置"));
        }
        side = side
            .child(
                Button::new("profile-switcher")
                    .accessibility_label(format!("切换配置：{}", self.target_label()))
                    .tooltip(self.target_label())
                    .min_w_0()
                    .w_full()
                    .h(px(58.))
                    .child(switch.child(icon(Lucide::ChevronsUpDown)))
                    .disabled(self.busy && self.browser_task.is_none())
                    .on_click(cx.listener(|v, _, _, cx| {
                        v.switcher = !v.switcher;
                        cx.notify();
                    })),
            )
            .child(div().pt_4().pb_1().px_2().child(caption("工作区")));
        for (page, key, name, glyph, count) in [
            (
                Page::Library,
                "library",
                "我的扩展",
                Lucide::Puzzle,
                Some(records.len()),
            ),
            (
                Page::Attention,
                "attention",
                "待处理",
                Lucide::CircleDot,
                Some(pending),
            ),
            (
                Page::History,
                "history",
                "操作记录",
                Lucide::RotateCcw,
                None,
            ),
        ] {
            let label = h_flex()
                .text_color(rgb(if self.page == page { GREEN } else { 0x1a1a1d }))
                .w_full()
                .gap_2()
                .child(icon(glyph))
                .child(div().flex_1().child(name))
                .when_some(count, |d, n| d.child(caption(n.to_string())));
            side = side.child(
                Button::new(key)
                    .accessibility_label(name)
                    .ghost()
                    .w_full()
                    .h(px(36.))
                    .child(label)
                    .selected(self.page == page)
                    .disabled(self.busy && self.browser_task.is_none())
                    .on_click(cx.listener(move |v, _, _, cx| {
                        v.cancel_browser_task();
                        v.page = page;
                        v.panel = Panel::None;
                        v.selected = None;
                        v.switcher = false;
                        cx.notify();
                    })),
            );
        }
        side.child(div().flex_1())
            .child(
                Button::new("native-manager")
                    .ghost()
                    .w_full()
                    .icon(Lucide::ExternalLink)
                    .label("浏览器扩展管理")
                    .disabled(!self.target_ready())
                    .on_click(cx.listener(|v, _, _, cx| v.open_native(cx))),
            )
            .child(
                Button::new("about")
                    .ghost()
                    .w_full()
                    .icon(Lucide::Info)
                    .label("关于")
                    .on_click(cx.listener(|v, _, _, cx| {
                        v.page = Page::About;
                        v.panel = Panel::None;
                        v.switcher = false;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
    pub(super) fn profile_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut menu = v_flex()
            .id("profile-menu")
            .w(px(310.))
            .max_h(px(440.))
            .overflow_y_scroll()
            .p_2()
            .gap_1()
            .bg(rgb(0xffffff))
            .rounded_xl()
            .border_1()
            .border_color(rgb(BORDER))
            .shadow_lg();
        if let Some(book) = &self.profiles {
            let mut profiles = book.profiles().to_vec();
            profiles.sort_by_key(|p| (!book.pinned_ids().contains(&p.id), p.label()));
            for p in profiles {
                let id = p.id.clone();
                let pin_id = p.id.clone();
                let pinned = book.pinned_ids().contains(&p.id);
                let active = book.selected().is_some_and(|t| t.id == p.id);
                let count = self
                    .records
                    .iter()
                    .filter(|r| r.target == p.target())
                    .count();
                let label = p.label();
                menu = menu.child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new(SharedString::from(format!("profile-{}", p.id)))
                                .ghost()
                                .flex_1()
                                .min_w_0()
                                .selected(active)
                                .tooltip(label.clone())
                                .accessibility_label(label.clone())
                                .child(
                                    h_flex()
                                        .w_full()
                                        .gap_2()
                                        .child(brand(&p.browser))
                                        .child(div().flex_1().truncate().child(label.clone()))
                                        .child(caption(count.to_string())),
                                )
                                .on_click(
                                    cx.listener(move |v, _, _, cx| v.select_profile(&id, cx)),
                                ),
                        )
                        .child(
                            Button::new(SharedString::from(format!("pin-{}", p.id)))
                                .ghost()
                                .icon(if pinned { Lucide::PinOff } else { Lucide::Pin })
                                .selected(pinned)
                                .tooltip(if pinned { "取消置顶" } else { "置顶" })
                                .accessibility_label(format!(
                                    "{} {label}",
                                    if pinned { "取消置顶" } else { "置顶" }
                                ))
                                .on_click(cx.listener(move |v, _, _, cx| v.pin(&pin_id, cx))),
                        ),
                );
            }
        }
        menu = menu.child(
            div()
                .border_t_1()
                .border_color(rgb(BORDER))
                .pt_2()
                .mt_1()
                .child(
                    Button::new("bind-profile")
                        .ghost()
                        .w_full()
                        .icon(Lucide::Plus)
                        .label("添加浏览器配置")
                        .on_click(cx.listener(|v, _, w, cx| {
                            v.switcher = false;
                            v.open_panel(Panel::Bind, w, cx)
                        })),
                ),
        );
        div()
            .absolute()
            .inset_0()
            .child(div().size_full().on_mouse_down(
                MouseButton::Left,
                cx.listener(|v, _, _, cx| {
                    v.switcher = false;
                    cx.notify();
                }),
            ))
            .child(div().absolute().left(px(12.)).top(px(108.)).child(menu))
            .into_any_element()
    }
    fn notice(&self) -> Div {
        div()
            .p_3()
            .rounded_lg()
            .text_sm()
            .bg(rgb(if self.error { 0xfff0ee } else { 0xeff3ff }))
            .text_color(rgb(if self.error { 0xb42318 } else { GREEN }))
            .child(self.status.clone())
    }
    pub(super) fn library(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.page == Page::About {
            return self.about(cx);
        }
        let disabled = self.busy || !self.target_ready() || self.manager.is_none();
        let records = self.scoped();
        let mut body = v_flex().flex_1().min_w_0().h_full().p_6().gap_5().child(
            h_flex()
                .justify_between()
                .gap_3()
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(26.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(match self.page {
                                    Page::Library => "我的扩展",
                                    Page::Attention => "待处理",
                                    _ => "操作记录",
                                }),
                        )
                        .child(caption(self.target_label())),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("check-all")
                                .ghost()
                                .icon(Lucide::RefreshCw)
                                .label("检查更新")
                                .disabled(disabled || records.is_empty())
                                .on_click(cx.listener(|v, _, _, cx| v.check_all(cx))),
                        )
                        .child(
                            Button::new("add-extension")
                                .primary()
                                .icon(Lucide::Plus)
                                .label("添加扩展")
                                .disabled(disabled)
                                .on_click(cx.listener(|v, _, w, cx| {
                                    v.add_step = 1;
                                    v.source_update_id = None;
                                    v.preview = None;
                                    v.open_panel(Panel::Add, w, cx)
                                })),
                        ),
                ),
        );
        if !self.status.is_empty() {
            body = body.child(self.notice());
        }
        if !self.target_ready() {
            return body
                .child(
                    card()
                        .child(heading(if self.target().is_none() {
                            "选择要维护的浏览器配置"
                        } else {
                            "找不到这个浏览器配置"
                        }))
                        .child(caption("安装记录仍保留。定位配置后即可继续。"))
                        .child(
                            Button::new("locate-profile")
                                .label("定位浏览器配置")
                                .on_click(
                                    cx.listener(|v, _, w, cx| v.open_panel(Panel::Bind, w, cx)),
                                ),
                        ),
                )
                .into_any_element();
        }
        if self.page == Page::History {
            let mut history = card().id("history-scroll").overflow_y_scroll().flex_1();
            for op in &self.operations {
                let name = op
                    .installation_id
                    .as_ref()
                    .and_then(|id| records.iter().find(|r| &r.id == id))
                    .map(|r| r.name.as_str())
                    .unwrap_or("来源操作");
                history = history.child(
                    v_flex()
                        .gap_1()
                        .pb_3()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .child(
                            h_flex()
                                .justify_between()
                                .child(format!(
                                    "{name} · {} · {}",
                                    op.stage.label(),
                                    op.status.label()
                                ))
                                .child(caption(time(op.at, false))),
                        )
                        .child(caption(op.reason.clone()))
                        .child(caption(op.next_step.clone())),
                );
            }
            if self.operations.is_empty() {
                history = history.child(caption("暂无操作记录"));
            }
            return body.child(history).into_any_element();
        }
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
                        .gap_2()
                        .child(caption(format!("{unbound} 份安装尚未定位")))
                        .child(Button::new("legacy").small().label("定位旧安装").on_click(
                            cx.listener(|v, _, w, cx| v.open_panel(Panel::Legacy, w, cx)),
                        )),
                );
        }
        let visible: Vec<_> = records
            .iter()
            .filter(|r| {
                self.page != Page::Attention
                    || r.verification == Verification::Pending
                    || r.waiting_identification()
                    || r.recovery_required
            })
            .collect();
        if visible.is_empty() {
            return body
                .child(
                    card()
                        .py_12()
                        .items_center()
                        .child(
                            Icon::new(Lucide::Puzzle)
                                .size(px(36.))
                                .text_color(rgb(MUTED)),
                        )
                        .child(heading(if records.is_empty() {
                            "添加你的第一个扩展"
                        } else {
                            "暂无待处理扩展"
                        }))
                        .when(records.is_empty(), |d| {
                            d.child(
                                Button::new("first-extension")
                                    .primary()
                                    .icon(Lucide::Plus)
                                    .label("添加扩展")
                                    .on_click(cx.listener(|v, _, w, cx| {
                                        v.add_step = 1;
                                        v.source_update_id = None;
                                        v.open_panel(Panel::Add, w, cx)
                                    })),
                            )
                        }),
                )
                .into_any_element();
        }
        let mut list = v_flex()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(BORDER))
            .rounded_xl()
            .overflow_hidden()
            .child(
                h_flex()
                    .px_4()
                    .py_3()
                    .justify_between()
                    .child(caption("已添加的扩展"))
                    .child(caption(format!("{} 个", visible.len()))),
            );
        for r in visible {
            let id = r.id.clone();
            let action_id = id.clone();
            let pending = r.verification == Verification::Pending;
            let recovery = r.recovery_required;
            let check = self.bulk_checks.iter().find(|c| c.id() == r.id);
            let update = check
                .and_then(|c| c.plan())
                .filter(|p| p.kind() == UpdateKind::Upgrade)
                .map(|p| p.version().to_owned());
            let mut info = v_flex()
                .gap_1()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(r.name.clone()),
                )
                .child(caption(format!("{} · {}", r.installed_version, status(r))))
                .child(caption(format!(
                    "安装于 {} · {}",
                    time(r.installed_at, true),
                    if r.generation > 1 {
                        format!("更新于 {}", updated(r, true))
                    } else {
                        updated(r, true)
                    }
                )));
            if let Some(version) = &update {
                info = info.child(
                    div()
                        .text_sm()
                        .text_color(rgb(GREEN))
                        .child(format!("有新版本 {version}")),
                );
            } else if let Some(check) = check {
                info = info.child(caption(if check.error().is_some() {
                    "检查失败"
                } else {
                    "没有新版本"
                }));
            }
            let mut row = h_flex()
                .gap_3()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(rgb(0xeaeaf0))
                .child(
                    Button::new(SharedString::from(format!("record-{id}")))
                        .ghost()
                        .flex_1()
                        .min_w_0()
                        .h_auto()
                        .p_0()
                        .tooltip(r.name.clone())
                        .accessibility_label(format!("查看 {} 详情", r.name))
                        .child(
                            h_flex()
                                .w_full()
                                .gap_3()
                                .child(self.extension_icon(r))
                                .child(info),
                        )
                        .on_click(cx.listener(move |v, _, w, cx| {
                            v.selected = Some(id.clone());
                            v.open_panel(Panel::Detail, w, cx)
                        })),
                );
            if pending || recovery || update.is_some() {
                row = row.child(
                    Button::new(SharedString::from(format!("next-{action_id}")))
                        .small()
                        .label(if recovery {
                            "处理异常"
                        } else if pending {
                            "继续加载"
                        } else {
                            "查看更新"
                        })
                        .disabled(disabled)
                        .on_click(cx.listener(move |v, _, w, cx| {
                            v.selected = Some(action_id.clone());
                            v.open_panel(
                                if pending && !recovery {
                                    Panel::Guide
                                } else {
                                    Panel::Detail
                                },
                                w,
                                cx,
                            )
                        })),
                );
            } else {
                row = row.child(icon(Lucide::ChevronRight).text_color(rgb(MUTED)));
            }
            list = list.child(row);
        }
        body.child(
            div()
                .id("extension-list")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(list),
        )
        .into_any_element()
    }
    pub(super) fn detail_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(r) = self.current_record() else {
            return self.library(cx);
        };
        let pending = r.verification == Verification::Pending;
        let disabled = self.busy || !self.target_ready() || r.recovery_required;
        let mut hero = h_flex().gap_3().child(self.extension_icon(&r)).child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .text_size(px(26.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(r.name.clone()),
                )
                .when_some(
                    self.artwork.get(&r.id).and_then(|m| m.description.clone()),
                    |d, description| d.child(caption(description)),
                ),
        );
        hero = hero.child(badge(status(&r), pending || r.recovery_required));
        let mut actions = h_flex().gap_2().flex_wrap();
        if pending && !r.recovery_required {
            actions = actions.child(
                Button::new("detail-guide")
                    .primary()
                    .icon(Lucide::ArrowRight)
                    .label(if r.generation > 1 {
                        "继续核对"
                    } else {
                        "继续加载"
                    })
                    .disabled(disabled)
                    .on_click(cx.listener(|v, _, w, cx| v.open_panel(Panel::Guide, w, cx))),
            );
        }
        if self.target().is_some_and(|p| p.browser == Browser::Edge) {
            actions = actions.child(
                Button::new("detail-browser-verify")
                    .label(if r.waiting_identification() {
                        "识别扩展"
                    } else {
                        "自动核对"
                    })
                    .disabled(disabled)
                    .on_click(cx.listener(|v, _, w, cx| v.open_panel(Panel::Guide, w, cx))),
            );
        }
        actions = actions
            .child(
                Button::new("check-update")
                    .icon(Lucide::RefreshCw)
                    .label("检查更新")
                    .disabled(disabled)
                    .on_click(cx.listener(|v, _, _, cx| v.check_update(cx))),
            )
            .child(
                Button::new("choose-version")
                    .icon(Lucide::GitBranch)
                    .label("选择版本")
                    .disabled(disabled)
                    .on_click(cx.listener(|v, _, w, cx| {
                        if v.current_record().is_some_and(|r| r.origin.is_some()) {
                            v.choose_remote_version(w, cx)
                        } else {
                            v.choose_local_version(cx)
                        }
                    })),
            )
            .child(
                Button::new("open-folder")
                    .icon(Lucide::Folder)
                    .label("打开文件夹")
                    .on_click(cx.listener(|v, _, _, cx| {
                        if let Some(r) = v.current_record() {
                            #[cfg(target_os = "macos")]
                            let result = std::process::Command::new("open")
                                .arg(&r.install_dir)
                                .spawn();
                            #[cfg(target_os = "windows")]
                            let result = std::process::Command::new("explorer")
                                .arg(&r.install_dir)
                                .spawn();
                            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                            let result = std::process::Command::new("xdg-open")
                                .arg(&r.install_dir)
                                .spawn();
                            v.message(
                                result
                                    .map(|mut child| {
                                        std::thread::spawn(move || {
                                            let _ = child.wait();
                                        });
                                    })
                                    .map_err(|e| format!("无法打开文件夹：{e}")),
                                "",
                                cx,
                            );
                        }
                    })),
            )
            .when(pending && r.generation > 1, |d| {
                d.child(
                    Button::new("restore-files")
                        .icon(Lucide::RotateCcw)
                        .label("恢复旧文件")
                        .disabled(disabled)
                        .on_click(cx.listener(|v, _, w, cx| v.open_panel(Panel::Restore, w, cx))),
                )
            });
        let check = self.bulk_checks.iter().find(|c| c.id() == r.id);
        let check_text = if let Some(c) = check {
            if let Some(p) = c.plan() {
                if p.kind() == UpdateKind::Upgrade {
                    format!("有新版本 {}", p.version())
                } else {
                    "没有新版本".into()
                }
            } else {
                "检查失败".into()
            }
        } else {
            "尚未检查".into()
        };
        let checked_at = self
            .operations
            .iter()
            .find(|o| {
                o.installation_id.as_deref() == Some(&r.id) && o.stage == OperationStage::Check
            })
            .map(|o| time(o.at, false))
            .unwrap_or_else(|| "尚未检查".into());
        let version = card()
            .flex_1()
            .flex_basis(px(0.))
            .child(div().font_weight(FontWeight::SEMIBOLD).child("版本与状态"))
            .child(field("文件版本", r.installed_version.clone()))
            .child(field(
                if pending {
                    "上次确认版本"
                } else {
                    "生效版本"
                },
                if pending {
                    r.historical_version.as_deref()
                } else {
                    r.effective_version.as_deref()
                }
                .unwrap_or("尚未确认")
                .to_owned(),
            ))
            .child(field("更新检查", check_text))
            .child(field(
                "确认方式",
                if pending {
                    "等待浏览器核对"
                } else if r.verification == Verification::Automatic {
                    "浏览器自动核对"
                } else {
                    "用户已确认"
                },
            ));
        let dates = card()
            .flex_1()
            .flex_basis(px(0.))
            .child(div().font_weight(FontWeight::SEMIBOLD).child("时间记录"))
            .child(field("安装时间", time(r.installed_at, false)))
            .child(field("最后更新时间", updated(&r, false)))
            .child(field("最近检查", checked_at));
        let related = self
            .manager
            .as_ref()
            .and_then(|m| m.lock().ok())
            .and_then(|m| m.related_installations(&r.id).ok())
            .unwrap_or_else(|| vec![r.clone()]);
        let mut compare = card().child(
            Button::new("cross-profiles")
                .accessibility_label(format!("各配置安装情况，{} 份受管安装", related.len()))
                .ghost()
                .w_full()
                .icon(if self.cross_profiles {
                    Lucide::ChevronDown
                } else {
                    Lucide::ChevronRight
                })
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .child("各配置安装情况")
                        .child(caption(format!("{} 份受管安装", related.len()))),
                )
                .on_click(cx.listener(|v, _, _, cx| {
                    v.cross_profiles = !v.cross_profiles;
                    cx.notify();
                })),
        );
        if self.cross_profiles {
            compare = compare.child(
                h_flex()
                    .gap_3()
                    .child(div().flex_1().child(caption("浏览器 / Profile")))
                    .child(div().w(px(110.)).child(caption("文件版本")))
                    .child(div().w(px(120.)).child(caption("已确认版本")))
                    .child(div().w(px(85.)).child(caption("状态"))),
            );
            for other in related {
                let current = other.id == r.id;
                let id = other.id.clone();
                let target = other.target.profile.clone();
                let label = self
                    .profiles
                    .as_ref()
                    .and_then(|b| b.get(&target).ok())
                    .map(|p| p.label())
                    .unwrap_or_else(|| "未定位配置".into());
                let historical = other.verification == Verification::Pending;
                compare = compare.child(
                    h_flex()
                        .gap_3()
                        .py_2()
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .child(
                            Button::new(SharedString::from(format!("related-{id}")))
                                .ghost()
                                .flex_1()
                                .min_w_0()
                                .disabled(current || self.busy)
                                .tooltip(label.clone())
                                .accessibility_label(label.clone())
                                .child(h_flex().gap_2().child(brand(&other.target.browser)).child(
                                    div().truncate().child(format!(
                                        "{label}{}",
                                        if current { " · 当前" } else { "" }
                                    )),
                                ))
                                .on_click(cx.listener(move |v, _, _, cx| {
                                    v.select_profile(&target, cx);
                                    if v.target().is_some_and(|p| p.id == target) {
                                        v.selected = Some(id.clone());
                                        v.panel = Panel::Detail;
                                        v.cross_profiles = false;
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .w(px(110.))
                                .text_sm()
                                .child(other.installed_version.clone()),
                        )
                        .child(div().w(px(120.)).text_sm().child(format!(
                                "{}{}",
                                if historical {
                                    other.historical_version.as_deref()
                                } else {
                                    other.effective_version.as_deref()
                                }
                                .unwrap_or("尚未确认"),
                                if historical && other.historical_version.is_some() {
                                    " · 上次"
                                } else {
                                    ""
                                }
                            )))
                        .child(div().w(px(85.)).text_sm().child(status(&other))),
                );
            }
        }
        let path = r.install_dir.display().to_string();

        let source = card()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("来源与安装位置"),
            )
            .child(field(
                "来源类型",
                match r.origin {
                    Some(RemoteOrigin::GitHubRelease { .. }) => "GitHub Release",
                    Some(RemoteOrigin::GitHubDirectory { .. }) => "GitHub 仓库目录",
                    None => "本地目录",
                },
            ))
            .child(field("来源", r.source.display().to_string()))
            .child(field("产物", origin_description(r.origin.as_ref())))
            .child(field("固定安装目录", path))
            .child(
                Button::new("detail-copy-path")
                    .ghost()
                    .icon(Lucide::Copy)
                    .label("复制完整路径")
                    .on_click(cx.listener(|v, _, _, cx| v.copy_installation_path(cx))),
            )
            .child(field(
                "扩展 ID",
                r.extension_id
                    .clone()
                    .unwrap_or_else(|| "等待浏览器识别".into()),
            ));
        let mut history = card().child(div().font_weight(FontWeight::SEMIBOLD).child("最近操作"));
        for op in self
            .operations
            .iter()
            .filter(|o| o.installation_id.as_deref() == Some(&r.id))
            .take(5)
        {
            history = history.child(
                h_flex()
                    .justify_between()
                    .gap_4()
                    .child(caption(format!(
                        "{} · {} · {}",
                        op.stage.label(),
                        op.status.label(),
                        op.reason
                    )))
                    .child(caption(time(op.at, false))),
            );
        }
        if !r.last_operation.is_empty() {
            history = history.child(caption(r.last_operation.clone()));
        }
        let mut body = v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .p_6()
            .gap_4()
            .child(
                h_flex()
                    .child(
                        Button::new("back-library")
                            .ghost()
                            .icon(Lucide::ArrowLeft)
                            .label("我的扩展")
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.panel = Panel::None;
                                v.selected = None;
                                cx.notify();
                            })),
                    )
                    .child(caption("扩展详情")),
            )
            .child(hero)
            .child(card().p_3().child(actions));
        if !self.status.is_empty() {
            body = body.child(self.notice());
        }
        body.child(
            v_flex()
                .id("detail-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .gap_3()
                .child(h_flex().items_start().gap_3().child(version).child(dates))
                .child(compare)
                .child(source)
                .child(history),
        )
        .into_any_element()
    }
    fn about(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex().flex_1().min_w_0().h_full().p_6().gap_5().child(heading("关于"))
            .child(card().gap_4().child(logo()).child(heading("Chromium Extension Package Manager")).child(caption(format!("版本 {}",env!("CARGO_PKG_VERSION"))))
                .child(div().child("管理非商店扩展的来源、安装与版本。"))
                .child(h_flex().gap_2().child(Button::new("project-home").icon(Lucide::ExternalLink).label("项目主页").on_click(|_,_,cx|cx.open_url("https://github.com/ren2019/chromium-extension-manager")))
                    .child(Button::new("project-issues").icon(Lucide::MessageCircle).label("问题反馈").on_click(|_,_,cx|cx.open_url("https://github.com/ren2019/chromium-extension-manager/issues")))))
            .child(Button::new("about-back").ghost().icon(Lucide::ArrowLeft).label("我的扩展").on_click(cx.listener(|v,_,_,cx|{v.page=Page::Library;cx.notify();}))).into_any_element()
    }
}
