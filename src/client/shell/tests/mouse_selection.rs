use super::*;

fn three_pane_snapshot() -> ClientShellSnapshot {
    let mut snapshot = snapshot();
    for pane_id in ["pane_2", "pane_3"] {
        let mut pane = snapshot.panes[0].clone();
        pane.pane_id = pane_id.to_owned();
        pane.focused = false;
        snapshot.panes.push(pane);
    }
    snapshot
}

fn three_pane_surface(mouse_reporting: bool) -> PaneSurfaceFrame {
    let mut surface = surface();
    let buffer = Buffer::with_lines(["AAA BBB CCC", "AAA BBB CCC"]);
    surface.frame = FrameData::from_ratatui_buffer_with_hyperlinks(&buffer, None, &[]);
    surface.panes[0].rect.width = 3;
    surface.panes[0].inner_rect.width = 3;
    surface.panes[0].mouse_reporting = mouse_reporting;
    for (pane_id, x) in [("pane_2", 4), ("pane_3", 8)] {
        let mut pane = surface.panes[0].clone();
        pane.pane_id = pane_id.to_owned();
        pane.rect.x = x;
        pane.inner_rect.x = x;
        surface.panes.push(pane);
    }
    surface
}

fn hover_mouse(kind: MouseEventKind, pane: &PaneHit) -> RawInputEvent {
    RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind,
        column: pane.inner_rect.x + 1,
        row: pane.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })
}

fn snapshot_focused(pane_id: &str, revision: u64) -> ClientShellSnapshot {
    let mut snapshot = three_pane_snapshot();
    snapshot.revision = revision;
    snapshot.focused_pane_id = Some(pane_id.to_owned());
    for pane in &mut snapshot.panes {
        pane.focused = pane.pane_id == pane_id;
    }
    snapshot
}

fn focus_request_id(input: &ClientShellInput) -> String {
    let [ClientShellAction::Endpoint { request, .. }] = &input.actions[..] else {
        panic!("expected hover pane focus request");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneFocus(_)
    ));
    request.id.clone()
}

const HOVER_PROFILE_SAMPLES: usize = 20;
const HOVER_PROFILE_WARMUP_BATCHES: usize = 3;
const HOVER_PROFILE_BATCHES_PER_SAMPLE: usize = 12;
const HOVER_PROFILE_COLS: u16 = 40;
const HOVER_PROFILE_ROWS: u16 = 20;
const HOVER_PROFILE_EVENT_COUNT: usize = 7;

#[derive(Clone, Copy)]
enum HoverProfileReplyMode {
    Immediate,
    Delayed,
}

impl HoverProfileReplyMode {
    fn label(self) -> &'static str {
        match self {
            Self::Immediate => "immediate",
            Self::Delayed => "delayed",
        }
    }
}

#[derive(Clone, Copy)]
struct HoverProfileStats {
    handler_median_us_per_event: f64,
    handler_p95_us_per_event: f64,
    reconciliation_median_us_per_event: f64,
    reconciliation_p95_us_per_event: f64,
    focus_requests_per_batch: usize,
    repaints_per_batch: usize,
}

fn hover_profile_snapshot(
    pane_count: usize,
    focused_pane_id: &str,
    revision: u64,
) -> ClientShellSnapshot {
    let mut benchmark_snapshot = snapshot();
    let template = benchmark_snapshot.panes[0].clone();
    benchmark_snapshot.revision = revision;
    benchmark_snapshot.focused_pane_id = Some(focused_pane_id.to_owned());
    benchmark_snapshot.panes = (0..pane_count)
        .map(|index| {
            let pane_id = format!("pane_{}", index + 1);
            let mut pane = template.clone();
            pane.pane_id = pane_id.clone();
            pane.focused = pane_id == focused_pane_id;
            pane
        })
        .collect();
    benchmark_snapshot
}

fn hover_profile_surface(pane_count: usize) -> PaneSurfaceFrame {
    assert!(matches!(pane_count, 1 | 16));
    let mut benchmark_surface = surface();
    let template = benchmark_surface.panes[0].clone();
    let columns: usize = if pane_count == 1 { 1 } else { 4 };
    let pane_width = HOVER_PROFILE_COLS / u16::try_from(columns).expect("profile columns");
    let pane_height = if pane_count == 1 {
        HOVER_PROFILE_ROWS
    } else {
        HOVER_PROFILE_ROWS / 4
    };
    benchmark_surface.frame = FrameData::from_ratatui_buffer_with_hyperlinks(
        &Buffer::empty(ratatui::layout::Rect::new(
            0,
            0,
            HOVER_PROFILE_COLS,
            HOVER_PROFILE_ROWS,
        )),
        None,
        &[],
    );
    benchmark_surface.panes = (0..pane_count)
        .map(|index| {
            let x = u16::try_from(index % columns).expect("profile column") * pane_width;
            let y = u16::try_from(index / columns).expect("profile row") * pane_height;
            let rect = SurfaceRect {
                x,
                y,
                width: pane_width,
                height: pane_height,
            };
            let mut pane = template.clone();
            pane.pane_id = format!("pane_{}", index + 1);
            pane.rect = rect;
            pane.inner_rect = rect;
            pane.focused = index == 0;
            pane
        })
        .collect();
    benchmark_surface
}

fn hover_profile_fixture(
    pane_count: usize,
    hover_enabled: bool,
) -> (
    ClientShellState,
    PaneSurfaceFrame,
    Vec<crossterm::event::MouseEvent>,
) {
    let mut config = Config::default();
    config.ui.focus_pane_on_hover = hover_enabled;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    let surface = hover_profile_surface(pane_count);
    state.set_snapshot(Box::new(hover_profile_snapshot(pane_count, "pane_1", 1)));
    state.set_pane_surface(surface.clone());
    state
        .compose(106, 30)
        .expect("hover profile fixture should compose");
    let origin = state.hits.panes[0].inner_rect;
    // The same fixed-geometry path reaches the tail of the 16-pane hit list.
    let points = [
        (1, 1),
        (2, 1),
        (31, 16),
        (32, 16),
        (21, 16),
        (11, 16),
        (12, 16),
    ];
    let events = points
        .into_iter()
        .map(|(x, y)| crossterm::event::MouseEvent {
            kind: MouseEventKind::Moved,
            column: origin.x + x,
            row: origin.y + y,
            modifiers: KeyModifiers::empty(),
        })
        .collect();
    (state, surface, events)
}

fn hover_profile_focus_action(input: &ClientShellInput) -> Option<(String, String)> {
    input.actions.iter().find_map(|action| {
        let ClientShellAction::Endpoint { request, .. } = action else {
            return None;
        };
        let crate::api::schema::Method::PaneFocus(target) = &request.method else {
            return None;
        };
        Some((request.id.clone(), target.pane_id.clone()))
    })
}

fn hover_profile_apply_snapshot(
    state: &mut ClientShellState,
    surface: &PaneSurfaceFrame,
    pane_count: usize,
    pane_id: &str,
    revision: u64,
) {
    let mut acknowledged_surface = surface.clone();
    acknowledged_surface.projection_revision = revision;
    acknowledged_surface.surface_revision = revision;
    state.set_snapshot(Box::new(hover_profile_snapshot(
        pane_count, pane_id, revision,
    )));
    state.set_pane_surface(acknowledged_surface);
    // A retained render legitimately returns None after an authoritative snapshot.
    std::hint::black_box(state.compose(106, 30));
}

fn hover_profile_batch(
    pane_count: usize,
    hover_enabled: bool,
    reply_mode: HoverProfileReplyMode,
) -> (std::time::Duration, std::time::Duration, usize, usize) {
    let (mut state, surface, events) = hover_profile_fixture(pane_count, hover_enabled);
    assert_eq!(events.len(), HOVER_PROFILE_EVENT_COUNT);
    let mut handler_elapsed = std::time::Duration::ZERO;
    let mut reconciliation_elapsed = std::time::Duration::ZERO;
    let mut focus_requests = Vec::new();
    let mut repaints = 0;
    let mut revision = 1;

    for event in events {
        let started = std::time::Instant::now();
        let input = state.handle_raw_events(vec![RawInputEvent::Mouse(event)]);
        handler_elapsed += started.elapsed();
        repaints += usize::from(input.repaint);
        assert!(
            state.pending_requests.len() <= 1,
            "one outstanding hover RPC"
        );
        if let Some(request) = hover_profile_focus_action(&input) {
            if matches!(reply_mode, HoverProfileReplyMode::Immediate) {
                let started = std::time::Instant::now();
                let (repaint, actions) = state.handle_endpoint_result(
                    "boot-1",
                    &request.0,
                    Ok(crate::api::schema::ResponseResult::Ok {}),
                );
                assert!(!repaint);
                assert!(actions.is_empty());
                revision += 1;
                hover_profile_apply_snapshot(
                    &mut state, &surface, pane_count, &request.1, revision,
                );
                reconciliation_elapsed += started.elapsed();
            }
            focus_requests.push(request);
        }
        std::hint::black_box(&input);
    }

    if matches!(reply_mode, HoverProfileReplyMode::Delayed) && !focus_requests.is_empty() {
        let started = std::time::Instant::now();
        // Deliver each authoritative snapshot before its serialized reply. The first
        // snapshot must preserve the newer pointer intent and the occupied RPC slot.
        let mut completed = 0;
        while completed < focus_requests.len() {
            assert!(focus_requests.len() <= 2, "delayed requests must coalesce");
            let (request_id, pane_id) = &focus_requests[completed];
            assert_eq!(state.pending_requests.len(), 1);
            assert_eq!(state.hover_slot.as_ref().unwrap().request_id, *request_id);
            revision += 1;
            hover_profile_apply_snapshot(&mut state, &surface, pane_count, pane_id, revision);
            assert_eq!(state.pending_requests.len(), 1);
            assert_eq!(state.hover_slot.as_ref().unwrap().request_id, *request_id);
            if completed == 0 {
                assert_eq!(pane_id, "pane_16");
                assert_eq!(state.desired_hover_pane_id.as_deref().unwrap(), "pane_14");
                let mut follow_up = ClientShellInput::default();
                state.flush_coalesced_hover_pane_focus(&mut follow_up);
                assert!(
                    follow_up.actions.is_empty(),
                    "snapshot cannot release the slot"
                );
                assert!(!follow_up.repaint);
            }
            let (repaint, actions) = state.handle_endpoint_result(
                "boot-1",
                request_id,
                Ok(crate::api::schema::ResponseResult::Ok {}),
            );
            assert!(!repaint);
            let follow_up = ClientShellInput {
                actions,
                ..Default::default()
            };
            assert!(follow_up.actions.len() <= 1);
            if let Some(request) = hover_profile_focus_action(&follow_up) {
                focus_requests.push(request);
            }
            completed += 1;
        }
        reconciliation_elapsed += started.elapsed();
    }

    let expected_requests = usize::from(hover_enabled && pane_count == 16)
        * match reply_mode {
            HoverProfileReplyMode::Immediate => 3,
            HoverProfileReplyMode::Delayed => 2,
        };
    assert_eq!(
        focus_requests.len(),
        expected_requests,
        "hover profile panes={pane_count} hover={hover_enabled} reply={}",
        reply_mode.label()
    );
    assert_eq!(
        repaints,
        0,
        "hover profile panes={pane_count} hover={hover_enabled} reply={}",
        reply_mode.label()
    );
    let expected_final_pane = if hover_enabled && pane_count == 16 {
        "pane_14"
    } else {
        "pane_1"
    };
    assert_eq!(
        state.focused_pane_id().as_deref(),
        Some(expected_final_pane),
        "hover profile panes={pane_count} hover={hover_enabled} reply={}",
        reply_mode.label()
    );
    assert!(
        state.desired_hover_pane_id.is_none(),
        "hover profile panes={pane_count} hover={hover_enabled} reply={}",
        reply_mode.label()
    );
    assert!(state.hover_slot.is_none());
    assert!(state.hover_awaiting_snapshot.is_none());
    assert!(state.pending_requests.is_empty());
    assert!(state.pending_manual_focuses.is_empty());
    (
        handler_elapsed,
        reconciliation_elapsed,
        focus_requests.len(),
        repaints,
    )
}

