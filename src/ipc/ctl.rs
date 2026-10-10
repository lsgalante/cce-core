//! The compositor's sockets, and the lines other processes exchange with it.
//!
//! The compositor (`cce-fx`) serves three sockets keyed by `$WAYLAND_DISPLAY`:
//! the **control** socket (one request line in, the reply until EOF, one
//! connection per request), the **status** socket (one topic line in, then a
//! line per change for as long as the client stays), and the **stream**
//! socket (window frames for cce-remote).
//!
//! Every name and line format that crosses a crate boundary is spelled here,
//! once, with its parser beside its builder — the compositor parses with the
//! same code a client builds with. Until 2026-10-10 each end spelled its own:
//! four crates hand-typed the control socket's path, the status socket's
//! documented name was not the one bound, and nothing failed when one side
//! changed. Commands only a person types (`ccectl zoom-in`) are not typed
//! here: `ccectl` passes its argv through as one line, and the compositor
//! parses those itself.

use std::fmt;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// Socket prefix of the control socket: `/tmp/cce-<display>.sock`.
pub const CONTROL_PREFIX: &str = "cce";
/// Socket prefix of the status socket: `/tmp/cce-status-interface-<display>.sock`.
pub const STATUS_PREFIX: &str = "cce-status-interface";
/// Socket prefix of the window-frame stream socket: `/tmp/cce-stream-<display>.sock`.
pub const STREAM_PREFIX: &str = "cce-stream";

/// The control socket for this process's `$WAYLAND_DISPLAY`.
pub fn control_socket() -> String {
    super::socket_path(CONTROL_PREFIX)
}

/// The status socket for this process's `$WAYLAND_DISPLAY`.
pub fn status_socket() -> String {
    super::socket_path(STATUS_PREFIX)
}

/// The window-frame stream socket for this process's `$WAYLAND_DISPLAY`.
pub fn stream_socket() -> String {
    super::socket_path(STREAM_PREFIX)
}

/// [`control_socket`] for a named display (the compositor's own side).
pub fn control_socket_for(display: Option<&str>) -> String {
    super::socket_path_for(CONTROL_PREFIX, display)
}

/// [`status_socket`] for a named display (the compositor's own side).
pub fn status_socket_for(display: Option<&str>) -> String {
    super::socket_path_for(STATUS_PREFIX, display)
}

/// [`stream_socket`] for a named display (the compositor's own side).
pub fn stream_socket_for(display: Option<&str>) -> String {
    super::socket_path_for(STREAM_PREFIX, display)
}

/// Send one line on the control socket and return the whole reply.
///
/// `timeout` bounds each write and read (`None` waits as long as the
/// compositor takes — only for a caller that is itself off any UI thread
/// and wants a slow reply, such as `ccectl focus-window --wait`). A line
/// containing a newline is refused: it would end this request and start
/// another one on the same connection.
pub fn send(line: &str, timeout: Option<Duration>) -> std::io::Result<String> {
    send_to(&control_socket(), line, timeout)
}

/// [`send`] for a [`Request`].
pub fn request(req: &Request, timeout: Option<Duration>) -> std::io::Result<String> {
    send(&req.to_string(), timeout)
}

/// [`send`] to an explicit socket path.
pub fn send_to(path: &str, line: &str, timeout: Option<Duration>) -> std::io::Result<String> {
    let line = line.trim_end_matches('\n');
    if line.contains(['\n', '\r']) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("control request spans lines: {line:?}"),
        ));
    }
    let mut stream = UnixStream::connect(path)?;
    stream.set_write_timeout(timeout)?;
    stream.set_read_timeout(timeout)?;
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    Ok(reply)
}

/// A status-socket topic: the line a subscriber sends first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StatusTopic {
    /// Tiling layout and output summary, on every change.
    Layout,
    /// The focused window's title.
    Title,
    /// Held modifiers.
    Modifiers,
    /// `on` / `off` as window-adjust mode (overview, or Super held) comes and goes.
    Adjust,
    /// Dismiss requests for transient surfaces.
    Dismiss,
    /// Portal shortcut edges ([`ShortcutEvent`]).
    Shortcuts,
    /// A press that landed on no X11 surface while an X11 popup was up.
    ClickAway,
    /// Desktop-image group moves ([`SelectionEvent`]).
    Selection,
}

