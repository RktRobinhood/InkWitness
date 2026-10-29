//! The "Observations & notes" rail at the right of the review page (FAL-448): the file's header
//! (name, student, assessment, history status), the event navigation, the card for the moment
//! shown, and a card for each observation in the history.

use egui::{CornerRadius, Rect, RichText, Sense, Stroke, Ui, vec2};
use serde_json::Value;
use wordcraft_ui_egui::WordApp;
use wordcraft_ui_egui::icons;
use wordcraft_ui_egui::theme::critic::{Accents, Tint};
use wordcraft_ui_egui::theme::{Tokens, medium, regular, semibold};

use crate::card::{self, Action, Card, Kind, Look, count, focus_ring};
use crate::review::{Consistency, Loaded, Review};

/// The rail's starting width.
pub const WIDTH: f32 = 360.0;

/// What a reviewer reads first about a file's history: its icon, its word, and a sentence.
pub fn badge(c: &Consistency) -> (&'static str, &'static str, String) {
    match c {
        Consistency::Consistent => ("check", "Consistent", "Every recorded event rebuilds the document saved in this file.".into()),
        Consistency::ExternallyChanged => (
            "info",
            "External changes",
            "The file's text differs from its last recorded state: it was changed where InkWitness didn't see it.".into(),
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

/// Draw the rail's contents. `go` receives the event to seek to.
pub fn show(ui: &mut Ui, review: &Review, app: &mut WordApp, go: &mut Option<u64>) {
    let t = Tokens::get(ui.ctx());
    let a = Accents::get(ui.ctx());
    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
    ui.add_space(4.0);
    ui.label(RichText::new("Observations & notes").font(semibold(16.0)).color(t.text));
    header(ui, review, app, &t, &a);
    let Some(l) = &review.loaded else { return };
    if l.consistency != Consistency::Missing {
        navigation(ui, review, l, &t, go);
    }
    section(ui, "Now showing", None, &t);
    match current_card(review, l) {
        Some(c) => {
            if let Some(Action::GoTo(seq)) = card::show(ui, &c, Look { current: true, expanded: true }) {
                *go = Some(seq);
            }
        }
        None => note(
            ui,
            &t,
            &a,
            if l.consistency == Consistency::Missing {
                badge(&l.consistency).2
            } else {
                "The start of the history, before the first recorded event.".into()
            },
        ),
    }
    let observed = observations(l);
    section(ui, "Observations", Some(&format!("{} · teacher only", count(observed.len(), "item", "items"))), &t);
    if observed.is_empty() {
        note(ui, &t, &a, "Nothing in this history is flagged as an observation.".into());
    }
    for seq in observed {
        if seq == l.state.seq {
            // Its card is the one above, expanded.
            continue;
        }
        if let Some(c) = event_card(l, seq, None)
            && let Some(Action::GoTo(s)) = card::show(ui, &c, Look { current: false, expanded: false })
        {
            *go = Some(s);
        }
    }
    ui.add_space(8.0);
}

/// The file's header: the mark, its name, who and what it is for, the history badge and caveat.
fn header(ui: &mut Ui, review: &Review, app: &mut WordApp, t: &Tokens, a: &Accents) {
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
        } else if ui.add(egui::Button::new(RichText::new("Open DOCX…").font(medium(12.5)).color(t.accent_text))).clicked()
            && let Some(open) = &app.services.open_async
        {
            open("document");
        }
        if let Some(e) = &review.error {
            ui.add(egui::Label::new(RichText::new(e).font(regular(12.0)).color(a.corrupt.text)).wrap());
        }
    });
}

/// First, previous, next, last, the event slider and Jump to paste. FAL-477 moves these to the
/// bottom dock.
fn navigation(ui: &mut Ui, review: &Review, l: &Loaded, t: &Tokens, go: &mut Option<u64>) {
    let a = Accents::get(ui.ctx());
    let seq = l.state.seq;
    let paste = review.first_paste();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(3.0, 0.0);
        let mut nav = |ui: &mut Ui, icon: &str, label: &str, to: u64, on: bool| {
            let r = ui.add_enabled(on, egui::Button::new("").min_size(vec2(30.0, 26.0)));
            icons::paint(
                ui.painter(),
                Rect::from_center_size(r.rect.center(), vec2(14.0, 14.0)),
                icon,
                if on { t.icon } else { t.text_disabled },
                t.accent,
            );
            let name = label.to_string();
            r.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, on, &name));
            focus_ring(ui, &r, &a);
            if r.on_hover_text(label).clicked() {
                *go = Some(to);
            }
        };
        nav(ui, "first", "First event", 0, seq > 0);
        nav(ui, "previous", "Previous event", seq.saturating_sub(1), seq > 0);
        nav(ui, "next", "Next event", seq + 1, seq < l.replayable);
        nav(ui, "last", "Last event", l.replayable, seq < l.replayable);
        ui.add_space(6.0);
        ui.label(RichText::new(format!("Event {seq} of {}", l.replay.last_seq())).font(medium(12.0)).color(t.text));
    });
    let mut at = seq;
    ui.spacing_mut().slider_width = ui.available_width().max(80.0);
    // "Event N of M" above says where it is.
    let slider = ui.add(egui::Slider::new(&mut at, 0..=l.replayable).show_value(false));
    slider.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Event"));
    if slider.changed() {
        *go = Some(at);
    }
    let jump = ui.add_enabled(paste.is_some(), egui::Button::new(RichText::new("Jump to paste").font(medium(12.0))));
    focus_ring(ui, &jump, &a);
    if jump.clicked() {
        *go = paste;
    }
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
    let mut c = event_card(l, l.state.seq, review.highlighted_text())?;
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
    let (iw, ic) = measure(inserted);
    let (dw, dc) = measure(deleted);
    match (ic > 0, dc > 0) {
        (true, false) => Some(format!("{} · {} added", count(iw, "word", "words"), count(ic, "character", "characters"))),
        (false, true) => Some(format!("{} · {} removed", count(dw, "word", "words"), count(dc, "character", "characters"))),
        (true, true) => Some(format!("{} added, {} removed", count(ic, "character", "characters"), count(dc, "character", "characters"))),
        (false, false) => None,
    }
}