fn hover_profile_stats(
    pane_count: usize,
    hover_enabled: bool,
    reply_mode: HoverProfileReplyMode,
) -> HoverProfileStats {
    for _ in 0..HOVER_PROFILE_WARMUP_BATCHES {
        std::hint::black_box(hover_profile_batch(pane_count, hover_enabled, reply_mode));
    }
    let mut handler_samples = Vec::with_capacity(HOVER_PROFILE_SAMPLES);
    let mut reconciliation_samples = Vec::with_capacity(HOVER_PROFILE_SAMPLES);
    let mut focus_requests_per_batch = None;
    let mut repaints_per_batch = None;
    for _ in 0..HOVER_PROFILE_SAMPLES {
        let mut handler_elapsed = std::time::Duration::ZERO;
        let mut reconciliation_elapsed = std::time::Duration::ZERO;
        for _ in 0..HOVER_PROFILE_BATCHES_PER_SAMPLE {
            let (handler, reconciliation, focus_requests, repaints) =
                hover_profile_batch(pane_count, hover_enabled, reply_mode);
            handler_elapsed += handler;
            reconciliation_elapsed += reconciliation;
            assert_eq!(
                *focus_requests_per_batch.get_or_insert(focus_requests),
                focus_requests
            );
            assert_eq!(*repaints_per_batch.get_or_insert(repaints), repaints);
        }
        let events = (HOVER_PROFILE_BATCHES_PER_SAMPLE * HOVER_PROFILE_EVENT_COUNT) as f64;
        handler_samples.push(handler_elapsed.as_secs_f64() * 1_000_000.0 / events);
        reconciliation_samples.push(reconciliation_elapsed.as_secs_f64() * 1_000_000.0 / events);
    }
    handler_samples.sort_by(f64::total_cmp);
    reconciliation_samples.sort_by(f64::total_cmp);
    let p95_index = (HOVER_PROFILE_SAMPLES - 1) * 95 / 100;
    HoverProfileStats {
        handler_median_us_per_event: handler_samples[HOVER_PROFILE_SAMPLES / 2],
        handler_p95_us_per_event: handler_samples[p95_index],
        reconciliation_median_us_per_event: reconciliation_samples[HOVER_PROFILE_SAMPLES / 2],
        reconciliation_p95_us_per_event: reconciliation_samples[p95_index],
        focus_requests_per_batch: focus_requests_per_batch.expect("profile samples"),
        repaints_per_batch: repaints_per_batch.expect("profile samples"),
    }
}

fn hover_profile_ratio(numerator: f64, denominator: f64) -> f64 {
    numerator / denominator.max(f64::EPSILON)
}

#[test]
fn hover_profile_fixture_maps_the_shared_pointer_path() {
    for pane_count in [1, 16] {
        let (state, _surface, events) = hover_profile_fixture(pane_count, true);
        assert_eq!(state.hits.panes.len(), pane_count);
        let targets = events
            .iter()
            .map(|event| {
                state
                    .hits
                    .panes
                    .iter()
                    .find(|hit| {
                        crate::client::shell::contains(hit.inner_rect, (event.column, event.row))
                    })
                    .map(|hit| hit.pane_id.as_str())
            })
            .collect::<Vec<_>>();
        let expected = if pane_count == 1 {
            vec![Some("pane_1"); HOVER_PROFILE_EVENT_COUNT]
        } else {
            vec![
                Some("pane_1"),
                Some("pane_1"),
                Some("pane_16"),
                Some("pane_16"),
                Some("pane_15"),
                Some("pane_14"),
                Some("pane_14"),
            ]
        };
        assert_eq!(targets, expected, "hover profile panes={pane_count}");
    }
}

#[test]
fn hover_profile_batch_smoke_exercises_immediate_and_delayed_reconciliation() {
    for reply_mode in [
        HoverProfileReplyMode::Immediate,
        HoverProfileReplyMode::Delayed,
    ] {
        for (pane_count, hover_enabled) in [(1, false), (1, true), (16, false), (16, true)] {
            let (_, _, focus_requests, repaints) =
                hover_profile_batch(pane_count, hover_enabled, reply_mode);
            assert_eq!(
                focus_requests,
                usize::from(hover_enabled && pane_count == 16)
                    * match reply_mode {
                        HoverProfileReplyMode::Immediate => 3,
                        HoverProfileReplyMode::Delayed => 2,
                    },
                "hover profile panes={pane_count} hover={hover_enabled} reply={}",
                reply_mode.label()
            );
            assert_eq!(repaints, 0);
        }
    }
}

#[test]
#[ignore = "manual release-mode hover focus profile"]
fn hover_focus_profile() {
    println!(
        "hover focus handler-only profile: {} samples × {} batches × {} events; reply/snapshot timing is separate and includes test surface recomposition",
        HOVER_PROFILE_SAMPLES, HOVER_PROFILE_BATCHES_PER_SAMPLE, HOVER_PROFILE_EVENT_COUNT
    );
    println!(
        "  reply      panes  hover  handler_median_us/event  handler_p95_us/event  reconcile_median_us/event  reconcile_p95_us/event  focus_requests/batch  repaints/batch"
    );
    for reply_mode in [
        HoverProfileReplyMode::Immediate,
        HoverProfileReplyMode::Delayed,
    ] {
        let rows =
            [(1, false), (1, true), (16, false), (16, true)].map(|(pane_count, hover_enabled)| {
                (
                    pane_count,
                    hover_enabled,
                    hover_profile_stats(pane_count, hover_enabled, reply_mode),
                )
            });
        for (pane_count, hover_enabled, stats) in rows {
            println!(
                "  {:>9}  {:>5}  {:>5}  {:>23.3}  {:>20.3}  {:>27.3}  {:>24.3}  {:>20}  {:>14}",
                reply_mode.label(),
                pane_count,
                if hover_enabled { "on" } else { "off" },
                stats.handler_median_us_per_event,
                stats.handler_p95_us_per_event,
                stats.reconciliation_median_us_per_event,
                stats.reconciliation_p95_us_per_event,
                stats.focus_requests_per_batch,
                stats.repaints_per_batch,
            );
        }
        let one_off = rows[0].2;
        let one_on = rows[1].2;
        let sixteen_off = rows[2].2;
        let sixteen_on = rows[3].2;
        println!(
            "  {} deltas: 16-vs-1 hover-on={:.2}x; 16-pane on-vs-off={:.2}x; 1-pane on-vs-off={:.2}x",
            reply_mode.label(),
            hover_profile_ratio(
                sixteen_on.handler_median_us_per_event,
                one_on.handler_median_us_per_event,
            ),
            hover_profile_ratio(
                sixteen_on.handler_median_us_per_event,
                sixteen_off.handler_median_us_per_event,
            ),
            hover_profile_ratio(
                one_on.handler_median_us_per_event,
                one_off.handler_median_us_per_event,
            ),
        );
    }
}

#[test]
fn hover_motion_source_borrows_hits_without_cloning() {
    let source = include_str!("../mouse.rs");
    let (_, motion_and_rest) = source.rsplit_once("MouseEventKind::Moved => {").unwrap();
    let (motion, _) = motion_and_rest
        .split_once("MouseEventKind::ScrollUp")
        .unwrap();
    // Motion must not copy a PaneHit (including its owned id) just to test
    // default-off hover policy or forward a non-reporting pane's no-op motion.
    assert!(motion.contains("let hit = &self.hits.panes[pane_index]"));
    for allocation in [".clone()", ".cloned()", ".to_owned()", ".to_string()"] {
        assert!(
            !motion.contains(allocation),
            "motion dispatch contains {allocation}"
        );
    }
}

#[test]
fn hover_repeated_motion_and_slot_flush_retain_desired_id_allocation() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();
    let initial = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let request_id = focus_request_id(&initial);

    // Exercise both the sent target and a newer coalesced target while the
    // first request owns the slot. Comparing live storage needs no allocator hook.
    for pane in [&pane_2, &pane_3] {
        let _ = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, pane)]);
        let allocation = state.desired_hover_pane_id.as_ref().unwrap().as_ptr();
        for _ in 0..8 {
            let input = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, pane)]);
            assert!(input.actions.is_empty());
            assert!(!input.repaint);
            let mut follow_up = ClientShellInput::default();
            state.flush_coalesced_hover_pane_focus(&mut follow_up);
            assert!(follow_up.actions.is_empty());
            assert!(!follow_up.repaint);
            assert_eq!(
                state.desired_hover_pane_id.as_ref().unwrap().as_ptr(),
                allocation
            );
            assert_eq!(state.hover_slot.as_ref().unwrap().request_id, request_id);
            assert_eq!(state.pending_requests.len(), 1);
        }
    }
}

#[test]
fn hover_motion_coalesces_while_focus_is_in_flight_and_forwards_reported_motion() {
    let mut state = hover_enabled_three_pane_state(true);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();

    let enter_pane_2 = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(!enter_pane_2.repaint);
    assert!(matches!(
        &enter_pane_2.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(
                &request.method,
                crate::api::schema::Method::PaneFocus(target) if target.pane_id == "pane_2"
            )
    ));
    assert!(matches!(
        &enter_pane_2.requests[..],
        [ClientMessage::ClientShellPaneInput { pane_id, .. }] if pane_id == "pane_2"
    ));

    let enter_pane_3 = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
    assert!(
        enter_pane_3.actions.is_empty(),
        "later hover crossings must coalesce onto the in-flight hover slot"
    );
    assert!(matches!(
        &enter_pane_3.requests[..],
        [ClientMessage::ClientShellPaneInput { pane_id, .. }] if pane_id == "pane_3"
    ));

    let reenter_pane_2 = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(!reenter_pane_2.repaint);
    assert!(reenter_pane_2.actions.is_empty());
    assert!(matches!(
        &reenter_pane_2.requests[..],
        [ClientMessage::ClientShellPaneInput { pane_id, .. }] if pane_id == "pane_2"
    ));
}

#[test]
fn hover_motion_is_disabled_without_local_mouse_capture_or_hover_preference() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(three_pane_snapshot()));
    state.set_pane_surface(three_pane_surface(false));
    state.compose(106, 20).expect("three-pane frame");
    let pane_2 = state.hits.panes[1].clone();
    let disabled = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(disabled.actions.is_empty());
    assert!(disabled.requests.is_empty());

    let mut config = Config::default();
    config.ui.focus_pane_on_hover = true;
    config.ui.mouse_capture = false;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(three_pane_snapshot()));
    state.set_pane_surface(three_pane_surface(true));
    state
        .compose(106, 20)
        .expect("capture-disabled three-pane frame");
    let pane_2 = state.hits.panes[1].clone();
    let capture_disabled =
        state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(capture_disabled.actions.is_empty());
    assert!(matches!(
        &capture_disabled.requests[..],
        [ClientMessage::ClientShellPaneInput { pane_id, .. }] if pane_id == "pane_2"
    ));
}

#[test]
fn hover_motion_does_not_retarget_selection_or_pane_mouse_gestures() {
    let mut selection_state = hover_enabled_three_pane_state(false);
    let pane_1 = selection_state.hits.panes[0].clone();
    let pane_2 = selection_state.hits.panes[1].clone();
    selection_state.handle_raw_events(vec![hover_mouse(
        MouseEventKind::Down(MouseButton::Left),
        &pane_1,
    )]);
    let selection_move =
        selection_state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(selection_move.actions.is_empty());

    let mut gesture_state = hover_enabled_three_pane_state(true);
    let pane_1 = gesture_state.hits.panes[0].clone();
    let pane_2 = gesture_state.hits.panes[1].clone();
    gesture_state.handle_raw_events(vec![hover_mouse(
        MouseEventKind::Down(MouseButton::Left),
        &pane_1,
    )]);
    let gesture_move =
        gesture_state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(gesture_move.actions.is_empty());
}

#[test]
fn hover_focus_live_config_reconciles_enablement_transitions() {
    let settings = [(true, true), (false, true), (true, false), (false, false)];
    for (old_hover, old_capture) in settings {
        for (new_hover, new_capture) in settings {
            let mut initial = Config::default();
            initial.ui.focus_pane_on_hover = old_hover;
            initial.ui.mouse_capture = old_capture;
            let mut state = ClientShellState::new(ClientShellConfig::from_config(&initial));
            state.set_snapshot(Box::new(three_pane_snapshot()));
            state.set_pane_surface(three_pane_surface(false));
            state.compose(106, 20).expect("three-pane frame");
            let pane_2 = state.hits.panes[1].clone();
            let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
            let was_enabled = old_hover && old_capture;
            assert_eq!(first.actions.len(), usize::from(was_enabled));

            let mut next = Config::default();
            next.ui.focus_pane_on_hover = new_hover;
            next.ui.mouse_capture = new_capture;
            let diagnostics = state.apply_live_client_config(&next, &[], &[]);
            let is_enabled = new_hover && new_capture;
            let transition =
                format!("({old_hover}, {old_capture}) -> ({new_hover}, {new_capture})");
            assert!(diagnostics.is_empty(), "{transition}");
            assert_eq!(state.config.focus_pane_on_hover, new_hover, "{transition}");
            assert_eq!(state.config.mouse_capture, new_capture, "{transition}");
            assert_eq!(
                state.pending_requests.len(),
                usize::from(was_enabled && is_enabled),
                "{transition}"
            );
            assert_eq!(
                state.desired_hover_pane_id.is_some(),
                was_enabled && is_enabled,
                "{transition}"
            );
            // Reload requests cancellation, but only transport can release an unsent slot.
            assert_eq!(state.hover_slot.is_some(), was_enabled, "{transition}");
            let cancelled = state.take_cancelled_unsent_endpoint_ids();
            if was_enabled && !is_enabled {
                assert_eq!(
                    cancelled,
                    vec![(state.active_endpoint_id.clone(), focus_request_id(&first))],
                    "{transition}"
                );
            } else {
                assert!(cancelled.is_empty(), "{transition}");
            }
            let after = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
            assert_eq!(
                pane_focus_actions(&after.actions).len(),
                usize::from(!was_enabled && is_enabled),
                "{transition}"
            );
        }
    }
}

#[test]
fn hover_focus_retargets_the_snapshot_focused_pane_after_newer_hover_intent() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_1 = state.hits.panes[0].clone();
    let pane_2 = state.hits.panes[1].clone();

    let hover_b = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert_eq!(hover_b.actions.len(), 1);
    let hover_a = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_1)]);
    assert!(
        hover_a.actions.is_empty(),
        "unacknowledged hover A must wait for the occupied hover slot"
    );
    assert_eq!(state.desired_hover_pane_id.as_deref(), Some("pane_1"));
}

