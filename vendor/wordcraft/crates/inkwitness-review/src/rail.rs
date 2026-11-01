//! The "Observations & notes" rail at the right of the review page (FAL-448): the file's header
//! (name, student, assessment, history status), the event navigation, the card for the moment
//! shown, and a card for each observation in the history.

use egui::{CornerRadius, Rect, RichText, Sense, Stroke, Ui, vec2};
use serde_json::Value;
use wordcraft_ui_egui::WordApp;
use wordcraft_ui_egui::theme::critic::{Accents, Tint};
use wordcraft_ui_egui::theme::{Tokens, medium, regular, semibold};

use crate::card::{self, Action, Card, Kind, Look, count, focus_ring};
use crate::findings::{Finding, Resolution};
use crate::flags::{Flag, FlagKind};
use crate::review::{Consistency, Loaded, RailTab, Review};

/// The rail's starting width.
pub const WIDTH: f32 = 360.0;

/// What a reviewer reads first about a file's history: its icon, its word, and a sentence.
pub fn badge(c: &Consistency) -> (&'static str, &'static str, String) {
    match c {
        Consistency::Consistent => ("check", "Consistent", "Every recorded event rebuilds the document saved in this file.".into()),
        Consistency::ExternallyChanged => (
            "info",
            "External changes",
            "The file's text differs from its last recorded state: it was changed where InkWitness didn't see it. The file as it stands is shown after the history.".into(),
        ),
        Consistency::Corrupt(why) => ("close", "Corrupt", format!("The history stops replaying: {why}")),
        Consistency::Missing => {
            ("search", "Missing", "This file has no InkWitness edit history. The document is shown as saved; there is nothing to replay.".into())
        }
    }
}

fn badge_tint(c: &Consistency, a: &Accents) -> Tint {
    match c {
        Consistency::Consistent => a.consistent,
        Consistency::ExternallyChanged => a.external,
        Consistency::Corrupt(_) => a.corrupt,
        Consistency::Missing => a.missing,
    }
}

/// The caveat every history carries, kept quiet.
pub const CAVEAT: &str = "Unsigned local history: not proof of authorship.";

/// What the reviewer asked the rail for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailAction {
    /// Show the state after this event.
    GoTo(u64),
    /// Open a comment draft pre-filled from this event (a flag, or any event).
    Promote(u64),
    /// Show the file as it stands, after the last recorded event.
    ShowOutside,
    /// Open a comment draft pre-filled from this change made outside InkWitness.
    PromoteOutside(usize),
    /// Save the draft as a comment.
    Save,
    /// Throw the draft away.
    Discard,
    /// Edit this comment.
    Edit(u32),
    Delete(u32),
    /// Go to the moment this comment is about.
    Show(u32),
    /// Download the review copy with the comments.
    Export,
    Tab(RailTab),
    /// Open the sample submission.
    Sample,
    /// Open a DOCX.
    Open,
    /// Take the name typed in the rail as the teacher's, for their tracked changes.
    SetName,
    /// Type another name.
    ChangeName,
}

/// A teacher's change to a finding's resolution or note (applied by [`Review::set_resolution`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub seq: u64,
    pub kind: FlagKind,
    pub resolution: Resolution,
    pub note: String,
}

