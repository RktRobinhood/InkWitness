//! The player dock across the bottom of the review page (FAL-519, from FAL-477): a video-style
//! timeline with a pin for every flag, and the transport (Play / Pause, previous and next event
//! or flag, speed, Skip idle, Pause at flags).
//!
//! Playback runs on each event's recorded `elapsedMonotonicMs`, sped up, with long pauses
//! squeezed out ([`Timeline`]), so a reviewer watches the writing at a pace they can follow, not
//! in real time. The timecode always shows the real recorded time. Playing forward only ever
//! applies the next events to the shown state ([`wordcraft_inkwitness::replay::Replay::advance`]),
//! and the page is redrawn at most [`STEPS_PER_SECOND`] times a second.

use egui::{Align, CornerRadius, Key, Layout, Painter, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui, pos2, vec2};
use serde_json::Value;
use wordcraft_ui_egui::icons;
use wordcraft_ui_egui::theme::critic::Accents;
use wordcraft_ui_egui::theme::{Tokens, medium, regular, semibold};

use crate::card::{self, focus_ring};
use crate::flags::{Flag, FlagKind};
use crate::review::Loaded;

/// A pause longer than this (ms) is idle time…
pub const IDLE_GAP_MS: u64 = 2_000;
/// …and, with Skip idle on, plays (and takes room on the timeline) as this long.
pub const IDLE_PLAYS_AS_MS: u64 = 500;
/// The speeds the menu offers.
pub const SPEEDS: [f32; 6] = [0.5, 1.0, 2.0, 4.0, 8.0, 16.0];
/// The speed a review starts at.
pub const DEFAULT_SPEED: f32 = 4.0;
/// Most visible replay steps a second while playing.
pub const STEPS_PER_SECOND: f64 = 15.0;
/// The dock's height.
pub const HEIGHT: f32 = 108.0;
/// Pins closer than this (points) merge into one pin with a count.
pub const CLUSTER_PX: f32 = 14.0;

/// The replay's time scale: where each event sits, in ms from where the writing starts, with
/// idle gaps squeezed out when Skip idle is on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timeline {
    /// Each event's place on the scale; index `seq - 1`.
    at: Vec<f64>,
    /// Each event's recorded time since the session started (ms); index `seq - 1`.
    real: Vec<u64>,
    /// Where an idle gap was squeezed out, on the scale.
    pub breaks: Vec<f64>,
    /// The scale's length (the last event's place).
    pub total: f64,
    /// Whether idle gaps are squeezed out.
    pub skip_idle: bool,
    /// Where the writing starts ([`crate::review::writing_starts`]): this event and the ones
    /// before it sit at the start of the scale, and Play starts from it.
    pub first: u64,
    /// The last event of each step ([`crate::steps::ends`]): a word of typing, or any other event.
    ends: Vec<u64>,
}

impl Timeline {
    /// The scale for the first `upto` of `events`, starting at event `first`.
    pub fn new(events: &[Value], first: u64, upto: u64, skip_idle: bool) -> Timeline {
        let mut t = Timeline { skip_idle, first, ..Timeline::default() };
        let (mut prev, mut pos) = (0u64, 0.0f64);
        for (i, e) in events.iter().take(usize::try_from(upto).unwrap_or(usize::MAX)).enumerate() {
            // Never backwards: a missing or earlier time counts as no time at all.
            let ms = e.get("elapsedMonotonicMs").and_then(Value::as_u64).unwrap_or(prev).max(prev);
            let gap = ms - prev;
            if (i as u64) < first {
                // Before the writing starts: takes no room.
            } else if skip_idle && gap > IDLE_GAP_MS {
                t.breaks.push(pos + IDLE_PLAYS_AS_MS as f64 / 2.0);
                pos += IDLE_PLAYS_AS_MS as f64;
            } else {
                pos += gap as f64;
            }
            t.at.push(pos);
            t.real.push(ms);
            prev = ms;
        }
        t.total = pos;
        t.ends = crate::steps::ends(events, upto);
        t
    }

    /// The last event of each step, in order.
    pub fn step_ends(&self) -> &[u64] {
        &self.ends
    }