#[test]
fn hover_focus_result_and_snapshot_order_preserve_latest_intent() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();

    let pane_2_request = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let pane_2_request_id = focus_request_id(&pane_2_request);
    let pane_3_coalesced =
        state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
    assert!(pane_3_coalesced.actions.is_empty());
    let (_, follow_up) = state.handle_endpoint_result(
        "boot-1",
        &pane_2_request_id,
        Err(ClientShellEndpointError {
            code: Some("stale_target".into()),
            message: "stale".into(),
        }),
    );
    let pane_3_request_id = pane_focus_actions(&follow_up)
        .into_iter()
        .find_map(|(id, pane_id)| (pane_id == "pane_3").then_some(id))
        .expect("changed hover target should emit after the rejected slot");
    assert_eq!(state.desired_hover_pane_id.as_deref(), Some("pane_3"));
    assert!(state
        .handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)])
        .actions
        .is_empty());

    state.handle_endpoint_result(
        "boot-1",
        &pane_3_request_id,
        Ok(crate::api::schema::ResponseResult::Ok {}),
    );
    assert_eq!(state.desired_hover_pane_id.as_deref(), Some("pane_3"));
    let mut stale_snapshot = snapshot_focused("pane_2", 2);
    stale_snapshot.focused_pane_id = Some("pane_2".into());
    state.set_snapshot(Box::new(stale_snapshot));
    assert_eq!(state.desired_hover_pane_id.as_deref(), Some("pane_3"));

    state.set_snapshot(Box::new(snapshot_focused("pane_3", 3)));
    assert!(state.desired_hover_pane_id.is_none());
}

#[test]
fn hover_focus_success_waits_for_snapshot_before_repeating_same_target() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();

    let request = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let request_id = focus_request_id(&request);
    state.handle_endpoint_result(
        "boot-1",
        &request_id,
        Ok(crate::api::schema::ResponseResult::Ok {}),
    );
    let repeated = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(repeated.actions.is_empty());
    assert!(state.desired_hover_pane_id.is_some());

    state.set_snapshot(Box::new(snapshot_focused("pane_2", 2)));
    assert!(state.desired_hover_pane_id.is_none());
}

#[test]
fn rejected_hover_focus_can_retry_without_an_endpoint_notice() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();

    let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let [ClientShellAction::Endpoint { request, .. }] = &first.actions[..] else {
        panic!("hover should request pane focus");
    };
    let (repaint, actions) = state.handle_endpoint_result(
        "boot-1",
        &request.id,
        Err(ClientShellEndpointError {
            code: Some("server_unavailable".into()),
            message: "unavailable".into(),
        }),
    );
    assert!(!repaint);
    assert!(actions.is_empty());
    assert!(state.pending_requests.is_empty());
    assert!(state.visible_endpoint_notice.is_none());

    let retry = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert_eq!(retry.actions.len(), 1);
}

#[test]
fn hover_focus_cleanup_handles_snapshot_removal_and_endpoint_reset() {
    let mut reset_state = hover_enabled_three_pane_state(false);
    let pane_2 = reset_state.hits.panes[1].clone();
    reset_state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(reset_state.desired_hover_pane_id.is_some());
    reset_state.reset_endpoint_projection();
    assert!(reset_state.desired_hover_pane_id.is_none());

    let mut removal_state = hover_enabled_three_pane_state(false);
    let pane_2 = removal_state.hits.panes[1].clone();
    removal_state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(removal_state.desired_hover_pane_id.is_some());
    let mut removed = snapshot_focused("pane_1", 2);
    removed.panes.retain(|pane| pane.pane_id != "pane_2");
    removal_state.set_snapshot(Box::new(removed));
    assert!(removal_state.desired_hover_pane_id.is_none());
}

#[test]
fn hover_focus_preflight_failures_are_quiet() {
    use crate::client::endpoint::ClientEndpointStatus;

    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let endpoint_id = state.active_endpoint_id.clone();

    state.set_endpoint_status(&endpoint_id, ClientEndpointStatus::Reconnecting);
    let offline = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(!offline.repaint);
    assert!(offline.actions.is_empty());
    assert!(state.visible_endpoint_notice.is_none());

    state.set_endpoint_status(&endpoint_id, ClientEndpointStatus::Online);
    state.set_endpoint_methods(Some(Vec::new()));
    let unsupported = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(!unsupported.repaint);
    assert!(unsupported.actions.is_empty());
    assert!(state.visible_endpoint_notice.is_none());
}

fn hover_enabled_three_pane_state(mouse_reporting: bool) -> ClientShellState {
    let mut config = Config::default();
    config.ui.focus_pane_on_hover = true;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(three_pane_snapshot()));
    state.set_pane_surface(three_pane_surface(mouse_reporting));
    state.compose(106, 20).expect("three-pane frame");
    state
}

fn pane_focus_actions(actions: &[ClientShellAction]) -> Vec<(String, String)> {
    actions
        .iter()
        .filter_map(|action| {
            let ClientShellAction::Endpoint { request, .. } = action else {
                return None;
            };
            let crate::api::schema::Method::PaneFocus(target) = &request.method else {
                return None;
            };
            Some((request.id.clone(), target.pane_id.clone()))
        })
        .collect()
}

fn endpoint_method_name(action: &ClientShellAction) -> Option<String> {
    let ClientShellAction::Endpoint { request, .. } = action else {
        return None;
    };
    Some(crate::api::api_method_name(&request.method).to_owned())
}

struct RecordingEndpointTransport;

impl crate::client::endpoint::EndpointTransport for RecordingEndpointTransport {
    fn send(&mut self, _: &ClientMessage) -> std::io::Result<()> {
        Ok(())
    }
}

struct SerializedEndpointLane {
    commands: crate::client::endpoint_commands::EndpointCommands,
    endpoints: crate::client::endpoint::EndpointRegistry,
}

impl SerializedEndpointLane {
    fn new() -> Self {
        Self {
            commands: crate::client::endpoint_commands::EndpointCommands::default(),
            endpoints: crate::client::endpoint::EndpointRegistry::new(
                RecordingEndpointTransport,
                1,
                crate::client::endpoint::EndpointNegotiation::default(),
            ),
        }
    }

    fn dispatch(&mut self, state: &mut ClientShellState, actions: Vec<ClientShellAction>) {
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        crate::client::shell_runtime::dispatch_client_shell_actions(
            actions,
            &mut self.commands,
            &mut self.endpoints,
            Some(state),
            &mut Vec::new(),
            &tx,
        )
        .expect("serialized endpoint dispatch");
    }

    fn in_flight_id(&self, known_ids: &[String]) -> Option<String> {
        known_ids.iter().find_map(|id| {
            self.commands
                .accepts_response(
                    &crate::client::endpoint::ClientEndpointId::Local,
                    1,
                    "boot-1",
                    id,
                )
                .then(|| id.clone())
        })
    }

    fn complete_ok(
        &mut self,
        state: &mut ClientShellState,
        request_id: &str,
    ) -> (Vec<(String, String)>, Vec<String>) {
        self.complete_result(
            state,
            request_id,
            Ok(crate::api::schema::ResponseResult::Ok {}),
        )
    }

    fn complete_result(
        &mut self,
        state: &mut ClientShellState,
        request_id: &str,
        result: Result<crate::api::schema::ResponseResult, crate::api::schema::ErrorBody>,
    ) -> (Vec<(String, String)>, Vec<String>) {
        let response = match result {
            Ok(result) => serde_json::to_vec(&crate::api::schema::SuccessResponse {
                id: request_id.to_owned(),
                result,
            }),
            Err(error) => serde_json::to_vec(&crate::api::schema::ErrorResponse {
                id: request_id.to_owned(),
                error,
            }),
        }
        .expect("encode endpoint response");
        let completed = self
            .commands
            .receive_chunk(
                &crate::client::endpoint::ClientEndpointId::Local,
                1,
                "boot-1",
                request_id,
                true,
                response,
            )
            .expect("receive endpoint chunk")
            .expect("completed endpoint command");
        let (_, actions) = state.handle_endpoint_result(
            &completed.boot_id,
            &completed.request_id,
            completed.result,
        );
        let focuses = pane_focus_actions(&actions);
        let request_ids = actions
            .iter()
            .filter_map(|action| match action {
                ClientShellAction::Endpoint { request, .. } => Some(request.id.clone()),
                _ => None,
            })
            .collect();
        self.dispatch(state, actions);
        (focuses, request_ids)
    }
}

fn drain_serialized_lane(
    lane: &mut SerializedEndpointLane,
    state: &mut ClientShellState,
    known_ids: &mut Vec<String>,
    focus_targets: &mut std::collections::HashMap<String, String>,
) -> Vec<String> {
    let mut executed = Vec::new();
    while let Some(request_id) = lane.in_flight_id(known_ids) {
        executed.push(request_id.clone());
        let (focuses, request_ids) = lane.complete_ok(state, &request_id);
        for (id, pane_id) in focuses {
            known_ids.push(id.clone());
            focus_targets.insert(id, pane_id);
        }
        for id in request_ids {
            if !known_ids.contains(&id) {
                known_ids.push(id);
            }
        }
    }
    executed
}

fn apply_hover_focus_frame(state: &mut ClientShellState, pane_id: &str, revision: u64) {
    state.set_snapshot(Box::new(snapshot_focused(pane_id, revision)));
    let mut surface = three_pane_surface(false);
    surface.projection_revision = revision;
    surface.surface_revision = revision;
    for pane in &mut surface.panes {
        pane.focused = pane.pane_id == pane_id;
        pane.scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
            offset_from_bottom: 0,
            max_offset_from_bottom: 20,
            viewport_rows: 2,
        });
    }
    state.set_pane_surface(surface);
    state.compose(106, 20).expect("coherent hover focus frame");
}

#[test]
fn hover_newer_snapshot_seen_success_retires_skipped_older_focus() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();
    let mut lane = SerializedEndpointLane::new();
    let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let first_id = focus_request_id(&first);
    lane.dispatch(&mut state, first.actions);
    let crossing = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
    assert!(crossing.actions.is_empty());
    lane.dispatch(&mut state, crossing.actions);

    // B succeeds without ever publishing B: the next published focus is C.
    let (focuses, _) = lane.complete_ok(&mut state, &first_id);
    assert_eq!(focuses.len(), 1);
    assert_eq!(focuses[0].1, "pane_3");
    let second_id = focuses[0].0.clone();
    let recross = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(recross.actions.is_empty());
    lane.dispatch(&mut state, recross.actions);
    apply_hover_focus_frame(&mut state, "pane_3", 3);
    assert!(state.hover_slot.as_ref().unwrap().snapshot_seen);

    let (focuses, ids) = lane.complete_ok(&mut state, &second_id);
    assert_eq!(
        focuses
            .iter()
            .map(|(_, pane)| pane.as_str())
            .collect::<Vec<_>>(),
        vec!["pane_2"],
        "C's acknowledged success must retire skipped B, allowing the latest B intent"
    );
    assert!(state.hover_awaiting_snapshot.is_none());
    assert_eq!(lane.in_flight_id(&ids), Some(focuses[0].0.clone()));
    apply_hover_focus_frame(&mut state, "pane_2", 4);
    let (follow_up, _) = lane.complete_ok(&mut state, &focuses[0].0);
    assert!(follow_up.is_empty());
    assert!(state.hover_slot.is_none());
    assert!(state.hover_awaiting_snapshot.is_none());
    let stay = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(stay.actions.is_empty());
}

fn assert_guard_discards_coalesced_hover(action: crate::input::KeybindAction) {
    for exit_before_reply in [false, true] {
        let mut state = hover_enabled_three_pane_state(false);
        let pane_2 = state.hits.panes[1].clone();
        let pane_3 = state.hits.panes[2].clone();
        let mut lane = SerializedEndpointLane::new();
        let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
        let first_id = focus_request_id(&first);
        lane.dispatch(&mut state, first.actions);
        let crossing = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
        assert!(crossing.actions.is_empty());
        lane.dispatch(&mut state, crossing.actions);
        apply_hover_focus_frame(&mut state, "pane_2", 2);

        let mut enter = ClientShellInput::default();
        state.record_binding(crate::input::KeybindMatch::Action(action), &mut enter);
        match action {
            crate::input::KeybindAction::CopyMode => {
                assert_eq!(state.mode, ClientShellMode::Copy);
                assert_eq!(state.copy_mode.as_ref().unwrap().pane_id, "pane_2");
            }
            crate::input::KeybindAction::EnterResizeMode => {
                assert_eq!(state.mode, ClientShellMode::Resize);
            }
            crate::input::KeybindAction::Help => {
                assert!(matches!(state.overlay, Some(ClientShellOverlay::Help(_))));
            }
            _ => panic!("unexpected guard"),
        }
        lane.dispatch(&mut state, enter.actions);
        if exit_before_reply {
            let exit = state.handle_raw_events(vec![RawInputEvent::Key(
                crate::input::TerminalKey::new(KeyCode::Esc, KeyModifiers::NONE),
            )]);
            lane.dispatch(&mut state, exit.actions);
            assert_eq!(state.mode, ClientShellMode::Terminal);
            assert!(state.overlay.is_none());
        }
        let (focuses, ids) = lane.complete_ok(&mut state, &first_id);
        assert!(
            focuses.is_empty(),
            "{action:?} must discard deferred hover, exit_before_reply={exit_before_reply}"
        );
        assert!(ids.is_empty());
        assert!(state.desired_hover_pane_id.is_none());
        if !exit_before_reply {
            let exit = state.handle_raw_events(vec![RawInputEvent::Key(
                crate::input::TerminalKey::new(KeyCode::Esc, KeyModifiers::NONE),
            )]);
            lane.dispatch(&mut state, exit.actions);
        }
        assert_eq!(state.mode, ClientShellMode::Terminal);
        assert!(state.overlay.is_none());
        assert!(
            state.hover_slot.is_none(),
            "leaving the guard must not replay C"
        );
        let fresh = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
        assert_eq!(pane_focus_actions(&fresh.actions)[0].1, "pane_3");
        assert!(state.visible_endpoint_notice.is_none());
    }
}

