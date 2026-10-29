// Throwaway feasibility probe, not the CEPM implementation.
use gpui_kit::component::{
    button::*,
    input::{Input, InputEvent, InputState},
    progress::Progress,
    *,
};
use gpui_kit::*;

struct Probe {
    input: Entity<InputState>,
    text: String,
    directory: String,
    progress: f32,
    _subscription: Subscription,
}

impl Probe {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("输入中文 / type here"));
        let subscription = cx.subscribe_in(&input, window, |this, state, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.text = state.read(cx).value().to_string();
                println!("INPUT {}", this.text);
                cx.notify();
            }
        });
        Self {
            input,
            text: String::new(),
            directory: "No folder selected".into(),
            progress: 0.,
            _subscription: subscription,
        }
    }
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_6()
            .gap_3()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child("CEPM · GPUI feasibility prototype")
            .child("验证目录选择、中文输入、列表和进度。No extension files are modified.")
            .child(Input::new(&self.input))
            .child(format!("Input echo: {}", self.text))
            .child(
                Button::new("folder")
                    .label("选择目录 / Choose folder")
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("CEPM prototype: choose a directory")
                            .pick_folder()
                        {
                            this.directory = path.display().to_string();
                            println!("FOLDER {}", this.directory);
                            cx.notify();
                        }
                    })),
            )
            .child(self.directory.clone())
            .child(
                Button::new("progress")
                    .primary()
                    .label("Advance simulated progress")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.progress = if this.progress >= 100. {
                            0.
                        } else {
                            this.progress + 25.
                        };
                        println!("PROGRESS {}", this.progress);
                        cx.notify();
                    })),
            )
            .child(
                Progress::new("progress-bar")
                    .value(self.progress)
                    .accessibility_label("Simulated progress"),
            )
            .child(format!("Progress: {}% (simulation only)", self.progress))
            .child(
                div()
                    .id("fixture-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .children((0..100).map(|i| {
                        div()
                            .p_2()
                            .child(format!("Fixture extension {:03} · v1.0.0 · 待验证", i + 1))
                    })),
            )
    }
}

fn main() {
    if std::env::args().any(|a| a == "--version") {
        println!(
            "CEPM throwaway GPUI probe 0.0.0; gpui-kit 0.8.1; {}",
            std::env::consts::OS
        );
        return;
    }
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        cx.activate(true);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(840.), px(680.)), cx)),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Probe::new(window, cx))
        })
        .expect("open prototype window");
        println!("WINDOW_CREATED");
    });
}
