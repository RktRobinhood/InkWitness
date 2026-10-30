//! The browser's side: the recorder's clock and random ids, the review page's status region, and
//! the review page's remembered choices (`localStorage`).

/// The browser's clock (`Date`, `performance.now()`) and random ids (`crypto.getRandomValues`).
pub struct WebEnv {
    start: f64,
    last: u64,
    fallback: u64,
}

impl WebEnv {
    pub fn new() -> Self {
        WebEnv { start: now_ms(), last: 0, fallback: 0 }
    }
}

impl Default for WebEnv {
    fn default() -> Self {
        Self::new()
    }
}

fn now_ms() -> f64 {
    web_sys::window().and_then(|w| w.performance()).map_or_else(js_sys::Date::now, |p| p.now())
}

/// The local time, RFC 3339 (UTC).
pub fn now_iso() -> String {
    js_sys::Date::new_0().to_iso_string().into()
}

impl wordcraft_inkwitness::Env for WebEnv {
    fn now_iso(&mut self) -> String {
        now_iso()
    }
    fn monotonic_ms(&mut self) -> u64 {
        let ms = (now_ms() - self.start).max(0.0);
        // Never backwards.
        self.last = self.last.max(if ms.is_finite() { ms as u64 } else { 0 });
        self.last
    }
    fn uuid(&mut self) -> String {
        let mut b = [0u8; 16];
        let filled = web_sys::window().and_then(|w| w.crypto().ok()).is_some_and(|c| c.get_random_values_with_u8_array(&mut b).is_ok());
        if !filled {
            // No crypto: still unique within this page (time and a counter).
            self.fallback = self.fallback.wrapping_add(1);
            let t = js_sys::Date::now().to_bits().to_le_bytes();
            for (i, x) in b.iter_mut().enumerate() {
                *x = t.get(i % 8).copied().unwrap_or(0) ^ self.fallback.to_le_bytes().get(i % 8).copied().unwrap_or(0) ^ (i as u8);
            }
        }
        wordcraft_inkwitness::uuid_v4(b)
    }
}

/// The review page's status region: a visually hidden live region screen readers announce,
/// with the shown event's highlighted text in `data-highlight` (what the browser test reads).
pub const STATUS_ID: &str = "inkwitness-review-status";

/// Write the review's status into the page's live region (made on first use).
pub fn publish(review: &crate::review::Review, line: &str) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let el = match document.get_element_by_id(STATUS_ID) {
        Some(el) => el,
        None => {
            let Ok(el) = document.create_element("div") else { return };
            el.set_id(STATUS_ID);
            let _ = el.set_attribute("role", "status");
            let _ = el.set_attribute("aria-live", "polite");
            // Visually hidden, still read out.
            let _ = el.set_attribute("style", "position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap");
            if let Some(body) = document.body() {
                let _ = body.append_child(&el);
            }
            el
        }
    };
    el.set_text_content(Some(line));
    let seq = review.loaded.as_ref().map_or(String::new(), |l| l.state.seq.to_string());
    let ty = review
        .loaded
        .as_ref()
        .and_then(|l| l.replay.event(l.state.seq))
        .and_then(|e| e.get("type").and_then(serde_json::Value::as_str).map(str::to_string))
        .unwrap_or_default();
    let _ = el.set_attribute("data-seq", &seq);
    let _ = el.set_attribute("data-type", &ty);
    let _ = el.set_attribute("data-highlight", &review.highlighted_text().unwrap_or_default());
    let consistency = review.loaded.as_ref().map_or(String::new(), |l| format!("{:?}", l.consistency));
    let _ = el.set_attribute("data-consistency", &consistency);
}

/// A choice the review page remembered in this browser (`localStorage`), if any.
pub fn stored(key: &str) -> Option<String> {
    web_sys::window()?.local_storage().ok()??.get_item(key).ok()?
}

/// Remember a choice in this browser. A browser that keeps nothing (private mode, storage off)
/// just forgets it.
pub fn store(key: &str, value: &str) {
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.set_item(key, value);
    }
}