#[test]
fn hover_deferred_focus_is_discarded_on_copy_entry() {
    assert_guard_discards_coalesced_hover(crate::input::KeybindAction::CopyMode);
}

#[test]
fn hover_deferred_focus_is_discarded_on_resize_entry() {
    assert_guard_discards_coalesced_hover(crate::input::KeybindAction::EnterResizeMode);
}

#[test]
fn hover_deferred_focus_is_discarded_on_overlay_entry() {
    assert_guard_discards_coalesced_hover(crate::input::KeybindAction::Help);
}

#[test]
fn hover_focused_split_supersedes_older_coalesced_intent_on_serialized_lane() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();
    let mut lane = SerializedEndpointLane::new();
    let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let first_id = focus_request_id(&first);
    lane.dispatch(&mut state, first.actions);
    let crossing = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
    lane.dispatch(&mut state, crossing.actions);
    apply_hover_focus_frame(&mut state, "pane_2", 2);
    let mut split = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::SplitVertical),
        &mut split,
    );
    let [ClientShellAction::Endpoint { request, .. }] = &split.actions[..] else {
        panic!("expected split request");
    };
    assert!(
        matches!(&request.method, crate::api::schema::Method::PaneSplit(params) if params.focus)
    );
    let split_id = request.id.clone();
    lane.dispatch(&mut state, split.actions);
    let (focuses, _) = lane.complete_ok(&mut state, &first_id);
    assert!(
        focuses.is_empty(),
        "newer explicit split must supersede coalesced C"
    );
    assert!(state.desired_hover_pane_id.is_none());
    assert_eq!(
        state.pending_manual_focuses.back().unwrap().request_id,
        split_id
    );
    assert_eq!(
        lane.in_flight_id(std::slice::from_ref(&split_id)),
        Some(split_id.clone())
    );

    let mut split_snapshot = snapshot_focused("pane_4", 3);
    let mut new_pane = split_snapshot.panes[0].clone();
    new_pane.pane_id = "pane_4".into();
    new_pane.focused = true;
    split_snapshot.panes.push(new_pane);
    state.set_snapshot(Box::new(split_snapshot));
    let mut split_surface = three_pane_surface(false);
    split_surface.projection_revision = 3;
    split_surface.surface_revision = 3;
    for pane in &mut split_surface.panes {
        pane.focused = false;
    }
    let mut new_pane = split_surface.panes[0].clone();
    new_pane.pane_id = "pane_4".into();
    new_pane.focused = true;
    new_pane.rect.x = 12;
    new_pane.inner_rect.x = 12;
    split_surface.panes.push(new_pane);
    split_surface.frame = FrameData::from_ratatui_buffer_with_hyperlinks(
        &Buffer::with_lines(["AAA BBB CCC DDD", "AAA BBB CCC DDD"]),
        None,
        &[],
    );
    state.set_pane_surface(split_surface);
    state.compose(106, 20).expect("split frame");
    let (focuses, _) = lane.complete_ok(&mut state, &split_id);
    assert!(focuses.is_empty());
    assert!(state.pending_manual_focuses.is_empty());
    assert!(state.hover_slot.is_none());
    assert_eq!(state.focused_pane_id().as_deref(), Some("pane_4"));
    let fresh = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
    assert_eq!(pane_focus_actions(&fresh.actions)[0].1, "pane_3");
}

#[test]
fn hover_unfocused_split_preserves_coalesced_intent_on_serialized_lane() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();
    let mut lane = SerializedEndpointLane::new();
    let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let first_id = focus_request_id(&first);
    lane.dispatch(&mut state, first.actions);
    let crossing = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
    lane.dispatch(&mut state, crossing.actions);
    let mut method = state
        .endpoint_method_for_action(crate::input::KeybindAction::SplitVertical)
        .unwrap();
    let crate::api::schema::Method::PaneSplit(params) = &mut method else {
        panic!("expected split method");
    };
    params.focus = false;
    let mut split = ClientShellInput::default();
    state.push_endpoint_method(method, &mut split);
    let [ClientShellAction::Endpoint { request, .. }] = &split.actions[..] else {
        panic!("expected split request");
    };
    let split_id = request.id.clone();
    assert!(state.pending_manual_focuses.is_empty());
    lane.dispatch(&mut state, split.actions);
    let (focuses, _) = lane.complete_ok(&mut state, &first_id);
    assert_eq!(focuses.len(), 1);
    assert_eq!(focuses[0].1, "pane_3");
    assert_eq!(
        lane.in_flight_id(std::slice::from_ref(&split_id)),
        Some(split_id.clone())
    );
    let (follow_up, _) = lane.complete_ok(&mut state, &split_id);
    assert!(follow_up.is_empty());
    assert_eq!(
        lane.in_flight_id(std::slice::from_ref(&focuses[0].0)),
        Some(focuses[0].0.clone())
    );
    apply_hover_focus_frame(&mut state, "pane_3", 3);
    assert!(lane.complete_ok(&mut state, &focuses[0].0).0.is_empty());
}

#[derive(Clone, Copy, Debug)]
enum HoverCreateKind {
    Tab,
    Workspace,
}

impl HoverCreateKind {
    fn issue(self, state: &mut ClientShellState) -> ClientShellInput {
        let mut input = ClientShellInput::default();
        state.config.prompt_new_tab_name = false;
        state.config.prompt_new_workspace_name = false;
        state.record_binding(
            crate::input::KeybindMatch::Action(match self {
                Self::Tab => crate::input::KeybindAction::NewTab,
                Self::Workspace => crate::input::KeybindAction::NewWorkspace,
            }),
            &mut input,
        );
        input
    }

    fn issue_unfocused(self, state: &mut ClientShellState) -> ClientShellInput {
        let mut fixture = hover_enabled_three_pane_state(false);
        let input = self.issue(&mut fixture);
        let [ClientShellAction::Endpoint { request, .. }] = &input.actions[..] else {
            panic!("expected create action");
        };
        let mut method = request.method.clone();
        match &mut method {
            crate::api::schema::Method::TabCreate(params) => params.focus = false,
            crate::api::schema::Method::WorkspaceCreate(params) => params.focus = false,
            _ => panic!("expected create method"),
        }
        let mut input = ClientShellInput::default();
        state.push_endpoint_method(method, &mut input);
        input
    }

    fn result(self, focus: bool) -> crate::api::schema::ResponseResult {
        let workspace_id = match self {
            Self::Tab => "ws_1",
            Self::Workspace => "ws_2",
        };
        let mut result = serde_json::json!({
            "type": match self { Self::Tab => "tab_created", Self::Workspace => "workspace_created" },
            "tab": {
                "tab_id": "tab_2", "workspace_id": workspace_id, "number": 2,
                "label": "created", "focused": focus, "pane_count": 1, "agent_status": "unknown"
            },
            "root_pane": {
                "pane_id": "pane_4", "terminal_id": "term_4", "workspace_id": workspace_id,
                "tab_id": "tab_2", "focused": focus, "agent_status": "unknown", "revision": 1
            }
        });
        if matches!(self, Self::Workspace) {
            result["workspace"] = serde_json::json!({
                "workspace_id": workspace_id, "number": 2, "label": "created",
                "focused": focus, "pane_count": 1, "tab_count": 1,
                "active_tab_id": "tab_2", "agent_status": "unknown"
            });
        }
        serde_json::from_value(result).expect("typed create result")
    }

    fn apply_created_context(self, state: &mut ClientShellState) {
        let mut snapshot = snapshot_focused("pane_4", 5);
        let workspace_id = match self {
            Self::Tab => "ws_1",
            Self::Workspace => "ws_2",
        };
        snapshot.focused_workspace_id = Some(workspace_id.into());
        snapshot.focused_tab_id = Some("tab_2".into());
        if matches!(self, Self::Workspace) {
            snapshot.workspaces[0].focused = false;
            let mut workspace = snapshot.workspaces[0].clone();
            workspace.workspace_id = workspace_id.into();
            workspace.active_tab_id = "tab_2".into();
            workspace.focused = true;
            snapshot.workspaces.push(workspace);
        } else {
            snapshot.workspaces[0].active_tab_id = "tab_2".into();
        }
        snapshot.tabs[0].focused = false;
        let mut tab = snapshot.tabs[0].clone();
        tab.tab_id = "tab_2".into();
        tab.workspace_id = workspace_id.into();
        tab.focused = true;
        snapshot.tabs.push(tab);
        let mut pane = snapshot.panes[0].clone();
        pane.pane_id = "pane_4".into();
        pane.tab_id = "tab_2".into();
        pane.workspace_id = workspace_id.into();
        pane.focused = true;
        snapshot.panes.push(pane);
        state.set_snapshot(Box::new(snapshot));
    }
}

fn create_request_id(input: &ClientShellInput) -> String {
    let [ClientShellAction::Endpoint { request, .. }] = &input.actions[..] else {
        panic!("expected create request");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::TabCreate(_) | crate::api::schema::Method::WorkspaceCreate(_)
    ));
    request.id.clone()
}

fn assert_hover_focused_create_supersedes_coalesced(kind: HoverCreateKind) {
    for snapshot_first in [false, true] {
        let mut state = hover_enabled_three_pane_state(false);
        let pane_2 = state.hits.panes[1].clone();
        let pane_3 = state.hits.panes[2].clone();
        let mut lane = SerializedEndpointLane::new();
        let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
        let first_id = focus_request_id(&first);
        lane.dispatch(&mut state, first.actions);
        let crossing = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
        assert!(crossing.actions.is_empty());
        lane.dispatch(&mut state, crossing.actions);
        let create = kind.issue(&mut state);
        let create_id = create_request_id(&create);
        lane.dispatch(&mut state, create.actions);
        // This pane-only change acknowledges B, not the queued creation.
        apply_hover_focus_frame(&mut state, "pane_2", 2);
        let (focuses, ids) = lane.complete_ok(&mut state, &first_id);
        assert!(
            focuses.is_empty(),
            "{kind:?}: old coalesced C must not follow create"
        );
        assert!(ids.is_empty());
        assert!(state.desired_hover_pane_id.is_none());
        assert_eq!(
            lane.in_flight_id(std::slice::from_ref(&create_id)),
            Some(create_id.clone())
        );
        assert!(!state.pending_manual_focuses.back().unwrap().snapshot_seen);
        if snapshot_first {
            kind.apply_created_context(&mut state);
        }
        assert!(lane
            .complete_result(&mut state, &create_id, Ok(kind.result(true)))
            .0
            .is_empty());
        if !snapshot_first {
            // Result-before-context must not let C get sent behind the creation.
            assert!(state.hover_slot.is_none());
            assert_eq!(state.pending_manual_focuses.len(), 1);
            apply_hover_focus_frame(&mut state, "pane_1", 3);
            assert_eq!(
                state.pending_manual_focuses.len(),
                1,
                "a pane-only snapshot must not acknowledge a created context"
            );
            match (&state.pending_manual_focuses.back().unwrap().target, kind) {
                (PendingManualFocusTarget::Tab(id), HoverCreateKind::Tab) => {
                    assert_eq!(id, "tab_2")
                }
                (PendingManualFocusTarget::Workspace(id), HoverCreateKind::Workspace) => {
                    assert_eq!(id, "ws_2")
                }
                _ => panic!("create response must resolve the context identity"),
            }
            kind.apply_created_context(&mut state);
        }
        lane.dispatch(&mut state, Vec::new());
        assert!(state.pending_manual_focuses.is_empty());
        assert!(state.hover_slot.is_none());
        assert!(state.pending_requests.is_empty());
        assert_eq!(state.focused_pane_id().as_deref(), Some("pane_4"));
    }
}

#[test]
fn hover_focused_tab_create_supersedes_coalesced_on_serialized_lane() {
    assert_hover_focused_create_supersedes_coalesced(HoverCreateKind::Tab);
}

#[test]
fn hover_focused_workspace_create_supersedes_coalesced_on_serialized_lane() {
    assert_hover_focused_create_supersedes_coalesced(HoverCreateKind::Workspace);
}