/// Draw the rail's contents.
pub fn show(ui: &mut Ui, review: &mut Review, app: &WordApp) -> Option<RailAction> {
    let t = Tokens::get(ui.ctx());
    let a = Accents::get(ui.ctx());
    let mut act = None;
    let mut change = None;
    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
    ui.add_space(4.0);
    ui.label(RichText::new("Observations & notes").font(semibold(16.0)).color(t.text));
    header(ui, review, app, &t, &a, &mut act);
    review.typing = false;
    if review.loaded.is_none() {
        return act;
    }
    if app.viewer_ribbon {
        edit_card(ui, review, app, &t, &a, &mut act);
    }
    if review.draft.is_some() {
        draft(ui, review, &t, &a, &mut act);
    }
    let Some(l) = &review.loaded else { return act };
    if l.outside.is_some() {
        outside_card(ui, review, l, &t, &a, &mut act);
    }
    section(ui, "Now showing", None, &t);
    match current_card(review, l).filter(|_| !review.at_outside) {
        Some(mut c) => {
            c.promoted = review.promoted(l.state.seq);
            c.promote = !c.promoted && review.draft.is_none();
            if let Some(f) = review.flags.iter().find(|f| f.seq == l.state.seq) {
                flag_look(&mut c, f, &review.flags, &a);
            }
            take(&mut act, card::show(ui, &c, Look { current: true, expanded: true }));
            for f in review.flags.iter().filter(|f| f.seq == l.state.seq) {
                finding_block(ui, review, l, f, "now", &t, &a, &mut change);
            }
        }
        None => note(
            ui,
            &t,
            &a,
            if l.consistency == Consistency::Missing {
                badge(&l.consistency).2
            } else if review.at_outside {
                "The file as it stands, after the last recorded event. The recorded moments are above and below.".into()
            } else {
                "The start of the history, before the first recorded event. Press Play below to watch the writing.".into()
            },
        ),
    }
    ui.add_space(6.0);
    let moments = crate::flags::moments(&review.flags);
    ui.horizontal(|ui| {
        for (tab, words) in
            [(RailTab::Flags, format!("Flags ({})", moments.len())), (RailTab::Comments, format!("Comments ({})", review.comments.len()))]
        {
            let on = review.tab == tab;
            let b = ui.add(
                egui::Button::new(RichText::new(words).font(semibold(13.0)).color(if on { t.on_accent } else { t.text }))
                    .fill(if on { t.accent } else { a.card })
                    .stroke(Stroke::new(1.0, if on { t.accent } else { t.border }))
                    .corner_radius(CornerRadius::same(14))
                    .min_size(vec2(0.0, 28.0)),
            );
            focus_ring(ui, &b, &a);
            if b.clicked() {
                act = Some(RailAction::Tab(tab));
            }
        }
    });
    match review.tab {
        RailTab::Flags => flag_list(ui, review, l, &moments, &t, &a, &mut act, &mut change),
        RailTab::Comments => comment_list(ui, review, l, &t, &a, &mut act),
    }
    ui.add_space(8.0);
    review.typing |= ui.ctx().data_mut(|d| d.remove_temp::<bool>(egui::Id::new("inkwitness_finding_typing"))).unwrap_or(false);
    if let Some(c) = change {
        review.set_resolution(c.seq, c.kind, c.resolution, &c.note);
    }
    act
}

/// A card's Go to or Promote, as the rail's action.
fn take(act: &mut Option<RailAction>, a: Option<Action>) {
    match a {
        Some(Action::GoTo(s)) => *act = Some(RailAction::GoTo(s)),
        Some(Action::Promote(s)) => *act = Some(RailAction::Promote(s)),
        None => {}
    }
}

/// The flag's tint: a neutral one per kind. None is the colour of a corrupt history, so no flag
/// reads as a verdict (FAL-665); the chip's icon and words tell the kinds apart.
pub fn flag_tint(kind: FlagKind, a: &Accents) -> Tint {
    match kind {
        FlagKind::Paste | FlagKind::LargePaste | FlagKind::LargeInsert | FlagKind::AiPhrase | FlagKind::OutsideAddition => a.observation,
        FlagKind::LargeDelete => a.word_comment,
        FlagKind::Imported | FlagKind::HistoryStops => a.missing,
    }
}

/// Dress an event's card as flag `f` (and the other flags on the same event).
fn flag_look(c: &mut Card, f: &Flag, all: &[Flag], a: &Accents) {
    c.kind = Kind::Observation;
    c.icon = f.kind.icon();
    c.label = f.kind.name().into();
    c.tint = Some(flag_tint(f.kind, a));
    let mut n = String::new();
    if let Some(m) = &f.matched {
        n.push_str(&format!("Matched wording: “{m}”. "));
    }
    n.push_str(f.kind.meaning());
    let also: Vec<&str> = all.iter().filter(|g| g.seq == f.seq && g.kind != f.kind).map(|g| g.label.as_str()).collect();
    if !also.is_empty() {
        n.push_str(&format!(" Also: {}.", also.join("; ")));
    }
    c.note = Some(n);
}

/// A card for each flagged moment, in the order they happened.
fn flag_list(
    ui: &mut Ui,
    review: &Review,
    l: &Loaded,
    moments: &[u64],
    t: &Tokens,
    a: &Accents,
    act: &mut Option<RailAction>,
    change: &mut Option<Change>,
) {
    ui.label(RichText::new("Teacher only. A flag says what happened, never why.").font(regular(11.5)).color(t.text_dim));
    if moments.is_empty() {
        note(ui, t, a, "Nothing in this history is flagged: no pastes, large insertions or deletions, and no text that was already there.".into());
    }
    for &seq in moments {
        let Some(f) = review.flags.iter().find(|f| f.seq == seq) else { continue };
        let Some(mut c) = event_card(l, seq, None) else { continue };
        flag_look(&mut c, f, &review.flags, a);
        c.key = format!("flag-{seq}");
        c.promoted = review.promoted(seq);
        c.promote = !c.promoted;
        take(act, card::show(ui, &c, Look { current: seq == l.state.seq, expanded: true }));
        for f in review.flags.iter().filter(|f| f.seq == seq) {
            finding_block(ui, review, l, f, "list", t, a, change);
        }
    }
}

