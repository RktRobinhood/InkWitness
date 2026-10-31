//! The review page's chrome (FAL-448): a viewer's toolbar in place of WordCraft's ribbon. The page
//! shows no control that edits the document. What a reviewer keeps is listed in [`SHOWN`]: the
//! teacher's own marks (Highlight, New comment), the file's comments (show or hide, previous,
//! next), the view (zoom, page view, page colour), Find, opening another file, and the Full ribbon
//! switch. WordCraft's title bar, ribbon, status bar and File page are not drawn
//! ([`WordApp::viewer_chrome`]); [`HIDDEN`] lists the editing controls that must not appear.
//!
//! Full ribbon (FAL-547) brings back WordCraft's ribbon under the toolbar, without its File tab or
//! its Share and Editing buttons ([`WordApp::viewer_ribbon`]). The browser remembers the choice.

use egui::{Align, Color32, CornerRadius, Layout, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_doc::props::Highlight;
use wordcraft_ui_egui::theme::critic::{self, Accents};
use wordcraft_ui_egui::theme::{Palette, medium, regular, semibold};
use wordcraft_ui_egui::{WordApp, icons};

use crate::card::ring;

/// The accessible names of the toolbar's controls (what a screen reader and the test find).
pub const SHOWN: &[&str] = &[
    "Highlight",
    "Highlight colour",
    "New comment",
    "Show comments",
    "Previous comment",
    "Next comment",
    "Zoom out",
    "Zoom level",
    "Zoom in",
    "One page",
    "Page width",
    "Dark page",
    "Find",
    "Open DOCX…",
    "Full ribbon",
];

/// Write controls that must never show on the review page (asserted by the tests).
pub const HIDDEN: &[&str] = &[
    "AutoSave",
    "Share",
    "Editing",
    "Reviewing",
    "Dictate",
    "Editor",
    "File",
    "Home",
    "Insert",
    "Draw",
    "Design",
    "Layout",
    "References",
    "Mailings",
    "Review",
    "View",
    "Developer",
    "Zotero",
    "Help",
    "Save",
    "Undo",
    "Redo",
    "New Comment",
    "Reply",
    "Resolve",
    "Delete",
];

/// Make `app` a viewer with the Critic palette, fitting the page to the window's width, with the
/// ribbon if the reviewer last asked for it. Call it after the stored preferences are applied
/// (they carry a zoom).
pub fn setup(app: &mut WordApp) {
    app.viewer_chrome = true;
    app.viewer_ribbon = stored_ribbon();
    app.palette = Palette::Critic;
    app.ui.backstage = false;
    let _ = app.run("view.zoom", json!({"value": "pageWidth"}));
}

/// The browser's key for the Full ribbon switch.
pub const RIBBON_KEY: &str = "inkwitness.review.fullRibbon";

/// Whether the reviewer last left Full ribbon on (in this browser).
fn stored_ribbon() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        crate::web::stored(RIBBON_KEY).as_deref() == Some("on")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

/// Turn Full ribbon on or off, and remember it in this browser.
pub fn set_ribbon(app: &mut WordApp, on: bool) {
    app.viewer_ribbon = on;
    #[cfg(target_arch = "wasm32")]
    crate::web::store(RIBBON_KEY, if on { "on" } else { "off" });
}

/// The highlight colours a teacher picks from, with their names: Word's own highlighter colours.
pub const COLOURS: [(Highlight, &str); 4] =
    [(Highlight::Yellow, "Yellow"), (Highlight::BrightGreen, "Green"), (Highlight::Turquoise, "Blue"), (Highlight::Pink, "Pink")];

/// A highlight colour's name ([`COLOURS`]).
pub fn colour_name(h: Highlight) -> &'static str {
    COLOURS.iter().find(|(c, _)| *c == h).map_or("Yellow", |(_, n)| n)
}