#[test]
fn hover_unfocused_creates_preserve_coalesced_intent_on_serialized_lane() {
    for kind in [HoverCreateKind::Tab, HoverCreateKind::Workspace] {
        let mut state = hover_enabled_three_pane_state(false);
        let pane_2 = state.hits.panes[1].clone();
        let pane_3 = state.hits.panes[2].clone();
        let mut lane = SerializedEndpointLane::new();
        let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
        let first_id = focus_request_id(&first);
        lane.dispatch(&mut state, first.actions);
        let crossing = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
        lane.dispatch(&mut state, crossing.actions);
        let create = kind.issue_unfocused(&mut state);
        let create_id = create_request_id(&create);
        lane.dispatch(&mut state, create.actions);
        assert!(state.pending_manual_focuses.is_empty());
        let (focuses, _) = lane.complete_ok(&mut state, &first_id);
        assert_eq!(focuses.len(), 1);
        assert_eq!(focuses[0].1, "pane_3");
        assert_eq!(
            lane.in_flight_id(std::slice::from_ref(&create_id)),
            Some(create_id.clone())
        );
        assert!(lane
            .complete_result(&mut state, &create_id, Ok(kind.result(false)))
            .0
            .is_empty());
        assert_eq!(
            lane.in_flight_id(std::slice::from_ref(&focuses[0].0)),
            Some(focuses[0].0.clone())
        );
        apply_hover_focus_frame(&mut state, "pane_3", 3);
        assert!(lane.complete_ok(&mut state, &focuses[0].0).0.is_empty());
        assert!(state.pending_manual_focuses.is_empty());
    }
}

#[test]
fn hover_rejected_creates_discard_old_intent_without_sticky_focus() {
    for kind in [HoverCreateKind::Tab, HoverCreateKind::Workspace] {
        let mut state = hover_enabled_three_pane_state(false);
        let pane_2 = state.hits.panes[1].clone();
        let pane_3 = state.hits.panes[2].clone();
        let mut lane = SerializedEndpointLane::new();
        let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
        let first_id = focus_request_id(&first);
        lane.dispatch(&mut state, first.actions);
        let crossing = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
        lane.dispatch(&mut state, crossing.actions);
        let create = kind.issue(&mut state);
        let create_id = create_request_id(&create);
        lane.dispatch(&mut state, create.actions);
        apply_hover_focus_frame(&mut state, "pane_2", 2);
        assert!(lane.complete_ok(&mut state, &first_id).0.is_empty());
        assert!(lane
            .complete_result(
                &mut state,
                &create_id,
                Err(crate::api::schema::ErrorBody {
                    code: "stale_target".into(),
                    message: "create rejected".into(),
                })
            )
            .0
            .is_empty());
        assert!(state.pending_manual_focuses.is_empty());
        assert!(state.desired_hover_pane_id.is_none());
        let same = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
        assert!(
            same.actions.is_empty(),
            "rejection must not leave unknown future focus"
        );
        let fresh = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
        assert_eq!(pane_focus_actions(&fresh.actions)[0].1, "pane_3");
    }
}

#[test]
fn hover_fresh_intent_after_create_keeps_serialized_order() {
    for kind in [HoverCreateKind::Tab, HoverCreateKind::Workspace] {
        for result_first in [false, true] {
            let mut state = hover_enabled_three_pane_state(false);
            let pane_2 = state.hits.panes[1].clone();
            let mut lane = SerializedEndpointLane::new();
            let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
            let first_id = focus_request_id(&first);
            lane.dispatch(&mut state, first.actions);
            let create = kind.issue(&mut state);
            let create_id = create_request_id(&create);
            lane.dispatch(&mut state, create.actions);
            apply_hover_focus_frame(&mut state, "pane_2", 2);
            assert!(lane.complete_ok(&mut state, &first_id).0.is_empty());
            if result_first {
                assert!(lane
                    .complete_result(&mut state, &create_id, Ok(kind.result(true)))
                    .0
                    .is_empty());
            }
            // Fresh B is newer than create despite B still being snapshot-focused.
            let fresh = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
            let fresh_id = focus_request_id(&fresh);
            lane.dispatch(&mut state, fresh.actions);
            if !result_first {
                assert_eq!(
                    lane.in_flight_id(&[create_id.clone(), fresh_id.clone()]),
                    Some(create_id.clone())
                );
                assert!(lane
                    .complete_result(&mut state, &create_id, Ok(kind.result(true)))
                    .0
                    .is_empty());
            }
            assert_eq!(
                lane.in_flight_id(std::slice::from_ref(&fresh_id)),
                Some(fresh_id.clone())
            );
            // The transport has already sent this newer request; context cleanup
            // must not cancel it, and the final authoritative frame wins.
            kind.apply_created_context(&mut state);
            assert!(lane.complete_ok(&mut state, &fresh_id).0.is_empty());
            apply_hover_focus_frame(&mut state, "pane_2", 6);
            assert!(state.pending_manual_focuses.is_empty());
            assert!(state.hover_slot.is_none());
            assert_eq!(state.focused_pane_id().as_deref(), Some("pane_2"));
        }
    }
}

#[test]
fn hover_then_manual_then_hover_keeps_latest_pointer_intent() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let mut lane = SerializedEndpointLane::new();
    let mut known_ids = Vec::new();
    let mut focus_targets = std::collections::HashMap::new();

    let hover_b = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    for (id, pane_id) in pane_focus_actions(&hover_b.actions) {
        known_ids.push(id.clone());
        focus_targets.insert(id, pane_id);
    }
    lane.dispatch(&mut state, hover_b.actions);

    let mut manual = ClientShellInput::default();
    state.push_endpoint_method(
        crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
            pane_id: "pane_3".into(),
        }),
        &mut manual,
    );
    for (id, pane_id) in pane_focus_actions(&manual.actions) {
        known_ids.push(id.clone());
        focus_targets.insert(id, pane_id);
    }
    lane.dispatch(&mut state, manual.actions);

    let hover_b_again = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    for (id, pane_id) in pane_focus_actions(&hover_b_again.actions) {
        known_ids.push(id.clone());
        focus_targets.insert(id, pane_id);
    }
    lane.dispatch(&mut state, hover_b_again.actions);

    let executed = drain_serialized_lane(&mut lane, &mut state, &mut known_ids, &mut focus_targets);
    let executed_panes = executed
        .iter()
        .filter_map(|id| focus_targets.get(id).cloned())
        .collect::<Vec<_>>();
    assert_eq!(
        executed_panes.last().map(String::as_str),
        Some("pane_2"),
        "hover B then manual C then hover B must finish on B, got {executed_panes:?}"
    );
    assert!(
        executed_panes.iter().any(|pane_id| pane_id == "pane_3"),
        "manual C must still run before the restored hover, got {executed_panes:?}"
    );
    assert!(state.visible_endpoint_notice.is_none());
}

#[test]
fn snapshot_focused_then_manual_then_hover_back_is_not_deduped() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_1 = state.hits.panes[0].clone();
    assert_eq!(state.focused_pane_id().as_deref(), Some("pane_1"));

    let mut manual = ClientShellInput::default();
    state.push_endpoint_method(
        crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
            pane_id: "pane_2".into(),
        }),
        &mut manual,
    );
    assert_eq!(pane_focus_actions(&manual.actions).len(), 1);

    let hover_a = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_1)]);
    let hover_targets = pane_focus_actions(&hover_a.actions);
    assert_eq!(
        hover_targets
            .iter()
            .map(|(_, pane_id)| pane_id.as_str())
            .collect::<Vec<_>>(),
        vec!["pane_1"],
        "pending manual B must not let snapshot-focused A suppress hover A"
    );
}

#[test]
fn hundreds_of_hover_crossings_coalesce_on_the_serialized_endpoint_lane() {
    let mut state = hover_enabled_three_pane_state(true);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();
    let mut lane = SerializedEndpointLane::new();
    let mut known_ids = Vec::new();
    let mut focus_targets = std::collections::HashMap::new();
    let mut hover_focus_emitted = 0;
    let mut forwarded_motion = 0;

    for serial in 0..300 {
        let pane = if serial % 2 == 0 { &pane_2 } else { &pane_3 };
        let input = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, pane)]);
        forwarded_motion += usize::from(input.requests.iter().any(|request| {
            matches!(
                request,
                ClientMessage::ClientShellPaneInput { pane_id, .. } if pane_id == &pane.pane_id
            )
        }));
        let focuses = pane_focus_actions(&input.actions);
        hover_focus_emitted += focuses.len();
        for (id, pane_id) in &focuses {
            known_ids.push(id.clone());
            focus_targets.insert(id.clone(), pane_id.clone());
        }
        lane.dispatch(&mut state, input.actions);
    }
    assert_eq!(forwarded_motion, 300);
    assert!(
        hover_focus_emitted <= 1,
        "300 delayed crossings must keep at most one hover PaneFocus in the lane, emitted {hover_focus_emitted}"
    );

    let mut manual = ClientShellInput::default();
    state.push_endpoint_method(
        crate::api::schema::Method::WorkspaceFocus(crate::api::schema::WorkspaceTarget {
            workspace_id: "ws_1".into(),
        }),
        &mut manual,
    );
    assert_eq!(
        manual
            .actions
            .iter()
            .filter_map(endpoint_method_name)
            .collect::<Vec<_>>(),
        vec!["workspace.focus"]
    );
    let [ClientShellAction::Endpoint { request, .. }] = &manual.actions[..] else {
        panic!("expected manual workspace request");
    };
    let workspace_id = request.id.clone();
    let hover_id = known_ids[0].clone();
    known_ids.push(workspace_id.clone());
    lane.dispatch(&mut state, manual.actions);

    let executed = drain_serialized_lane(&mut lane, &mut state, &mut known_ids, &mut focus_targets);
    assert!(
        executed.len() <= 3,
        "serialized hover history must not replay, executed {} commands",
        executed.len()
    );
    assert_eq!(
        focus_targets.get(&hover_id).map(String::as_str),
        Some("pane_2")
    );
    assert_eq!(
        executed,
        vec![hover_id, workspace_id],
        "the sent hover must finish before the manual workspace action; older coalesced C must not replay"
    );
    assert!(state.visible_endpoint_notice.is_none());
}

#[test]
fn hover_disable_does_not_replay_queued_hover_history() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();
    let mut lane = SerializedEndpointLane::new();
    let mut known_ids = Vec::new();
    let mut hover_ids = Vec::new();

    let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    for (id, _) in pane_focus_actions(&first.actions) {
        known_ids.push(id.clone());
        hover_ids.push(id);
    }
    lane.dispatch(&mut state, first.actions);
    for _ in 0..80 {
        let crossing = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
        for (id, _) in pane_focus_actions(&crossing.actions) {
            known_ids.push(id.clone());
            hover_ids.push(id);
        }
        lane.dispatch(&mut state, crossing.actions);
        let recross = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
        for (id, _) in pane_focus_actions(&recross.actions) {
            known_ids.push(id.clone());
            hover_ids.push(id);
        }
        lane.dispatch(&mut state, recross.actions);
    }

    let disabled = Config::default();
    state.apply_live_client_config(&disabled, &[], &[]);
    assert!(state.desired_hover_pane_id.is_none());
    assert!(state
        .pending_requests
        .values()
        .all(|pending| { !matches!(pending.kind, PendingEndpointKind::HoverPaneFocus { .. }) }));

    let mut focus_targets = std::collections::HashMap::new();
    let executed = drain_serialized_lane(&mut lane, &mut state, &mut known_ids, &mut focus_targets);
    let _ = focus_targets;
    let executed_hover = executed
        .iter()
        .filter(|id| hover_ids.iter().any(|hover_id| hover_id == *id))
        .count();
    assert!(
        executed_hover <= 1,
        "disable must drop unsent hover history; executed {executed_hover} hover commands"
    );
    assert!(lane.in_flight_id(&known_ids).is_none());
    assert!(state.visible_endpoint_notice.is_none());
}

#[test]
fn unsent_hover_behind_in_flight_manual_is_dropped_on_disable() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let mut lane = SerializedEndpointLane::new();
    let mut known_ids = Vec::new();
    let mut hover_ids = Vec::new();

    let mut manual = ClientShellInput::default();
    state.push_endpoint_method(
        crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
            pane_id: "pane_3".into(),
        }),
        &mut manual,
    );
    known_ids.extend(manual.actions.iter().filter_map(|action| match action {
        ClientShellAction::Endpoint { request, .. } => Some(request.id.clone()),
        _ => None,
    }));
    lane.dispatch(&mut state, manual.actions);
    assert!(
        lane.in_flight_id(&known_ids).is_some(),
        "manual focus must occupy the serialized lane"
    );

    let hover = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    for (id, _) in pane_focus_actions(&hover.actions) {
        known_ids.push(id.clone());
        hover_ids.push(id);
    }
    lane.dispatch(&mut state, hover.actions);
    assert!(
        !hover_ids.is_empty(),
        "one hover may sit unsent behind an in-flight manual command"
    );
    assert!(hover_ids
        .iter()
        .all(|id| lane.in_flight_id(std::slice::from_ref(id)).is_none()));

    let disabled = Config::default();
    state.apply_live_client_config(&disabled, &[], &[]);
    assert!(state.desired_hover_pane_id.is_none());

    let mut focus_targets = std::collections::HashMap::new();
    let executed = drain_serialized_lane(&mut lane, &mut state, &mut known_ids, &mut focus_targets);
    assert!(
        executed
            .iter()
            .all(|id| !hover_ids.iter().any(|hover_id| hover_id == id)),
        "disable must drop the unsent hover behind a manual command; executed {executed:?}"
    );
    assert!(state.visible_endpoint_notice.is_none());
}

