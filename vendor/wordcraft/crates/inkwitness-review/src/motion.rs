//! How the review page moves (FAL-478, `docs/REPLAY_UI.md` "Smooth scrolling and visibility").
//!
//! [`Follower`] decides, every frame, whether the view moves and where. It knows nothing about
//! egui: a frame is the clock, the scroll offset and the layout's facts ([`ReviewView`]), and the
//! answer is the offset to scroll to, how dim the page is and how strongly the mark shows. So the
//! rules are tested with synthetic frames.
//!
//! - **Follow only when needed**: the mark inside the visible area, less a comfort margin at the
//!   top and bottom, means no scroll; otherwise the nearest comfortable position.
//! - **[`Motion`]**: Smooth eases (and takes a long jump in steps), Reduced goes straight there
//!   with a short cross-fade, Instant goes straight there.
//! - **No bouncing while playing**: the view follows text that stays off screen, at most once in
//!   [`PLAY_GAP`] seconds.
//! - **The teacher's scrolling wins**: a scroll nobody here asked for turns following off.

use wordcraft_ui_egui::canvas::ReviewView;

/// `localStorage` key of the remembered [`Motion`] (a preference only, no student data).
pub const KEY: &str = "inkwitness.review.motion";
/// `localStorage` key of "pause playback when I scroll".
pub const PAUSE_KEY: &str = "inkwitness.review.pause-on-scroll";

/// Room kept at the top and bottom of the visible area, as a share of its height.
pub const COMFORT: f32 = 0.18;
/// A jump of more than this many pages is a long jump.
pub const LONG_JUMP_PAGES: f32 = 2.0;
/// The fastest the eased scroll is allowed to go, in pages per second (its peak).
pub const MAX_SPEED_PAGES_PER_S: f32 = 2.2;
/// A long jump leaves this much (in pages) to ease in.
pub const LONG_FINAL_PAGES: f32 = 0.5;
/// Seconds a long jump dims the page before it repositions, and takes to undim after.
pub const DIM_IN: f64 = 0.15;
pub const DIM_OUT: f64 = 0.2;
/// The page cue ("Page 1 → Page 7") stays this long.
pub const CUE_SECS: f64 = 1.5;
/// Reduced motion's cross-fade.
pub const FADE_SECS: f64 = 0.15;
/// Smooth: the mark fades in over this long.
pub const REVEAL_SECS: f64 = 0.25;
/// Playing: the view follows at most once in this many seconds…
pub const PLAY_GAP: f64 = 0.6;
/// …and only text that has stayed off screen this long.
pub const PLAY_DWELL: f64 = 0.4;
/// How far a scroll the follower didn't make may differ before it counts as the teacher's (points).
const MANUAL_SLACK: f32 = 1.5;

/// The Motion menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    #[default]
    Smooth,
    Reduced,
    Instant,
}

impl Motion {
    pub const ALL: [Motion; 3] = [Motion::Smooth, Motion::Reduced, Motion::Instant];

    /// What is saved.
    pub fn code(self) -> &'static str {
        match self {
            Motion::Smooth => "smooth",
            Motion::Reduced => "reduced",
            Motion::Instant => "instant",
        }
    }

    pub fn from_code(code: &str) -> Option<Motion> {
        Motion::ALL.into_iter().find(|m| m.code().eq_ignore_ascii_case(code.trim()))
    }

    pub fn label(self) -> &'static str {
        match self {
            Motion::Smooth => "Smooth",
            Motion::Reduced => "Reduced",
            Motion::Instant => "Instant",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Motion::Smooth => "Eases the page into view; a long jump dims the page and lands near the target",
            Motion::Reduced => "No scrolling movement: goes straight to the text with a brief fade",
            Motion::Instant => "No animation at all",
        }
    }

    /// The saved choice, else what the system asks for: Reduced when animations are off (the
    /// Animations setting, which follows `prefers-reduced-motion` in the browser), otherwise Smooth.
    pub fn resolve(saved: Option<&str>, animations_on: bool) -> Motion {
        saved.and_then(Motion::from_code).unwrap_or(if animations_on { Motion::Smooth } else { Motion::Reduced })
    }
}