/// The finding for flag `f`, as the rail reads it ([`crate::findings`]): what was observed, the
/// supporting record (the card above), other explanations, what to check next, what it doesn't
/// prove, its method version and the teacher's resolution with a note. No score, no total.
pub fn finding_of(review: &Review, l: &Loaded, f: &Flag) -> Finding {
    let card = event_card(l, f.seq, None);
    let record = card
        .map(|c| [Some(format!("Event {}", f.seq)), c.recorded, c.elapsed, c.size].into_iter().flatten().collect::<Vec<_>>().join(" · "))
        .unwrap_or_else(|| format!("Event {}", f.seq));
    let mut finding = Finding::of(f, record);
    finding.resolution = review.resolution_of(f.seq, f.kind).0;
    finding
}

#[allow(clippy::too_many_arguments)]
fn finding_block(ui: &mut Ui, review: &Review, l: &Loaded, f: &Flag, scope: &str, t: &Tokens, a: &Accents, change: &mut Option<Change>) {
    let finding = finding_of(review, l, f);
    let (res, note) = review.resolution_of(f.seq, f.kind);
    let tint = flag_tint(f.kind, a);
    egui::Frame::NONE.fill(a.card).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(8)).inner_margin(10).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        let head = |ui: &mut Ui, words: &str| {
            ui.label(RichText::new(words).font(semibold(12.0)).color(t.text));
        };
        let line = |ui: &mut Ui, words: &str| {
            ui.add(egui::Label::new(RichText::new(words).font(regular(12.0)).color(t.text_dim)).wrap());
        };
        ui.horizontal_wrapped(|ui| {
            card::chip(ui, f.kind.icon(), &format!("Finding · {}", f.kind.name()), tint);
        });
        head(ui, "Observed");
        line(ui, &finding.observed);
        head(ui, "Other explanations");
        for alt in &finding.alternatives {
            line(ui, &format!("• {alt}"));
        }
        head(ui, "Check next");
        for c in &finding.next_check {
            line(ui, &format!("• {c}"));
        }
        head(ui, "What this does not prove");
        line(ui, finding.not_proven);
        ui.label(RichText::new(format!("{} · method {}", finding.rule, finding.method)).font(regular(11.0)).color(t.text_dim));
        head(ui, "Resolution");
        ui.horizontal_wrapped(|ui| {
            for r in Resolution::ALL {
                let on = r == res;
                let b = ui.add(
                    egui::Button::new(RichText::new(r.name()).font(medium(12.0)).color(if on { t.on_accent } else { t.text }))
                        .fill(if on { t.accent } else { a.card })
                        .stroke(Stroke::new(1.0, if on { t.accent } else { t.border }))
                        .corner_radius(CornerRadius::same(12)),
                );
                focus_ring(ui, &b, a);
                if b.on_hover_text("Kept with your comments, and in the review copy").clicked() && !on {
                    *change = Some(Change { seq: f.seq, kind: f.kind, resolution: r, note: note.to_string() });
                }
            }
        });
        let mut text = note.to_string();
        let edit = ui.add(
            egui::TextEdit::multiline(&mut text)
                .id(egui::Id::new(("inkwitness_finding_note", scope, f.seq, f.kind.rule())))
                .hint_text("Note on this finding (optional)")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        focus_ring(ui, &edit, a);
        if edit.has_focus() {
            // The dock's shortcuts wait while a box has the keyboard.
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("inkwitness_finding_typing"), true));
        }
        if edit.changed() {
            *change = Some(Change { seq: f.seq, kind: f.kind, resolution: res, note: text });
        }
    });
}

