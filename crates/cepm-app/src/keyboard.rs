//! Connect the component library's tab order and focus trap to CEPM's page/overlay lifecycle.
use gpui_kit::*;

pub(super) struct Keyboard {
    pub root: FocusHandle,
    pub profile: FocusHandle,
    pub overlay: FocusHandle,
    scope: u8,
    return_to: Option<FocusHandle>,
}
impl Keyboard {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        let root = cx.focus_handle().tab_stop(false);
        root.focus(window, cx);
        Self {
            root,
            profile: cx.focus_handle(),
            overlay: cx.focus_handle().tab_stop(false),
            scope: 0,
            return_to: None,
        }
    }
    /// 0 = page, 1 = Profile menu, 2 = task dialog. Buttons retain their own handles.
    pub fn sync(&mut self, scope: u8, window: &mut Window, cx: &mut App) {
        let root = self.root.clone();
        window.on_next_frame(move |window, cx| {
            // Keyboard activation can replace a focused list row with a detail page.
            // Re-seed the existing Root key context if that handle has left the tree.
            if window
                .focused(cx)
                .is_none_or(|focused| !root.contains(&focused, window))
            {
                root.focus(window, cx);
            }
        });
        if scope == self.scope {
            return;
        }
        if scope != 0 {
            if self.scope == 0 {
                self.return_to = window.focused(cx);
            }
            self.overlay.focus(window, cx);
        } else {
            let target = if self.scope == 1 {
                Some(self.profile.clone())
            } else {
                self.return_to.take()
            };
            let root = self.root.clone();
            let fallback = self.profile.clone();
            // Wait until the closing overlay is absent from the rendered focus tree.
            window.on_next_frame(move |window, cx| {
                if let Some(target) = target.filter(|h| root.contains(h, window) && *h != root) {
                    target.focus(window, cx);
                } else {
                    fallback.focus(window, cx);
                }
            });
        }
        self.scope = scope;
    }
}