    /// The event that ends the step before the one `seq` is in (not back past `first`).
    pub fn prev_step(&self, seq: u64) -> Option<u64> {
        if seq <= self.first {
            return None;
        }
        let before = self.ends.partition_point(|&e| e < seq);
        Some(before.checked_sub(1).and_then(|i| self.ends.get(i)).copied().unwrap_or(seq - 1).max(self.first))
    }

    /// The event that ends the step after the one `seq` is in (not past `last`).
    pub fn next_step(&self, seq: u64, last: u64) -> Option<u64> {
        if seq >= last {
            return None;
        }
        let at = self.ends.partition_point(|&e| e <= seq);
        Some(self.ends.get(at).copied().unwrap_or(seq + 1).min(last))
    }

    /// The last step end at or before event `seq`: playback shows whole words, not half of one.
    fn snap(&self, seq: u64) -> u64 {
        let at = self.ends.partition_point(|&e| e <= seq);
        at.checked_sub(1).and_then(|i| self.ends.get(i)).copied().unwrap_or(0)
    }

    /// Event `seq`'s place on the scale (0: where the writing starts, or before).
    pub fn time_of(&self, seq: u64) -> f64 {
        seq.checked_sub(1).and_then(|i| self.at.get(usize::try_from(i).ok()?)).copied().unwrap_or(0.0)
    }

    /// Event `seq`'s recorded time since the session started (ms).
    pub fn real_of(&self, seq: u64) -> u64 {
        seq.checked_sub(1).and_then(|i| self.real.get(usize::try_from(i).ok()?)).copied().unwrap_or(0)
    }

    /// The recorded length of the session (ms).
    pub fn real_total(&self) -> u64 {
        self.real.last().copied().unwrap_or(0)
    }

    /// The last event at or before `t` on the scale (at least [`Timeline::first`]).
    pub fn seq_at(&self, t: f64) -> u64 {
        u64::try_from(self.at.partition_point(|&x| x <= t)).unwrap_or(0)
    }

    /// How many events there are.
    pub fn len(&self) -> usize {
        self.at.len()
    }

    pub fn is_empty(&self) -> bool {
        self.at.is_empty()
    }
}

/// The transport's state.
#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub playing: bool,
    pub speed: f32,
    pub skip_idle: bool,
    pub pause_at_flags: bool,
    /// Where playback is, on the [`Timeline`]'s scale.
    pub clock: f64,
    last_time: Option<f64>,
    last_step: f64,
}

impl Default for Player {
    fn default() -> Self {
        Player { playing: false, speed: DEFAULT_SPEED, skip_idle: true, pause_at_flags: true, clock: 0.0, last_time: None, last_step: f64::MIN }
    }
}

impl Player {
    /// Start playing from event `seq`. At the end, or before the writing starts, playback starts
    /// where the writing starts: the event to show first.
    pub fn play(&mut self, tl: &Timeline, seq: u64, last: u64) -> Option<u64> {
        self.playing = true;
        self.last_time = None;
        if seq >= last || seq < tl.first {
            self.clock = 0.0;
            return Some(tl.first);
        }
        self.clock = tl.time_of(seq);
        None
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.last_time = None;
    }

    /// The reviewer moved to `seq` by hand: playback pauses there.
    pub fn moved(&mut self, tl: &Timeline, seq: u64) {
        self.pause();
        self.clock = tl.time_of(seq);
    }

    /// Run the clock to `now` (seconds). Returns the event to show, when a later one than `seq`
    /// is due and a step is allowed. With Pause at flags on, playback stops on the first flagged
    /// event it reaches (`flagged`: their `seq`, in order); at `last` it stops.
    pub fn tick(&mut self, now: f64, tl: &Timeline, seq: u64, last: u64, flagged: &[u64]) -> Option<u64> {
        if !self.playing {
            self.last_time = None;
            return None;
        }
        let dt = self.last_time.map_or(0.0, |t| (now - t).clamp(0.0, 0.25));
        self.last_time = Some(now);
        self.clock += dt * 1000.0 * f64::from(self.speed);
        if seq >= last {
            self.pause();
            return None;
        }
        if now - self.last_step < 1.0 / STEPS_PER_SECOND {
            return None;
        }
        let mut due = tl.snap(tl.seq_at(self.clock).min(last));
        if due <= seq {
            return None;
        }
        if self.pause_at_flags
            && let Some(&f) = flagged.iter().find(|&&f| f > seq && f <= due)
        {
            due = f;
            self.pause();
            self.clock = tl.time_of(f);
        }
        if due >= last {
            self.pause();
        }
        self.last_step = now;
        Some(due)
    }
}