/// The teacher's comments and highlights, and the review copy.
fn comment_list(ui: &mut Ui, review: &Review, l: &Loaded, t: &Tokens, a: &Accents, act: &mut Option<RailAction>) {
    ui.label(
        RichText::new("Kept apart from the student's file. They show on the page, and go into the review copy as Word comments and highlights.")
            .font(regular(11.5))
            .color(t.text_dim),
    );
    let export = ui.add_enabled(
        !review.comments.is_empty(),
        egui::Button::new(RichText::new("Download review copy (.docx)").font(semibold(12.5)).color(t.on_accent))
            .fill(t.accent)
            .min_size(vec2(0.0, 30.0)),
    );
    focus_ring(ui, &export, a);
    if export.on_hover_text("A copy of the final document with your comments and highlights. The student's file is unchanged.").clicked() {
        *act = Some(RailAction::Export);
    }
    if let Some(n) = &review.notice {
        ui.add(egui::Label::new(RichText::new(n).font(regular(12.0)).color(t.text)).wrap());
    }
    if review.comments.is_empty() {
        note(
            ui,
            t,
            a,
            "No comments or highlights yet. Select text on the page, then press Highlight or New comment at the top. Or press Promote to comment on a flag.".into(),
        );
    }
    let tc = Kind::TeacherComment.tint(a);
    for c in &review.comments {
        let editing = review.draft.as_ref().is_some_and(|d| d.editing == Some(c.id));
        let edge = if editing { Stroke::new(1.5, t.accent) } else { Stroke::new(1.0, t.border) };
        let frame = egui::Frame::NONE.fill(a.card).stroke(edge).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin {
            left: 14,
            right: 10,
            top: 9,
            bottom: 9,
        });
        let shown = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
            ui.horizontal_wrapped(|ui| match c.highlight {
                Some(h) => {
                    card::chip(ui, "highlight", &format!("Highlight · {}", crate::toolbar::colour_name(h)), tc);
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(3), crate::toolbar::page_colour(h));
                }
                None if c.resolution.is_some() => {
                    if let (Some(r), Some((_, k))) = (c.resolution, c.from_flag) {
                        card::chip(ui, k.icon(), &format!("Finding resolution · {}", r.name()), flag_tint(k, a));
                    }
                }
                None => {
                    card::chip(ui, "comment", "Teacher comment", tc);
                    if let Some((_, k)) = c.from_flag {
                        card::chip(ui, k.icon(), &format!("From: {}", k.name()), flag_tint(k, a));
                    }
                }
            });
            if let Some(q) = c.anchor.as_ref().map(|x| x.quote.as_str()).filter(|q| !q.trim().is_empty()) {
                ui.label(RichText::new(short(q, 140)).font(regular(12.0)).color(t.text_dim).italics());
            }
            if c.moved {
                let why = "The commented text was removed later: the comment sits at the nearest paragraph that remains.";
                ui.add(egui::Label::new(RichText::new(why).font(regular(11.5)).color(t.text_dim)).wrap());
            }
            if c.highlight.is_none() {
                if !c.text.is_empty() || c.resolution.is_none() {
                    ui.add(egui::Label::new(RichText::new(&c.text).font(regular(13.0)).color(t.text)).wrap());
                }
            }
            if let Some((s, e)) = c.seq.and_then(|s| l.replay.event(s).map(|e| (s, e))) {
                let el = e.get("elapsedMonotonicMs").and_then(Value::as_u64).map(elapsed).unwrap_or_default();
                ui.label(RichText::new(format!("Event {s} · {el}")).font(regular(11.5)).color(t.text_dim));
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
                let mut button = |ui: &mut Ui, words: &str, tip: &str, what: RailAction| {
                    let b = ui.add(egui::Button::new(RichText::new(words).font(medium(12.0)).color(t.accent_text)).small());
                    focus_ring(ui, &b, a);
                    if b.on_hover_text(tip).clicked() {
                        *act = Some(what);
                    }
                };
                if c.highlight.is_some() {
                    button(ui, "Show", "Go to the moment this highlight was made at", RailAction::Show(c.id));
                    button(ui, "Remove", "Remove this highlight", RailAction::Delete(c.id));
                } else {
                    button(ui, "Show", "Go to the moment this comment is about", RailAction::Show(c.id));
                    button(ui, "Edit", "Change the comment's text", RailAction::Edit(c.id));
                    button(ui, "Remove", "Remove this comment", RailAction::Delete(c.id));
                }
            });
        });
        let r = shown.response.rect;
        let stripe = Rect::from_min_max(egui::pos2(r.min.x + 1.0, r.min.y + 1.0), egui::pos2(r.min.x + 5.0, r.max.y - 1.0));
        ui.painter().rect_filled(stripe, CornerRadius { nw: 7, sw: 7, ne: 0, se: 0 }, tc.edge);
    }
}

/// `text` on one line, cut at `n` characters, quoted.
fn short(text: &str, n: usize) -> String {
    let flat = text.replace(wordcraft_inkwitness::PARA_MARK, " ");
    let flat = crate::card::unquote(flat.trim());
    let mut out: String = flat.chars().take(n).collect();
    if flat.chars().count() > n {
        out = format!("{}…", out.trim_end());
    }
    format!("“{out}”")
}