impl StatusTopic {
    pub const ALL: [StatusTopic; 8] = [
        StatusTopic::Layout,
        StatusTopic::Title,
        StatusTopic::Modifiers,
        StatusTopic::Adjust,
        StatusTopic::Dismiss,
        StatusTopic::Shortcuts,
        StatusTopic::ClickAway,
        StatusTopic::Selection,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            StatusTopic::Layout => "layout",
            StatusTopic::Title => "title",
            StatusTopic::Modifiers => "modifiers",
            StatusTopic::Adjust => "adjust",
            StatusTopic::Dismiss => "dismiss",
            StatusTopic::Shortcuts => "shortcuts",
            StatusTopic::ClickAway => "clickaway",
            StatusTopic::Selection => "selection",
        }
    }

    /// The topic a subscription line names, or `None` for one this build
    /// does not know.
    pub fn parse(line: &str) -> Option<StatusTopic> {
        let line = line.trim();
        StatusTopic::ALL.into_iter().find(|t| t.as_str() == line)
    }
}

impl fmt::Display for StatusTopic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One desktop image cce-grid reports (`grid-items`): its id and virtual rect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DesktopItem {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl DesktopItem {
    /// One `<id>:<x>:<y>:<w>:<h>` token.
    pub fn parse(token: &str) -> Option<DesktopItem> {
        let mut f = token.split(':');
        let id = f.next()?.parse().ok()?;
        let x = finite(f.next()?)?;
        let y = finite(f.next()?)?;
        let w = finite(f.next()?)?;
        let h = finite(f.next()?)?;
        f.next().is_none().then_some(DesktopItem { id, x, y, w, h })
    }
}

impl fmt::Display for DesktopItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{:.2}:{:.2}:{:.2}:{:.2}", self.id, self.x, self.y, self.w, self.h)
    }
}

/// A line on the `selection` topic: where a group move carried the desktop
/// images, or that it ended.
#[derive(Clone, Debug, PartialEq)]
pub enum SelectionEvent {
    /// `move <id>:<x>:<y> ...` — the images' new virtual positions.
    Move(Vec<(u64, f64, f64)>),
    /// `drop` — the move is over; report the settled list back.
    Drop,
}

impl SelectionEvent {
    /// `None` for anything else, including a `move` with nothing parseable.
    pub fn parse(line: &str) -> Option<SelectionEvent> {
        let mut words = line.split_whitespace();
        match words.next()? {
            "drop" => Some(SelectionEvent::Drop),
            "move" => {
                let moves: Vec<(u64, f64, f64)> = words
                    .filter_map(|tok| {
                        let mut f = tok.split(':');
                        let id = f.next()?.parse().ok()?;
                        let x = finite(f.next()?)?;
                        let y = finite(f.next()?)?;
                        Some((id, x, y))
                    })
                    .collect();
                (!moves.is_empty()).then_some(SelectionEvent::Move(moves))
            }
            _ => None,
        }
    }
}

impl fmt::Display for SelectionEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelectionEvent::Drop => f.write_str("drop"),
            SelectionEvent::Move(moves) => {
                f.write_str("move")?;
                for (id, x, y) in moves {
                    write!(f, " {id}:{x:.2}:{y:.2}")?;
                }
                Ok(())
            }
        }
    }
}

/// A line on the `shortcuts` topic: a portal shortcut's chord went down or up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutEvent {
    /// `true` for `activated`, `false` for `deactivated`.
    pub activated: bool,
    /// The portal session's object path.
    pub session: String,
    /// The shortcut id, as bound (percent-encoded by the portal).
    pub id: String,
    pub time_msec: u32,
}

impl ShortcutEvent {
    /// `activated|deactivated <session> <id> [<time_msec>]`; a missing or
    /// unparseable time reads as 0.
    pub fn parse(line: &str) -> Option<ShortcutEvent> {
        let mut it = line.split_whitespace();
        let activated = match it.next()? {
            "activated" => true,
            "deactivated" => false,
            _ => return None,
        };
        let session = it.next()?.to_string();
        let id = it.next()?.to_string();
        let time_msec = it.next().and_then(|t| t.parse().ok()).unwrap_or(0);
        Some(ShortcutEvent { activated, session, id, time_msec })
    }
}

