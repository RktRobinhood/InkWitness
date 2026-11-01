//! InkWitness on WordCraft's egui app (`docs/SPEC.md`, InkWitness repository).
//!
//! - [`writer`]: the `/write/` route records every committed edit ([`wordcraft_inkwitness`]) and
//!   Save puts the history into the DOCX as its evidence part.
//! - [`review`]: the `/review/` route opens such a DOCX from a local file (picker or drop, no
//!   network), rebuilds any recorded state in WordCraft's own page renderer, read-only, and
//!   seeks to an event with its range highlighted and scrolled into view. With Full ribbon on,
//!   the teacher can edit the final document as tracked changes in their name ([`edits`]).
//!
//! Which route a page is comes from its URL ([`Route::from_url`]).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod card;
pub mod comments;
pub mod edits;
pub mod export;
pub mod findings;
pub mod flags;
pub mod motion;
pub mod outside;
pub mod player;
pub mod rail;
pub mod review;
pub mod sample;
pub mod steps;
pub mod toolbar;
#[cfg(target_arch = "wasm32")]
pub mod web;
pub mod writer;

/// The InkWitness route a page is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Write,
    Review,
}

impl Route {
    /// The route for a page at `pathname` with query `search` (`?a=b&c`): `/review/` (its last
    /// path segment is `review`, or `review.html`), or `?mode=review`, is Review; anything else
    /// is Write. The query is split into its parameters, never searched as a string.
    pub fn from_url(pathname: &str, search: &str) -> Route {
        let last = pathname.split('/').rfind(|s| !s.is_empty()).unwrap_or_default();
        let by_path = last == "review" || last == "review.html";
        let by_query = search.trim_start_matches('?').split('&').any(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            k == "mode" && v == "review"
        });
        if by_path || by_query { Route::Review } else { Route::Write }
    }
}

#[cfg(test)]
mod tests;