/// The comment being written: where it sits, its text, Save and Discard.
fn draft(ui: &mut Ui, review: &mut Review, t: &Tokens, a: &Accents, act: &mut Option<RailAction>) {
    let Some(d) = review.draft.as_mut() else { return };
    let flag = d.from_flag;
    let title = match (d.editing, flag) {
        (Some(_), _) => "Edit comment",
        (None, Some(_)) => "Promote to comment",
        (None, None) => "New comment",
    };
    let mut typing = false;
    let mut review_focus = false;
    egui::Frame::NONE.fill(a.card).stroke(Stroke::new(1.5, t.accent)).corner_radius(CornerRadius::same(8)).inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(6.0, 5.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(title).font(semibold(14.0)).color(t.text));
            if let Some((_, k)) = flag {
                card::chip(ui, k.icon(), k.name(), flag_tint(k, a));
            }
        });
        let on = match &d.anchor {
            Some(x) if !x.quote.trim().is_empty() => format!("On {}", short(&x.quote, 120)),
            Some(_) => "At a point in the text (nothing selected)".into(),
            None => "On the document as a whole".into(),
        };
        ui.add(egui::Label::new(RichText::new(on).font(regular(12.0)).color(t.text_dim).italics()).wrap());
        if d.moved {
            let why = "This text was removed later, so the comment will sit at the nearest paragraph that remains.";
            ui.add(egui::Label::new(RichText::new(why).font(regular(11.5)).color(t.text_dim)).wrap());
        }
        let edit = ui.add(
            egui::TextEdit::multiline(&mut d.text).desired_rows(7).desired_width(f32::INFINITY).font(regular(13.0)).hint_text("Write your comment…"),
        );
        edit.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Comment text"));
        if d.focus {
            edit.request_focus();
            d.focus = false;
            review_focus = true;
        }
        typing = edit.has_focus();
        if flag.is_some() && d.editing.is_none() {
            ui.label(RichText::new("Pre-filled from the record: trim or rewrite it before you save.").font(regular(11.5)).color(t.text_dim));
        }
        ui.horizontal(|ui| {
            let empty = d.text.trim().is_empty();
            let save = ui.add_enabled(
                !empty,
                egui::Button::new(RichText::new("Save comment").font(semibold(12.5)).color(t.on_accent)).fill(t.accent).min_size(vec2(0.0, 28.0)),
            );
            focus_ring(ui, &save, a);
            let shortcut = typing && !empty && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter));
            if save.on_hover_text("Save  Ctrl+Enter").clicked() || shortcut {
                *act = Some(RailAction::Save);
            }
            let discard = ui.add(egui::Button::new(RichText::new("Discard").font(medium(12.5)).color(t.text)).min_size(vec2(0.0, 28.0)));
            focus_ring(ui, &discard, a);
            if discard.clicked() {
                *act = Some(RailAction::Discard);
            }
        });
    });
    review.typing |= typing || review_focus;
    review.took_focus = review_focus;
}

/// With Full ribbon on: the teacher's edits. Their name first (this browser remembers it), then
/// where they can edit: the final document, at the end of the history.
fn edit_card(ui: &mut Ui, review: &mut Review, app: &WordApp, t: &Tokens, a: &Accents, act: &mut Option<RailAction>) {
    let Some(l) = &review.loaded else { return };
    let (end, at_end) = (l.replayable, review.final_shown());
    let editing = review.editing(app);
    let mut typing = false;
    egui::Frame::NONE.fill(a.card).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(8)).inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(6.0, 5.0);
        ui.label(RichText::new("Your edits").font(semibold(14.0)).color(t.text));
        let say = |ui: &mut Ui, text: &str| ui.add(egui::Label::new(RichText::new(text).font(regular(12.0)).color(t.text_dim)).wrap());
        if review.teacher.trim().is_empty() {
            say(ui, "Type your name to edit the final document. Every change you make is a tracked change in your name, and goes only into the review copy.");
            let edit = ui.add(egui::TextEdit::singleline(&mut review.name_draft).desired_width(f32::INFINITY).font(regular(13.0)).hint_text("Your name"));
            edit.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Your name"));
            typing = edit.has_focus();
            let entered = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let ready = !review.name_draft.trim().is_empty();
            let start = ui.add_enabled(
                ready,
                egui::Button::new(RichText::new("Start editing").font(semibold(12.5)).color(t.on_accent)).fill(t.accent).min_size(vec2(0.0, 28.0)),
            );
            focus_ring(ui, &start, a);
            if ready && (start.clicked() || entered) {
                *act = Some(RailAction::SetName);
            }
            return;
        }
        if editing {
            say(ui, &format!("Editing as {}. Every change is a tracked change in your name. Your edits go only into the review copy; the student's file is never changed.", review.teacher.trim()));
            if let Some(n) = &review.edit_note {
                ui.add(egui::Label::new(RichText::new(n).font(regular(12.0)).color(t.text)).wrap());
            }
        } else if !at_end {
            say(ui, "Your edits go into the final document. Stepping through the history shows the student's text; your edits show again at the end.");
            let go = ui.add(egui::Button::new(RichText::new("Go to the end").font(medium(12.5)).color(t.text)).min_size(vec2(0.0, 26.0)));
            focus_ring(ui, &go, a);
            if go.clicked() {
                *act = Some(if l.outside.is_some() { RailAction::ShowOutside } else { RailAction::GoTo(end) });
            }
        }
        let change = ui.add(egui::Button::new(RichText::new("Change name").font(medium(12.0)).color(t.text)).min_size(vec2(0.0, 24.0)));
        focus_ring(ui, &change, a);
        if change.on_hover_text("Your changes so far keep the name they were made with.").clicked() {
            *act = Some(RailAction::ChangeName);
        }
    });
    review.typing |= typing;
}