/// What the reviewer asked the dock for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockAction {
    /// Show the state after this event (playback pauses).
    Seek(u64),
    Play,
    Pause,
    /// Open a comment draft on the selection, or on the moment shown.
    Comment,
}

/// "1:05" (or "1:02:05") for `ms`.
pub fn timecode(ms: u64) -> String {
    let secs = ms / 1000;
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// The flagged event before `seq`, and the one after it.
pub fn neighbours(flagged: &[u64], seq: u64) -> (Option<u64>, Option<u64>) {
    (flagged.iter().rev().find(|&&f| f < seq).copied(), flagged.iter().find(|&&f| f > seq).copied())
}

/// The keyboard shortcuts, when no text field has the keyboard: Space or K plays and pauses,
/// ← and → step one step: a word of typing, or one other event (not back past `first`, where the writing starts), [ and ] go to the
/// previous and next flag.
pub fn shortcuts(ui: &Ui, playing: bool, seq: u64, last: u64, flagged: &[u64], tl: &Timeline) -> Option<DockAction> {
    let (prev_flag, next_flag) = neighbours(flagged, seq);
    ui.input_mut(|i| {
        let mut key = |k: Key| i.consume_key(egui::Modifiers::NONE, k);
        if key(Key::Space) || key(Key::K) {
            Some(if playing { DockAction::Pause } else { DockAction::Play })
        } else if key(Key::ArrowLeft) {
            tl.prev_step(seq).map(DockAction::Seek)
        } else if key(Key::ArrowRight) {
            tl.next_step(seq, last).map(DockAction::Seek)
        } else if key(Key::OpenBracket) {
            prev_flag.map(DockAction::Seek)
        } else if key(Key::CloseBracket) {
            next_flag.map(DockAction::Seek)
        } else {
            None
        }
    })
}

/// Draw the dock: the timeline, then the transport. Speed, Skip idle and Pause at flags change
/// `player` directly.
pub fn show(ui: &mut Ui, player: &mut Player, l: &Loaded, flags: &[Flag], tl: &Timeline) -> Option<DockAction> {
    let t = Tokens::get(ui.ctx());
    let a = Accents::get(ui.ctx());
    let seq = l.state.seq;
    let last = l.replayable;
    let flagged = crate::flags::moments(flags);
    let (prev_flag, next_flag) = neighbours(&flagged, seq);
    let mut action = None;
    ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
    if let Some(s) = timeline(ui, l, flags, tl, &t, &a) {
        action = Some(DockAction::Seek(s));
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
        let go = |ui: &mut Ui, icon: &str, name: &str, tip: &str, to: Option<u64>| {
            if icon_button(ui, icon, name, tip, to.is_some(), &t, &a) { to.map(DockAction::Seek) } else { None }
        };
        let mut pressed = [
            go(ui, "replayRewind", "Previous flag", "Previous flag  [", prev_flag),
            go(ui, "mediaPrev", "Previous event", "Previous step (a word of typing, or one event)  ←", tl.prev_step(seq)),
            play_button(ui, player.playing, &t, &a).then_some(if player.playing { DockAction::Pause } else { DockAction::Play }),
        ]
        .to_vec();
        pressed.push(go(ui, "mediaNext", "Next event", "Next step (a word of typing, or one event)  →", tl.next_step(seq, last)));
        pressed.push(go(ui, "replayForward", "Next flag", "Next flag  ]", next_flag));
        if let Some(p) = pressed.into_iter().flatten().next() {
            action = Some(p);
        }
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{} / {}", timecode(tl.real_of(seq)), timecode(tl.real_total()))).font(semibold(15.0)).color(t.text))
                .on_hover_text("Recorded time into the writing session");
            ui.label(RichText::new(format!("Event {seq} of {}", l.replay.last_seq())).font(regular(12.0)).color(t.text_dim));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 0.0);
            let comment = ui.add(
                egui::Button::new(RichText::new("+ Add comment").font(semibold(13.0)).color(t.on_accent)).fill(t.accent).min_size(vec2(124.0, 36.0)),
            );
            focus_ring(ui, &comment, &a);
            if comment.on_hover_text("Comment on the text you selected on the page, or on the moment shown").clicked() {
                action = Some(DockAction::Comment);
            }
            ui.separator();
            let pause = ui.checkbox(&mut player.pause_at_flags, RichText::new("Pause at flags").font(medium(12.5)).color(t.text));
            focus_ring(ui, &pause, &a);
            pause.on_hover_text("Stop playing at each flagged moment");
            let idle = ui.checkbox(&mut player.skip_idle, RichText::new("Skip idle").font(medium(12.5)).color(t.text));
            focus_ring(ui, &idle, &a);
            idle.on_hover_text(format!(
                "Pauses longer than {} s play as {} s. The timecode still shows the real time.",
                IDLE_GAP_MS / 1000,
                IDLE_PLAYS_AS_MS as f64 / 1000.0
            ));
            let speed = egui::ComboBox::from_id_salt("inkwitness_speed")
                .selected_text(RichText::new(format!("{}×", player.speed)).font(medium(12.5)))
                .width(64.0)
                .show_ui(ui, |ui| {
                    for s in SPEEDS {
                        ui.selectable_value(&mut player.speed, s, format!("{s}×"));
                    }
                });
            speed.response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, "Playback speed"));
            speed.response.on_hover_text("Playback speed");
            ui.label(RichText::new("Speed").font(regular(12.0)).color(t.text_dim));
        });
    });
    action
}