/// How a newly shown event reaches the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A seek, a pin or a button: follow at once, and following is back on.
    Seek,
    /// Playback stepped to it: follow only if it stays off screen.
    Play,
}

/// One frame's facts.
#[derive(Clone, Debug)]
pub struct Frame<'a> {
    /// Seconds, any origin.
    pub now: f64,
    /// The scroll offset the page really has, and the most it can have.
    pub offset: f32,
    pub max: f32,
    pub view: &'a ReviewView,
}

/// What to do this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Out {
    /// Scroll the page here.
    pub scroll_to: Option<f32>,
    /// How dim the page is (0 to 1): a long jump in Smooth.
    pub dim: f32,
    /// How opaque the cross-fade veil is (0 to 1): Reduced's jump.
    pub fade: f32,
    /// How strongly the mark shows (0 to 1).
    pub mark_alpha: f32,
    /// "Page 1 → Page 7", for a moment after a jump.
    pub cue: Option<String>,
    /// The teacher scrolled by hand this frame.
    pub manual: bool,
    /// Something is still moving or waiting: draw another frame.
    pub busy: bool,
}

#[derive(Clone, Copy, Debug)]
enum Anim {
    /// An eased scroll.
    Ease { from: f32, to: f32, start: f64, dur: f64 },
    /// A long jump: dim, reposition near the target, ease the last stretch, undim.
    Long { from: f32, near: f32, to: f32, start: f64 },
}

/// The long jump's last stretch eases for this long.
const LONG_EASE: f64 = 0.45;

#[derive(Clone, Copy, Debug)]
struct Candidate {
    top: f32,
    since: f64,
}

/// Decides the view's movement.
#[derive(Clone, Debug)]
pub struct Follower {
    pub motion: Motion,
    /// The view follows playback (the teacher's scrolling turns it off; a seek or the Follow
    /// button turns it on).
    pub following: bool,
    /// Scrolling by hand also pauses playback.
    pub pause_on_scroll: bool,
    pending: Option<Kind>,
    /// The page's epoch the pending event was shown at: the layout facts must be from then or later.
    epoch: u64,
    candidate: Option<Candidate>,
    anim: Option<Anim>,
    /// Where the page should be after what was asked last frame.
    expected: Option<f32>,
    last_follow: f64,
    layout: Option<(u32, u32, usize)>,
    reveal_since: Option<f64>,
    fade_start: Option<f64>,
    cue: Option<(String, f64)>,
    /// The mark can't be found on the current layout.
    pub unresolved: bool,
}

impl Default for Follower {
    fn default() -> Self {
        Follower {
            motion: Motion::Smooth,
            following: true,
            pause_on_scroll: false,
            pending: None,
            epoch: 0,
            candidate: None,
            anim: None,
            expected: None,
            last_follow: f64::MIN,
            layout: None,
            reveal_since: None,
            fade_start: None,
            cue: None,
            unresolved: false,
        }
    }
}

/// Smoothstep: 0 at 0, 1 at 1, no jolt at either end.
pub fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Seconds an eased scroll of `pages` pages (up to [`LONG_JUMP_PAGES`]) takes: 350 to 700 ms for
/// up to a page, then slower so the peak speed stays under [`MAX_SPEED_PAGES_PER_S`].
pub fn ease_duration(pages: f32) -> f64 {
    let pages = f64::from(pages.max(0.0));
    let base = 0.35 + 0.35 * pages.min(1.0);
    let capped = 1.5 * pages / f64::from(MAX_SPEED_PAGES_PER_S);
    base.max(capped).max(pages * 0.7)
}