impl fmt::Display for ShortcutEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = if self.activated { "activated" } else { "deactivated" };
        write!(f, "{kind} {} {} {}", self.session, self.id, self.time_msec)
    }
}

/// `shortcut …` on the control socket: what the GlobalShortcuts portal binds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutRequest {
    /// `shortcut bind <session> <id> <trigger>`; the three are single tokens
    /// (the portal percent-encodes them).
    Bind { session: String, id: String, trigger: String },
    /// `shortcut unbind <session> [<id>]` — every id of the session when `None`.
    Unbind { session: String, id: Option<String> },
    /// `shortcut clear` — a fresh portal drops whatever a predecessor bound.
    Clear,
    /// `shortcut list`.
    List,
}

/// `idle …` on the control socket, as the desktop portal sends it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdleRequest {
    /// `idle inhibit <token> <ttl_s> <who...>` — take or renew a lease; the
    /// compositor accepts a ttl of 1–600 s and lets the lease lapse unless
    /// it is renewed.
    Inhibit { token: String, ttl_s: u64, who: String },
    /// `idle uninhibit <token>`.
    Uninhibit { token: String },
    /// `idle wake` — activity, as if the user touched the input.
    Wake,
}

/// A control-socket request one process sends another from code.
///
/// Not every command the compositor serves: the ones a person types through
/// `ccectl` stay plain lines. These are the ones a crate builds and the
/// compositor must parse the same way.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// `fade-out` — dissolve the caller's surfaces; the reply is the
    /// duration in ms (the caller is found by its pid).
    FadeOut,
    /// `focus-window [--wait] <query>` — `query` is a window id or an app_id;
    /// `--wait` holds the reply until the window stops moving.
    FocusWindow { query: String, wait: bool },
    /// `pointer-location` — replies `x=<x> y=<y>` (see [`parse_pointer_location`]).
    PointerLocation,
    /// `place-next <app_id> <x> <y>` (or `place-next-cell` when `cell`):
    /// where the next map of that app's floating toplevel lands.
    PlaceNext { app_id: String, x: f64, y: f64, cell: bool },
    /// `windows --json` — one [`WindowInfo`] JSON object per line.
    WindowsJson,
    /// `grid-items <item> ...` — every desktop image cce-grid holds, in draw order.
    GridItems(Vec<DesktopItem>),
    /// `screenshot window <id>` — replies `ok <png path>`.
    ScreenshotWindow { id: u64 },
    /// `lock` — lock the session.
    Lock,
    Shortcut(ShortcutRequest),
    Idle(IdleRequest),
}

