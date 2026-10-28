//! The writer: every committed edit is recorded, and Save writes the history into the DOCX.

use wordcraft_inkwitness::evidence::{self, Attempt};
use wordcraft_inkwitness::{Config, Env, Recorder, envelope};
use wordcraft_ui_egui::WordApp;

/// The InkWitness writer version written into evidence (`writerVersion`).
pub const WRITER_VERSION: &str = concat!("inkwitness-g0-preview/", env!("CARGO_PKG_VERSION"));

/// Attach a recorder to the app's session and make Save write its history into the DOCX
/// (`Services::prepare_save`). `now` gives the local time for the export record.
///
/// G0 has no student onboarding or attestation yet (G1): the attempt's identity and policy
/// acknowledgment are placeholders, and say so.
pub fn attach(app: &mut WordApp, mut env: Box<dyn Env>, now: Box<dyn Fn() -> String>) -> Recorder {
    let started = env.now_iso();
    let config = Config { attempt_id: env.uuid(), session_span_id: env.uuid(), field_id: "essay".into(), section_id: Some("main".into()) };
    let attempt = Attempt {
        assignment_id: "g0-technical-preview".into(),
        student_display_name: "Not entered (G0 technical preview)".into(),
        identity_entered_by_student: false,
        entered_at_local: started.clone(),
        policy_version: "g0-preview-no-attestation".into(),
        acknowledged_at_local: started.clone(),
        writer_version: WRITER_VERSION.into(),
        started_at_local: started,
    };
    let recorder = Recorder::attach(&mut app.session, env, config);
    let r = recorder.clone();
    app.services.prepare_save = Some(Box::new(move |name, doc| {
        let ext = std::path::Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        if wordcraft_docx::Flavor::from_ext(&ext).is_none() {
            // Only a Word document carries the evidence part.
            return Ok(None);
        }
        let payload = evidence::document(&r, doc, &attempt, &now());
        let bytes = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        let mut out = doc.clone();
        envelope::embed(&mut out, &r.config().attempt_id, &bytes).map_err(|e| format!("the edit history couldn't be added: {e}"))?;
        Ok(Some(out))
    }));
    recorder
}