/// The longest passage a change shows in the rail, in characters.
const SHOWN_CHARS: usize = 400;

/// `text` (paragraph marks as line breaks) cut to [`SHOWN_CHARS`].
fn excerpt(text: &str) -> String {
    let flat = text.replace(wordcraft_inkwitness::PARA_MARK, "\n");
    let flat = flat.trim();
    if flat.chars().count() > SHOWN_CHARS {
        format!("{}…", flat.chars().take(SHOWN_CHARS).collect::<String>().trim_end())
    } else {
        flat.to_string()
    }
}

/// The final section: what the file has that its history doesn't end with. Collapsed to a line
/// and a button while a recorded moment is shown; with every change, its flags and a Promote
/// button while the file as it stands is shown.
fn outside_card(ui: &mut Ui, review: &Review, l: &Loaded, t: &Tokens, a: &Accents, act: &mut Option<RailAction>) {
    let Some(o) = &l.outside else { return };
    let here = review.at_outside;
    egui::Frame::NONE
        .fill(a.card)
        .stroke(Stroke::new(1.0, if here { a.external.edge } else { t.border }))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(6.0, 5.0);
            card::chip(ui, "info", crate::outside::TITLE, a.external);
            let say = |ui: &mut Ui, text: &str, c: egui::Color32| ui.add(egui::Label::new(RichText::new(text).font(regular(12.0)).color(c)).wrap());
            say(ui, &o.summary(), t.text);
            if !here {
                say(ui, "After the last recorded event, the file has text the history doesn't account for.", t.text_dim);
                let show = ui.add(
                    egui::Button::new(RichText::new("Show the file as it stands").font(semibold(12.5)).color(t.on_accent))
                        .fill(t.accent)
                        .min_size(vec2(0.0, 28.0)),
                );
                focus_ring(ui, &show, a);
                if show.clicked() {
                    *act = Some(RailAction::ShowOutside);
                }
                return;
            }
            say(ui, crate::outside::MEANING, t.text_dim);
            for (i, h) in o.hunks.iter().enumerate().take(MAX_SHOWN_CHANGES) {
                ui.separator();
                ui.label(RichText::new(format!("Change {} of {}", i + 1, o.hunks.len())).font(semibold(12.5)).color(t.text));
                if !h.added.trim().is_empty() {
                    ui.label(RichText::new("Added").font(medium(11.5)).color(t.text_dim));
                    say(ui, &excerpt(&h.added), t.text);
                }
                if !h.removed.trim().is_empty() {
                    ui.label(RichText::new("Removed").font(medium(11.5)).color(t.text_dim));
                    ui.add(egui::Label::new(RichText::new(excerpt(&h.removed)).font(regular(12.0)).color(t.text_dim).strikethrough()).wrap());
                }
                for f in &h.flags {
                    card::chip(ui, f.kind.icon(), &f.label, flag_tint(f.kind, a));
                    let meaning = match &f.matched {
                        Some(m) => format!("Matched wording: “{m}”. {}", f.kind.meaning()),
                        None => f.kind.meaning().to_string(),
                    };
                    say(ui, &meaning, t.text_dim);
                }
                if review.draft.is_none() {
                    let b = ui.add(egui::Button::new(RichText::new("Promote to comment").font(medium(12.0)).color(t.text)).min_size(vec2(0.0, 24.0)));
                    focus_ring(ui, &b, a);
                    if b.on_hover_text("Start a comment on this change, with a question to ask").clicked() {
                        *act = Some(RailAction::PromoteOutside(i));
                    }
                }
            }
            if o.hunks.len() > MAX_SHOWN_CHANGES {
                say(ui, &format!("…and {} more changes, all in the page and the review copy.", o.hunks.len() - MAX_SHOWN_CHANGES), t.text_dim);
            }
        });
}

/// The most changes the rail lists.
const MAX_SHOWN_CHANGES: usize = 20;