/// A square transport button with an icon, at least 36 points, named for screen readers.
fn icon_button(ui: &mut Ui, icon: &str, name: &str, tip: &str, on: bool, t: &Tokens, a: &Accents) -> bool {
    let r = ui.add_enabled(on, egui::Button::new("").min_size(vec2(38.0, 38.0)).corner_radius(CornerRadius::same(19)));
    icons::paint(ui.painter(), Rect::from_center_size(r.rect.center(), vec2(18.0, 18.0)), icon, if on { t.icon } else { t.text_disabled }, t.accent);
    let label = name.to_string();
    r.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, on, &label));
    focus_ring(ui, &r, a);
    r.on_hover_text(tip).clicked()
}

/// The big round Play / Pause button.
fn play_button(ui: &mut Ui, playing: bool, t: &Tokens, a: &Accents) -> bool {
    let (name, icon) = if playing { ("Pause", "mediaPause") } else { ("Play", "mediaPlay") };
    let r = ui.add(egui::Button::new("").min_size(vec2(48.0, 48.0)).corner_radius(CornerRadius::same(24)).fill(t.accent));
    icons::paint(ui.painter(), Rect::from_center_size(r.rect.center(), vec2(22.0, 22.0)), icon, t.on_accent, t.on_accent);
    r.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    focus_ring(ui, &r, a);
    r.on_hover_text(format!("{name}  Space")).clicked()
}

/// Pins that sit within [`CLUSTER_PX`] of each other: their x, and the flags.
fn clusters(flags: &[Flag], x_of: impl Fn(u64) -> f32) -> Vec<(f32, Vec<&Flag>)> {
    let mut out: Vec<(f32, Vec<&Flag>)> = Vec::new();
    for f in flags {
        let x = x_of(f.seq);
        match out.last_mut() {
            Some((cx, v)) if x - *cx < CLUSTER_PX => v.push(f),
            _ => out.push((x, vec![f])),
        }
    }
    out
}