#[test]
fn rejected_manual_focus_does_not_stick_as_future_focus() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_1 = state.hits.panes[0].clone();
    let pane_2 = state.hits.panes[1].clone();
    let mut manual = ClientShellInput::default();
    state.push_endpoint_method(
        crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
            pane_id: "pane_2".into(),
        }),
        &mut manual,
    );
    let manual_id = focus_request_id(&manual);
    state.handle_endpoint_result(
        "boot-1",
        &manual_id,
        Err(ClientShellEndpointError {
            code: Some("stale_target".into()),
            message: "stale".into(),
        }),
    );

    let hover_a = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_1)]);
    assert!(
        pane_focus_actions(&hover_a.actions).is_empty(),
        "rejected manual B must not force a hover of snapshot-focused A"
    );
    let hover_b = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert_eq!(
        pane_focus_actions(&hover_b.actions)
            .iter()
            .map(|(_, pane_id)| pane_id.as_str())
            .collect::<Vec<_>>(),
        vec!["pane_2"],
        "rejected manual B must not stick and suppress a later hover of B"
    );
}

#[test]
fn pending_workspace_focus_does_not_treat_snapshot_pane_as_current() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_1 = state.hits.panes[0].clone();
    let mut workspace = ClientShellInput::default();
    state.push_endpoint_method(
        crate::api::schema::Method::WorkspaceFocus(crate::api::schema::WorkspaceTarget {
            workspace_id: "ws_1".into(),
        }),
        &mut workspace,
    );
    assert!(!workspace.actions.is_empty());

    let hover_a = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_1)]);
    assert_eq!(
        pane_focus_actions(&hover_a.actions)
            .iter()
            .map(|(_, pane_id)| pane_id.as_str())
            .collect::<Vec<_>>(),
        vec!["pane_1"],
        "in-flight workspace focus must not let snapshot pane A suppress hover A"
    );
}

#[test]
fn hover_snapshot_before_result_does_not_wait_for_another_snapshot() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();
    let request = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let request_id = focus_request_id(&request);
    state.set_snapshot(Box::new(snapshot_focused("pane_2", 2)));
    let mut acknowledged_surface = three_pane_surface(false);
    acknowledged_surface.projection_revision = 2;
    acknowledged_surface.surface_revision = 2;
    state.set_pane_surface(acknowledged_surface);
    state
        .compose(106, 20)
        .expect("acknowledged three-pane frame");
    assert!(state.desired_hover_pane_id.is_none());
    let (_, follow_up) = state.handle_endpoint_result(
        "boot-1",
        &request_id,
        Ok(crate::api::schema::ResponseResult::Ok {}),
    );
    assert!(follow_up.is_empty());
    let stay = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(stay.actions.is_empty());
    let next = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
    assert_eq!(
        pane_focus_actions(&next.actions)
            .iter()
            .map(|(_, pane_id)| pane_id.as_str())
            .collect::<Vec<_>>(),
        vec!["pane_3"],
        "snapshot-before-result must free the hover slot without waiting for a later snapshot"
    );
}

#[test]
fn hover_stale_manual_success_keeps_intent_until_snapshot() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_1 = state.hits.panes[0].clone();
    let pane_2 = state.hits.panes[1].clone();
    let mut manual = ClientShellInput::default();
    state.push_endpoint_method(
        crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
            pane_id: "pane_2".into(),
        }),
        &mut manual,
    );
    let manual_id = focus_request_id(&manual);
    state.handle_endpoint_result(
        "boot-1",
        &manual_id,
        Ok(crate::api::schema::ResponseResult::Ok {}),
    );
    let hover_b = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(
        pane_focus_actions(&hover_b.actions).is_empty(),
        "successful manual B must still count as current until snapshot acknowledgement"
    );
    let hover_a = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_1)]);
    assert_eq!(
        pane_focus_actions(&hover_a.actions)
            .iter()
            .map(|(_, pane_id)| pane_id.as_str())
            .collect::<Vec<_>>(),
        vec!["pane_1"],
        "successful manual B without a snapshot must not treat snapshot A as current"
    );
}

#[test]
fn hover_multiple_manual_focus_uses_latest_pending() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();
    let mut first = ClientShellInput::default();
    state.push_endpoint_method(
        crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
            pane_id: "pane_2".into(),
        }),
        &mut first,
    );
    let first_id = focus_request_id(&first);
    let mut second = ClientShellInput::default();
    state.push_endpoint_method(
        crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
            pane_id: "pane_3".into(),
        }),
        &mut second,
    );
    let second_id = focus_request_id(&second);
    assert!(
        pane_focus_actions(
            &state
                .handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)])
                .actions
        )
        .is_empty(),
        "latest pending manual C must count as current"
    );
    let (_, follow_up) = state.handle_endpoint_result(
        "boot-1",
        &second_id,
        Err(ClientShellEndpointError {
            code: Some("stale_target".into()),
            message: "stale".into(),
        }),
    );
    let hover_c = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
    assert!(
        hover_c.actions.is_empty(),
        "the rejection already emitted the coalesced target"
    );
    assert_eq!(
        pane_focus_actions(&follow_up)
            .iter()
            .map(|(_, pane_id)| pane_id.as_str())
            .collect::<Vec<_>>(),
        vec!["pane_3"],
        "rejected later manual must not stick; earlier pending B remains until it completes"
    );
    let hover_b = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    assert!(
        pane_focus_actions(&hover_b.actions).is_empty(),
        "earlier pending manual B must still suppress hover B"
    );
    let _ = first_id;
}

#[test]
fn hover_disable_reenable_while_slot_inflight_does_not_backlog() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();
    let mut lane = SerializedEndpointLane::new();
    let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let first_id = focus_request_id(&first);
    lane.dispatch(&mut state, first.actions);

    let disabled = Config::default();
    state.apply_live_client_config(&disabled, &[], &[]);
    let mut enabled = Config::default();
    enabled.ui.focus_pane_on_hover = true;
    state.apply_live_client_config(&enabled, &[], &[]);

    let mut extra = 0;
    for _ in 0..40 {
        extra += pane_focus_actions(
            &state
                .handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)])
                .actions,
        )
        .len();
    }
    assert_eq!(extra, 0, "inflight hover slot must block reenable backlog");

    let (_, follow_up) = state.handle_endpoint_result(
        "boot-1",
        &first_id,
        Ok(crate::api::schema::ResponseResult::Ok {}),
    );
    assert_eq!(
        pane_focus_actions(&follow_up)
            .iter()
            .map(|(_, pane_id)| pane_id.as_str())
            .collect::<Vec<_>>(),
        vec!["pane_3"]
    );
}

#[test]
fn hover_reenable_retargets_snapshot_pane_after_sent_focus_completes() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_1 = state.hits.panes[0].clone();
    let pane_2 = state.hits.panes[1].clone();
    let mut lane = SerializedEndpointLane::new();
    let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let first_id = focus_request_id(&first);
    lane.dispatch(&mut state, first.actions);

    let mut config = Config::default();
    state.apply_live_client_config(&config, &[], &[]);
    lane.dispatch(&mut state, Vec::new());
    config.ui.focus_pane_on_hover = true;
    state.apply_live_client_config(&config, &[], &[]);
    let latest = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_1)]);
    assert!(
        latest.actions.is_empty(),
        "sent B still occupies the hover slot"
    );
    lane.dispatch(&mut state, latest.actions);
    let (follow_up, _) = lane.complete_ok(&mut state, &first_id);
    assert_eq!(
        follow_up
            .iter()
            .map(|(_, pane)| pane.as_str())
            .collect::<Vec<_>>(),
        vec!["pane_1"]
    );
    let (repeated, _) = lane.complete_ok(&mut state, &follow_up[0].0);
    assert!(
        repeated.is_empty(),
        "a successful corrective focus must not resend without a snapshot"
    );
}

fn assert_late_hover_success_after_snapshot_cleanup_stays_quiet(reset: ClientShellSnapshot) {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let mut lane = SerializedEndpointLane::new();
    let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let first_id = focus_request_id(&first);
    lane.dispatch(&mut state, first.actions);
    assert_eq!(
        lane.in_flight_id(std::slice::from_ref(&first_id)),
        Some(first_id.clone())
    );

    state.set_snapshot(Box::new(reset));
    let mut new_surface = surface();
    new_surface.projection_revision = 2;
    new_surface.surface_revision = 2;
    new_surface.panes[0].pane_id = "pane_3".into();
    state.set_pane_surface(new_surface);
    state.compose(106, 20).expect("new focused pane frame");
    let focused_pane = state.hits.panes[0].clone();
    assert_eq!(focused_pane.pane_id, "pane_3");
    assert_eq!(state.focused_pane_id().as_deref(), Some("pane_3"));
    assert!(state.desired_hover_pane_id.is_none());
    assert!(state.hover_awaiting_snapshot.is_none());
    assert!(!state.pending_requests.contains_key(&first_id));
    lane.dispatch(&mut state, Vec::new());
    assert!(
        state.hover_slot.is_some(),
        "cleanup cannot cancel a sent RPC"
    );
    assert_eq!(
        lane.in_flight_id(std::slice::from_ref(&first_id)),
        Some(first_id.clone())
    );

    let (follow_up, request_ids) = lane.complete_ok(&mut state, &first_id);
    assert!(follow_up.is_empty());
    assert!(request_ids.is_empty());
    assert!(
        state.hover_slot.is_none(),
        "the completed transport slot must retire"
    );
    assert!(lane.in_flight_id(std::slice::from_ref(&first_id)).is_none());
    let stay = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &focused_pane)]);
    assert!(
        pane_focus_actions(&stay.actions).is_empty(),
        "late success for the obsolete target must not refocus the new snapshot-focused pane"
    );
    lane.dispatch(&mut state, stay.actions);
    assert!(state.hover_awaiting_snapshot.is_none());
    assert!(state.desired_hover_pane_id.is_none());
    assert!(state.hover_slot.is_none());
    assert!(state.pending_requests.is_empty());
}

#[test]
fn hover_late_success_after_tab_snapshot_cleanup_stays_quiet() {
    let mut reset = snapshot_focused("pane_3", 2);
    reset.focused_tab_id = Some("tab_2".into());
    reset.workspaces[0].active_tab_id = "tab_2".into();
    reset.tabs[0].focused = false;
    let mut tab = reset.tabs[0].clone();
    tab.tab_id = "tab_2".into();
    tab.focused = true;
    reset.tabs.push(tab);
    reset.panes[2].tab_id = "tab_2".into();
    assert_late_hover_success_after_snapshot_cleanup_stays_quiet(reset);
}

#[test]
fn hover_late_success_after_workspace_snapshot_cleanup_stays_quiet() {
    let mut reset = snapshot_focused("pane_3", 2);
    reset.focused_workspace_id = Some("ws_2".into());
    reset.focused_tab_id = Some("tab_2".into());
    reset.workspaces[0].focused = false;
    let mut workspace = reset.workspaces[0].clone();
    workspace.workspace_id = "ws_2".into();
    workspace.active_tab_id = "tab_2".into();
    workspace.focused = true;
    reset.workspaces.push(workspace);
    reset.tabs[0].focused = false;
    let mut tab = reset.tabs[0].clone();
    tab.tab_id = "tab_2".into();
    tab.workspace_id = "ws_2".into();
    tab.focused = true;
    reset.tabs.push(tab);
    reset.panes[2].workspace_id = "ws_2".into();
    reset.panes[2].tab_id = "tab_2".into();
    assert_late_hover_success_after_snapshot_cleanup_stays_quiet(reset);
}

#[test]
fn hover_late_success_after_target_removal_stays_quiet() {
    let mut reset = snapshot_focused("pane_3", 2);
    reset.panes.retain(|pane| pane.pane_id != "pane_2");
    assert_late_hover_success_after_snapshot_cleanup_stays_quiet(reset);
}

#[test]
fn hover_matching_snapshot_does_not_erase_intent_after_interposed_manual_focus() {
    for snapshot_before_result in [false, true] {
        let mut state = hover_enabled_three_pane_state(false);
        let pane_2 = state.hits.panes[1].clone();
        let mut lane = SerializedEndpointLane::new();
        let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
        let first_id = focus_request_id(&first);
        lane.dispatch(&mut state, first.actions);
        let mut manual = ClientShellInput::default();
        state.push_endpoint_method(
            crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
                pane_id: "pane_3".into(),
            }),
            &mut manual,
        );
        let manual_id = focus_request_id(&manual);
        lane.dispatch(&mut state, manual.actions);
        let latest = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
        assert!(latest.actions.is_empty());
        lane.dispatch(&mut state, latest.actions);
        if snapshot_before_result {
            state.set_snapshot(Box::new(snapshot_focused("pane_2", 2)));
        }
        let (follow_up, _) = lane.complete_ok(&mut state, &first_id);
        assert_eq!(
            follow_up
                .iter()
                .map(|(_, pane)| pane.as_str())
                .collect::<Vec<_>>(),
            vec!["pane_2"]
        );
        if !snapshot_before_result {
            state.set_snapshot(Box::new(snapshot_focused("pane_2", 2)));
        }
        let (extra, _) = lane.complete_ok(&mut state, &manual_id);
        assert!(extra.is_empty());
        let (extra, _) = lane.complete_ok(&mut state, &follow_up[0].0);
        assert!(extra.is_empty());
        state.set_snapshot(Box::new(snapshot_focused("pane_2", 3)));
        assert!(state.desired_hover_pane_id.is_none());
        assert!(state.hover_slot.is_none());
    }
}

