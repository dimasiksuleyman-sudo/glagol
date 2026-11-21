//! Deliver dictation to our main WebView without synthesizing keystrokes.

use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, EventTarget, Listener, Manager};

use super::PasteOrigin;

const REQUEST_EVENT: &str = "dictation-insert-in-app";
const REPLY_EVENT: &str = "dictation-insert-result";
// Leave time for the reply after the frontend stops accepting the request.
const INSERT_BUDGET: Duration = Duration::from_millis(500);
const REPLY_BUDGET: Duration = Duration::from_secs(1);

#[derive(Clone, Serialize)]
struct Request<'a> {
    id: &'a str,
    text: &'a str,
    expires_at_ms: u64,
}

#[derive(Deserialize)]
struct Reply {
    id: String,
    inserted: bool,
}

#[derive(Debug)]
pub(super) enum Delivery {
    Inserted,
    Declined,
    WindowUnavailable,
    EventFailed,
    NoReply,
}

/// A failed request falls back to the clipboard at the insertion boundary.
pub(super) fn deliver(app: &AppHandle, origin: PasteOrigin, text: &str) -> Delivery {
    let Some(window) = app.get_webview_window("main") else {
        return Delivery::WindowUnavailable;
    };
    if !window
        .hwnd()
        .is_ok_and(|hwnd| origin.is_window(hwnd.0 as usize))
        || origin.ensure_current().is_err()
    {
        return Delivery::WindowUnavailable;
    }

    let id = uuid::Uuid::new_v4().to_string();
    let expected_id = id.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    let listener = app.listen(REPLY_EVENT, move |event| {
        if let Ok(reply) = serde_json::from_str::<Reply>(event.payload()) {
            if reply.id == expected_id {
                let _ = tx.try_send(reply.inserted);
            }
        }
    });
    let expires_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .saturating_add(INSERT_BUDGET)
        .as_millis() as u64;
    let result = if app
        .emit_to(
            EventTarget::webview_window(window.label()),
            REQUEST_EVENT,
            Request {
                id: &id,
                text,
                expires_at_ms,
            },
        )
        .is_err()
    {
        Delivery::EventFailed
    } else {
        match rx.recv_timeout(REPLY_BUDGET) {
            Ok(true) => Delivery::Inserted,
            Ok(false) => Delivery::Declined,
            Err(_) => Delivery::NoReply,
        }
    };
    app.unlisten(listener);
    result
}