/// The timeline: track, played part, idle breaks, editing density, flag pins and the playhead.
/// Returns the event to seek to.
fn timeline(ui: &mut Ui, l: &Loaded, flags: &[Flag], tl: &Timeline, t: &Tokens, a: &Accents) -> Option<u64> {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Replay timeline"));
    let p = ui.painter_at(rect.expand(4.0));
    let pad = 12.0;
    let (x0, x1) = (rect.left() + pad, rect.right() - pad);
    let total = tl.total.max(1.0);
    let x_at = |ms: f64| x0 + (x1 - x0) * (ms / total).clamp(0.0, 1.0) as f32;
    let x_of = |seq: u64| x_at(tl.time_of(seq));
    let track_y = rect.top() + 30.0;
    let hot = resp.hovered() || resp.dragged();
    let thick = if hot { 8.0 } else { 5.0 };
    let seq = l.state.seq;
    // Editing density: events per bucket, a quiet strip under the track.
    let buckets = ((x1 - x0) / 4.0).max(1.0) as usize;
    let mut density = vec![0u32; buckets];
    for s in 1..=u64::try_from(tl.len()).unwrap_or(0) {
        let b = (((x_of(s) - x0) / (x1 - x0)) * (buckets as f32 - 1.0)).round() as usize;
        if let Some(d) = density.get_mut(b.min(buckets - 1)) {
            *d += 1;
        }
    }
    let most = density.iter().copied().max().unwrap_or(0).max(1) as f32;
    for (i, d) in density.iter().enumerate().filter(|(_, d)| **d > 0) {
        let x = x0 + (x1 - x0) * i as f32 / (buckets as f32 - 1.0).max(1.0);
        let h = 2.0 + 6.0 * (*d as f32 / most);
        p.rect_filled(
            Rect::from_min_size(pos2(x - 1.5, track_y + thick / 2.0 + 3.0), vec2(3.0, h)),
            CornerRadius::same(1),
            t.text_dim.gamma_multiply(0.35),
        );
    }
    // The track and its played part.
    let track = Rect::from_min_max(pos2(x0, track_y - thick / 2.0), pos2(x1, track_y + thick / 2.0));
    p.rect_filled(track, CornerRadius::same(4), t.border);
    let head = x_of(seq);
    p.rect_filled(Rect::from_min_max(track.min, pos2(head, track.max.y)), CornerRadius::same(4), t.accent);
    // Squeezed-out idle time: a small break in the track.
    for &b in &tl.breaks {
        let x = x_at(b);
        p.line_segment([pos2(x - 1.5, track.max.y + 1.0), pos2(x + 1.5, track.min.y - 1.0)], Stroke::new(1.5, a.card));
    }
    // Flag pins, above the track.
    let pointer = ui.ctx().pointer_hover_pos();
    let pin_y = track_y - 17.0;
    let mut hovered: Option<Vec<&Flag>> = None;
    for (x, group) in clusters(flags, x_of) {
        let c = pos2(x, pin_y);
        let over = hot && pointer.is_some_and(|q| (q.x - x).abs() <= 9.0 && q.y <= track_y - 2.0);
        let lead = group.iter().copied().find(|f| f.kind.strong()).unwrap_or(group[0]);
        p.line_segment([pos2(x, pin_y + 6.0), pos2(x, track.min.y)], Stroke::new(1.0, a.observation.text.gamma_multiply(0.6)));
        pin(&p, c, lead.kind, a, over);
        if group.len() > 1 {
            let n = group.len().to_string();
            let g = p.layout_no_wrap(n, semibold(10.0), a.observation.text);
            let br = Rect::from_center_size(pos2(x + 9.0, pin_y - 8.0), vec2(g.size().x + 7.0, 14.0));
            p.rect(br, CornerRadius::same(7), a.observation.fill, Stroke::new(1.0, a.observation.edge), egui::StrokeKind::Inside);
            p.galley(br.center() - g.size() / 2.0, g, a.observation.text);
        }
        if over {
            hovered = Some(group);
        }
    }
    // The playhead.
    let r = if hot { 10.0 } else { 8.0 };
    p.circle(pos2(head, track_y), r, t.accent, Stroke::new(2.0, a.card));
    focus_ring(ui, &resp, a);
    // Hover: a pin's card, or the time under the pointer.
    let mut seek = None;
    if let Some(group) = &hovered {
        egui::Tooltip::always_open(ui.ctx().clone(), ui.layer_id(), egui::Id::new("inkwitness_pin_tip"), egui::PopupAnchor::Pointer).show(|ui| {
            ui.set_max_width(320.0);
            for (i, f) in group.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                pin_card(ui, l, f, t, a);
            }
            ui.label(
                RichText::new(if group.len() > 1 { "Click to jump to the first" } else { "Click to jump" }).font(regular(11.5)).color(t.text_dim),
            );
        });
        if resp.clicked() {
            seek = group.first().map(|f| f.seq);
        }
    } else if let Some(q) = pointer.filter(|q| rect.contains(*q)) {
        let at = tl.seq_at(f64::from((q.x - x0) / (x1 - x0)).clamp(0.0, 1.0) * total);
        egui::Tooltip::always_open(ui.ctx().clone(), ui.layer_id(), egui::Id::new("inkwitness_time_tip"), egui::PopupAnchor::Pointer).show(|ui| {
            ui.label(RichText::new(format!("{} · Event {at}", timecode(tl.real_of(at)))).font(medium(12.0)));
        });
        if resp.clicked() || resp.dragged() {
            seek = Some(at);
        }
    }
    seek.filter(|s| *s != seq || resp.clicked())
}