#[test]
fn hover_unsent_manual_supersession_cancels_queued_and_batched_actions() {
    for batched in [false, true] {
        let mut state = hover_enabled_three_pane_state(false);
        let pane_3 = state.hits.panes[2].clone();
        let mut lane = SerializedEndpointLane::new();
        let mut first = ClientShellInput::default();
        state.push_endpoint_method(
            crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
                pane_id: "pane_2".into(),
            }),
            &mut first,
        );
        let first_id = focus_request_id(&first);
        lane.dispatch(&mut state, first.actions);
        let hover = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
        let hover_id = focus_request_id(&hover);
        let mut actions = hover.actions;
        if !batched {
            lane.dispatch(&mut state, std::mem::take(&mut actions));
        }
        let mut last = ClientShellInput::default();
        state.push_endpoint_method(
            crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
                pane_id: "pane_1".into(),
            }),
            &mut last,
        );
        let last_id = focus_request_id(&last);
        actions.extend(last.actions);
        lane.dispatch(&mut state, actions);
        assert!(state.hover_slot.is_none());
        assert!(!state.pending_requests.contains_key(&hover_id));
        let (follow_up, _) = lane.complete_ok(&mut state, &first_id);
        assert!(follow_up.is_empty());
        assert_eq!(
            lane.in_flight_id(&[hover_id, last_id.clone()]),
            Some(last_id.clone())
        );
        let (follow_up, _) = lane.complete_ok(&mut state, &last_id);
        assert!(follow_up.is_empty());
    }
}

#[test]
fn hover_late_result_after_unsent_cancel_does_not_reissue() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let mut lane = SerializedEndpointLane::new();
    let mut known_ids = Vec::new();

    let mut manual = ClientShellInput::default();
    state.push_endpoint_method(
        crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
            pane_id: "pane_3".into(),
        }),
        &mut manual,
    );
    known_ids.extend(manual.actions.iter().filter_map(|action| match action {
        ClientShellAction::Endpoint { request, .. } => Some(request.id.clone()),
        _ => None,
    }));
    lane.dispatch(&mut state, manual.actions);

    let hover = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let hover_ids = pane_focus_actions(&hover.actions)
        .into_iter()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    known_ids.extend(hover_ids.iter().cloned());
    lane.dispatch(&mut state, hover.actions);

    let disabled = Config::default();
    state.apply_live_client_config(&disabled, &[], &[]);
    let mut focus_targets = std::collections::HashMap::new();
    let _ = drain_serialized_lane(&mut lane, &mut state, &mut known_ids, &mut focus_targets);

    for hover_id in hover_ids {
        let (_, actions) = state.handle_endpoint_result(
            "boot-1",
            &hover_id,
            Ok(crate::api::schema::ResponseResult::Ok {}),
        );
        assert!(actions.is_empty());
    }
    assert!(state.visible_endpoint_notice.is_none());
}

#[test]
fn workspace_reset_does_not_replay_queued_hover_history() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();
    let mut lane = SerializedEndpointLane::new();
    let mut known_ids = Vec::new();
    let mut hover_ids = Vec::new();

    let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    for (id, _) in pane_focus_actions(&first.actions) {
        known_ids.push(id.clone());
        hover_ids.push(id);
    }
    lane.dispatch(&mut state, first.actions);
    let later = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
    for (id, _) in pane_focus_actions(&later.actions) {
        known_ids.push(id.clone());
        hover_ids.push(id);
    }
    lane.dispatch(&mut state, later.actions);

    let mut reset = snapshot_focused("pane_1", 2);
    reset.focused_workspace_id = Some("ws_other".into());
    state.set_snapshot(Box::new(reset));
    assert!(state.desired_hover_pane_id.is_none());
    assert!(state
        .pending_requests
        .values()
        .all(|pending| { !matches!(pending.kind, PendingEndpointKind::HoverPaneFocus { .. }) }));

    let mut focus_targets = std::collections::HashMap::new();
    let executed = drain_serialized_lane(&mut lane, &mut state, &mut known_ids, &mut focus_targets);
    let executed_hover = executed
        .iter()
        .filter(|id| hover_ids.iter().any(|hover_id| hover_id == *id))
        .count();
    assert!(
        executed_hover <= 1,
        "workspace reset must drop unsent hover history; executed {executed_hover} hover commands"
    );
}

#[test]
fn hover_same_pane_and_stale_results_stay_quiet_and_latest() {
    let mut state = hover_enabled_three_pane_state(false);
    let pane_2 = state.hits.panes[1].clone();
    let pane_3 = state.hits.panes[2].clone();
    let mut lane = SerializedEndpointLane::new();
    let mut known_ids = Vec::new();

    let first = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
    let first_ids = pane_focus_actions(&first.actions);
    known_ids.extend(first_ids.iter().map(|(id, _)| id.clone()));
    lane.dispatch(&mut state, first.actions);
    for _ in 0..50 {
        let stay = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_2)]);
        assert!(
            pane_focus_actions(&stay.actions).is_empty(),
            "same-pane hover must not enqueue another PaneFocus"
        );
        lane.dispatch(&mut state, stay.actions);
    }
    let retarget = state.handle_raw_events(vec![hover_mouse(MouseEventKind::Moved, &pane_3)]);
    known_ids.extend(
        pane_focus_actions(&retarget.actions)
            .into_iter()
            .map(|(id, _)| id),
    );
    lane.dispatch(&mut state, retarget.actions);

    let Some(first_id) = lane.in_flight_id(&known_ids) else {
        panic!("first hover should occupy the serialized lane");
    };
    let (_, request_ids) = lane.complete_result(
        &mut state,
        &first_id,
        Err(crate::api::schema::ErrorBody {
            code: "stale_target".into(),
            message: "stale".into(),
        }),
    );
    known_ids.extend(request_ids);
    assert!(state.visible_endpoint_notice.is_none());

    state.set_snapshot(Box::new(snapshot_focused("pane_2", 2)));
    let mut focus_targets = std::collections::HashMap::new();
    let executed = drain_serialized_lane(&mut lane, &mut state, &mut known_ids, &mut focus_targets);
    assert!(
        executed.len() <= 2,
        "stale success/error must not replay hover history, executed {}",
        executed.len()
    );
}

#[test]
fn ctrl_click_routes_link_activation_through_endpoint_then_client_host() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("pane frame");
    let pane = state.hits.panes[0].clone();
    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: pane.inner_rect.x + 2,
        row: pane.inner_rect.y + 1,
        modifiers: KeyModifiers::CONTROL,
    };
    let activate = state.handle_raw_events(vec![RawInputEvent::Mouse(down)]);
    let [ClientShellAction::Endpoint { request, .. }] = &activate.actions[..] else {
        panic!("expected link activation request");
    };
    let request_id = request.id.clone();
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneLinkActivate(params)
            if params.pane_id == "pane_1" && params.viewport_row == 1 && params.col == 2
    ));

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        ..down
    };
    let held = state.handle_raw_events(vec![RawInputEvent::Mouse(up)]);
    assert!(held.requests.is_empty() && held.actions.is_empty());
    let (_, actions) = state.handle_endpoint_result(
        "boot-1",
        &request_id,
        Ok(crate::api::schema::ResponseResult::PaneLinkActivated {
            url: Some("https://example.test".to_owned()),
            handled: false,
        }),
    );
    assert!(matches!(
        &actions[..],
        [ClientShellAction::OpenSafeWebUrl(url)] if url == "https://example.test"
    ));
    assert!(!state.url_click_consumes_until_up);
}

#[test]
fn ctrl_click_without_a_link_replays_the_original_gesture() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("pane frame");
    let pane = state.hits.panes[0].clone();
    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: pane.inner_rect.x + 2,
        row: pane.inner_rect.y + 1,
        modifiers: KeyModifiers::CONTROL,
    };
    let activate = state.handle_raw_events(vec![RawInputEvent::Mouse(down)]);
    let request_id = match &activate.actions[..] {
        [ClientShellAction::Endpoint { request, .. }] => request.id.clone(),
        _ => panic!("expected link activation request"),
    };
    let (_, actions) = state.handle_endpoint_result(
        "boot-1",
        &request_id,
        Ok(crate::api::schema::ResponseResult::PaneLinkActivated {
            url: None,
            handled: false,
        }),
    );
    assert!(matches!(
        &actions[..],
        [ClientShellAction::ReplayMouse(events)] if events == &vec![down]
    ));
    let replay = match actions.into_iter().next().expect("replay action") {
        ClientShellAction::ReplayMouse(events) => state.replay_mouse_events(events),
        _ => unreachable!(),
    };
    assert!(matches!(
        &replay.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(request.method, crate::api::schema::Method::PaneFocus(_))
    ));
    assert!(state.selection.is_some());
}

#[test]
fn pane_split_drag_uses_projected_handle_and_stable_tab_path() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.splits.push(PaneSurfaceSplit {
        direction: PaneSurfaceSplitDirection::Horizontal,
        pos: 40,
        area: SurfaceRect {
            x: 0,
            y: 0,
            width: 80,
            height: 19,
        },
        hit_rect: SurfaceRect {
            x: 40,
            y: 0,
            width: 1,
            height: 19,
        },
        path: vec![false, true],
    });
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("split pane surface");
    let split = state.hits.pane_splits[0].clone();

    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: split.hit_rect.x,
        row: split.hit_rect.y + 2,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(
        state.chrome_drag,
        Some(ClientChromeDrag::PaneSplit { .. })
    ));
    let mut replacement = snapshot();
    replacement.revision = 2;
    replacement
        .tab_bar_right
        .push(crate::protocol::ClientShellTabStatusSegment {
            text: "updated".into(),
            accent: false,
        });
    let mut replacement_surface = surface();
    replacement_surface.projection_revision = 2;
    replacement_surface.splits.push(PaneSurfaceSplit {
        direction: PaneSurfaceSplitDirection::Horizontal,
        pos: 40,
        area: SurfaceRect {
            x: 0,
            y: 0,
            width: 80,
            height: 19,
        },
        hit_rect: SurfaceRect {
            x: 40,
            y: 0,
            width: 1,
            height: 19,
        },
        path: vec![false, true],
    });
    state.set_snapshot(Box::new(replacement));
    state.set_pane_surface(replacement_surface);
    let drag = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: split.area.x + 48,
        row: split.hit_rect.y + 2,
        modifiers: KeyModifiers::empty(),
    })]);
    let [ClientShellAction::Endpoint { request, .. }] = &drag.actions[..] else {
        panic!("pane split drag should use endpoint API");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::LayoutSetSplitRatio(params)
            if params.tab_id.as_deref() == Some("tab_1")
                && params.path == vec![false, true]
                && (params.ratio - 0.6).abs() < f32::EPSILON
    ));
    let release =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: split.area.x + 48,
            row: split.hit_rect.y + 2,
            modifiers: KeyModifiers::empty(),
        })]);
    assert!(release.actions.is_empty());
    assert!(state.chrome_drag.is_none());
}

#[test]
fn disabled_mouse_chrome_keeps_tab_wheel_but_removes_split_drag_hits() {
    let mut config = Config::default();
    config.ui.mouse_capture = false;
    let mut projected = snapshot();
    let mut second_tab = projected.tabs[0].clone();
    second_tab.tab_id = "tab_2".into();
    second_tab.number = 2;
    second_tab.label = "2".into();
    second_tab.focused = false;
    projected.tabs.push(second_tab);
    let mut pane_surface = surface();
    pane_surface.splits.push(PaneSurfaceSplit {
        direction: PaneSurfaceSplitDirection::Horizontal,
        pos: 40,
        area: SurfaceRect {
            x: 0,
            y: 0,
            width: 80,
            height: 19,
        },
        hit_rect: SurfaceRect {
            x: 40,
            y: 0,
            width: 1,
            height: 19,
        },
        path: Vec::new(),
    });
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("mouse-disabled shell");
    assert!(state.hits.pane_splits.is_empty());
    let first_tab = state.hits.tabs[0].0;
    let wheel = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: first_tab.x,
        row: first_tab.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(
        &wheel.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(
                &request.method,
                crate::api::schema::Method::TabFocus(target) if target.tab_id == "tab_2"
            )
    ));
}

