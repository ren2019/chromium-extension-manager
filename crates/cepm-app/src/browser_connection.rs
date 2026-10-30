use super::*;
use cepm_core::browser::{AutomaticRemovalConfirmation, BrowserCancellation, EdgeSession};

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

    pub(crate) fn uninstall_browser(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(record) = self.uninstall_record.clone() else {
            return;
        };
        let Some(profile) = self.target() else {
            return;
        };
        if record.target != profile.target() || record.extension_id.is_none() {
            return;
        }
        let Some(manager) = self.manager.clone() else {
            return;
        };
        self.cancel_browser_task();
        let cancel = BrowserCancellation::default();
        self.browser_task = Some(cancel.clone());
        let task_id = self.browser_task_id;
        let target = profile.target();
        let history_target = target.clone();
        self.busy = true;
        self.error = false;
        self.status = "正在连接 Edge，核对目标并卸载…".into();
        cx.notify();
        cx.spawn(async move |this,cx| {
            let outcome=cx.background_spawn(async move {
                let result=EdgeSession::connect(&profile,cancel).and_then(|session| {
                    manager.lock().map_err(|_|"安装记录不可用".to_string())?.uninstall_edge(&record.id,&record.target,record.generation,session,AutomaticRemovalConfirmation::Confirmed)
                });
                let data=manager.lock().map_err(|_|"安装记录不可用".to_string()).and_then(|m|Ok((m.list()?,m.operation_history(&history_target)?)));
                (result,data)
            }).await;
            this.update(cx,|view,cx| {
                let (mut result,data)=outcome;
                let current=view.browser_task_id==task_id && view.target().map(|p|p.target())==Some(target);
                match data {
                    Ok((records,history)) => { view.records=records; if current {view.operations=history;} },
                    Err(error) => { result=Err(error); }
                }
                if !current {cx.notify();return;}
                view.cancel_browser_task();
                match result {
                    Ok(())=>{view.panel=Panel::None;view.selected=None;view.uninstall_record=None;view.error=false;view.status="已卸载并清理这份安装。本次连接已断开；可在 Edge 授权页关闭远程调试。".into();}
                    Err(error)=>{view.error=true;view.status=format!("{error}。本次连接已结束；可手动移除或重试。浏览器调试开关仍需在 Edge 中关闭。");view.uninstall_record=view.current_record();}
                }
                cx.notify();
            }).ok();
        }).detach();
    }

    pub(crate) fn uninstall_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(record) = &self.uninstall_record else {
            return div().into_any_element();
        };
        let retry = record.removal.is_some();
        let manual = self.uninstall_manual || retry;
        let disabled = self.busy || !self.target_ready();
        let connecting = self.browser_task.is_some();
        let mut content=v_flex().gap_3()
            .child(heading(record.name.clone()))
            .child(caption(self.target_label()))
            .child(caption(if retry {"浏览器移除已经确认。继续清理这份安装的文件、备份及记录，无需再次移除浏览器扩展。"} else {"卸载可能删除扩展在浏览器中保存的数据。只移除这个配置中的安装，并清理对应的受管文件、备份与记录。"}));
        if !manual {
            content = content
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("uninstall-auto")
                                .primary()
                                .label("连接 Edge 并卸载")
                                .disabled(disabled)
                                .on_click(cx.listener(|v, _, _, cx| v.uninstall_browser(cx))),
                        )
                        .child(
                            Button::new("uninstall-manual")
                                .ghost()
                                .label("手动移除")
                                .disabled(disabled)
                                .on_click(cx.listener(|v, _, _, cx| {
                                    v.uninstall_manual = true;
                                    v.status.clear();
                                    v.error = false;
                                    cx.notify();
                                })),
                        ),
                )
                .when(self.error, |d| {
                    d.child(
                        h_flex().child(
                            Button::new("uninstall-authorize")
                                .ghost()
                                .label("打开连接授权")
                                .disabled(disabled)
                                .on_click(
                                    cx.listener(|v, _, _, cx| v.open_browser_authorization(cx)),
                                ),
                        ),
                    )
                });
        } else if !retry {
            content = content
                .child(caption("请先在浏览器中移除扩展，再回来确认清理。"))
                .child(
                    h_flex().child(
                        Button::new("uninstall-native")
                            .ghost()
                            .icon(gpui_kit::assets::IconName::ExternalLink)
                            .label("浏览器扩展管理")
                            .disabled(disabled)
                            .on_click(cx.listener(|v, _, _, cx| v.open_native(cx))),
                    ),
                )
                .child(
                    Checkbox::new("removed-confirmation")
                        .label("我已在此配置中移除这个扩展")
                        .checked(self.checked)
                        .disabled(disabled)
                        .on_click(cx.listener(|v, value, _, cx| {
                            v.checked = *value;
                            cx.notify();
                        })),
                )
                .when(self.error || self.show_technical, |d| {
                    d.child(caption(format!(
                        "也可在目标窗口手动打开 {}，移除后回来确认。",
                        self.target()
                            .map(|p| p.management_url())
                            .unwrap_or("edge://extensions")
                    )))
                    .child(
                        h_flex().child(
                            Button::new("uninstall-copy-url")
                                .label("复制管理页地址")
                                .on_click(cx.listener(|v, _, _, cx| v.copy_management_url(cx))),
                        ),
                    )
                });
        }
        content = content
            .when_some(
                record
                    .removal
                    .as_ref()
                    .and_then(|s| s.error())
                    .map(str::to_owned),
                |d, error| d.child(caption(error)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .justify_between()
                    .child(
                        Button::new("cancel-uninstall")
                            .ghost()
                            .label(if connecting { "取消连接" } else { "取消" })
                            .disabled(self.busy && !connecting)
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.cancel_browser_task();
                                v.panel = Panel::Detail;
                                v.uninstall_record = None;
                                v.checked = false;
                                cx.notify();
                            })),
                    )
                    .when(manual, |d| {
                        d.child(
                            Button::new("finish-uninstall")
                                .primary()
                                .label(if retry {
                                    "重试清理"
                                } else {
                                    "确认移除并清理"
                                })
                                .disabled(disabled || (!retry && !self.checked))
                                .on_click(cx.listener(|v, _, _, cx| v.finish_manual_removal(cx))),
                        )
                    }),
            );
        content.into_any_element()
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
                    .when(self.error || self.show_technical, |row| {
                        row.child(
                            Button::new("browser-authorize")
                                .ghost()
                                .label("打开连接授权")
                                .disabled(self.busy)
                                .on_click(
                                    cx.listener(|v, _, _, cx| v.open_browser_authorization(cx)),
                                ),
                        )
                    })
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
            .when(connecting || self.error, |column| {
                column.child(caption(
                    "本次结束后断开 APP 连接；Edge 的调试开关可在授权页关闭。",
                ))
            })
            .into_any_element()
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        self.cancel_browser_task();
    }
}