/// A flag's hover card: its chip, when, how much, and the start of its text.
fn pin_card(ui: &mut Ui, l: &Loaded, f: &Flag, t: &Tokens, a: &Accents) {
    let Some(c) = crate::rail::event_card(l, f.seq, None) else { return };
    ui.spacing_mut().item_spacing = vec2(6.0, 3.0);
    card::chip(ui, f.kind.icon(), f.kind.name(), crate::rail::flag_tint(f.kind, a));
    if let Some(m) = &f.matched {
        ui.label(RichText::new(format!("Matched wording: “{m}”")).font(medium(12.0)).color(t.text));
    }
    let when: Vec<String> = [c.recorded, c.elapsed].into_iter().flatten().collect();
    if !when.is_empty() {
        ui.label(RichText::new(when.join(" · ")).font(regular(12.0)).color(t.text));
    }
    if let Some(s) = &c.size {
        ui.label(RichText::new(s).font(regular(12.0)).color(t.text_dim));
    }
    if let Some(x) = &c.excerpt {
        let short: String = x.replace(wordcraft_inkwitness::PARA_MARK, " ").chars().take(100).collect();
        let more = if x.chars().count() > 100 { "…" } else { "" };
        ui.label(RichText::new(format!("“{}{more}”", short.trim())).font(regular(12.0)).color(t.text).italics());
    }
}

