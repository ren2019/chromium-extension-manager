use super::*;
use cepm_core::browser::{BrowserCancellation, EdgeSession};

impl Desktop {
    pub(crate) fn cancel_browser_task(&mut self) {
        if let Some(task) = self.browser_task.take() {
            task.cancel();
            self.busy = false;
        }
        self.browser_task_id = self.browser_task_id.wrapping_add(1);
    }

    pub(crate) fn open_browser_authorization(&mut self, cx: &mut Context<Self>) {
        let result = self
            .target()
            .ok_or("请先选择 Edge 配置".into())
            .and_then(|profile| {
                if profile.browser != Browser::Edge {
                    return Err("当前仅支持 Edge 自动核对".into());
                }
                let mut request = profile.management_request()?;
                if let Some(url) = request.arguments.last_mut() {
                    *url = "edge://inspect/#remote-debugging".into();
                }
                request.launch()
            });
        self.message(
            result,
            "在 Edge 中允许远程调试，并批准本次连接。完成后返回这里核对；也可以继续手动确认。",
            cx,
        );
    }

    pub(crate) fn connect_browser(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(profile) = self.target() else {
            return;
        };
        let Some(record) = self.current_record() else {
            return;
        };
        if profile.browser != Browser::Edge || record.target != profile.target() {
            return;
        }
        self.cancel_browser_task();
        let cancel = BrowserCancellation::default();
        self.browser_task = Some(cancel.clone());
        let task_id = self.browser_task_id;
        let expected = record.clone();
        let target = profile.target();
        self.busy = true;
        self.error = false;
        self.status = "正在连接 Edge 并核对扩展，请在浏览器允许本次连接…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                let mut session = EdgeSession::connect(&profile,cancel)?;
                session.verify(&record)
                // Session and its own version tab close before the result is committed.
            }).await;
            this.update(cx, |view,cx| {
                if view.browser_task_id != task_id || view.browser_task.as_ref().is_none_or(|c|c.is_cancelled()) || view.target().map(|p|p.target()) != Some(target.clone()) || view.current_record().is_none_or(|r|r.id != expected.id || r.generation != expected.generation) {
                    return;
                }
                view.browser_task = None;
                view.busy = false;
                let result = result.and_then(|evidence| {
                    let manager = view.manager.as_ref().ok_or("安装记录不可用")?;
                    let mut manager = manager.lock().map_err(|_| "安装记录不可用")?;
                    let confirmed = manager.confirm_browser(evidence, &target)?;
                    manager.record_operation(&target,Some(&confirmed.id),OperationStage::Confirm,Ok(()))?;
                    manager.list()
                });
                match result {
                    Ok(records) => {
                        view.records = records;
                        view.guide_record = None;
                        view.panel = Panel::Detail;
                        view.error = false;
                        view.status = "浏览器已自动核对身份、版本与启用状态。本次连接已断开；可在 Edge 授权页关闭远程调试。".into();
                        let _ = view.refresh_history();
                    }
                    Err(error) => { view.error = true; view.status = format!("{error}。本次 APP 连接已结束；浏览器的远程调试开关需在 Edge 中关闭。"); }
                }
                cx.notify();
            }).ok();
        }).detach();
    }

    pub(crate) fn browser_verification_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.target().is_none_or(|p| p.browser != Browser::Edge) {
            return div().into_any_element();
        }
        let connecting = self.browser_task.is_some();
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("browser-connect")
                            .label("连接 Edge 并核对")
                            .disabled(self.busy)
                            .on_click(cx.listener(|v, _, _, cx| v.connect_browser(cx))),
                    )
                    .child(
                        Button::new("browser-authorize")
                            .ghost()
                            .label("打开连接授权")
                            .disabled(self.busy)
                            .on_click(cx.listener(|v, _, _, cx| v.open_browser_authorization(cx))),
                    )
                    .when(connecting, |v| {
                        v.child(
                            Button::new("browser-cancel")
                                .ghost()
                                .label("取消连接")
                                .on_click(cx.listener(|v, _, _, cx| {
                                    v.cancel_browser_task();
                                    v.status =
                                        "已取消，本次连接正在关闭；可以继续手动确认。".into();
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(caption(
                "只核对本次扩展。完成或取消后断开 APP 连接；Edge 的调试开关可在授权页关闭。",
            ))
            .into_any_element()
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        self.cancel_browser_task();
    }
}