/// A highlight colour as painted over the page: Word's colour, see-through so the text shows.
pub fn page_colour(h: Highlight) -> Color32 {
    let c = h.rgb().unwrap_or(wordcraft_doc::Rgb(255, 255, 0));
    Color32::from_rgba_unmultiplied(c.0, c.1, c.2, 110)
}

/// What the reviewer asked the toolbar for, that the review page does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarAction {
    /// Highlight the text selected on the page in this colour.
    Highlight(Highlight),
    /// Open a comment draft on the text selected on the page.
    Comment,
}

/// The toolbar's height.
pub const HEIGHT: f32 = 46.0;

/// Draw the toolbar across the top of the window. `editing` is the teacher's name while they edit
/// ([`crate::edits`]); `colour` is the Highlight button's colour (a pick from its menu changes it).
pub fn show(ui: &mut Ui, app: &mut WordApp, has_file: bool, editing: Option<&str>, colour: &mut Highlight) -> Option<ToolbarAction> {
    let a = Accents::get(ui.ctx());
    let mut act = None;
    egui::Panel::top("inkwitness_viewer_bar")
        .exact_size(HEIGHT)
        .frame(egui::Frame::NONE.fill(a.header).inner_margin(egui::Margin { left: 14, right: 12, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                mark(ui.painter(), r);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, "InkWitness Review mark"));
                ui.add_space(8.0);
                wordmark(ui, &a);
                ui.add_space(14.0);
                match editing {
                    Some(name) => ui.label(RichText::new(format!("Editing as {name}")).font(regular(11.5)).color(a.on_header_dim)).on_hover_text(
                        "Your changes are tracked changes in your name. They go only into the review copy; the student's file is never changed.",
                    ),
                    None => ui
                        .label(RichText::new("Read-only").font(regular(11.5)).color(a.on_header_dim))
                        .on_hover_text("Viewing only: nothing on this page changes the student's file."),
                };
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                    let ribbon = app.viewer_ribbon;
                    if switch(ui, &a, "Full ribbon", ribbon, "Show WordCraft's whole ribbon under this bar. This browser remembers the choice.")
                        .clicked()
                    {
                        set_ribbon(app, !ribbon);
                    }
                    ui.add_space(8.0);
                    if open_button(ui, &a).clicked()
                        && let Some(open) = &app.services.open_async
                    {
                        open("document");
                    }
                    ui.add_space(10.0);
                    ui.add_enabled_ui(has_file, |ui| {
                        tools(ui, app, &a);
                        divider(ui, &a);
                        act = marks(ui, &a, colour);
                    });
                });
            });
        });
    act
}

/// The teacher's own marks, right to left: New comment, then Highlight with its colour menu.
fn marks(ui: &mut Ui, a: &Accents, colour: &mut Highlight) -> Option<ToolbarAction> {
    let mut act = None;
    if tool(ui, a, "comment", "New comment", true, false, "Comment on the text selected on the page").clicked() {
        act = Some(ToolbarAction::Comment);
    }
    ui.add_space(4.0);
    let more = menu_arrow(ui, a, "Highlight colour", "Choose the highlight colour");
    egui::Popup::menu(&more).show(|ui| {
        for (h, name) in COLOURS {
            let row = ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(3), h.rgb().map_or(Color32::YELLOW, |c| Color32::from_rgb(c.0, c.1, c.2)));
                ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
                ui.selectable_label(*colour == h, name)
            });
            if row.inner.clicked() {
                *colour = h;
                act = Some(ToolbarAction::Highlight(h));
                ui.close();
            }
        }
    });
    let swatch = colour.rgb().map_or(Color32::YELLOW, |c| Color32::from_rgb(c.0, c.1, c.2));
    let tip = format!("Highlight the text selected on the page in {}", colour_name(*colour).to_lowercase());
    if tool_in(ui, a, "highlight", "Highlight", true, false, &tip, swatch).clicked() {
        act = Some(ToolbarAction::Highlight(*colour));
    }
    act
}

