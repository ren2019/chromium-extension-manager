use cepm_core::{Browser, Installation, Manager, Preview, Target, Verification};
use gpui_kit::component::{
    button::*,
    input::{Input, InputState},
    *,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

struct Desktop {
    manager: Option<Arc<Mutex<Manager>>>,
    records: Vec<Installation>,
    selected: Option<String>,
    preview: Option<Preview>,
    browser: Browser,
    profile: Entity<InputState>,
    extension_id: Entity<InputState>,
    version: Entity<InputState>,
    status: String,
    busy: bool,
}

impl Desktop {
    fn new(
        manager: cepm_core::Result<Manager>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (manager, records, status) = match manager {
            Ok(manager) => match manager.list() {
                Ok(records) => (
                    Some(Arc::new(Mutex::new(manager))),
                    records,
                    "选择本地扩展目录开始安装".into(),
                ),
                Err(error) => (None, Vec::new(), error),
            },
            Err(error) => (None, Vec::new(), error),
        };
        Self {
            manager,
            records,
            selected: None,
            preview: None,
            browser: Browser::Chrome,
            profile: cx.new(|cx| {
                InputState::new(window, cx).placeholder("目标配置名称或路径，例如：工作配置")
            }),
            extension_id: cx
                .new(|cx| InputState::new(window, cx).placeholder("从目标浏览器复制 32 位扩展 ID")),
            version: cx
                .new(|cx| InputState::new(window, cx).placeholder("输入浏览器中实际显示的版本")),
            status,
            busy: false,
        }
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
        self.status = label.into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut manager = manager
                        .lock()
                        .map_err(|_| "操作异常，请重启应用后检查安装记录".to_string())?;
                    operation(&mut manager)
                })
                .await;
            this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(value) => apply(view, value),
                    Err(error) => view.status = error,
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
        self.status = "请选择包含 manifest.json 的扩展目录".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let selected = rfd::AsyncFileDialog::new()
                .set_title("选择完整的本地扩展目录")
                .pick_folder()
                .await;
            this.update(cx, |view, cx| {
                view.busy = false;
                if let Some(folder) = selected {
                    let path = folder.path().to_path_buf();
                    view.work(
                        "正在复制并验证预览…",
                        cx,
                        move |manager| manager.preview_local(&path),
                        |view, preview| {
                            view.status = "已验证预览。确认名称、版本和目标浏览器配置后安装".into();
                            view.preview = Some(preview);
                        },
                    );
                } else {
                    view.status = "已取消选择".into();
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let profile = self.profile.read(cx).value().to_string();
        if profile.trim().is_empty() {
            self.status = "请先填写目标浏览器配置，避免在错误配置加载".into();
            cx.notify();
            return;
        }
        let Some(preview) = self.preview.take() else {
            return;
        };
        self.extension_id
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.version
            .update(cx, |state, cx| state.set_value("", window, cx));
        let target = Target {
            browser: self.browser.clone(),
            profile,
        };
        self.work(
            "正在部署到固定安装目录…",
            cx,
            move |manager| {
                let record = manager.install_local(preview, target)?;
                Ok((record, manager.list()?))
            },
            |view, (record, records)| {
                view.selected = Some(record.id);
                view.records = records;
                view.status = "文件已部署，等待在浏览器中加载并核对版本".into();
            },
        );
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let extension_id = self.extension_id.read(cx).value().trim().to_owned();
        let version = self.version.read(cx).value().trim().to_owned();
        self.work(
            "正在保存用户确认…",
            cx,
            move |manager| {
                manager.confirm(&id, &extension_id, &version)?;
                manager.list()
            },
            |view, records| {
                view.records = records;
                view.status = "已记录用户确认；此结果不是自动验证".into();
            },
        );
    }
}

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.busy || self.manager.is_none();
        let selected = self
            .records
            .iter()
            .find(|r| Some(&r.id) == self.selected.as_ref())
            .cloned();
        v_flex().size_full().p_6().gap_3().bg(cx.theme().background).text_color(cx.theme().foreground)
            .child(div().text_xl().child("CEPM · 扩展包管理器"))
            .child("本地目录安装 · 首次加载由你在浏览器中确认")
            .child(Button::new("choose").label("选择本地扩展目录").disabled(disabled)
                .on_click(cx.listener(|view, _, _, cx| view.choose_source(cx))))
            .child(h_flex().gap_2()
                .child(Button::new("chrome").label(if self.browser == Browser::Chrome { "✓ Chrome" } else { "Chrome" }).disabled(disabled)
                    .on_click(cx.listener(|view, _, _, cx| { view.browser = Browser::Chrome; cx.notify(); })))
                .child(Button::new("edge").label(if self.browser == Browser::Edge { "✓ Edge" } else { "Edge" }).disabled(disabled)
                    .on_click(cx.listener(|view, _, _, cx| { view.browser = Browser::Edge; cx.notify(); }))))
            .child(Input::new(&self.profile).disabled(disabled))
            .when_some(self.preview.as_ref(), |layout, preview| layout
                .child(format!("准备安装：{} · {}", preview.name(), preview.version()))
                .child(format!("来源目录：{}", preview.source().display()))
                .child(Button::new("install").primary().label("确认安装到固定目录").disabled(disabled)
                    .on_click(cx.listener(|view, _, window, cx| view.install(window, cx)))))
            .child(div().p_2().border_1().rounded_md().child(self.status.clone()))
            .child(div().id("installations").flex_1().overflow_y_scroll().children(self.records.iter().map(|record| {
                let id = record.id.clone();
                let state = match record.verification { Verification::Pending => "待加载 / 待验证", Verification::UserConfirmed => "用户已确认" };
                v_flex().p_2().gap_1()
                    .child(Button::new(SharedString::from(format!("record-{}", record.id)))
                        .label(format!("{} · 文件 {} · {}", record.name, record.installed_version, state)).disabled(disabled)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.selected = Some(id.clone());
                            view.extension_id.update(cx, |state, cx| state.set_value("", window, cx));
                            view.version.update(cx, |state, cx| state.set_value("", window, cx));
                            cx.notify();
                        })))
                    .child(format!("{:?} · {}", record.target.browser, record.target.profile))
            })))
            .when_some(selected, |layout, record| {
                let path = record.install_dir.display().to_string();
                let url = match record.target.browser { Browser::Chrome => "chrome://extensions", Browser::Edge => "edge://extensions" };
                layout.child(v_flex().gap_2().p_3().border_1().rounded_md()
                    .child(format!("{} / {} · {}", record.name, record.target.profile, url))
                    .child(format!("来源目录：{}", record.source.display()))
                    .child(format!("固定安装目录：{path}"))
                    .child(Button::new("copy-path").label("复制安装目录").on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(path.clone()))))
                    .child("在目标配置打开扩展管理页 → 开启开发者模式 → 加载未打包扩展 → 选择此目录。")
                    .child("核对该扩展的 ID 和版本后填写下方信息。应用不会关闭或重启浏览器。")
                    .child(format!("磁盘安装版本：{}  ·  生效版本：{}", record.installed_version, record.effective_version.as_deref().unwrap_or("尚未确认")))
                    .child(Input::new(&self.extension_id).disabled(disabled))
                    .child(Input::new(&self.version).disabled(disabled))
                    .child(Button::new("confirm").label("我已在目标配置核对 ID 和版本").disabled(disabled)
                        .on_click(cx.listener(|view, _, _, cx| view.confirm(cx)))))
            })
    }
}

fn data_root() -> cepm_core::Result<PathBuf> {
    if let Some(path) = std::env::var_os("CEPM_DATA_DIR") {
        return Ok(path.into());
    }
    directories::ProjectDirs::from("dev", "CEPM", "CEPM")
        .map(|dirs| dirs.data_local_dir().to_path_buf())
        .ok_or("无法定位应用数据目录".into())
}

fn main() {
    let manager = data_root().and_then(|root| Manager::open(&root));
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
                title: Some("CEPM · 扩展包管理器".into()),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::centered(size(px(960.), px(880.)), cx)),
            ..Default::default()
        };
        if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Desktop::new(manager, window, cx))
        }) {
            eprintln!("无法打开 CEPM 窗口：{error}");
            cx.quit();
        }
    });
}