impl Request {
    /// The request a control line spells, or `None` when it is not one of
    /// these (or is malformed).
    pub fn parse(line: &str) -> Option<Request> {
        let words: Vec<&str> = line.split_whitespace().collect();
        Some(match words.as_slice() {
            ["fade-out"] => Request::FadeOut,
            ["focus-window", "--wait"] => return None,
            ["focus-window", "--wait", query @ ..] if !query.is_empty() => {
                Request::FocusWindow { query: query.join(" "), wait: true }
            }
            ["focus-window", query @ ..] if !query.is_empty() => {
                Request::FocusWindow { query: query.join(" "), wait: false }
            }
            ["pointer-location"] => Request::PointerLocation,
            [verb @ ("place-next" | "place-next-cell"), app_id, x, y] => Request::PlaceNext {
                app_id: app_id.to_string(),
                x: finite(x)?,
                y: finite(y)?,
                cell: *verb == "place-next-cell",
            },
            ["windows", "--json"] => Request::WindowsJson,
            ["grid-items", items @ ..] => {
                Request::GridItems(items.iter().filter_map(|t| DesktopItem::parse(t)).collect())
            }
            ["screenshot", "window", id] => Request::ScreenshotWindow { id: id.parse().ok()? },
            ["lock"] => Request::Lock,
            ["shortcut", "bind", session, id, trigger] => Request::Shortcut(ShortcutRequest::Bind {
                session: session.to_string(),
                id: id.to_string(),
                trigger: trigger.to_string(),
            }),
            ["shortcut", "unbind", session] => {
                Request::Shortcut(ShortcutRequest::Unbind { session: session.to_string(), id: None })
            }
            ["shortcut", "unbind", session, id] => Request::Shortcut(ShortcutRequest::Unbind {
                session: session.to_string(),
                id: Some(id.to_string()),
            }),
            ["shortcut", "clear"] => Request::Shortcut(ShortcutRequest::Clear),
            ["shortcut", "list"] => Request::Shortcut(ShortcutRequest::List),
            ["idle", "inhibit", token, ttl, who @ ..] if !who.is_empty() => Request::Idle(IdleRequest::Inhibit {
                token: token.to_string(),
                ttl_s: ttl.parse().ok()?,
                who: who.join(" "),
            }),
            ["idle", "uninhibit", token] => Request::Idle(IdleRequest::Uninhibit { token: token.to_string() }),
            ["idle", "wake"] => Request::Idle(IdleRequest::Wake),
            _ => return None,
        })
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Request::FadeOut => f.write_str("fade-out"),
            Request::FocusWindow { query, wait: true } => write!(f, "focus-window --wait {query}"),
            Request::FocusWindow { query, wait: false } => write!(f, "focus-window {query}"),
            Request::PointerLocation => f.write_str("pointer-location"),
            Request::PlaceNext { app_id, x, y, cell } => {
                let verb = if *cell { "place-next-cell" } else { "place-next" };
                write!(f, "{verb} {app_id} {x} {y}")
            }
            Request::WindowsJson => f.write_str("windows --json"),
            Request::GridItems(items) => {
                f.write_str("grid-items")?;
                for item in items {
                    write!(f, " {item}")?;
                }
                Ok(())
            }
            Request::ScreenshotWindow { id } => write!(f, "screenshot window {id}"),
            Request::Lock => f.write_str("lock"),
            Request::Shortcut(ShortcutRequest::Bind { session, id, trigger }) => {
                write!(f, "shortcut bind {session} {id} {trigger}")
            }
            Request::Shortcut(ShortcutRequest::Unbind { session, id: None }) => write!(f, "shortcut unbind {session}"),
            Request::Shortcut(ShortcutRequest::Unbind { session, id: Some(id) }) => {
                write!(f, "shortcut unbind {session} {id}")
            }
            Request::Shortcut(ShortcutRequest::Clear) => f.write_str("shortcut clear"),
            Request::Shortcut(ShortcutRequest::List) => f.write_str("shortcut list"),
            Request::Idle(IdleRequest::Inhibit { token, ttl_s, who }) => write!(f, "idle inhibit {token} {ttl_s} {who}"),
            Request::Idle(IdleRequest::Uninhibit { token }) => write!(f, "idle uninhibit {token}"),
            Request::Idle(IdleRequest::Wake) => f.write_str("idle wake"),
        }
    }
}

/// The `pointer-location` reply, `x=<x> y=<y>`, as layout coordinates.
pub fn parse_pointer_location(reply: &str) -> Option<(f64, f64)> {
    let mut x = None;
    let mut y = None;
    for word in reply.split_whitespace() {
        if let Some(v) = word.strip_prefix("x=") {
            x = finite(v);
        } else if let Some(v) = word.strip_prefix("y=") {
            y = finite(v);
        }
    }
    Some((x?, y?))
}

/// One line of the `windows --json` reply.
///
/// Positions are layout px (`x`, `y`, `w`, `h`) and virtual units (`vx`,
/// `vy`); `stack` is the window's place in the render list, bottom 0, -1 when
/// unlinked. Built by the compositor and read by the status bar, its OSD and
/// cce-remote through [`WindowInfo::from_json_line`]; fields a reader does not
/// need are still parsed, so the struct is the whole contract.
#[cfg(feature = "json")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowInfo {
    pub id: u64,
    pub app_id: String,
    pub title: String,
    /// The tiling mode's name (`Floating`, `Tiled`, `Status`, …).
    pub mode: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub vx: f64,
    pub vy: f64,
    /// The grid cell span, e.g. `A1:C4`.
    pub cell: String,
    pub minimized: bool,
    pub has_parent: bool,
    pub focused: bool,
    pub stack: i64,
    pub ssd: bool,
    pub decorated: bool,
    pub beveled: bool,
}