/// The viewing tools, right to left: Find, View, Comments.
fn tools(ui: &mut Ui, app: &mut WordApp, a: &Accents) {
    let v = &app.session.view;
    let (comments, dark_page, fit) = (v.comments_pane, v.dark_mode, v.fit.clone());
    let zoom = (v.zoom * 100.0).round();
    let nav = v.nav_pane;
    let mut run: Option<(&str, serde_json::Value)> = None;
    if tool(ui, a, "find", "Find", true, nav, "Find text in the document (Navigation pane)").clicked() {
        run = Some(("view.navigationPane", json!({"value": !nav})));
    }
    divider(ui, a);
    if tool(ui, a, "darkMode", "Dark page", false, dark_page, "Show the pages dark, for reading").clicked() {
        run = Some(("view.darkMode", json!({"value": !dark_page})));
    }
    if tool(ui, a, "pageWidth", "Page width", false, fit == "pageWidth", "Fit the page to the window's width").clicked() {
        run = Some(("view.zoom", json!({"value": "pageWidth"})));
    }
    if tool(ui, a, "onePage", "One page", false, fit == "onePage", "Show one whole page").clicked() {
        run = Some(("view.zoom", json!({"value": "onePage"})));
    }
    if tool(ui, a, "zoomIn", "Zoom in", false, false, "Zoom in").clicked() {
        run = Some(("view.zoomIn", json!({})));
    }
    let level = text_tool(ui, a, &format!("{zoom}%"), "Zoom level");
    if level.on_hover_text("Zoom…").clicked() {
        run = Some(("ui.dialog", json!({"name": "zoom"})));
    }
    if tool(ui, a, "zoomOut", "Zoom out", false, false, "Zoom out").clicked() {
        run = Some(("view.zoomOut", json!({})));
    }
    divider(ui, a);
    if tool(ui, a, "nextComment", "Next comment", false, false, "Next comment in the file").clicked() {
        run = Some(("review.nextComment", json!({})));
    }
    if tool(ui, a, "prevComment", "Previous comment", false, false, "Previous comment in the file").clicked() {
        run = Some(("review.previousComment", json!({})));
    }
    if tool(ui, a, "showComments", "Show comments", true, comments, "Show or hide the file's comments").clicked() {
        run = Some(("view.commentsPane", json!({"value": !comments})));
    }
    if let Some((id, params)) = run
        && let Err(e) = app.run(id, params)
    {
        app.status(e);
    }
}

/// A toolbar button on the navy bar: an icon, its words when `words`, a lighter fill while `on`.
fn tool(ui: &mut Ui, a: &Accents, icon: &str, label: &str, words: bool, on: bool, tip: &str) -> egui::Response {
    tool_in(ui, a, icon, label, words, on, tip, critic::REVIEW_GOLD)
}

/// [`tool`] with the icon's accent colour (the Highlight button's colour bar).
#[allow(clippy::too_many_arguments)]
fn tool_in(ui: &mut Ui, a: &Accents, icon: &str, label: &str, words: bool, on: bool, tip: &str, accent: Color32) -> egui::Response {
    let font = medium(12.5);
    let shown = if label == "Show comments" { "Comments" } else { label };
    let galley = words.then(|| ui.painter().layout_no_wrap(shown.to_string(), font, a.on_header));
    let w = 32.0 + galley.as_ref().map_or(0.0, |g| g.size().x + 6.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    let enabled = ui.is_enabled();
    let fg = if enabled { a.on_header } else { a.on_header_dim };
    let p = ui.painter();
    if on {
        p.rect_filled(r, CornerRadius::same(6), a.header.lerp_to_gamma(a.on_header, 0.2));
    } else if resp.hovered() && enabled {
        p.rect_filled(r, CornerRadius::same(6), a.header.lerp_to_gamma(a.on_header, 0.12));
    }
    icons::paint(p, Rect::from_center_size(pos2(r.min.x + 16.0, r.center().y), vec2(16.0, 16.0)), icon, fg, accent);
    if let Some(g) = galley {
        p.galley(pos2(r.min.x + 28.0, r.center().y - g.size().y / 2.0), g, fg);
    }
    let label = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, &label));
    ring(ui, &resp, a.header_focus);
    resp.on_hover_text(tip)
}

