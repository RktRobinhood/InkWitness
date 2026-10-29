//! A card in the review rail: one recorded event, observation or comment (`docs/REPLAY_UI.md`,
//! "Right-hand comment rail"). The same component draws every kind of rail item, and the
//! timeline's hover cards: a type chip with an icon, when it was recorded, how much it changed,
//! an excerpt, a Go to action, and the technical fields folded under Details.
//!
//! Colour is never the only sign of a kind: each chip has its own icon and words.

use egui::{Align, CornerRadius, Layout, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use wordcraft_ui_egui::icons;
use wordcraft_ui_egui::theme::critic::{Accents, Tint};
use wordcraft_ui_egui::theme::{Tokens, medium, regular, semibold};

/// The kinds of rail item. They are always told apart, by icon, words and colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Something InkWitness noticed in the history (a paste, an imported document…). Shown to
    /// the teacher only; never a misconduct finding.
    Observation,
    /// Any other recorded edit (typing, deleting, formatting).
    Event,
    /// A comment already in the DOCX: everyone who opens the file sees it.
    WordComment,
    /// A comment the teacher writes (FAL-446 T2): the student sees it once the review copy is
    /// returned.
    TeacherComment,
}

impl Kind {
    /// The chip colours for this kind.
    pub fn tint(self, a: &Accents) -> Tint {
        match self {
            Kind::Observation => a.observation,
            Kind::Event | Kind::WordComment => a.word_comment,
            Kind::TeacherComment => a.action,
        }
    }

    /// Who can see an item of this kind.
    pub fn audience(self) -> &'static str {
        match self {
            Kind::Observation | Kind::Event => "Teacher only",
            Kind::WordComment => "In the file",
            Kind::TeacherComment => "Teacher, and the student once returned",
        }
    }
}

/// What a card shows. Built from the evidence ([`crate::rail::event_card`]); every field but the
/// chip is optional, so comments (T2) and hover cards fill in only what they have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// Unique among the cards shown at once (it keys the Details section's open state).
    pub key: String,
    pub kind: Kind,
    /// An [`icons`] name.
    pub icon: &'static str,
    /// The chip's words: "Paste", "Typing", "Imported document"…
    pub label: String,
    /// The event it is about, for Go to.
    pub seq: Option<u64>,
    /// When it was recorded ("28 Oct 2026, 09:00:00 UTC").
    pub recorded: Option<String>,
    /// How far into the writing session ("1:05 into the session").
    pub elapsed: Option<String>,
    /// How much it changed ("28 words · 185 characters added").
    pub size: Option<String>,
    pub excerpt: Option<String>,
    /// Technical fields (gestureId, the anchor check, clipboard formats, source), under Details.
    pub details: Vec<(String, String)>,
}

/// What the reader asked a card to do. T3 adds Promote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Show the state after this event.
    GoTo(u64),
}

/// How a card is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Look {
    /// The page shows this card's moment now: it says "Showing", has no Go to, and a stronger edge.
    pub current: bool,
    /// Excerpt and Details are shown; otherwise the card is one compact block.
    pub expanded: bool,
}

/// The longest excerpt a card shows, in characters.
pub const MAX_EXCERPT: usize = 280;