#[cfg(feature = "json")]
impl WindowInfo {
    /// The JSON object, on one line (no trailing newline).
    pub fn to_json_line(&self) -> String {
        serde_json::json!({
            "id": self.id,
            "app_id": self.app_id,
            "title": self.title,
            "mode": self.mode,
            "x": self.x,
            "y": self.y,
            "w": self.w,
            "h": self.h,
            "vx": self.vx,
            "vy": self.vy,
            "cell": self.cell,
            "minimized": self.minimized,
            "has_parent": self.has_parent,
            "focused": self.focused,
            "stack": self.stack,
            "ssd": self.ssd,
            "decorated": self.decorated,
            "beveled": self.beveled,
        })
        .to_string()
    }

    /// One reply line. `None` when it is not a JSON object or lacks `id` or
    /// `app_id`; any other field that is missing reads as its default, so an
    /// older compositor's line still parses.
    pub fn from_json_line(line: &str) -> Option<WindowInfo> {
        let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let i = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or_default();
        let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or_default();
        let b = |k: &str| v.get(k).and_then(|x| x.as_bool()).unwrap_or_default();
        Some(WindowInfo {
            id: v.get("id")?.as_u64()?,
            app_id: v.get("app_id")?.as_str()?.to_string(),
            title: s("title"),
            mode: s("mode"),
            x: i("x") as i32,
            y: i("y") as i32,
            w: i("w") as i32,
            h: i("h") as i32,
            vx: f("vx"),
            vy: f("vy"),
            cell: s("cell"),
            minimized: b("minimized"),
            has_parent: b("has_parent"),
            focused: b("focused"),
            stack: v.get("stack").and_then(|x| x.as_i64()).unwrap_or(-1),
            ssd: b("ssd"),
            decorated: b("decorated"),
            beveled: b("beveled"),
        })
    }

    /// Every window in a `windows --json` reply, skipping lines that do not parse.
    pub fn parse_all(reply: &str) -> Vec<WindowInfo> {
        reply.lines().filter_map(WindowInfo::from_json_line).collect()
    }
}