/// The narrow ▾ beside a split button, that opens its menu.
fn menu_arrow(ui: &mut Ui, a: &Accents, label: &str, tip: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(16.0, 32.0), Sense::click());
    let enabled = ui.is_enabled();
    if resp.hovered() && enabled {
        ui.painter().rect_filled(r, CornerRadius::same(6), a.header.lerp_to_gamma(a.on_header, 0.12));
    }
    let fg = if enabled { a.on_header } else { a.on_header_dim };
    icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(10.0, 10.0)), "chevronDown", fg, fg);
    let label = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
    ring(ui, &resp, a.header_focus);
    resp.on_hover_text(tip)
}

/// An on/off switch on the navy bar: its words, then a small track with the knob at the right
/// while `on`.
fn switch(ui: &mut Ui, a: &Accents, label: &str, on: bool, tip: &str) -> egui::Response {
    let g = ui.painter().layout_no_wrap(label.to_string(), medium(12.5), a.on_header);
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 48.0, 32.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(6), a.header.lerp_to_gamma(a.on_header, 0.12));
    }
    ui.painter().galley(pos2(r.min.x + 8.0, r.center().y - g.size().y / 2.0), g, a.on_header);
    let track = Rect::from_center_size(pos2(r.max.x - 20.0, r.center().y), vec2(26.0, 14.0));
    let t = wordcraft_ui_egui::theme::Tokens::get(ui.ctx());
    let fill = if on { t.accent } else { a.header.lerp_to_gamma(a.on_header, 0.3) };
    ui.painter().rect_filled(track, CornerRadius::same(7), fill);
    let knob = if on { track.max.x - 7.0 } else { track.min.x + 7.0 };
    ui.painter().circle_filled(pos2(knob, track.center().y), 5.0, Color32::WHITE);
    let label = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, &label));
    ring(ui, &resp, a.header_focus);
    resp.on_hover_text(tip)
}

/// A toolbar button that is only text (the zoom level).
fn text_tool(ui: &mut Ui, a: &Accents, text: &str, label: &str) -> egui::Response {
    let font = medium(12.5);
    let g = ui.painter().layout_no_wrap(text.to_string(), font, a.on_header);
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x.max(36.0) + 12.0, 32.0), Sense::click());
    if resp.hovered() && ui.is_enabled() {
        ui.painter().rect_filled(r, CornerRadius::same(6), a.header.lerp_to_gamma(a.on_header, 0.12));
    }
    let fg = if ui.is_enabled() { a.on_header } else { a.on_header_dim };
    ui.painter().galley(r.center() - g.size() / 2.0, g, fg);
    let label = label.to_string();
    let enabled = ui.is_enabled();
    let shown = text.to_string();
    resp.widget_info(move || {
        let mut info = egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label);
        info.current_text_value = Some(shown.clone());
        info
    });
    ring(ui, &resp, a.header_focus);
    resp
}

/// "Open DOCX…": the one filled button, in Witness Blue.
fn open_button(ui: &mut Ui, a: &Accents) -> egui::Response {
    let t = wordcraft_ui_egui::theme::Tokens::get(ui.ctx());
    let text = "Open DOCX…";
    let g = ui.painter().layout_no_wrap(text.to_string(), medium(12.5), t.on_accent);
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 40.0, 30.0), Sense::click());
    let fill = if resp.hovered() { t.accent_text } else { t.accent };
    ui.painter().rect_filled(r, CornerRadius::same(6), fill);
    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 15.0, r.center().y), vec2(15.0, 15.0)), "open", t.on_accent, t.on_accent);
    ui.painter().galley(pos2(r.min.x + 28.0, r.center().y - g.size().y / 2.0), g, t.on_accent);
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    ring(ui, &resp, a.header_focus);
    resp.on_hover_text("Open a DOCX saved by InkWitness Write. It stays on this computer.")
}