/// The offset that puts the span `mark` (top, bottom) comfortably in a view of height `h` at
/// `offset`, or `None` when it already is: inside the view less the comfort margin, or, for a
/// span taller than that, any of it in the comfortable part.
pub fn comfortable_offset(mark: (f32, f32), offset: f32, h: f32, max: f32) -> Option<f32> {
    if !(h.is_finite() && h > 0.0) {
        return None;
    }
    let margin = h * COMFORT;
    let (lo, hi) = (offset + margin, offset + h - margin);
    let (top, bottom) = mark;
    let target = if bottom - top <= hi - lo {
        if top >= lo && bottom <= hi {
            return None;
        }
        if top < lo { top - margin } else { bottom - h + margin }
    } else {
        if bottom > lo && top < hi {
            return None;
        }
        top - margin
    };
    let target = target.clamp(0.0, max.max(0.0));
    ((target - offset).abs() >= 0.5).then_some(target)
}

fn page_at(view: &ReviewView, y: f32) -> usize {
    view.pages.iter().position(|(_, bottom)| y <= *bottom).unwrap_or(view.pages.len().saturating_sub(1))
}

/// The distance between page tops.
fn pitch(view: &ReviewView) -> f32 {
    match (view.pages.first(), view.pages.last()) {
        (Some(first), Some(last)) if view.pages.len() > 1 => (last.0 - first.0) / (view.pages.len() - 1) as f32,
        (Some(first), _) => (first.1 - first.0).max(1.0),
        _ => view.view_h.max(1.0),
    }
}

impl Follower {
    /// A new event is shown: `kind` says how the view reaches it.
    pub fn shown(&mut self, kind: Kind) {
        self.shown_at(kind, 0);
    }

    /// [`Follower::shown`], for the page as of `epoch` ([`ReviewView::epoch`]).
    pub fn shown_at(&mut self, kind: Kind, epoch: u64) {
        self.epoch = epoch;
        match kind {
            Kind::Seek => {
                self.following = true;
                self.pending = Some(Kind::Seek);
                self.candidate = None;
                self.reveal_since = None;
            }
            Kind::Play => {
                if self.pending != Some(Kind::Seek) {
                    self.pending = Some(Kind::Play);
                }
            }
        }
    }

    /// The Follow playback button: following is on, and the view goes to the event now.
    pub fn follow_again(&mut self) {
        self.shown(Kind::Seek);
    }

    /// Whether the view is following but the page stopped (nothing pending, nothing moving).
    pub fn idle(&self) -> bool {
        self.pending.is_none() && self.anim.is_none()
    }

    fn start(&mut self, view: &ReviewView, f: &Frame, target: f32, mark_top: f32, instant: bool) -> Option<f32> {
        let from = f.offset;
        let p = pitch(view);
        let cue = {
            let (a, b) = (page_at(view, from + view.view_h / 2.0), page_at(view, mark_top));
            (a != b).then(|| format!("Page {} → Page {}", a + 1, b + 1))
        };
        if let Some(c) = cue {
            self.cue = Some((c, f.now + CUE_SECS));
        }
        self.last_follow = f.now;
        if instant || self.motion != Motion::Smooth {
            if self.motion == Motion::Reduced && !instant {
                self.fade_start = Some(f.now);
            }
            self.anim = None;
            return Some(target);
        }
        let pages = (target - from).abs() / p;
        self.anim = Some(if pages > LONG_JUMP_PAGES {
            let dir = (target - from).signum();
            let near = (target - dir * LONG_FINAL_PAGES * p).clamp(0.0, f.max.max(0.0));
            Anim::Long { from, near, to: target, start: f.now }
        } else {
            Anim::Ease { from, to: target, start: f.now, dur: ease_duration(pages) }
        });
        self.step(f)
    }

    /// The offset an animation wants now (and ends it when done).
    fn step(&mut self, f: &Frame) -> Option<f32> {
        match self.anim? {
            Anim::Ease { from, to, start, dur } => {
                let t = (f.now - start) / dur;
                if t >= 1.0 {
                    self.anim = None;
                    return Some(to);
                }
                Some(from + (to - from) * ease(t) as f32)
            }
            Anim::Long { from, near, to, start } => {
                let e = f.now - start;
                if e < DIM_IN {
                    return Some(from);
                }
                let t = (e - DIM_IN) / LONG_EASE;
                if t >= 1.0 {
                    if e >= DIM_IN + LONG_EASE + DIM_OUT {
                        self.anim = None;
                    }
                    return Some(to);
                }
                Some(near + (to - near) * ease(t) as f32)
            }
        }
    }