/// A finite f64, or `None` (NaN and the infinities would poison placement).
fn finite(s: &str) -> Option<f64> {
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(req: Request) {
        let line = req.to_string();
        assert_eq!(Request::parse(&line), Some(req), "{line}");
    }

    #[test]
    fn every_request_parses_back_from_its_own_line() {
        round_trip(Request::FadeOut);
        round_trip(Request::FocusWindow { query: "cce-notes".into(), wait: false });
        round_trip(Request::FocusWindow { query: "42".into(), wait: true });
        round_trip(Request::PointerLocation);
        round_trip(Request::PlaceNext { app_id: "cce-ramp".into(), x: 120.0, y: -4.5, cell: false });
        round_trip(Request::PlaceNext { app_id: "cce-files".into(), x: 0.0, y: 3.0, cell: true });
        round_trip(Request::WindowsJson);
        round_trip(Request::GridItems(vec![
            DesktopItem { id: 7, x: 1.5, y: 2.25, w: 100.0, h: 50.0 },
            DesktopItem { id: 9, x: -3.0, y: 0.0, w: 1.0, h: 1.0 },
        ]));
        round_trip(Request::GridItems(Vec::new()));
        round_trip(Request::ScreenshotWindow { id: 3 });
        round_trip(Request::Lock);
        round_trip(Request::Shortcut(ShortcutRequest::Bind {
            session: "/org/fd/s/1".into(),
            id: "Quick%20Access".into(),
            trigger: "CTRL+ALT+q".into(),
        }));
        round_trip(Request::Shortcut(ShortcutRequest::Unbind { session: "/s".into(), id: None }));
        round_trip(Request::Shortcut(ShortcutRequest::Unbind { session: "/s".into(), id: Some("a".into()) }));
        round_trip(Request::Shortcut(ShortcutRequest::Clear));
        round_trip(Request::Shortcut(ShortcutRequest::List));
        round_trip(Request::Idle(IdleRequest::Inhibit {
            token: "t1".into(),
            ttl_s: 60,
            who: "org.mozilla.firefox playing video".into(),
        }));
        round_trip(Request::Idle(IdleRequest::Uninhibit { token: "t1".into() }));
        round_trip(Request::Idle(IdleRequest::Wake));
    }

    #[test]
    fn malformed_requests_are_not_requests() {
        for line in [
            "",
            "focus-window",
            "focus-window --wait",
            "place-next cce-ramp 1",
            "place-next cce-ramp x 2",
            "place-next cce-ramp NaN 2",
            "screenshot window abc",
            "shortcut bind /s id",
            "idle inhibit t 60",
            "idle inhibit t sixty who",
            "zoom-in",
        ] {
            assert_eq!(Request::parse(line), None, "{line:?}");
        }
    }

    #[test]
    fn grid_items_skip_a_bad_token_and_keep_the_rest() {
        let Some(Request::GridItems(items)) = Request::parse("grid-items 1:0:0:10:10 junk 2:5:5:1:1:9 3:1:2:3:4") else {
            panic!("not grid-items");
        };
        assert_eq!(items.iter().map(|i| i.id).collect::<Vec<_>>(), vec![1, 3]);
    }

    #[test]
    fn status_topics_round_trip_and_unknown_is_none() {
        for t in StatusTopic::ALL {
            assert_eq!(StatusTopic::parse(&format!("{t}\n")), Some(t));
        }
        assert_eq!(StatusTopic::parse("viewport"), None);
    }

    #[test]
    fn selection_and_shortcut_events_round_trip() {
        let mv = SelectionEvent::Move(vec![(4, 10.5, -2.0), (5, 0.0, 0.25)]);
        assert_eq!(SelectionEvent::parse(&mv.to_string()), Some(mv));
        assert_eq!(SelectionEvent::parse("drop"), Some(SelectionEvent::Drop));
        assert_eq!(SelectionEvent::parse("move junk"), None);
        let ev = ShortcutEvent { activated: false, session: "/s/1".into(), id: "id%20x".into(), time_msec: 99 };
        assert_eq!(ShortcutEvent::parse(&ev.to_string()), Some(ev));
        assert_eq!(ShortcutEvent::parse("activated /s/1 a").map(|e| e.time_msec), Some(0));
        assert_eq!(ShortcutEvent::parse("pressed /s/1 a 1"), None);
    }

    #[test]
    fn socket_names_for_a_display() {
        assert_eq!(control_socket_for(Some("wayland-1")), "/tmp/cce-wayland-1.sock");
        assert_eq!(status_socket_for(Some("wayland-1")), "/tmp/cce-status-interface-wayland-1.sock");
        assert_eq!(stream_socket_for(Some("")), "/tmp/cce-stream.sock");
        assert_eq!(control_socket_for(None), "/tmp/cce.sock");
    }

    #[test]
    fn pointer_location_reply() {
        assert_eq!(parse_pointer_location("x=12.5 y=-3\n"), Some((12.5, -3.0)));
        assert_eq!(parse_pointer_location("error: no seat\n"), None);
    }

    #[test]
    fn a_request_spanning_lines_is_refused_before_connecting() {
        let err = send_to("/nonexistent/socket", "lock\nexit", None).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[cfg(feature = "json")]
    #[test]
    fn window_info_round_trips_and_tolerates_missing_fields() {
        let w = WindowInfo {
            id: 3,
            app_id: "cce-files".into(),
            title: "Home \"quoted\"".into(),
            mode: "Floating".into(),
            x: 10,
            y: -20,
            w: 800,
            h: 600,
            vx: 1.5,
            vy: 2.5,
            cell: "A1:C4".into(),
            focused: true,
            stack: 4,
            ..Default::default()
        };
        assert_eq!(WindowInfo::from_json_line(&w.to_json_line()), Some(w));
        let old = WindowInfo::from_json_line(r#"{"id":1,"app_id":"x"}"#).unwrap();
        assert_eq!((old.stack, old.focused), (-1, false));
        assert_eq!(WindowInfo::from_json_line(r#"{"app_id":"x"}"#), None);
        assert_eq!(WindowInfo::parse_all("{\"id\":1,\"app_id\":\"a\"}\nnot json\n").len(), 1);
    }
}