/// "InkWitness Review", the route's name, as one label.
fn wordmark(ui: &mut Ui, a: &Accents) {
    let ink = ui.painter().layout_no_wrap("InkWitness".into(), semibold(15.0), a.on_header);
    let review = ui.painter().layout_no_wrap("Review".into(), regular(15.0), critic::REVIEW_GOLD);
    let (r, resp) = ui.allocate_exact_size(vec2(ink.size().x + 5.0 + review.size().x, ink.size().y), Sense::hover());
    let y = r.center().y - ink.size().y / 2.0;
    let x = r.min.x + ink.size().x + 5.0;
    ui.painter().galley(pos2(r.min.x, y), ink, a.on_header);
    ui.painter().galley(pos2(x, y), review, critic::REVIEW_GOLD);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "InkWitness Review"));
}

fn divider(ui: &mut Ui, a: &Accents) {
    let (r, _) = ui.allocate_exact_size(vec2(13.0, 24.0), Sense::hover());
    ui.painter().vline(r.center().x, r.y_range(), Stroke::new(1.0, a.on_header_dim.gamma_multiply(0.45)));
}

/// The InkWitness Review mark, drawn in code from `assets/brand/inkwitness-review.svg`'s shapes: a
/// page with a folded corner, an eye with a Witness Blue iris, and the gold Review badge.
pub fn mark(p: &egui::Painter, r: Rect) {
    let s = r.width().min(r.height()) / 128.0;
    let at = |x: f32, y: f32| pos2(r.min.x + x * s, r.min.y + y * s);
    p.rect_filled(r, CornerRadius::same((27.0 * s).round().clamp(0.0, 255.0) as u8), critic::INK_NAVY);
    let page = vec![at(36.0, 19.0), at(82.0, 19.0), at(100.0, 37.0), at(100.0, 106.0), at(36.0, 106.0)];
    p.add(egui::Shape::convex_polygon(page, egui::Color32::from_rgb(0xF9, 0xFC, 0xFF), Stroke::NONE));
    p.line_segment([at(46.0, 46.0), at(72.0, 46.0)], Stroke::new(4.0 * s, egui::Color32::from_rgb(0xA7, 0xCE, 0xFA)));
    p.line_segment([at(46.0, 55.0), at(78.0, 55.0)], Stroke::new(4.0 * s, egui::Color32::from_rgb(0xA7, 0xCE, 0xFA)));
    // The eye: an almond outline around the iris.
    let eye: Vec<_> = (0..=24)
        .map(|i| {
            let u = i as f32 / 24.0 * std::f32::consts::TAU;
            at(69.0 + 26.0 * u.cos(), 81.0 + 13.0 * u.sin() * (1.0 - 0.25 * u.cos().abs()))
        })
        .collect();
    p.add(egui::Shape::convex_polygon(eye, egui::Color32::WHITE, Stroke::new(4.0 * s, critic::INK_NAVY)));
    p.circle_filled(at(69.0, 81.0), 11.0 * s, critic::WITNESS_BLUE);
    p.circle_filled(at(69.0, 81.0), 5.3 * s, egui::Color32::from_rgb(0x14, 0x27, 0x46));
    p.circle_filled(at(104.0, 107.0), 18.0 * s, critic::REVIEW_GOLD);
    p.add(egui::Shape::convex_polygon(vec![at(100.0, 99.0), at(100.0, 115.0), at(111.0, 107.0)], critic::INK_NAVY, Stroke::NONE));
}