/// Draw `card`; returns what the reader asked it to do.
pub fn show(ui: &mut Ui, card: &Card, look: Look) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let a = Accents::get(ui.ctx());
    let tint = card.kind.tint(&a);
    let mut action = None;
    let edge = if look.current { Stroke::new(1.5, t.accent) } else { Stroke::new(1.0, t.border) };
    let frame = egui::Frame::NONE.fill(a.card).stroke(edge).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin {
        left: 14,
        right: 10,
        top: 9,
        bottom: 9,
    });
    let shown = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        ui.horizontal(|ui| {
            chip(ui, card.icon, &card.label, tint);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if look.current {
                    status_pill(ui, "Showing", &t);
                } else if let Some(seq) = card.seq {
                    let go = ui.add(egui::Button::new(RichText::new("Go to").font(medium(12.0)).color(t.accent_text)).small());
                    focus_ring(ui, &go, &a);
                    if go.on_hover_text(format!("Show the document as it was after event {seq}")).clicked() {
                        action = Some(Action::GoTo(seq));
                    }
                }
                if let Some(seq) = card.seq {
                    ui.label(RichText::new(format!("Event {seq}")).font(regular(11.5)).color(t.text_dim));
                }
            });
        });
        if let Some(r) = &card.recorded {
            ui.label(RichText::new(r).font(medium(12.5)).color(t.text));
        }
        let meta: Vec<&str> = [card.elapsed.as_deref(), card.size.as_deref()].into_iter().flatten().collect();
        if !meta.is_empty() {
            ui.label(RichText::new(meta.join(" · ")).font(regular(12.0)).color(t.text_dim));
        }
        if look.expanded
            && let Some(x) = card.excerpt.as_deref().filter(|x| !x.trim().is_empty())
        {
            ui.add_space(2.0);
            egui::Frame::NONE.fill(t.panel).corner_radius(CornerRadius::same(4)).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(excerpt(x)).font(regular(12.5)).color(t.text).italics());
            });
        }
        if look.expanded && !card.details.is_empty() {
            let header = egui::CollapsingHeader::new(RichText::new("Details").font(medium(12.0)).color(t.text_dim))
                .id_salt(("inkwitness_card_details", &card.key))
                .default_open(false)
                .show(ui, |ui| {
                    egui::Grid::new(("inkwitness_card_grid", &card.key)).num_columns(2).spacing(vec2(10.0, 3.0)).show(ui, |ui| {
                        for (k, v) in &card.details {
                            ui.label(RichText::new(k).font(regular(11.5)).color(t.text_dim));
                            ui.add(egui::Label::new(RichText::new(v).font(regular(11.5)).color(t.text)).wrap());
                            ui.end_row();
                        }
                    });
                });
            focus_ring(ui, &header.header_response, &a);
        }
    });
    // The kind's stripe down the card's left edge (with the chip's icon and words, never alone).
    let r = shown.response.rect;
    let stripe = Rect::from_min_max(pos2(r.min.x + 1.0, r.min.y + 1.0), pos2(r.min.x + 5.0, r.max.y - 1.0));
    ui.painter().rect_filled(stripe, CornerRadius { nw: 7, sw: 7, ne: 0, se: 0 }, tint.edge);
    action
}

/// The excerpt as shown: paragraph marks as line breaks, quoted, cut at [`MAX_EXCERPT`].
pub fn excerpt(text: &str) -> String {
    let flat = text.replace(wordcraft_inkwitness::PARA_MARK, "\n");
    let trimmed = flat.trim();
    let mut out: String = trimmed.chars().take(MAX_EXCERPT).collect();
    if trimmed.chars().count() > MAX_EXCERPT {
        out = out.trim_end().to_string();
        out.push('…');
    }
    format!("“{out}”")
}

/// A type chip: the kind's tint with an icon and words.
pub fn chip(ui: &mut Ui, icon: &str, label: &str, tint: Tint) -> egui::Response {
    let font = semibold(11.5);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, tint.text);
    let size = vec2(galley.size().x + 32.0, 22.0);
    let (r, resp) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect(r, CornerRadius::same(11), tint.fill, Stroke::new(1.0, tint.edge), egui::StrokeKind::Inside);
    icons::paint(p, Rect::from_center_size(pos2(r.min.x + 14.0, r.center().y), vec2(13.0, 13.0)), icon, tint.text, tint.text);
    p.galley(pos2(r.min.x + 24.0, r.center().y - galley.size().y / 2.0), galley, tint.text);
    let label = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &label));
    resp
}

/// A small "Showing" pill in the accent colour, with words (not colour alone).
fn status_pill(ui: &mut Ui, text: &str, t: &Tokens) {
    let font = semibold(11.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, t.on_accent);
    let (r, resp) = ui.allocate_exact_size(vec2(galley.size().x + 14.0, 18.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(9), t.accent);
    ui.painter().galley(r.center() - galley.size() / 2.0, galley, t.on_accent);
    let text = text.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
}

/// A 2-point focus ring around `resp` while it has the keyboard focus (3:1 against every Critic
/// surface; `theme` tests).
pub fn focus_ring(ui: &Ui, resp: &egui::Response, a: &Accents) {
    ring(ui, resp, a.focus);
}

/// [`focus_ring`] in `color` (the header's ring: [`Accents::header_focus`]).
pub fn ring(ui: &Ui, resp: &egui::Response, color: egui::Color32) {
    if resp.has_focus() {
        ui.painter().rect_stroke(resp.rect.expand(2.0), CornerRadius::same(5), Stroke::new(2.0, color), egui::StrokeKind::Outside);
    }
}

/// `n` and a noun, singular for one ("1 paste", "2 pastes").
pub fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