/// Draw a pin for a flag of `kind` centred at `c`. Each kind has its own shape, so colour is
/// never the only cue.
pub fn pin(p: &Painter, c: Pos2, kind: FlagKind, a: &Accents, hot: bool) {
    let s = if hot { 1.25 } else { 1.0 };
    let gold = a.observation.edge;
    let ink = Stroke::new(1.5, a.observation.text);
    let poly = |pts: Vec<Pos2>, fill| Shape::convex_polygon(pts, fill, ink);
    match kind {
        FlagKind::Paste => {
            p.circle(c, 4.5 * s, a.card, Stroke::new(2.0, a.observation.text));
        }
        FlagKind::LargePaste => {
            let r = 7.5 * s;
            p.add(poly(vec![c + vec2(0.0, -r), c + vec2(r, 0.0), c + vec2(0.0, r), c + vec2(-r, 0.0)], gold));
        }
        FlagKind::LargeInsert => {
            p.rect(Rect::from_center_size(c, vec2(11.0, 11.0) * s), CornerRadius::same(2), gold, ink, egui::StrokeKind::Inside);
        }
        FlagKind::LargeDelete => {
            let r = 7.0 * s;
            p.add(poly(vec![c + vec2(-r, -r * 0.7), c + vec2(r, -r * 0.7), c + vec2(0.0, r)], gold));
        }
        FlagKind::Imported => {
            p.rect(Rect::from_center_size(c, vec2(6.0, 13.0) * s), CornerRadius::same(1), a.word_comment.edge, ink, egui::StrokeKind::Inside);
        }
        FlagKind::AiPhrase => {
            // An eight-pointed star: the strongest lead stands out by shape, not colour alone.
            let (ro, ri) = (9.0 * s, 4.2 * s);
            let pts: Vec<Pos2> = (0..16)
                .map(|i| {
                    let ang = std::f32::consts::PI * i as f32 / 8.0 - std::f32::consts::FRAC_PI_2;
                    let r = if i % 2 == 0 { ro } else { ri };
                    c + vec2(ang.cos(), ang.sin()) * r
                })
                .collect();
            p.add(Shape::Path(egui::epaint::PathShape::closed_line(pts.clone(), ink)));
            // Fill: the star is not convex, so fill it as triangles from the centre.
            let mut mesh = egui::Mesh::default();
            mesh.colored_vertex(c, gold);
            for q in &pts {
                mesh.colored_vertex(*q, gold);
            }
            let n = pts.len() as u32;
            for i in 0..n {
                mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
            }
            p.add(Shape::mesh(mesh));
            p.add(Shape::Path(egui::epaint::PathShape::closed_line(pts, ink)));
        }
        FlagKind::HistoryStops => {
            let r = 6.0 * s;
            let st = Stroke::new(3.0, a.corrupt.edge);
            p.line_segment([c + vec2(-r, -r), c + vec2(r, r)], st);
            p.line_segment([c + vec2(-r, r), c + vec2(r, -r)], st);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn events(ms: &[u64]) -> Vec<Value> {
        ms.iter().enumerate().map(|(i, m)| json!({"seq": i + 1, "elapsedMonotonicMs": m})).collect()
    }

    #[test]
    fn skip_idle_squeezes_long_pauses_and_keeps_the_real_time() {
        let ev = events(&[100, 300, 60_300, 60_400]);
        let tl = Timeline::new(&ev, 0, 4, true);
        assert_eq!(tl.time_of(0), 0.0);
        assert_eq!(tl.time_of(2), 300.0);
        // The minute's pause plays as half a second.
        assert_eq!(tl.time_of(3), 300.0 + IDLE_PLAYS_AS_MS as f64);
        assert_eq!(tl.breaks.len(), 1);
        assert_eq!(tl.real_of(3), 60_300);
        assert_eq!(tl.real_total(), 60_400);
        let full = Timeline::new(&ev, 0, 4, false);
        assert_eq!(full.time_of(3), 60_300.0);
        assert!(full.breaks.is_empty());
        assert_eq!(tl.seq_at(0.0), 0);
        assert_eq!(tl.seq_at(299.0), 1);
        assert_eq!(tl.seq_at(10_000.0), 4);
    }

    #[test]
    fn playback_runs_at_speed_steps_at_most_fifteen_times_a_second_and_stops_at_flags() {
        let ev = events(&[100, 200, 300, 400, 500, 600]);
        let tl = Timeline::new(&ev, 0, 6, true);
        let mut p = Player { speed: 1.0, ..Player::default() };
        assert_eq!(p.play(&tl, 0, 6), None);
        assert_eq!(p.tick(0.0, &tl, 0, 6, &[]), None, "no time has passed");
        // 250 ms later at 1×: events 1 and 2 are due, shown as one step.
        assert_eq!(p.tick(0.25, &tl, 0, 6, &[]), Some(2));
        // Too soon for another step.
        assert_eq!(p.tick(0.26, &tl, 2, 6, &[]), None);
        // Pause at flags: playback stops on flagged event 4 even though 5 is due.
        assert_eq!(p.tick(0.5, &tl, 2, 6, &[4]), Some(4));
        assert!(!p.playing);
        // Playing again from the flag doesn't stop on it again.
        assert_eq!(p.play(&tl, 4, 6), None);
        p.tick(1.0, &tl, 4, 6, &[4]);
        assert_eq!(p.tick(1.25, &tl, 4, 6, &[4]), Some(6));
        assert!(!p.playing, "stops at the end");
        // From the end, Play starts over.
        assert_eq!(p.play(&tl, 6, 6), Some(0));
    }

    #[test]
    fn the_scale_starts_where_the_writing_starts() {
        // Events 1 and 2 are the writer starting up; the writing starts after event 2.
        let ev = events(&[1_276, 1_279, 2_545, 2_692, 60_000]);
        let tl = Timeline::new(&ev, 2, 5, true);
        assert_eq!((tl.time_of(1), tl.time_of(2)), (0.0, 0.0));
        assert_eq!(tl.time_of(3), 1_266.0);
        assert_eq!(tl.seq_at(0.0), 2);
        // The real time still counts from the start of the session.
        assert_eq!(tl.real_of(3), 2_545);
        let mut p = Player::default();
        assert_eq!(p.play(&tl, 5, 5), Some(2), "Play from the end starts over at the writing");
        assert_eq!(p.play(&tl, 1, 5), Some(2));
        assert_eq!(p.play(&tl, 3, 5), None);
    }

    #[test]
    fn flags_on_either_side() {
        assert_eq!(neighbours(&[3, 8, 12], 8), (Some(3), Some(12)));
        assert_eq!(neighbours(&[3, 8, 12], 0), (None, Some(3)));
        assert_eq!(timecode(65_000), "1:05");
        assert_eq!(timecode(3_725_000), "1:02:05");
    }
}