    fn dim(&self, now: f64) -> f32 {
        const MAX: f64 = 0.55;
        match self.anim {
            Some(Anim::Long { start, .. }) => {
                let e = now - start;
                let d = if e < DIM_IN {
                    ease(e / DIM_IN)
                } else if e < DIM_IN + LONG_EASE {
                    1.0
                } else {
                    1.0 - ease((e - DIM_IN - LONG_EASE) / DIM_OUT)
                };
                (d * MAX) as f32
            }
            _ => 0.0,
        }
    }

    /// Run one frame.
    pub fn update(&mut self, f: &Frame) -> Out {
        let view = f.view;
        let mut out = Out::default();
        let sized = view.view_h > 0.0 && !view.pages.is_empty();
        if !sized {
            return Out { mark_alpha: 1.0, ..Out::default() };
        }
        // A scroll nobody here asked for is the teacher's.
        let layout = (view.view_h.to_bits(), view.scale.to_bits(), view.cols);
        let relaid = self.layout.is_some_and(|l| l != layout);
        self.layout = Some(layout);
        if let Some(exp) = self.expected
            && !relaid
            && (f.offset - exp.clamp(0.0, f.max.max(0.0))).abs() > MANUAL_SLACK
        {
            self.anim = None;
            self.pending = None;
            self.candidate = None;
            self.following = false;
            out.manual = true;
        }
        // A new layout (window width, zoom) puts the text somewhere else: go to where it is now.
        if relaid && self.following && self.pending.is_none() && self.anim.is_none() {
            self.pending = Some(Kind::Seek);
            self.reveal_since = None;
        }
        self.unresolved = view.mark.is_none();
        // Facts from before the page showed this event say nothing about where it is.
        let ready = view.epoch >= self.epoch;
        if let Some(kind) = self.pending
            && self.following
            && ready
        {
            match (view.mark, kind) {
                (None, _) => self.pending = None,
                (Some(mark), Kind::Seek) => {
                    self.pending = None;
                    if let Some(target) = comfortable_offset(mark, f.offset, view.view_h, f.max) {
                        out.scroll_to = self.start(view, f, target, mark.0, self.motion == Motion::Instant || relaid);
                    }
                }
                (Some(mark), Kind::Play) => match comfortable_offset(mark, f.offset, view.view_h, f.max) {
                    None => {
                        self.pending = None;
                        self.candidate = None;
                    }
                    Some(target) => {
                        let c = match self.candidate {
                            Some(c) if (c.top - mark.0).abs() < pitch(view) * 0.5 => c,
                            _ => Candidate { top: mark.0, since: f.now },
                        };
                        self.candidate = Some(c);
                        if f.now - c.since >= PLAY_DWELL && f.now - self.last_follow >= PLAY_GAP {
                            self.pending = None;
                            self.candidate = None;
                            out.scroll_to = self.start(view, f, target, mark.0, false);
                        }
                    }
                },
            }
        } else if self.pending.is_some() && ready && !self.following {
            // Not following: the page stays where the teacher put it.
            self.pending = None;
        }
        if out.scroll_to.is_none() && self.anim.is_some() {
            out.scroll_to = self.step(f);
        }
        out.dim = self.dim(f.now);
        // The baseline for spotting a scroll made by hand.
        self.expected = Some(out.scroll_to.unwrap_or(f.offset));
        let at = out.scroll_to.unwrap_or(f.offset);
        // The mark shows once its text is on screen.
        let visible = view.mark.is_some_and(|(top, bottom)| bottom >= at && top <= at + view.view_h);
        out.mark_alpha = if !visible {
            self.reveal_since = None;
            0.0
        } else if self.motion != Motion::Smooth {
            1.0
        } else {
            let since = *self.reveal_since.get_or_insert(f.now);
            ease((f.now - since) / REVEAL_SECS) as f32
        };
        if let Some(start) = self.fade_start {
            let t = (f.now - start) / FADE_SECS;
            if t >= 1.0 {
                self.fade_start = None;
            } else {
                out.fade = (1.0 - t) as f32;
            }
        }
        if let Some((text, until)) = &self.cue {
            if f.now >= *until {
                self.cue = None;
            } else {
                out.cue = Some(text.clone());
            }
        }
        out.busy = self.anim.is_some()
            || self.pending.is_some()
            || self.fade_start.is_some()
            || self.cue.is_some()
            || (self.motion == Motion::Smooth && visible && out.mark_alpha < 1.0);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: f32 = 800.0;
    const PAGE: f32 = 1000.0;
    const PITCH: f32 = 1018.0;

    fn view(mark: Option<(f32, f32)>) -> ReviewView {
        ReviewView {
            epoch: 0,
            mark,
            pages: (0..7).map(|i| (18.0 + i as f32 * PITCH, 18.0 + i as f32 * PITCH + PAGE)).collect(),
            view_h: H,
            scale: 1.0,
            cols: 1,
        }
    }

    fn page_top(p: usize) -> f32 {
        18.0 + p as f32 * PITCH
    }

    const MAX: f32 = 7.0 * PITCH - H;

    /// Run `follower` for `secs` at 60 fps from `offset`, applying what it asks; the offsets seen.
    fn run(f: &mut Follower, v: &ReviewView, mut offset: f32, t0: f64, secs: f64) -> Vec<f32> {
        let mut seen = vec![];
        let mut t = t0;
        while t < t0 + secs {
            let out = f.update(&Frame { now: t, offset, max: MAX, view: v });
            if let Some(y) = out.scroll_to {
                offset = y.clamp(0.0, MAX);
            }
            seen.push(offset);
            t += 1.0 / 60.0;
        }
        seen
    }

    fn at(p: usize) -> (f32, f32) {
        (page_top(p) + 100.0, page_top(p) + 160.0)
    }

    /// Where the view goes to show the text on page `p` from `from`.
    fn want(p: usize, from: f32) -> f32 {
        comfortable_offset(at(p), from, H, MAX).unwrap_or(from)
    }

    #[test]
    fn visible_text_inside_the_margin_gives_no_scroll() {
        // Offset 0, view 800, margin 144: 144 to 656 is comfortable.
        assert_eq!(comfortable_offset((200.0, 260.0), 0.0, H, MAX), None);
        assert_eq!(comfortable_offset((144.0, 656.0), 0.0, H, MAX), None);
        // Visible but in the margin: nearest comfortable position.
        assert_eq!(comfortable_offset((690.0, 740.0), 0.0, H, MAX), Some(740.0 - H + 144.0));
        assert_eq!(comfortable_offset((1000.0, 1060.0), 900.0, H, MAX), Some(856.0));
        // A span taller than the comfortable part: no scroll while some of it is in the comfortable part.
        assert_eq!(comfortable_offset((300.0, 1500.0), 0.0, H, MAX), None);
        assert_eq!(comfortable_offset((2000.0, 3500.0), 0.0, H, MAX), Some(1856.0));
        // Clamped to the page's ends.
        assert_eq!(comfortable_offset((10.0, 40.0), 500.0, H, MAX), Some(0.0));
    }

    #[test]
    fn already_in_view_does_not_scroll() {
        let v = view(Some(at(2)));
        let mut f = Follower::default();
        f.shown(Kind::Seek);
        let seen = run(&mut f, &v, page_top(2) - 100.0, 0.0, 2.0);
        assert!(seen.iter().all(|y| *y == page_top(2) - 100.0), "{seen:?}");
    }

    #[test]
    fn duration_and_speed_cap_by_distance() {
        assert!((0.349..=0.4).contains(&ease_duration(0.05)));
        assert!((0.69..=0.71).contains(&ease_duration(1.0)));
        for pages in [0.05f32, 0.3, 0.7, 1.0, 1.3, 1.7, 2.0] {
            let d = ease_duration(pages);
            assert!(d >= 0.35, "{pages}: {d}");
            // The eased scroll peaks at 1.5 times its average speed.
            let peak = 1.5 * f64::from(pages) / d;
            assert!(peak <= f64::from(MAX_SPEED_PAGES_PER_S) + 1e-9, "{pages} pages in {d}s peaks at {peak}");
        }
        assert!(ease_duration(2.0) > ease_duration(1.0));
        // Frame by frame.
        let v = view(Some(at(1)));
        let mut f = Follower::default();
        f.shown(Kind::Seek);
        let seen = run(&mut f, &v, 0.0, 0.0, 2.0);
        let steps: Vec<f32> = seen.windows(2).map(|w| w[1] - w[0]).collect();
        let cap = MAX_SPEED_PAGES_PER_S * PITCH / 60.0;
        assert!(steps.iter().all(|s| *s >= 0.0 && *s <= cap * 1.02), "{steps:?}");
        assert!(seen.last().is_some_and(|y| (*y - want(1, 0.0)).abs() < 0.5));
    }

    #[test]
    fn a_jump_of_more_than_two_pages_takes_the_long_jump_path() {
        let v = view(Some(at(6)));
        let mut f = Follower::default();
        f.shown(Kind::Seek);
        let mut offset = page_top(2);
        let (mut t, mut seen, mut dims, mut cues) = (0.0, vec![], vec![], vec![]);
        while t < 2.0 {
            let out = f.update(&Frame { now: t, offset, max: MAX, view: &v });
            if let Some(y) = out.scroll_to {
                offset = y.clamp(0.0, MAX);
            }
            seen.push(offset);
            dims.push(out.dim);
            cues.extend(out.cue);
            t += 1.0 / 60.0;
        }
        let target = want(6, page_top(2));
        // It never scrolls past the pages in between: it holds, then is within half a page.
        let first_move = seen.iter().position(|y| *y != page_top(2)).unwrap_or(0);
        assert!(first_move >= 8, "dims first: moved at frame {first_move}");
        assert!(dims[3] > 0.0 && dims[3] < dims[10]);
        assert!(seen[first_move] >= target - 0.5 * PITCH - 1.0, "{} vs {target}", seen[first_move]);
        assert!(seen.iter().all(|y| *y == page_top(2) || *y >= target - 0.5 * PITCH - 1.0));
        // The last stretch rises steadily to the target.
        let tail: Vec<f32> = seen[first_move..].iter().copied().take_while(|y| *y < target).collect();
        assert!(tail.windows(2).all(|w| w[1] >= w[0]));
        assert!((seen.last().copied().unwrap_or(0.0) - target).abs() < 0.5);
        // Dimmed while it moved, clear again after.
        assert!(dims.iter().copied().fold(0.0, f32::max) > 0.4);
        assert_eq!(dims.last().copied(), Some(0.0));
        assert!(cues.iter().all(|c| c == "Page 3 → Page 7") && !cues.is_empty(), "{cues:?}");
    }

    #[test]
    fn reduced_and_instant_make_no_in_between_positions() {
        for motion in [Motion::Reduced, Motion::Instant] {
            let v = view(Some(at(6)));
            let mut f = Follower { motion, ..Follower::default() };
            f.shown(Kind::Seek);
            let seen = run(&mut f, &v, page_top(2), 0.0, 2.0);
            let target = want(6, page_top(2));
            let distinct: std::collections::BTreeSet<u32> = seen.iter().map(|y| y.to_bits()).chain([page_top(2).to_bits()]).collect();
            assert_eq!(distinct.len(), 2, "{motion:?}: {seen:?}");
            assert_eq!(seen[0], target);
        }
        // Reduced fades (opacity only, 150 ms); Instant doesn't; both show the cue.
        let v = view(Some(at(6)));
        for (motion, fades) in [(Motion::Reduced, true), (Motion::Instant, false)] {
            let mut f = Follower { motion, ..Follower::default() };
            f.shown(Kind::Seek);
            let out = f.update(&Frame { now: 0.0, offset: page_top(2), max: MAX, view: &v });
            assert_eq!(out.fade > 0.0, fades, "{motion:?}");
            assert_eq!(out.cue.as_deref(), Some("Page 3 → Page 7"));
            assert_eq!(out.mark_alpha, 1.0);
            let later = f.update(&Frame { now: 0.2, offset: out.scroll_to.unwrap_or(0.0), max: MAX, view: &v });
            assert_eq!(later.fade, 0.0);
        }
    }

    #[test]
    fn the_mark_shows_only_once_its_text_is_on_screen() {
        let v = view(Some(at(6)));
        let mut f = Follower::default();
        f.shown(Kind::Seek);
        // Still on page 3 when the frame begins: the jump holds (dim), the mark is not shown.
        let out = f.update(&Frame { now: 0.0, offset: page_top(2), max: MAX, view: &v });
        assert_eq!(out.mark_alpha, 0.0);
        let seen = run(&mut f, &v, page_top(2), 0.0, 2.0);
        assert!(seen.len() > 60);
        // Smooth fades it in once there.
        let mut g = Follower::default();
        g.shown(Kind::Seek);
        let near = page_top(6) - 44.0;
        let out = g.update(&Frame { now: 10.0, offset: near, max: MAX, view: &v });
        assert_eq!(out.mark_alpha, 0.0, "starts its fade");
        let out = g.update(&Frame { now: 10.3, offset: out.scroll_to.unwrap_or(near), max: MAX, view: &v });
        assert_eq!(out.mark_alpha, 1.0);
    }

    #[test]
    fn alternating_anchors_do_not_bounce() {
        // Editing alternates between two places on pages 1 and 5, one event every 120 ms,
        // with the view on page 1.
        let (a, b) = (view(Some(at(0))), view(Some(at(4))));
        let mut f = Follower::default();
        let mut offset = 0.0;
        let mut t = 0.0;
        let mut moves = 0;
        while t < 10.0 {
            f.shown(Kind::Play);
            let v = if ((t / 0.12) as u32).is_multiple_of(2) { &a } else { &b };
            let out = f.update(&Frame { now: t, offset, max: MAX, view: v });
            if let Some(y) = out.scroll_to {
                offset = y;
                moves += 1;
            }
            t += 1.0 / 60.0;
        }
        assert_eq!(moves, 0, "the view swung while text alternated");
    }

    #[test]
    fn playing_follows_text_that_stays_off_screen_at_most_every_600_ms() {
        let mut f = Follower::default();
        let (mut offset, mut t, mut moving) = (0.0f32, 0.0f64, false);
        let mut followed: Vec<f64> = vec![];
        // Text moves on a page every 2 seconds, and stays there; an event arrives every 100 ms.
        while t < 9.0 {
            let page = (t / 2.0) as usize * 2;
            let v = view(Some(at(page.min(6))));
            if ((t * 60.0).round() as i64) % 6 == 0 {
                f.shown(Kind::Play);
            }
            let out = f.update(&Frame { now: t, offset, max: MAX, view: &v });
            let moved = out.scroll_to.is_some_and(|y| y != offset);
            if moved && !moving {
                followed.push(t);
            }
            moving = moved;
            offset = out.scroll_to.unwrap_or(offset);
            t += 1.0 / 60.0;
        }
        assert!(followed.len() >= 3, "{followed:?}");
        // Not on first sight: after the dwell.
        assert!(followed[0] >= 2.0 + PLAY_DWELL - 0.02, "{followed:?}");
        assert!(followed.windows(2).all(|w| w[1] - w[0] >= PLAY_GAP));
    }

    #[test]
    fn a_manual_scroll_turns_following_off_and_follow_or_a_seek_turns_it_on() {
        let v = view(Some(at(1)));
        let mut f = Follower::default();
        f.shown(Kind::Seek);
        let seen = run(&mut f, &v, 0.0, 0.0, 2.0);
        let rest = seen.last().copied().unwrap_or(0.0);
        // The wheel moves the page by 300 during the next frame.
        let out = f.update(&Frame { now: 3.0, offset: rest + 300.0, max: MAX, view: &v });
        assert!(out.manual && !f.following);
        // Playback goes on, but the page stays where the teacher put it.
        let far = view(Some(at(5)));
        f.shown(Kind::Play);
        let seen = run(&mut f, &far, rest + 300.0, 3.1, 3.0);
        assert!(seen.iter().all(|y| *y == rest + 300.0), "followed while off");
        // Follow playback turns it back on and goes to the event.
        f.follow_again();
        assert!(f.following);
        let seen = run(&mut f, &far, rest + 300.0, 7.0, 3.0);
        assert!((seen.last().copied().unwrap_or(0.0) - want(5, rest + 300.0)).abs() < 0.5);
        // So does a seek.
        let mut g = Follower { following: false, ..Follower::default() };
        g.shown(Kind::Seek);
        assert!(g.following);
    }

    #[test]
    fn a_manual_scroll_interrupts_an_animation() {
        let v = view(Some(at(6)));
        let mut f = Follower::default();
        f.shown(Kind::Seek);
        let mut offset = 0.0;
        for i in 0..20 {
            let out = f.update(&Frame { now: f64::from(i) / 60.0, offset, max: MAX, view: &v });
            offset = out.scroll_to.unwrap_or(offset);
        }
        let out = f.update(&Frame { now: 0.4, offset: offset - 400.0, max: MAX, view: &v });
        assert!(out.manual);
        assert_eq!(out.scroll_to, None);
        assert_eq!(out.dim, 0.0);
        let again = f.update(&Frame { now: 0.45, offset: offset - 400.0, max: MAX, view: &v });
        assert_eq!(again.scroll_to, None);
    }

    #[test]
    fn a_layout_change_is_not_a_manual_scroll_and_finds_the_text_again() {
        let mut f = Follower::default();
        let v = view(Some(at(1)));
        f.shown(Kind::Seek);
        let seen = run(&mut f, &v, 0.0, 0.0, 2.0);
        let rest = seen.last().copied().unwrap_or(0.0);
        // The window got narrower: the same text is much lower down.
        let mut narrow = view(Some((at(1).0 + 3000.0, at(1).1 + 3000.0)));
        narrow.cols = 1;
        narrow.view_h = 700.0;
        let out = f.update(&Frame { now: 3.0, offset: rest, max: MAX, view: &narrow });
        assert!(!out.manual && f.following);
        // Straight there, worked out from the new layout.
        let now_at = out.scroll_to.unwrap_or(rest);
        assert!((now_at - (at(1).1 + 3000.0 - 700.0 * (1.0 - COMFORT))).abs() < 0.5, "{now_at}");
        let seen = run(&mut f, &narrow, now_at, 3.02, 1.0);
        assert!(seen.iter().all(|y| *y == now_at) && f.following);
    }

    #[test]
    fn an_unresolved_anchor_never_scrolls() {
        let v = view(None);
        let mut f = Follower::default();
        f.shown(Kind::Seek);
        let seen = run(&mut f, &v, 1234.0, 0.0, 1.0);
        assert!(seen.iter().all(|y| *y == 1234.0));
        assert!(f.unresolved);
    }

    #[test]
    fn the_saved_choice_and_the_system_default() {
        assert_eq!(Motion::resolve(None, true), Motion::Smooth);
        assert_eq!(Motion::resolve(None, false), Motion::Reduced);
        assert_eq!(Motion::resolve(Some("instant"), true), Motion::Instant);
        assert_eq!(Motion::resolve(Some("Smooth"), false), Motion::Smooth);
        assert_eq!(Motion::resolve(Some("garbage"), false), Motion::Reduced);
        for m in Motion::ALL {
            assert_eq!(Motion::from_code(m.code()), Some(m));
        }
    }
}
