use super::*;

enum HoverEffectiveFocus<'a> {
    Untrusted,
    PendingPane(&'a str),
    Snapshot,
}

impl ClientShellState {
    pub(super) fn record_binding(
        &mut self,
        binding: crate::input::KeybindMatch,
        outcome: &mut ClientShellInput,
    ) {
        match binding {
            crate::input::KeybindMatch::Action(crate::input::KeybindAction::Detach) => {
                outcome.detach = true;
            }
            crate::input::KeybindMatch::Action(crate::input::KeybindAction::ToggleSidebar) => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
                self.sidebar_collapsed_manual = true;
                self.invalidate_pane_surface();
                outcome.repaint = true;
                outcome.resize = true;
                self.persist_chrome_preferences(outcome);
            }
            crate::input::KeybindMatch::Action(action) => {
                if matches!(
                    action,
                    crate::input::KeybindAction::NewWorktree
                        | crate::input::KeybindAction::OpenWorktree
                        | crate::input::KeybindAction::RemoveWorktree
                ) {
                    self.begin_worktree_action(action, outcome);
                    return;
                }
                if action == crate::input::KeybindAction::OpenNavigator {
                    self.open_navigator_overlay();
                    outcome.repaint = true;
                    return;
                }
                if action == crate::input::KeybindAction::Help {
                    self.overlay = Some(ClientShellOverlay::Help(ClientHelpOverlay {
                        query: String::new(),
                        search_focused: false,
                        scroll: 0,
                    }));
                    outcome.repaint = true;
                    return;
                }
                if action == crate::input::KeybindAction::Settings {
                    self.open_settings_overlay();
                    outcome.repaint = true;
                    return;
                }
                if action == crate::input::KeybindAction::OpenNotificationTarget {
                    self.focus_visible_notification(outcome);
                    return;
                }
                if action == crate::input::KeybindAction::ReloadConfig {
                    self.push_endpoint_method_with_kind(
                        crate::api::schema::Method::ServerReloadConfig(
                            crate::api::schema::EmptyParams::default(),
                        ),
                        PendingEndpointKind::ReloadConfig,
                        outcome,
                    );
                    self.reload_client_config();
                    self.invalidate_pane_surface();
                    outcome.repaint = true;
                    outcome.resize = true;
                    return;
                }
                if action == crate::input::KeybindAction::NewWorkspace {
                    if self.config.prompt_new_workspace_name {
                        self.open_new_workspace_overlay();
                    } else {
                        self.push_endpoint_method(
                            crate::api::schema::Method::WorkspaceCreate(
                                crate::api::schema::WorkspaceCreateParams {
                                    source_workspace_id: self.workspace_action_id(),
                                    cwd: None,
                                    focus: true,
                                    label: None,
                                    env: Default::default(),
                                },
                            ),
                            outcome,
                        );
                    }
                    outcome.repaint = true;
                    return;
                }
                if action == crate::input::KeybindAction::RenameWorkspace {
                    self.open_rename_workspace_overlay();
                    outcome.repaint = true;
                    return;
                }
                if action == crate::input::KeybindAction::CloseWorkspace {
                    if let Some(workspace_id) = self.workspace_action_id() {
                        if self.config.confirm_close {
                            self.open_confirm_close_overlay(workspace_id);
                        } else {
                            self.push_endpoint_method(
                                crate::api::schema::Method::WorkspaceClose(
                                    crate::api::schema::WorkspaceCloseParams {
                                        workspace_id,
                                        close_group: true,
                                    },
                                ),
                                outcome,
                            );
                        }
                    }
                    outcome.repaint = true;
                    return;
                }
                if action == crate::input::KeybindAction::NewTab && self.config.prompt_new_tab_name
                {
                    self.open_new_tab_overlay();
                    outcome.repaint = true;
                    return;
                }
                if action == crate::input::KeybindAction::RenameTab {
                    self.open_rename_tab_overlay();
                    outcome.repaint = true;
                    return;
                }
                if action == crate::input::KeybindAction::RenamePane {
                    self.open_rename_pane_overlay();
                    outcome.repaint = true;
                    return;
                }
                if action == crate::input::KeybindAction::WorkspacePicker {
                    self.mobile_switcher_scroll = 0;
                    self.reveal_mobile_workspace = false;
                    self.mode = ClientShellMode::Navigate;
                    self.navigate_workspace_id = self
                        .snapshot
                        .as_deref()
                        .and_then(|snapshot| snapshot.focused_workspace_id.clone());
                    outcome.repaint = true;
                    return;
                }
                if action == crate::input::KeybindAction::EnterResizeMode {
                    self.mode = ClientShellMode::Resize;
                    outcome.repaint = true;
                    return;
                }
                if action == crate::input::KeybindAction::CopyMode {
                    if self.enter_copy_mode(outcome) {
                        outcome.repaint = true;
                    }
                    return;
                }
                if self.handle_endpoint_navigation(action, outcome) {
                    return;
                }
                if let Some(method) = self.endpoint_method_for_action(action) {
                    self.push_endpoint_method(method, outcome);
                    return;
                }
                outcome.actions.push(ClientShellAction::Keybind(action));
            }
            crate::input::KeybindMatch::Command(command) => {
                let action = command.action.into();
                let resolved_labels = command.bindings.labels();
                let command_id = self.snapshot.as_deref().and_then(|snapshot| {
                    if let Some(candidate) = snapshot.commands.iter().find(|candidate| {
                        candidate.command_id == command.command && candidate.action == action
                    }) {
                        return Some(candidate.command_id.clone());
                    }
                    let mut candidates = snapshot.commands.iter().filter(|candidate| {
                        candidate.action == action
                            && !resolved_labels.is_empty()
                            && resolved_labels
                                .iter()
                                .all(|label| candidate.binding_labels.contains(label))
                    });
                    let candidate = candidates.next()?;
                    candidates
                        .next()
                        .is_none()
                        .then(|| candidate.command_id.clone())
                });
                let Some(command_id) = command_id else {
                    self.endpoint_error = Some(
                        "custom command is not available on this endpoint; reload configuration"
                            .to_owned(),
                    );
                    outcome.repaint = true;
                    return;
                };
                let Some(snapshot) = self.snapshot.as_deref() else {
                    return;
                };
                let selection = (action == crate::protocol::ClientShellCommandAction::PluginAction)
                    .then(|| {
                        let selection = self.selection.as_ref()?;
                        if !selection.is_visible() {
                            return None;
                        }
                        if snapshot.focused_pane_id.as_deref() != Some(selection.pane_id.as_str()) {
                            return None;
                        }
                        let content_revision = self
                            .pane_surface
                            .as_ref()?
                            .panes
                            .iter()
                            .find(|pane| pane.pane_id == selection.pane_id)?
                            .content_revision;
                        let (anchor, cursor) = selection.ordered_cells();
                        Some(crate::api::schema::PaneSelectionReadParams {
                            pane_id: selection.pane_id.clone(),
                            anchor: crate::api::schema::PaneTextPoint {
                                row: anchor.0,
                                col: anchor.1,
                            },
                            cursor: crate::api::schema::PaneTextPoint {
                                row: cursor.0,
                                col: cursor.1,
                            },
                            content_revision: Some(content_revision),
                        })
                    })
                    .flatten();
                let params = crate::api::schema::CommandInvokeParams {
                    command_id,
                    workspace_id: snapshot.focused_workspace_id.clone(),
                    tab_id: snapshot.focused_tab_id.clone(),
                    pane_id: snapshot.focused_pane_id.clone(),
                    selection,
                };
                if action == crate::protocol::ClientShellCommandAction::Popup {
                    self.popup_pending = true;
                    self.popup_pending_deadline = None;
                    if !self.push_endpoint_method_with_kind(
                        crate::api::schema::Method::CommandInvoke(params),
                        PendingEndpointKind::PopupCommand,
                        outcome,
                    ) {
                        self.popup_pending = false;
                    }
                } else {
                    self.push_endpoint_method(
                        crate::api::schema::Method::CommandInvoke(params),
                        outcome,
                    );
                }
            }
        }
    }

    pub(super) fn request_selection_copy(&mut self, outcome: &mut ClientShellInput, live: bool) {
        let Some(selection) = self.selection.as_ref() else {
            return;
        };
        let pane_id = selection.pane_id.clone();
        let content_revision = self
            .pane_surface
            .as_ref()
            .and_then(|surface| surface.panes.iter().find(|pane| pane.pane_id == pane_id))
            .map(|pane| pane.content_revision)
            // Read a manual mouse selection atomically from the live terminal. Output
            // between the displayed frame and this request must not reject the copy.
            .filter(|_| !live);
        let (anchor, cursor) = selection.ordered_cells();
        self.push_endpoint_method_with_kind(
            crate::api::schema::Method::PaneSelectionRead(
                crate::api::schema::PaneSelectionReadParams {
                    pane_id,
                    anchor: crate::api::schema::PaneTextPoint {
                        row: anchor.0,
                        col: anchor.1,
                    },
                    cursor: crate::api::schema::PaneTextPoint {
                        row: cursor.0,
                        col: cursor.1,
                    },
                    content_revision,
                },
            ),
            PendingEndpointKind::SelectionCopy,
            outcome,
        );
    }

    pub(super) fn request_word_selection(
        &mut self,
        hit: &PaneHit,
        viewport_row: u16,
        col: u16,
        outcome: &mut ClientShellInput,
    ) {
        let absolute_row = crate::selection::absolute_row_for_viewport(viewport_row, hit.scroll);
        let content_revision = self
            .pane_surface
            .as_ref()
            .and_then(|surface| {
                surface
                    .panes
                    .iter()
                    .find(|pane| pane.pane_id == hit.pane_id)
            })
            .map(|pane| pane.content_revision);
        self.word_selection_generation = self.word_selection_generation.saturating_add(1);
        let generation = self.word_selection_generation;
        self.pending_word_selection = Some(generation);
        if !self.push_endpoint_method_with_kind(
            crate::api::schema::Method::PaneSelectionRead(
                crate::api::schema::PaneSelectionReadParams {
                    pane_id: hit.pane_id.clone(),
                    anchor: crate::api::schema::PaneTextPoint {
                        row: absolute_row,
                        col: 0,
                    },
                    cursor: crate::api::schema::PaneTextPoint {
                        row: absolute_row,
                        col: hit.inner_rect.width.saturating_sub(1),
                    },
                    content_revision,
                },
            ),
            PendingEndpointKind::WordSelection {
                pane_id: hit.pane_id.clone(),
                absolute_row,
                col,
                generation,
            },
            outcome,
        ) {
            self.pending_word_selection = None;
        }
    }

    pub(super) fn push_endpoint_method(
        &mut self,
        method: crate::api::schema::Method,
        outcome: &mut ClientShellInput,
    ) {
        self.push_endpoint_method_with_kind(method, PendingEndpointKind::Generic, outcome);
    }

    fn push_endpoint_notice(
        &mut self,
        kind: ClientEndpointNoticeKind,
        code: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> bool {
        let key = ClientEndpointNoticeKey {
            boot_id: self
                .snapshot
                .as_deref()
                .map(|snapshot| snapshot.boot_id.clone())
                .unwrap_or_else(|| "disconnected".to_owned()),
            kind,
            code: code.into(),
        };
        let body = body.into();
        if kind == ClientEndpointNoticeKind::Rejected {
            if self
                .visible_endpoint_notice
                .as_ref()
                .is_some_and(|notice| notice.key == key && notice.body == body)
            {
                return false;
            }
        } else if !self.endpoint_notice_seen.insert(key.clone()) {
            return false;
        }
        let duration_seconds = if kind == ClientEndpointNoticeKind::Rejected {
            3
        } else {
            8
        };
        self.visible_endpoint_notice = Some(ClientVisibleEndpointNotice {
            key,
            title: title.into(),
            body,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(duration_seconds),
        });
        true
    }

    pub(super) fn push_endpoint_method_with_kind(
        &mut self,
        method: crate::api::schema::Method,
        kind: PendingEndpointKind,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if !self.endpoint_is_online(&self.active_endpoint_id) {
            let label = self.active_endpoint_label().to_owned();
            outcome.repaint |= self.receive_endpoint_unavailable(format!("{label} is not ready"));
            return false;
        }
        let method_name = crate::api::api_method_name(&method).to_owned();
        if !self.supports_endpoint_method(&method) {
            outcome.repaint |= self.push_endpoint_notice(
                ClientEndpointNoticeKind::Unsupported,
                method_name.clone(),
                "Action unavailable",
                format!(
                    "This server does not support {method_name} yet. Update and restart it to enable this action."
                ),
            );
            return false;
        }
        let Some(snapshot) = self.snapshot.as_deref() else {
            return false;
        };
        let boot_id = snapshot.boot_id.clone();
        let confirmation_workspace_id = match &method {
            crate::api::schema::Method::TabClose(target) => snapshot
                .tabs
                .iter()
                .find(|tab| tab.tab_id == target.tab_id)
                .map(|tab| tab.workspace_id.clone()),
            crate::api::schema::Method::PaneClose(target) => snapshot
                .panes
                .iter()
                .find(|pane| pane.pane_id == target.pane_id)
                .map(|pane| pane.workspace_id.clone()),
            _ => None,
        };
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request_id = format!("client-shell:{request_id}");
        let record_generic_focus = matches!(kind, PendingEndpointKind::Generic);
        self.pending_requests.insert(
            request_id.clone(),
            PendingEndpointRequest {
                boot_id: boot_id.clone(),
                method_name,
                confirmation_workspace_id,
                kind,
            },
        );
        if record_generic_focus {
            self.record_generic_focus_intent(&method, &request_id);
        }
        outcome.actions.push(ClientShellAction::Endpoint {
            endpoint_id: self.active_endpoint_id.clone(),
            boot_id,
            request: Box::new(crate::api::schema::Request {
                id: request_id,
                method,
            }),
        });
        true
    }

    pub(super) fn request_hover_pane_focus(
        &mut self,
        pane_index: usize,
        outcome: &mut ClientShellInput,
    ) {
        if !self.endpoint_is_online(&self.active_endpoint_id)
            || !self.supports_endpoint_method_name("pane.focus")
        {
            return;
        }
        let pane_id = &self.hits.panes[pane_index].pane_id;
        if self.hover_slot.is_none()
            && matches!(self.hover_effective_focus(), HoverEffectiveFocus::Snapshot)
            && self
                .snapshot
                .as_deref()
                .and_then(|snapshot| snapshot.focused_pane_id.as_deref())
                == Some(pane_id.as_str())
        {
            self.hover_pane_focus = None;
            return;
        }
        // Retain ownership only for a changed pointer intent, not each motion cell.
        // Still consult ordered focus effects below: the snapshot may precede a
        // manual focus request even when it already names this pane.
        if let Some(hover) = self
            .hover_pane_focus
            .as_mut()
            .filter(|hover| hover.pane_id == *pane_id)
        {
            hover.generation = self.next_focus_generation;
        } else {
            self.hover_pane_focus = Some(ClientHoverPaneFocus {
                pane_id: pane_id.clone(),
                generation: self.next_focus_generation,
            });
        }
        if self.hover_slot.is_some() || self.should_skip_hover(pane_id) {
            return;
        }
        self.emit_hover_pane_focus(pane_id.clone(), outcome);
    }

    fn emit_hover_pane_focus(&mut self, pane_id: String, outcome: &mut ClientShellInput) {
        let method = crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
            pane_id: pane_id.clone(),
        });
        let generation = self.next_focus_generation;
        self.next_focus_generation = self.next_focus_generation.saturating_add(1);
        let action_index = outcome.actions.len();
        if !self.push_endpoint_method_with_kind(
            method,
            PendingEndpointKind::HoverPaneFocus {
                pane_id: pane_id.clone(),
                generation,
            },
            outcome,
        ) {
            return;
        }
        let Some(request_id) = outcome.actions.get(action_index).and_then(|action| {
            let ClientShellAction::Endpoint { request, .. } = action else {
                return None;
            };
            Some(request.id.clone())
        }) else {
            return;
        };
        self.hover_pane_focus = Some(ClientHoverPaneFocus {
            pane_id: pane_id.clone(),
            generation,
        });
        self.hover_slot = Some(ClientHoverSlot {
            endpoint_id: self.active_endpoint_id.clone(),
            request_id,
            pane_id,
            generation,
            snapshot_seen: false,
            invalidated: false,
        });
    }

    pub(crate) fn flush_coalesced_hover_pane_focus(&mut self, outcome: &mut ClientShellInput) {
        if !(self.config.focus_pane_on_hover && self.config.mouse_capture) {
            return;
        }
        let Some(hover) = self.hover_pane_focus.as_ref() else {
            return;
        };
        if self.hover_slot.is_some() || self.should_skip_hover(&hover.pane_id) {
            return;
        }
        if !self.endpoint_is_online(&self.active_endpoint_id)
            || !self.supports_endpoint_method_name("pane.focus")
        {
            return;
        }
        self.emit_hover_pane_focus(hover.pane_id.clone(), outcome);
    }

    fn should_skip_hover(&self, pane_id: &str) -> bool {
        match self.hover_effective_focus() {
            HoverEffectiveFocus::Untrusted => false,
            HoverEffectiveFocus::PendingPane(id) => id == pane_id,
            HoverEffectiveFocus::Snapshot => {
                self.snapshot
                    .as_deref()
                    .and_then(|snapshot| snapshot.focused_pane_id.as_deref())
                    == Some(pane_id)
            }
        }
    }

    fn hover_effective_focus(&self) -> HoverEffectiveFocus<'_> {
        let hover = self.hover_awaiting_snapshot.as_ref();
        let slot = self.hover_slot.as_ref();
        let mut latest = hover.map(|hover| (hover.generation, hover.pane_id.as_str()));
        if let Some(slot) = slot {
            if latest.is_none_or(|(generation, _)| slot.generation > generation) {
                latest = Some((slot.generation, slot.pane_id.as_str()));
            }
        }
        if let Some(manual) = self.pending_manual_focuses.back() {
            if latest.is_none_or(|(generation, _)| manual.generation > generation) {
                return match &manual.target {
                    PendingManualFocusTarget::Pane(id) => HoverEffectiveFocus::PendingPane(id),
                    PendingManualFocusTarget::Tab(_)
                    | PendingManualFocusTarget::Workspace(_)
                    | PendingManualFocusTarget::PaneDirection => HoverEffectiveFocus::Untrusted,
                };
            }
        }
        latest.map_or(HoverEffectiveFocus::Snapshot, |(_, pane_id)| {
            HoverEffectiveFocus::PendingPane(pane_id)
        })
    }

    fn record_generic_focus_intent(
        &mut self,
        method: &crate::api::schema::Method,
        request_id: &str,
    ) {
        let target = match method {
            crate::api::schema::Method::PaneFocus(target) => {
                PendingManualFocusTarget::Pane(target.pane_id.clone())
            }
            crate::api::schema::Method::TabFocus(target) => {
                PendingManualFocusTarget::Tab(target.tab_id.clone())
            }
            crate::api::schema::Method::WorkspaceFocus(target) => {
                PendingManualFocusTarget::Workspace(target.workspace_id.clone())
            }
            crate::api::schema::Method::PaneFocusDirection(_) => {
                PendingManualFocusTarget::PaneDirection
            }
            _ => return,
        };
        self.hover_pane_focus = None;
        self.queue_hover_slot_cancel();
        let generation = self.next_focus_generation;
        self.next_focus_generation = self.next_focus_generation.saturating_add(1);
        self.pending_manual_focuses.push_back(PendingManualFocus {
            request_id: request_id.to_owned(),
            generation,
            target,
            awaiting_snapshot: false,
            snapshot_seen: false,
        });
    }

    fn complete_manual_focus(&mut self, request_id: &str, success: bool) {
        let Some(index) = self
            .pending_manual_focuses
            .iter()
            .position(|pending| pending.request_id == request_id)
        else {
            return;
        };
        if !success {
            self.pending_manual_focuses.remove(index);
            return;
        }
        let generation = self.pending_manual_focuses[index].generation;
        self.pending_manual_focuses[index].awaiting_snapshot = true;
        // A later successful request supersedes earlier focus effects, even if their
        // snapshots were skipped. Keep newer requests until their own result arrives.
        self.pending_manual_focuses.retain(|pending| {
            pending.generation > generation
                || (pending.generation == generation && !pending.snapshot_seen)
        });
        if self
            .hover_awaiting_snapshot
            .as_ref()
            .is_some_and(|hover| hover.generation < generation)
        {
            self.hover_awaiting_snapshot = None;
        }
    }

    fn complete_hover_focus_success(
        &mut self,
        pane_id: String,
        generation: u64,
        snapshot_seen: bool,
    ) {
        self.pending_manual_focuses
            .retain(|manual| manual.generation > generation);
        if !snapshot_seen
            && self
                .hover_awaiting_snapshot
                .as_ref()
                .is_none_or(|hover| hover.generation < generation)
        {
            self.hover_awaiting_snapshot = Some(ClientHoverPaneFocus {
                pane_id,
                generation,
            });
        }
    }

    pub(super) fn clear_pending_hover_pane_focuses(&mut self) {
        self.hover_pane_focus = None;
        self.hover_awaiting_snapshot = None;
        self.pending_requests.retain(|_, pending| {
            !matches!(&pending.kind, PendingEndpointKind::HoverPaneFocus { .. })
        });
        self.queue_hover_slot_cancel();
    }

    fn queue_hover_slot_cancel(&mut self) {
        let Some(slot) = &self.hover_slot else {
            return;
        };
        if !self
            .cancelled_unsent_request_ids
            .iter()
            .any(|(_, request_id)| request_id == &slot.request_id)
        {
            self.cancelled_unsent_request_ids
                .push((slot.endpoint_id.clone(), slot.request_id.clone()));
        }
    }

    pub(crate) fn take_cancelled_unsent_endpoint_ids(&mut self) -> Vec<(ClientEndpointId, String)> {
        std::mem::take(&mut self.cancelled_unsent_request_ids)
    }

    pub(crate) fn cancel_unsent_hover_request(&mut self, request_id: &str) {
        self.release_hover_slot(request_id);
        self.pending_requests.remove(request_id);
    }

    pub(crate) fn release_hover_slot(&mut self, request_id: &str) {
        if self
            .hover_slot
            .as_ref()
            .is_some_and(|slot| slot.request_id == request_id)
        {
            self.hover_slot = None;
        }
    }

    pub(super) fn reconcile_pending_hover_pane_focuses(
        &mut self,
        snapshot: &ClientShellSnapshot,
        workspace_changed: bool,
    ) {
        if workspace_changed {
            if let Some(slot) = self.hover_slot.as_mut() {
                slot.invalidated = true;
            }
            self.hover_pane_focus = None;
            self.hover_awaiting_snapshot = None;
            self.pending_manual_focuses.clear();
            self.pending_requests.retain(|_, pending| {
                !matches!(&pending.kind, PendingEndpointKind::HoverPaneFocus { .. })
            });
            self.queue_hover_slot_cancel();
            return;
        }
        if let Some(focused) = snapshot.focused_pane_id.as_deref() {
            // A matching snapshot must not erase a newer pointer intent when an
            // interposed manual request (or a different hover) can still move focus.
            let settled = match self.hover_effective_focus() {
                HoverEffectiveFocus::Snapshot => true,
                HoverEffectiveFocus::PendingPane(pane_id) => pane_id == focused,
                HoverEffectiveFocus::Untrusted => false,
            };
            if settled
                && self
                    .hover_pane_focus
                    .as_ref()
                    .is_some_and(|hover| hover.pane_id == focused)
            {
                self.hover_pane_focus = None;
            }
            if let Some(slot) = self.hover_slot.as_mut() {
                slot.snapshot_seen |= slot.pane_id == focused;
            }
            if self
                .hover_awaiting_snapshot
                .as_ref()
                .is_some_and(|hover| hover.pane_id == focused)
            {
                self.hover_awaiting_snapshot = None;
            }
        }
        let pane_focus_changed = self
            .snapshot
            .as_deref()
            .is_some_and(|current| current.focused_pane_id != snapshot.focused_pane_id);
        for pending in &mut self.pending_manual_focuses {
            pending.snapshot_seen |= match &pending.target {
                PendingManualFocusTarget::Pane(pane_id) => {
                    snapshot.focused_pane_id.as_deref() == Some(pane_id.as_str())
                }
                PendingManualFocusTarget::Tab(tab_id) => {
                    snapshot.focused_tab_id.as_deref() == Some(tab_id.as_str())
                }
                PendingManualFocusTarget::Workspace(workspace_id) => {
                    snapshot.focused_workspace_id.as_deref() == Some(workspace_id.as_str())
                }
                PendingManualFocusTarget::PaneDirection => {
                    pending.awaiting_snapshot || pane_focus_changed
                }
            };
        }
        self.pending_manual_focuses
            .retain(|pending| !(pending.awaiting_snapshot && pending.snapshot_seen));
        let pane_missing =
            |pane_id: &str| !snapshot.panes.iter().any(|pane| pane.pane_id == pane_id);
        if self
            .hover_pane_focus
            .as_ref()
            .is_some_and(|hover| pane_missing(&hover.pane_id))
        {
            self.hover_pane_focus = None;
        }
        if self
            .hover_awaiting_snapshot
            .as_ref()
            .is_some_and(|hover| pane_missing(&hover.pane_id))
        {
            self.hover_awaiting_snapshot = None;
        }
        if let Some(slot) = self
            .hover_slot
            .as_mut()
            .filter(|slot| pane_missing(&slot.pane_id))
        {
            slot.invalidated = true;
            self.pending_requests.retain(|_, pending| {
                !matches!(&pending.kind, PendingEndpointKind::HoverPaneFocus { .. })
            });
            self.queue_hover_slot_cancel();
        }
        // Snapshot acknowledgement is not transport completion: retain request
        // metadata and the slot until its result or confirmed unsent cancellation.
    }

    pub(crate) fn receive_endpoint_error(&mut self, message: String) -> bool {
        self.push_endpoint_notice(
            ClientEndpointNoticeKind::Rejected,
            "paste_rejected",
            "Paste rejected",
            message,
        )
    }

    pub(crate) fn receive_endpoint_unavailable(&mut self, message: String) -> bool {
        self.push_endpoint_notice(
            ClientEndpointNoticeKind::Unavailable,
            message.clone(),
            "Endpoint unavailable",
            message,
        )
    }

    pub(crate) fn focus_endpoint_target(
        &mut self,
        target: ClientEndpointFocusTarget,
    ) -> Vec<ClientShellAction> {
        let method = match target {
            ClientEndpointFocusTarget::Workspace(workspace_id) => {
                crate::api::schema::Method::WorkspaceFocus(crate::api::schema::WorkspaceTarget {
                    workspace_id,
                })
            }
            ClientEndpointFocusTarget::Tab(tab_id) => {
                crate::api::schema::Method::TabFocus(crate::api::schema::TabTarget { tab_id })
            }
            ClientEndpointFocusTarget::Pane(pane_id) => {
                crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget { pane_id })
            }
        };
        let mut outcome = ClientShellInput::default();
        self.push_endpoint_method(method, &mut outcome);
        outcome.actions
    }

    pub(crate) fn cancel_endpoint_request(&mut self, request_id: &str) -> bool {
        self.release_hover_slot(request_id);
        // Transport cancellation must not create replacement work on an unavailable lane.
        self.hover_pane_focus = None;
        let Some(pending) = self.pending_requests.get(request_id) else {
            return false;
        };
        let boot_id = pending.boot_id.clone();
        let (repaint, actions) = self.handle_endpoint_result(
            &boot_id,
            request_id,
            Err(ClientShellEndpointError {
                code: Some("endpoint_cancelled".into()),
                message: "This server action was interrupted. Check its state before retrying."
                    .into(),
            }),
        );
        debug_assert!(
            actions.is_empty(),
            "cancellation must not start another action"
        );
        repaint
    }

    pub(crate) fn handle_endpoint_result(
        &mut self,
        boot_id: &str,
        request_id: &str,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        let completed_slot = if self
            .hover_slot
            .as_ref()
            .is_some_and(|slot| slot.request_id == request_id)
        {
            self.hover_slot.take()
        } else {
            None
        };
        let Some(pending) = self.pending_requests.remove(request_id) else {
            let mut outcome = ClientShellInput::default();
            if let Some(slot) = completed_slot {
                // Config disable never cancels a sent RPC: its effect still matters
                // after re-enable, unless snapshot cleanup invalidated the target.
                if !slot.invalidated
                    && slot.endpoint_id == self.active_endpoint_id
                    && self
                        .snapshot
                        .as_deref()
                        .is_some_and(|snapshot| snapshot.boot_id == boot_id)
                    && result.is_ok()
                {
                    self.complete_hover_focus_success(
                        slot.pane_id,
                        slot.generation,
                        slot.snapshot_seen,
                    );
                } else if result.is_err()
                    && self
                        .hover_pane_focus
                        .as_ref()
                        .is_some_and(|hover| hover.pane_id == slot.pane_id)
                {
                    self.hover_pane_focus = None;
                }
                self.flush_coalesced_hover_pane_focus(&mut outcome);
            }
            return (false, outcome.actions);
        };
        if pending.boot_id != boot_id
            || self
                .snapshot
                .as_deref()
                .is_none_or(|snapshot| snapshot.boot_id != boot_id)
        {
            return (false, Vec::new());
        }
        if result.is_ok() {
            let timeout_key = ClientEndpointNoticeKey {
                boot_id: boot_id.to_owned(),
                kind: ClientEndpointNoticeKind::Timeout,
                code: pending.method_name.clone(),
            };
            self.endpoint_notice_seen.remove(&timeout_key);
        }
        if let Err(error) = &result {
            let code = error.code.as_deref().unwrap_or("invalid_response");
            let suppress_notice =
                matches!(&pending.kind, PendingEndpointKind::HoverPaneFocus { .. });
            if !suppress_notice
                && !matches!(
                    code,
                    "confirmation_required" | "stale_content" | "stale_target"
                )
            {
                let (kind, notice_code, title, body) = match code {
                    "endpoint_timeout" => (
                        ClientEndpointNoticeKind::Timeout,
                        pending.method_name.clone(),
                        "Server timed out",
                        format!("This server did not respond to {}.", pending.method_name),
                    ),
                    "endpoint_cancelled" => (
                        ClientEndpointNoticeKind::Unavailable,
                        "cancelled".to_owned(),
                        "Action interrupted",
                        error.message.clone(),
                    ),
                    "server_unavailable" => (
                        ClientEndpointNoticeKind::Unavailable,
                        "server".to_owned(),
                        "Server unavailable",
                        error.message.clone(),
                    ),
                    _ => (
                        ClientEndpointNoticeKind::Rejected,
                        format!("{}:{code}", pending.method_name),
                        "Action rejected",
                        error.message.clone(),
                    ),
                };
                self.push_endpoint_notice(kind, notice_code, title, body);
            }
        }
        match pending.kind {
            PendingEndpointKind::HoverPaneFocus {
                pane_id,
                generation,
            } => {
                let mut outcome = ClientShellInput::default();
                if result.is_err() {
                    if self
                        .hover_awaiting_snapshot
                        .as_ref()
                        .is_some_and(|hover| hover.generation == generation)
                    {
                        self.hover_awaiting_snapshot = None;
                    }
                    if self
                        .hover_pane_focus
                        .as_ref()
                        .is_some_and(|hover| hover.pane_id == pane_id)
                        && self
                            .pending_manual_focuses
                            .back()
                            .is_none_or(|manual| manual.generation <= generation)
                    {
                        self.hover_pane_focus = None;
                    } else {
                        self.flush_coalesced_hover_pane_focus(&mut outcome);
                    }
                } else {
                    if completed_slot.as_ref().is_none_or(|slot| !slot.invalidated) {
                        self.complete_hover_focus_success(
                            pane_id,
                            generation,
                            completed_slot
                                .as_ref()
                                .is_some_and(|slot| slot.snapshot_seen),
                        );
                    }
                    self.flush_coalesced_hover_pane_focus(&mut outcome);
                }
                return (false, outcome.actions);
            }
            PendingEndpointKind::Generic => {
                self.complete_manual_focus(request_id, result.is_ok());
            }
            PendingEndpointKind::ProductAnnouncementDismiss { version, id } => {
                return match result {
                    Ok(_) => (false, Vec::new()),
                    Err(_) => {
                        let key = (version.clone(), id.clone());
                        if self.dismissed_product_announcement.as_ref() == Some(&key) {
                            self.dismissed_product_announcement = None;
                        }
                        if self.overlay.is_none() {
                            if let Some(announcement) = self
                                .snapshot
                                .as_deref()
                                .and_then(|snapshot| snapshot.product_announcement.as_ref())
                                .filter(|announcement| {
                                    announcement.version == version && announcement.id == id
                                })
                            {
                                self.overlay = Some(ClientShellOverlay::ProductAnnouncement(
                                    product_announcement_state(announcement),
                                ));
                            }
                        }
                        (true, Vec::new())
                    }
                };
            }
            PendingEndpointKind::ReleaseNotesDismiss => {
                return match result {
                    Ok(_) => (false, Vec::new()),
                    Err(_) => {
                        if self.overlay.is_none() {
                            if let Some(notes) = self
                                .snapshot
                                .as_deref()
                                .and_then(|snapshot| snapshot.release_notes.as_ref())
                            {
                                self.overlay = Some(ClientShellOverlay::ReleaseNotes(
                                    release_notes_state(notes),
                                ));
                            }
                        }
                        (true, Vec::new())
                    }
                };
            }
            PendingEndpointKind::PopupCommand => {
                return match result {
                    Ok(_) => {
                        self.popup_pending_deadline =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(1));
                        (false, Vec::new())
                    }
                    Err(_) => {
                        self.popup_pending = false;
                        self.popup_pending_deadline = None;
                        (true, Vec::new())
                    }
                };
            }
            PendingEndpointKind::PaneScroll { pane_id, serial } => {
                let mut outcome = ClientShellInput::default();
                let repaint = self.complete_pane_scroll(pane_id, serial, result, &mut outcome);
                return (repaint, outcome.actions);
            }
            PendingEndpointKind::SelectionCopy => {
                return match result {
                    Ok(crate::api::schema::ResponseResult::PaneSelection { text, .. })
                        if !text.is_empty() =>
                    {
                        let repaint = self.show_copy_feedback(std::time::Instant::now());
                        (
                            repaint,
                            vec![ClientShellAction::ClipboardWrite(text.into_bytes())],
                        )
                    }
                    Ok(crate::api::schema::ResponseResult::PaneSelection { .. }) => {
                        (false, Vec::new())
                    }
                    Ok(_) => {
                        self.endpoint_error =
                            Some("endpoint returned an unexpected selection result".to_owned());
                        (true, Vec::new())
                    }
                    Err(_) => (true, Vec::new()),
                };
            }
            PendingEndpointKind::WordSelection {
                pane_id,
                absolute_row,
                col,
                generation,
            } => {
                if self.pending_word_selection != Some(generation)
                    || self.snapshot.as_deref().is_none_or(|snapshot| {
                        !snapshot.panes.iter().any(|pane| pane.pane_id == pane_id)
                    })
                {
                    return (false, Vec::new());
                }
                self.pending_word_selection = None;
                let row_text = match result {
                    Ok(crate::api::schema::ResponseResult::PaneSelection {
                        pane_id: returned_pane_id,
                        text,
                    }) if returned_pane_id == pane_id => text,
                    Ok(crate::api::schema::ResponseResult::PaneSelection { .. }) => {
                        return (false, Vec::new())
                    }
                    Ok(_) => {
                        self.endpoint_error = Some(
                            "endpoint returned an unexpected word-selection result".to_owned(),
                        );
                        return (true, Vec::new());
                    }
                    Err(_) => return (true, Vec::new()),
                };
                let Some((start_col, end_col)) =
                    crate::app::actions::word_bounds_at_column(&row_text, col)
                else {
                    self.selection = None;
                    return (true, Vec::new());
                };
                let mut selection = crate::selection::Selection::absolute_range(
                    pane_id,
                    (absolute_row, start_col),
                    (absolute_row, end_col),
                );
                if !selection.finish() {
                    return (false, Vec::new());
                }
                self.selection = Some(selection);
                self.selection_autoscroll = None;
                self.selection_autoscroll_deadline = None;
                if !self.config.copy_on_select {
                    return (true, Vec::new());
                }
                self.selection_highlight_clear_deadline =
                    Some(std::time::Instant::now() + std::time::Duration::from_millis(500));
                let mut outcome = ClientShellInput::default();
                self.request_selection_copy(&mut outcome, false);
                return (true, outcome.actions);
            }
            PendingEndpointKind::PaneLinkActivate {
                pane_id,
                inner_rect,
                fallback_events,
            } => {
                let completed_before_release = !fallback_events.iter().any(|event| {
                    event.kind
                        == crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left)
                });
                let replay = (self.mode == ClientShellMode::Terminal
                    && self.overlay.is_none()
                    && self
                        .hits
                        .panes
                        .iter()
                        .any(|hit| hit.pane_id == pane_id && hit.inner_rect == inner_rect))
                .then_some(fallback_events);
                if replay.is_none() {
                    self.url_click_consumes_until_up = completed_before_release;
                }
                let replay_action = |events: Option<Vec<crossterm::event::MouseEvent>>| {
                    events
                        .map(ClientShellAction::ReplayMouse)
                        .into_iter()
                        .collect()
                };
                return match result {
                    Ok(crate::api::schema::ResponseResult::PaneLinkActivated {
                        handled: true,
                        ..
                    }) => {
                        self.url_click_consumes_until_up = completed_before_release;
                        (false, Vec::new())
                    }
                    Ok(crate::api::schema::ResponseResult::PaneLinkActivated {
                        url: Some(url),
                        handled: false,
                    }) if crate::app::actions::safe_web_url(&url).is_some() => {
                        self.url_click_consumes_until_up = completed_before_release;
                        (false, vec![ClientShellAction::OpenSafeWebUrl(url)])
                    }
                    Ok(crate::api::schema::ResponseResult::PaneLinkActivated { .. }) => {
                        (false, replay_action(replay))
                    }
                    Ok(_) => {
                        self.endpoint_error =
                            Some("endpoint returned an unexpected link result".to_owned());
                        (true, replay_action(replay))
                    }
                    Err(error)
                        if matches!(
                            error.code.as_deref(),
                            Some("stale_content" | "stale_target" | "endpoint_cancelled")
                        ) =>
                    {
                        self.url_click_consumes_until_up = completed_before_release;
                        (false, Vec::new())
                    }
                    Err(_) => (true, replay_action(replay)),
                };
            }
            PendingEndpointKind::CopyMotion {
                pane_id,
                origin,
                session_generation,
            } => {
                let mut outcome = ClientShellInput::default();
                let (repaint, continue_queue) = match result {
                    Ok(crate::api::schema::ResponseResult::PaneCopyMotion {
                        pane_id: returned_pane_id,
                        cursor,
                        content_revision,
                    }) if returned_pane_id == pane_id => (
                        self.apply_copy_motion_target(
                            &pane_id,
                            origin,
                            cursor,
                            content_revision,
                            &mut outcome,
                        ),
                        true,
                    ),
                    Ok(crate::api::schema::ResponseResult::PaneCopyMotion { .. }) => (false, false),
                    Ok(_) => {
                        self.endpoint_error =
                            Some("endpoint returned an unexpected copy-motion result".to_owned());
                        (true, false)
                    }
                    Err(_) => (true, false),
                };
                self.complete_copy_operation(session_generation, continue_queue, &mut outcome);
                return (repaint || outcome.repaint, outcome.actions);
            }
            PendingEndpointKind::CopySearch {
                pane_id,
                origin,
                query,
                direction,
                repeat,
                generation,
                session_generation,
            } => {
                let mut outcome = ClientShellInput::default();
                let (repaint, continue_queue) = match result {
                    Ok(crate::api::schema::ResponseResult::PaneCopySearch {
                        pane_id: returned_pane_id,
                        content_revision,
                        matches,
                        total,
                        current,
                        current_global,
                    }) if returned_pane_id == pane_id => {
                        let repaint = self.apply_copy_search_result(
                            &pane_id,
                            origin,
                            query,
                            direction,
                            repeat,
                            generation,
                            ClientCopySearchResult {
                                content_revision,
                                matches,
                                total,
                                current: current.and_then(|index| usize::try_from(index).ok()),
                                current_global,
                            },
                            &mut outcome,
                        );
                        if !repaint {
                            self.cancel_deferred_copy_after_search(generation);
                        }
                        (repaint, repaint)
                    }
                    Ok(crate::api::schema::ResponseResult::PaneCopySearch { .. }) => {
                        self.cancel_deferred_copy_after_search(generation);
                        (false, false)
                    }
                    Ok(_) => {
                        self.cancel_deferred_copy_after_search(generation);
                        self.endpoint_error =
                            Some("endpoint returned an unexpected copy-search result".to_owned());
                        (true, false)
                    }
                    Err(_) => {
                        self.cancel_deferred_copy_after_search(generation);
                        (true, false)
                    }
                };
                self.complete_copy_operation(session_generation, continue_queue, &mut outcome);
                return (repaint || outcome.repaint, outcome.actions);
            }
            PendingEndpointKind::ReloadConfig => {
                let repaint = match result {
                    Ok(crate::api::schema::ResponseResult::ConfigReload { .. }) => false,
                    Ok(_) => {
                        self.endpoint_error =
                            Some("endpoint returned an unexpected config reload result".to_owned());
                        true
                    }
                    Err(_) => true,
                };
                return (repaint, Vec::new());
            }
            kind @ (PendingEndpointKind::IntegrationList
            | PendingEndpointKind::IntegrationInstall) => {
                return self.handle_settings_endpoint_result(kind, result);
            }
            kind => {
                return (
                    self.handle_worktree_endpoint_result(kind, result),
                    Vec::new(),
                );
            }
        }
        let repaint = match result {
            Ok(_) => false,
            Err(error)
                if self.config.confirm_close
                    && error.code.as_deref() == Some("confirmation_required")
                    && pending.confirmation_workspace_id.is_some() =>
            {
                if let Some(workspace_id) = pending.confirmation_workspace_id {
                    self.open_confirm_close_overlay(workspace_id);
                }
                true
            }
            Err(_) => true,
        };
        let mut outcome = ClientShellInput::default();
        self.flush_coalesced_hover_pane_focus(&mut outcome);
        (repaint, outcome.actions)
    }

    pub(super) fn endpoint_method_for_action(
        &mut self,
        action: crate::input::KeybindAction,
    ) -> Option<crate::api::schema::Method> {
        use crate::api::schema::{
            Method, PaneDirection, PaneFocusDirectionParams, PaneResizeParams, PaneSplitParams,
            PaneSwapParams, PaneTarget, PaneZoomMode, PaneZoomParams, SplitDirection,
            TabCreateParams, TabMoveParams, TabTarget, WorkspaceTarget,
        };
        use crate::input::KeybindAction;

        let snapshot = self.snapshot.as_deref()?;
        let focused_workspace = snapshot.focused_workspace_id.clone()?;
        let focused_tab = snapshot.focused_tab_id.clone();
        let focused_pane = snapshot.focused_pane_id.clone();
        let direction = |action| match action {
            KeybindAction::FocusPaneLeft
            | KeybindAction::SwapPaneLeft
            | KeybindAction::ResizePaneLeft => Some(PaneDirection::Left),
            KeybindAction::FocusPaneDown
            | KeybindAction::SwapPaneDown
            | KeybindAction::ResizePaneDown => Some(PaneDirection::Down),
            KeybindAction::FocusPaneUp
            | KeybindAction::SwapPaneUp
            | KeybindAction::ResizePaneUp => Some(PaneDirection::Up),
            KeybindAction::FocusPaneRight
            | KeybindAction::SwapPaneRight
            | KeybindAction::ResizePaneRight => Some(PaneDirection::Right),
            _ => None,
        };

        match action {
            KeybindAction::FocusAgent(index) => {
                let agents = super::agent_sidebar::ordered_agent_pane_ids(
                    snapshot,
                    self.config.agent_panel_sort,
                );
                Some(Method::PaneFocus(PaneTarget {
                    pane_id: agents.get(index)?.clone(),
                }))
            }
            KeybindAction::PreviousAgent | KeybindAction::NextAgent => {
                let agents = super::agent_sidebar::ordered_agent_pane_ids(
                    snapshot,
                    self.config.agent_panel_sort,
                );
                if agents.is_empty() {
                    return None;
                }
                let current = agents.iter().position(|pane_id| {
                    Some(pane_id.as_str()) == snapshot.focused_pane_id.as_deref()
                });
                let next = match (current, action) {
                    (Some(current), KeybindAction::PreviousAgent) => {
                        (current + agents.len() - 1) % agents.len()
                    }
                    (Some(current), KeybindAction::NextAgent) => (current + 1) % agents.len(),
                    (None, KeybindAction::PreviousAgent) => agents.len() - 1,
                    (None, KeybindAction::NextAgent) => 0,
                    _ => unreachable!("relative agent action"),
                };
                let pane_id = agents[next].clone();
                if !self
                    .hits
                    .agents
                    .iter()
                    .any(|(_, visible_pane_id)| visible_pane_id == &pane_id)
                {
                    self.agent_scroll = next.min(self.hits.agent_max_scroll);
                }
                Some(Method::PaneFocus(PaneTarget { pane_id }))
            }
            KeybindAction::SwitchWorkspace(index) => {
                let entries = self.navigation_workspace_entries(snapshot);
                let workspace_id = snapshot
                    .workspaces
                    .get(entries.get(index)?.index)?
                    .workspace_id
                    .clone();
                self.reveal_workspace(&workspace_id);
                Some(Method::WorkspaceFocus(WorkspaceTarget { workspace_id }))
            }
            KeybindAction::PreviousWorkspace | KeybindAction::NextWorkspace => {
                let entries = self.navigation_workspace_entries(snapshot);
                if entries.is_empty() {
                    return None;
                }
                let current = entries
                    .iter()
                    .position(|entry| {
                        snapshot.workspaces[entry.index].workspace_id == focused_workspace
                    })
                    .unwrap_or(0);
                let delta = if action == KeybindAction::PreviousWorkspace {
                    -1
                } else {
                    1
                };
                let next = (current as isize + delta).rem_euclid(entries.len() as isize) as usize;
                let workspace_id = snapshot.workspaces[entries[next].index]
                    .workspace_id
                    .clone();
                self.reveal_workspace(&workspace_id);
                Some(Method::WorkspaceFocus(WorkspaceTarget { workspace_id }))
            }
            KeybindAction::SwitchTab(index) => {
                let tabs = snapshot
                    .tabs
                    .iter()
                    .filter(|tab| tab.workspace_id == focused_workspace)
                    .collect::<Vec<_>>();
                Some(Method::TabFocus(TabTarget {
                    tab_id: tabs.get(index)?.tab_id.clone(),
                }))
            }
            KeybindAction::PreviousTab | KeybindAction::NextTab => {
                let tabs = snapshot
                    .tabs
                    .iter()
                    .filter(|tab| tab.workspace_id == focused_workspace)
                    .collect::<Vec<_>>();
                let focused_tab = focused_tab?;
                let current = tabs.iter().position(|tab| tab.tab_id == focused_tab)?;
                let delta = if action == KeybindAction::PreviousTab {
                    -1
                } else {
                    1
                };
                let next = (current as isize + delta).rem_euclid(tabs.len() as isize) as usize;
                Some(Method::TabFocus(TabTarget {
                    tab_id: tabs[next].tab_id.clone(),
                }))
            }
            KeybindAction::MoveTabPrevious | KeybindAction::MoveTabNext => {
                let tabs = snapshot
                    .tabs
                    .iter()
                    .filter(|tab| tab.workspace_id == focused_workspace)
                    .collect::<Vec<_>>();
                if tabs.len() <= 1 {
                    return None;
                }
                let focused_tab = focused_tab?;
                let source = tabs.iter().position(|tab| tab.tab_id == focused_tab)?;
                let insert_index = if action == KeybindAction::MoveTabNext {
                    if source + 1 >= tabs.len() {
                        0
                    } else {
                        source + 2
                    }
                } else if source == 0 {
                    tabs.len()
                } else {
                    source - 1
                };
                Some(Method::TabMove(TabMoveParams {
                    tab_id: focused_tab,
                    insert_index,
                }))
            }
            KeybindAction::NewTab if !self.config.prompt_new_tab_name => {
                Some(Method::TabCreate(TabCreateParams {
                    workspace_id: Some(focused_workspace),
                    cwd: None,
                    focus: true,
                    label: None,
                    env: Default::default(),
                }))
            }
            KeybindAction::FocusPaneLeft
            | KeybindAction::FocusPaneDown
            | KeybindAction::FocusPaneUp
            | KeybindAction::FocusPaneRight => {
                Some(Method::PaneFocusDirection(PaneFocusDirectionParams {
                    pane_id: focused_pane,
                    direction: direction(action)?,
                }))
            }
            KeybindAction::SwapPaneLeft
            | KeybindAction::SwapPaneDown
            | KeybindAction::SwapPaneUp
            | KeybindAction::SwapPaneRight => Some(Method::PaneSwap(PaneSwapParams {
                pane_id: focused_pane,
                direction: Some(direction(action)?),
                source_pane_id: None,
                target_pane_id: None,
            })),
            KeybindAction::SplitVertical | KeybindAction::SplitHorizontal => {
                Some(Method::PaneSplit(PaneSplitParams {
                    workspace_id: Some(focused_workspace),
                    target_pane_id: focused_pane,
                    direction: if action == KeybindAction::SplitVertical {
                        SplitDirection::Right
                    } else {
                        SplitDirection::Down
                    },
                    ratio: None,
                    cwd: None,
                    focus: true,
                    right_click: Default::default(),
                    env: Default::default(),
                }))
            }
            KeybindAction::CloseTab => Some(Method::TabClose(TabTarget {
                tab_id: focused_tab?,
            })),
            KeybindAction::ClosePane => Some(Method::PaneClose(PaneTarget {
                pane_id: focused_pane.clone()?,
            })),
            KeybindAction::CyclePaneNext | KeybindAction::CyclePanePrevious => {
                let focused_tab = focused_tab?;
                let panes = snapshot
                    .panes
                    .iter()
                    .filter(|pane| pane.tab_id == focused_tab)
                    .collect::<Vec<_>>();
                if panes.is_empty() {
                    return None;
                }
                let focused_pane = focused_pane?;
                let current = panes
                    .iter()
                    .position(|pane| pane.pane_id == focused_pane)
                    .unwrap_or(0);
                let next = if action == KeybindAction::CyclePanePrevious {
                    (current + panes.len() - 1) % panes.len()
                } else {
                    (current + 1) % panes.len()
                };
                Some(Method::PaneFocus(PaneTarget {
                    pane_id: panes[next].pane_id.clone(),
                }))
            }
            KeybindAction::LastPane => {
                let pane_id = self.previous_pane_id.as_ref()?;
                if Some(pane_id.as_str()) == focused_pane.as_deref()
                    || !snapshot.panes.iter().any(|pane| &pane.pane_id == pane_id)
                {
                    return None;
                }
                Some(Method::PaneFocus(PaneTarget {
                    pane_id: pane_id.clone(),
                }))
            }
            KeybindAction::Zoom => Some(Method::PaneZoom(PaneZoomParams {
                pane_id: focused_pane,
                mode: PaneZoomMode::Toggle,
            })),
            KeybindAction::EditScrollback => Some(Method::PaneEditScrollback(PaneTarget {
                pane_id: focused_pane?,
            })),
            KeybindAction::ResizePaneLeft
            | KeybindAction::ResizePaneDown
            | KeybindAction::ResizePaneUp
            | KeybindAction::ResizePaneRight => Some(Method::PaneResize(PaneResizeParams {
                pane_id: focused_pane,
                direction: direction(action)?,
                amount: None,
            })),
            _ => None,
        }
    }
}