#[test]
fn client_double_click_selects_and_copies_endpoint_row_word() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("composed frame");
    let pane = state.hits.panes[0].clone();
    let click = || {
        RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: pane.inner_rect.x + 1,
            row: pane.inner_rect.y,
            modifiers: KeyModifiers::empty(),
        })
    };
    let release = || {
        RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: pane.inner_rect.x + 1,
            row: pane.inner_rect.y,
            modifiers: KeyModifiers::empty(),
        })
    };

    state.handle_raw_events(vec![click()]);
    state.handle_raw_events(vec![release()]);
    let second = state.handle_raw_events(vec![click()]);
    let ClientShellAction::Endpoint { request, .. } = second
        .actions
        .iter()
        .find(|action| {
            matches!(
                action,
                ClientShellAction::Endpoint { request, .. }
                    if matches!(request.method, crate::api::schema::Method::PaneSelectionRead(_))
            )
        })
        .expect("word-row read")
    else {
        unreachable!()
    };
    let word_request_id = request.id.clone();
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneSelectionRead(params)
            if params.anchor == crate::api::schema::PaneTextPoint { row: 0, col: 0 }
                && params.cursor == crate::api::schema::PaneTextPoint { row: 0, col: 3 }
    ));

    let (repaint, actions) = state.handle_endpoint_result(
        "boot-1",
        &word_request_id,
        Ok(crate::api::schema::ResponseResult::PaneSelection {
            pane_id: "pane_1".into(),
            text: "LIVE".into(),
        }),
    );
    assert!(repaint);
    assert!(state
        .selection
        .as_ref()
        .is_some_and(crate::selection::Selection::is_finalized));
    let [ClientShellAction::Endpoint { request, .. }] = &actions[..] else {
        panic!("auto-copy should read the selected word");
    };
    let copy_request_id = request.id.clone();
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneSelectionRead(params)
            if params.anchor.col == 0 && params.cursor.col == 3
    ));
    let (_, actions) = state.handle_endpoint_result(
        "boot-1",
        &copy_request_id,
        Ok(crate::api::schema::ResponseResult::PaneSelection {
            pane_id: "pane_1".into(),
            text: "LIVE".into(),
        }),
    );
    assert!(matches!(
        &actions[..],
        [ClientShellAction::ClipboardWrite(bytes)] if bytes == b"LIVE"
    ));
}

#[test]
fn pane_content_updates_preserve_active_selection_only_when_selected_cells_stay_stable() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let surface_at = |surface_revision, content_revision, alternate_screen_active| {
        let mut pane_surface = surface();
        pane_surface.surface_revision = surface_revision;
        pane_surface.panes[0].content_revision = content_revision;
        pane_surface.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
            offset_from_bottom: 0,
            max_offset_from_bottom: 11,
            viewport_rows: 2,
        });
        pane_surface.panes[0].alternate_screen_active = alternate_screen_active;
        pane_surface
    };
    state.set_pane_surface(surface_at(1, 0, true));
    state.compose(106, 20).expect("composed frame");
    let pane = state.hits.panes[0].clone();
    let mouse = |kind, column, row| {
        RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::empty(),
        })
    };

    state.handle_raw_events(vec![mouse(
        MouseEventKind::Down(MouseButton::Left),
        pane.inner_rect.x,
        pane.inner_rect.y + 1,
    )]);
    let mut updated_surface = surface_at(2, 2, true);
    updated_surface.frame.cells[0].symbol = "W".into();
    state.set_pane_surface(updated_surface);
    state.compose(106, 20).expect("updated frame");

    let drag = state.handle_raw_events(vec![mouse(
        MouseEventKind::Drag(MouseButton::Left),
        pane.inner_rect.x + 1,
        pane.inner_rect.y + 1,
    )]);

    assert!(drag.repaint);
    let selection = state.selection.as_ref().expect("visible selection");
    assert!(selection.is_visible());
    assert_eq!(selection.ordered_cells(), ((12, 0), (12, 1)));

    state.selection = Some(crate::selection::Selection::absolute_anchor(
        "pane_1".to_owned(),
        (12, 0),
    ));
    let mut replaced_surface = surface_at(3, 4, true);
    replaced_surface.frame.cells[4].symbol = "X".into();
    state.set_pane_surface(replaced_surface);
    assert!(state.selection.is_none());

    for (surface_revision, content_revision, width, alternate_screen_active) in
        [(4, 6, 4, false), (5, 8, 3, false), (6, 9, 3, false)]
    {
        state.selection = Some(crate::selection::Selection::absolute_anchor(
            "pane_1".to_owned(),
            (12, 0),
        ));
        let mut changed_surface =
            surface_at(surface_revision, content_revision, alternate_screen_active);
        changed_surface.panes[0].inner_rect.width = width;
        changed_surface.panes[0].alternate_screen_active = alternate_screen_active;
        state.set_pane_surface(changed_surface);
        assert!(state.selection.is_none());
    }
}

#[test]
fn pane_mouse_input_keeps_stable_target_and_endpoint_encoding() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].mouse_reporting = true;
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");
    let pane = state.hits.panes[0].clone();

    let click = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: pane.inner_rect.x + 2,
        row: pane.inner_rect.y + 1,
        modifiers: KeyModifiers::ALT,
    })]);
    let [ClientMessage::ClientShellPaneInput { pane_id, events }] = &click.requests[..] else {
        panic!("pane application click should use targeted canonical input");
    };
    assert_eq!(pane_id, "pane_1");
    assert!(matches!(
        &events[..],
        [ClientPaneInputEvent::Mouse {
            kind: crate::protocol::ClientMouseKind::Down(
                crate::protocol::ClientMouseButton::Left
            ),
            position: ClientMousePosition::Cell { column: 2, row: 1 },
            modifiers,
            ..
        }] if *modifiers == KeyModifiers::ALT.bits()
    ));
    let moved = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Moved,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::ALT,
    })]);
    assert!(moved.requests.is_empty());
    assert!(state.pane_mouse_gesture.is_some());
    state.hits.panes.clear();
    let release =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::ALT,
        })]);
    assert!(matches!(
        &release.requests[..],
        [ClientMessage::ClientShellPaneInput { pane_id, events }]
            if pane_id == "pane_1"
                && matches!(
                    &events[..],
                    [ClientPaneInputEvent::Mouse {
                        kind: crate::protocol::ClientMouseKind::Up(
                            crate::protocol::ClientMouseButton::Left
                        ),
                        ..
                    }]
                )
    ));
    assert!(state.pane_mouse_gesture.is_none());
}

#[test]
fn pane_pixel_mouse_preserves_pane_relative_pixel_coordinates() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].mouse_reporting = true;
    pane_surface.panes[0].sgr_pixel_mouse = true;
    pane_surface.panes[0].pixel_width = 39;
    pane_surface.panes[0].pixel_height = 38;
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");
    let pane = state.hits.panes[0].clone();
    let geometry =
        crate::input::mouse::HostGeometry::new(106, 20, 1060, 400).expect("host geometry");
    let x = u32::from(pane.inner_rect.x) * 10 + 21;
    let y = u32::from(pane.inner_rect.y) * 20 + 21;
    let report = format!("\x1b[<0;{x};{y}M");

    let outcome = state.handle_pixel_mouse(report.as_bytes(), geometry);
    assert!(matches!(
        &outcome.requests[..],
        [ClientMessage::ClientShellPaneInput { pane_id, events }]
            if pane_id == "pane_1"
                && matches!(
                    &events[..],
                    [ClientPaneInputEvent::Mouse {
                        kind: crate::protocol::ClientMouseKind::Down(
                            crate::protocol::ClientMouseButton::Left
                        ),
                        position: ClientMousePosition::Pixels { x: 20, y: 20, .. },
                        ..
                    }]
                )
    ));

    let lost = state.handle_raw_events(vec![RawInputEvent::OuterFocusLost]);
    assert!(matches!(
        &lost.requests[..],
        [
            ClientMessage::ClientShellPaneInput { pane_id, events },
            ClientMessage::ClientShellFocus { focused: false }
        ] if pane_id == "pane_1" && matches!(
            &events[..],
            [ClientPaneInputEvent::Mouse {
                kind: crate::protocol::ClientMouseKind::Up(
                    crate::protocol::ClientMouseButton::Left
                ),
                position: ClientMousePosition::Pixels { x: 20, y: 20, .. },
                ..
            }]
        )
    ));
}

#[test]
fn pane_owned_right_click_forwards_the_complete_gesture() {
    let mut snapshot = snapshot();
    snapshot.panes[0].right_click_passthrough = true;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot));
    let mut pane_surface = surface();
    pane_surface.panes[0].mouse_reporting = true;
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");
    let pane = state.hits.panes[0].clone();

    let down = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: pane.inner_rect.x + 1,
        row: pane.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(
        &down.requests[..],
        [ClientMessage::ClientShellPaneInput { pane_id, .. }] if pane_id == "pane_1"
    ));
    assert!(state.overlay.is_none());
    assert!(state.pane_mouse_gesture.is_some());

    let up = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Right),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(
        &up.requests[..],
        [ClientMessage::ClientShellPaneInput { pane_id, events }]
            if pane_id == "pane_1"
                && matches!(
                    &events[..],
                    [ClientPaneInputEvent::Mouse {
                        kind: crate::protocol::ClientMouseKind::Up(
                            crate::protocol::ClientMouseButton::Right
                        ),
                        ..
                    }]
                )
    ));
    assert!(state.pane_mouse_gesture.is_none());
}

#[test]
fn tab_click_waits_for_release_and_drag_reorders_by_stable_id() {
    let mut projected = snapshot();
    for index in 2..=3 {
        let mut tab = projected.tabs[0].clone();
        tab.tab_id = format!("tab_{index}");
        tab.number = index;
        tab.label = index.to_string();
        tab.focused = false;
        projected.tabs.push(tab);
    }
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("three tabs");
    let first = state.hits.tabs[0].0;
    let third = state.hits.tabs[2].0;

    let down = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: first.x + 1,
        row: first.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(down.actions.is_empty());
    let drag = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: third.right().saturating_sub(1),
        row: third.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(drag.repaint);
    assert!(matches!(
        state.chrome_drag,
        Some(ClientChromeDrag::Tab {
            ref tab_id,
            insert_index: Some(3),
            ..
        }) if tab_id == "tab_1"
    ));
    let frame = state.compose(106, 20).expect("tab drop indicator");
    assert!(frame
        .cells
        .iter()
        .take(frame.width as usize)
        .any(|cell| cell.symbol == "│"));

    let release =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: third.right().saturating_sub(1),
            row: third.y,
            modifiers: KeyModifiers::empty(),
        })]);
    let [ClientShellAction::Endpoint { request, .. }] = &release.actions[..] else {
        panic!("tab drag should use endpoint API");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::TabMove(params)
            if params.tab_id == "tab_1" && params.insert_index == 3
    ));

    state.compose(106, 20).expect("tabs after drag");
    let second = state.hits.tabs[1].0;
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: second.x + 1,
        row: second.y,
        modifiers: KeyModifiers::empty(),
    })]);
    let click = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: second.x + 1,
        row: second.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(
        &click.actions[0],
        ClientShellAction::Endpoint { request, .. }
            if matches!(&request.method, crate::api::schema::Method::TabFocus(target) if target.tab_id == "tab_2")
    ));
}

#[test]
fn tab_drag_clears_its_drop_target_after_leaving_the_tab_row() {
    let mut projected = snapshot();
    for index in 2..=3 {
        let mut tab = projected.tabs[0].clone();
        tab.tab_id = format!("tab_{index}");
        tab.number = index;
        tab.label = index.to_string();
        tab.focused = false;
        projected.tabs.push(tab);
    }
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("three tabs");
    let first = state.hits.tabs[0].0;
    let third = state.hits.tabs[2].0;
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: first.x + 1,
        row: first.y,
        modifiers: KeyModifiers::empty(),
    })]);
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: third.x,
        row: third.y,
        modifiers: KeyModifiers::empty(),
    })]);
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: third.x,
        row: third.y + 1,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(
        state.chrome_drag,
        Some(ClientChromeDrag::Tab {
            insert_index: None,
            ..
        })
    ));
    let release =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: third.x,
            row: third.y + 1,
            modifiers: KeyModifiers::empty(),
        })]);
    assert!(release.actions.is_empty());
}

#[test]
fn tab_wheel_switches_tabs_without_changing_overflow_scroll() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("tab bar");
    let tab = state.hits.tabs[0].0;

    let outcome =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: tab.x,
            row: tab.y,
            modifiers: KeyModifiers::empty(),
        })]);
    assert!(matches!(
        &outcome.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(
                &request.method,
                crate::api::schema::Method::TabFocus(target) if target.tab_id == "tab_1"
            )
    ));
    assert_eq!(state.tab_scroll, 0);
    state.compose(106, 20).expect("tab bar after wheel");
    assert!(state.hits.tabs.iter().any(|(_, tab_id)| tab_id == "tab_1"));
}

#[test]
fn context_menu_keyboard_and_outside_click_are_client_owned() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("composed frame");
    let tab = state.hits.tabs[0].0;
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: tab.x + 1,
        row: tab.y,
        modifiers: KeyModifiers::empty(),
    })]);
    state.compose(106, 20).expect("tab context menu");
    let moved = state.handle_input_bytes(b"\x1b[B");
    assert!(moved.repaint);
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::ContextMenu(ClientContextMenuOverlay {
            highlighted: 1,
            ..
        }))
    ));
    let text = state.handle_raw_events(vec![RawInputEvent::Text(crate::input::TextCommit::new(
        "not pane input",
    ))]);
    assert!(text.requests.is_empty());
    let paste = state.handle_raw_events(vec![RawInputEvent::Paste("not pane input".into())]);
    assert!(paste.requests.is_empty());
    let outside =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 105,
            row: 19,
            modifiers: KeyModifiers::empty(),
        })]);
    assert!(outside.repaint);
    assert!(state.overlay.is_none());
}
