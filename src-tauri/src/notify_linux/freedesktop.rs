//! Client for the freedesktop notification service over the session D-Bus, free of Tauri types.
//!
//! One worker thread posts notifications and one listener thread reads signals, however many notifications
//! are open. A notification with the same origin replaces the previous one, so the table stays bounded by
//! the number of live sessions.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::Value;
use zbus::MatchRule;

const BUS: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
/// Action key the specification reserves for clicking the notification body. Servers do not show its label as a button.
const DEFAULT_ACTION: &str = "default";
/// Let the server decide how long the notification stays on screen.
const SERVER_TIMEOUT: i32 = -1;

/// The window and session a notification belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub window_label: String,
    pub session_id: String,
}

pub struct Notice {
    pub origin: Origin,
    pub title: String,
    pub body: String,
    pub sound: bool,
}

/// Notification ID to origin. Guarded across each post so a signal cannot be read before its ID is recorded.
type Live = Arc<Mutex<HashMap<u32, Origin>>>;

pub struct Client {
    jobs: Sender<Notice>,
}

impl Client {
    /// Start the worker. `on_click` receives the origin and the activation token when the server sent one;
    /// `on_failed` receives notices that could not be posted, including all of them when no bus is available.
    pub fn start(
        app_name: String,
        icon: String,
        on_click: impl Fn(Origin, Option<String>) + Send + 'static,
        on_failed: impl Fn(Notice) + Send + 'static,
    ) -> Self {
        let (jobs, inbox) = channel();
        std::thread::spawn(move || run(inbox, app_name, icon, on_click, on_failed));
        Self { jobs }
    }

    /// Queue a notification without blocking on D-Bus. Returns false when the worker is gone.
    pub fn send(&self, notice: Notice) -> bool {
        self.jobs.send(notice).is_ok()
    }
}

/// Connect and subscribe before anything is posted, so no signal for a posted notification can be missed.
fn connect() -> zbus::Result<(Connection, MessageIterator)> {
    let connection = Connection::session()?;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(BUS)?
        .path(PATH)?
        .build();
    let signals = MessageIterator::for_match_rule(rule, &connection, None)?;
    Ok((connection, signals))
}

fn run(
    inbox: Receiver<Notice>,
    app_name: String,
    icon: String,
    on_click: impl Fn(Origin, Option<String>) + Send + 'static,
    on_failed: impl Fn(Notice) + Send + 'static,
) {
    let (connection, signals) = match connect() {
        Ok(pair) => pair,
        Err(e) => {
            crate::diagnostic_warn!("cannot reach the notification service: {e}");
            for notice in inbox {
                on_failed(notice);
            }
            return;
        }
    };
    let live: Live = Arc::default();
    let listener_live = live.clone();
    std::thread::spawn(move || listen(signals, listener_live, on_click));

    for notice in inbox {
        let mut table = live.lock().unwrap();
        let replaces = table
            .iter()
            .find_map(|(id, origin)| (*origin == notice.origin).then_some(*id))
            .unwrap_or(0);
        match post(&connection, &app_name, &icon, replaces, &notice) {
            Ok(id) => {
                table.remove(&replaces);
                table.insert(id, notice.origin);
            }
            Err(e) => {
                drop(table);
                crate::diagnostic_warn!("failed to post a Linux notification: {e}");
                on_failed(notice);
            }
        }
    }
}

fn post(
    connection: &Connection,
    app_name: &str,
    icon: &str,
    replaces: u32,
    notice: &Notice,
) -> zbus::Result<u32> {
    let actions = vec![DEFAULT_ACTION, "Open"];
    let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
    if notice.sound {
        hints.insert("sound-name", Value::from("message-new-instant"));
    } else {
        hints.insert("suppress-sound", Value::from(true));
    }
    let reply = connection.call_method(
        Some(BUS),
        PATH,
        Some(BUS),
        "Notify",
        &(app_name, replaces, icon, &notice.title, &notice.body, actions, hints, SERVER_TIMEOUT),
    )?;
    reply.body().deserialize::<u32>()
}

/// Read signals for this client's notifications. Other applications' IDs are not in the table and are ignored.
fn listen(
    signals: MessageIterator,
    live: Live,
    on_click: impl Fn(Origin, Option<String>),
) {
    let mut tokens: HashMap<u32, String> = HashMap::new();
    for message in signals {
        let Ok(message) = message else { continue };
        let header = message.header();
        let Some(member) = header.member() else { continue };
        let body = message.body();
        match member.as_str() {
            // Sent just before ActionInvoked; it lets a Wayland compositor accept the focus request.
            "ActivationToken" => {
                if let Ok((id, token)) = body.deserialize::<(u32, String)>() {
                    if live.lock().unwrap().contains_key(&id) {
                        tokens.insert(id, token);
                    }
                }
            }
            "ActionInvoked" => {
                let Ok((id, action)) = body.deserialize::<(u32, String)>() else { continue };
                if action != DEFAULT_ACTION {
                    continue;
                }
                let origin = live.lock().unwrap().get(&id).cloned();
                if let Some(origin) = origin {
                    on_click(origin, tokens.remove(&id));
                }
            }
            "NotificationClosed" => {
                if let Ok((id, _reason)) = body.deserialize::<(u32, u32)>() {
                    live.lock().unwrap().remove(&id);
                    tokens.remove(&id);
                }
            }
            _ => {}
        }
    }
}