/// The file's header: the mark, its name, who and what it is for, the history badge and caveat.
fn header(ui: &mut Ui, review: &Review, app: &WordApp, t: &Tokens, a: &Accents, act: &mut Option<RailAction>) {
    egui::Frame::NONE.fill(a.card).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(8)).inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(6.0, 5.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
            crate::toolbar::mark(ui.painter(), r);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                match &review.loaded {
                    Some(l) => {
                        ui.add(egui::Label::new(RichText::new(&l.name).font(semibold(14.0)).color(t.text)).truncate()).on_hover_text(&l.name);
                        let who = format!(
                            "{} · {}",
                            l.student.as_deref().unwrap_or("Student: not entered"),
                            l.assessment.as_deref().unwrap_or("Assessment: not entered")
                        );
                        ui.add(egui::Label::new(RichText::new(who).font(regular(12.0)).color(t.text_dim)).wrap());
                    }
                    None => {
                        ui.label(RichText::new("No file open").font(semibold(14.0)).color(t.text));
                        ui.label(
                            RichText::new("Open a DOCX saved by InkWitness Write, or drop it on the page.").font(regular(12.0)).color(t.text_dim),
                        );
                    }
                }
            });
        });
        if let Some(l) = &review.loaded {
            let (icon, word, why) = badge(&l.consistency);
            let shown = card::chip(ui, icon, &format!("History: {word}"), badge_tint(&l.consistency, a));
            shown.on_hover_text(&why);
            if l.consistency != Consistency::Consistent {
                ui.add(egui::Label::new(RichText::new(why).font(regular(12.0)).color(t.text)).wrap());
            }
            let pastes = l.replay.pastes().len();
            if l.consistency != Consistency::Missing {
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        count(l.replay.last_seq() as usize, "recorded event", "recorded events"),
                        count(pastes, "paste", "pastes")
                    ))
                    .font(regular(12.0))
                    .color(t.text_dim),
                );
            }
            ui.label(RichText::new(CAVEAT).font(regular(11.5)).color(t.text_dim)).on_hover_text(l.consistency.label());
        } else {
            ui.horizontal(|ui| {
                if app.services.open_async.is_some()
                    && ui.add(egui::Button::new(RichText::new("Open DOCX…").font(medium(12.5)).color(t.accent_text))).clicked()
                {
                    *act = Some(RailAction::Open);
                }
                let sample =
                    ui.add(egui::Button::new(RichText::new("Try a sample submission").font(semibold(12.5)).color(t.on_accent)).fill(t.accent));
                focus_ring(ui, &sample, a);
                if sample.on_hover_text("A made-up essay by Example Student, written in InkWitness Write, with pastes to look at").clicked() {
                    *act = Some(RailAction::Sample);
                }
            });
        }
        if let Some(e) = &review.error {
            ui.add(egui::Label::new(RichText::new(e).font(regular(12.0)).color(a.corrupt.text)).wrap());
        }
    });
}

fn section(ui: &mut Ui, title: &str, aside: Option<&str>, t: &Tokens) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(semibold(13.0)).color(t.text));
        if let Some(s) = aside {
            ui.label(RichText::new(s).font(regular(11.5)).color(t.text_dim));
        }
    });
}

fn note(ui: &mut Ui, t: &Tokens, a: &Accents, text: String) {
    egui::Frame::NONE.fill(a.card).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(8)).inner_margin(10).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(text).font(regular(12.5)).color(t.text_dim)).wrap());
    });
}

/// The `seq` of each observation in the history: pastes and imported documents.
pub fn observations(l: &Loaded) -> Vec<u64> {
    l.replay
        .events()
        .iter()
        .filter(|e| observed(e.get("type").and_then(Value::as_str).unwrap_or_default()))
        .filter_map(|e| e.get("seq")?.as_u64())
        .filter(|s| *s <= l.replayable)
        .collect()
}

/// Whether events of type `ty` are observations (`docs/REPLAY_UI.md`: clipboard paste, imported
/// pre-existing document; the other categories come with the observation taxonomy).
pub fn observed(ty: &str) -> bool {
    matches!(ty, "paste" | "import_document")
}

/// The card for the shown event, with its highlighted text and anchor check.
fn current_card(review: &Review, l: &Loaded) -> Option<Card> {
    let anchor = match &l.mark {
        Some(m) if m.context_matches => "Resolved; the surrounding text matches",
        Some(_) => "Resolved; the surrounding text differs or wasn't recorded (uncertain)",
        None => "Nothing to highlight in this state",
    };
    // A deletion has nothing left to highlight: its card quotes what was removed.
    let mut c = event_card(l, l.state.seq, review.highlighted_text().filter(|t| !t.trim().is_empty()))?;
    c.details.push(("Anchor".into(), anchor.into()));
    Some(c)
}

/// The card for event `seq`. `excerpt` replaces the event's own text (the shown event's
/// highlighted range, read from the rebuilt document).
pub fn event_card(l: &Loaded, seq: u64, excerpt: Option<String>) -> Option<Card> {
    let e = l.replay.event(seq)?;
    let s = |k: &str| e.get(k).and_then(Value::as_str);
    let ty = s("type").unwrap_or_default();
    let (icon, label) = kind_of(ty);
    let inserted = s("insertedText").unwrap_or_default();
    let deleted = s("deletedText").unwrap_or_default();
    let mut details = vec![("Event".into(), format!("{seq} of {}", l.replay.last_seq())), ("Origin".into(), s("origin").unwrap_or("—").to_string())];
    details.push(("gestureId".into(), s("gestureId").unwrap_or("none").to_string()));
    if let Some(line) = crate::steps::typing_line(e) {
        details.push(("Typing".into(), line));
    }
    if ty == "paste" {
        let formats = match e.get("clipboardTypes").and_then(Value::as_array) {
            Some(v) if !v.is_empty() => v.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),
            _ => "Not exposed by the browser".into(),
        };
        details.push(("Clipboard formats".into(), formats));
        details.push(("Source".into(), "Unknown: a browser can't tell where pasted text came from".into()));
    }
    let text = if inserted.is_empty() { deleted } else { inserted };
    Some(Card {
        key: format!("event-{seq}"),
        kind: if observed(ty) { Kind::Observation } else { Kind::Event },
        icon,
        label: label.into(),
        seq: Some(seq),
        recorded: s("atLocalIso").map(recorded),
        elapsed: e.get("elapsedMonotonicMs").and_then(Value::as_u64).map(elapsed),
        size: size(inserted, deleted),
        excerpt: excerpt.or_else(|| (!text.is_empty()).then(|| text.to_string())),
        details,
        note: None,
        tint: None,
        promote: false,
        promoted: false,
    })
}

/// The chip's icon and words for an event type.
pub fn kind_of(ty: &str) -> (&'static str, &'static str) {
    match ty {
        "paste" => ("paste", "Paste"),
        "import_document" => ("open", "Imported document"),
        "cut" => ("cut", "Cut"),
        "undo" => ("undo", "Undo"),
        "redo" => ("redo", "Redo"),
        "text_insert" => ("pencil", "Typing"),
        "text_delete" => ("eraser", "Deletion"),
        "text_replace" => ("replace", "Replacement"),
        "format_change" => ("fontdialog", "Formatting"),
        "structure_change" => ("pilcrow", "Paragraphs"),
        _ => ("pencil", "Edit"),
    }
}

/// An RFC 3339 time as "28 Oct 2026, 09:00:00 UTC" (as recorded: the browser writes UTC).
pub fn recorded(iso: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(iso) {
        Ok(d) if d.offset().local_minus_utc() == 0 => d.format("%-d %b %Y, %H:%M:%S UTC").to_string(),
        Ok(d) => d.format("%-d %b %Y, %H:%M:%S (UTC%:z)").to_string(),
        Err(_) => iso.to_string(),
    }
}

/// Milliseconds since the session started, as "1:05 into the session" (or "1:02:05").
pub fn elapsed(ms: u64) -> String {
    let secs = ms / 1000;
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02} into the session") } else { format!("{m}:{s:02} into the session") }
}

/// What an event added or removed, in words and characters (paragraph marks not counted).
pub fn size(inserted: &str, deleted: &str) -> Option<String> {
    let measure = |t: &str| {
        let chars = t.chars().filter(|c| *c != wordcraft_inkwitness::PARA_MARK).count();
        let words = t.split(|c: char| c.is_whitespace() || c == wordcraft_inkwitness::PARA_MARK).filter(|w| !w.is_empty()).count();
        (words, chars)
    };
    // Only spaces or paragraph marks: "1 character added", not "0 words · 1 character added".
    let both = |w: usize, c: usize| {
        let chars = count(c, "character", "characters");
        if w == 0 { chars } else { format!("{} · {chars}", count(w, "word", "words")) }
    };
    let (iw, ic) = measure(inserted);
    let (dw, dc) = measure(deleted);
    match (ic > 0, dc > 0) {
        (true, false) => Some(format!("{} added", both(iw, ic))),
        (false, true) => Some(format!("{} removed", both(dw, dc))),
        (true, true) => Some(format!("{} added, {} removed", count(ic, "character", "characters"), count(dc, "character", "characters"))),
        (false, false) => None,
    }
}
