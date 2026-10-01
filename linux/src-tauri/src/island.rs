// Island window: attached to one edge of the screen, centred along it, with the
// compositor doing the placement.
//
// A Wayland client cannot position itself or stay above other windows, so the
// island is a wlr-layer-shell surface (KWin, Sway, Hyprland… all speak it) on the
// overlay layer, anchored to a single edge: anchoring one edge and none of the
// others is what makes the compositor centre it along that edge. It never takes
// the keyboard, and the click-through is the surface's input region: only the
// island's own shape receives the mouse, every other pixel of the transparent
// window lets it fall through to whatever is underneath.
//
// Under X11 there is no layer shell, so the island falls back to a plain
// undecorated always-on-top window that we position ourselves.
//
// Wayland also has no global cursor position, which is why none of this polls:
// the page hears the pointer through ordinary DOM events, and the compositor
// delivers those only while the pointer is over the input region.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use gtk::cairo::{RectangleInt, Region};
use gtk::gdk::{EventMask, NotifyType};
use gtk::glib::Propagation;
use gtk::prelude::*;
use gtk_layer_shell::{Edge as LayerEdge, KeyboardMode, Layer, LayerShell};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

/// Logical size of the full window — the largest island view, like the macOS panel.
pub const PANEL_W: f64 = 720.0;
pub const PANEL_H: f64 = 320.0;
/// The invisible strip that wakes the island when it is hidden: long along the
/// edge, thin across it.
pub const STRIP_LONG: f64 = 240.0;
pub const STRIP_THIN: f64 = 6.0;

pub const WINDOW_LABEL: &str = "island";

/// Margin around the island that still counts as "on the island", in logical px.
/// Wider than the macOS 6 pt because a click must never be swallowed.
const HIT_MARGIN: f64 = 14.0;

/// Which screen edge the island is attached to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    /// Anything unrecognised is the top edge, the default.
    pub fn parse(name: &str) -> Self {
        match name {
            "bottom" => Edge::Bottom,
            "left" => Edge::Left,
            "right" => Edge::Right,
            _ => Edge::Top,
        }
    }

    fn layer_edge(self) -> LayerEdge {
        match self {
            Edge::Top => LayerEdge::Top,
            Edge::Bottom => LayerEdge::Bottom,
            Edge::Left => LayerEdge::Left,
            Edge::Right => LayerEdge::Right,
        }
    }

    fn is_vertical(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }

    /// Logical size of the window: the full panel, or just the wake strip.
    pub fn window_size(self, collapsed: bool) -> (f64, f64) {
        match (collapsed, self.is_vertical()) {
            (false, _) => (PANEL_W, PANEL_H),
            (true, false) => (STRIP_LONG, STRIP_THIN),
            (true, true) => (STRIP_THIN, STRIP_LONG),
        }
    }
}

#[derive(Serialize, Clone)]
pub struct ScreenInfo {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

/// The island shape in window-logical coordinates, pushed by the front end.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct IslandRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// What the window currently is. Lives in `Shared`.
pub struct Island {
    /// Last island shape the page told us about.
    pub rect: Mutex<IslandRect>,
    /// True while the window is just the wake strip.
    pub collapsed: AtomicBool,
    /// True when the window is a layer-shell surface (Wayland), false for the
    /// X11 fallback.
    layer: AtomicBool,
}

impl Island {
    pub fn new() -> Self {
        Self {
            rect: Mutex::new(IslandRect::default()),
            collapsed: AtomicBool::new(false),
            layer: AtomicBool::new(false),
        }
    }

    pub fn is_layer(&self) -> bool {
        self.layer.load(Ordering::Relaxed)
    }
}

pub fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(WINDOW_LABEL)
}

/// Turns the island window into a layer-shell surface when the compositor
/// supports it. Must run on the main thread, after the window exists and
/// before it is first shown — layer-shell refuses a window that is already
/// mapped, which is why the island starts hidden in tauri.conf.json.
pub fn init(app: &AppHandle, edge: Edge) {
    let Some(win) = window(app) else { return };
    let Ok(gw) = win.gtk_window() else { return };
    let island = &app.state::<crate::Shared>().island;

    if gtk_layer_shell::is_supported() {
        gw.init_layer_shell();
        gw.set_layer(Layer::Overlay);
        gw.set_namespace("coucou");
        // 0: stay clear of panels and docks rather than draw over them, so a
        // bottom island sits on top of the Plasma panel, not under it.
        gw.set_exclusive_zone(0);
        // The keyboard belongs to the island only while the chat field is up.
        gw.set_keyboard_mode(KeyboardMode::None);
        island.layer.store(true, Ordering::Relaxed);
        crate::log::line("island: wlr-layer-shell surface");
    } else {
        gw.set_skip_taskbar_hint(true);
        gw.set_skip_pager_hint(true);
        gw.set_keep_above(true);
        gw.set_accept_focus(false);
        crate::log::line("island: no layer-shell here — plain always-on-top window");
    }
    watch_pointer_leaving(app, &gw);
    apply_on_main(app, edge, false);
}

/// Tells the page when the pointer leaves the window. WebKit does not turn the
/// compositor's leave into a DOM `mouseleave` on this surface, and without it the
/// island could never learn the pointer went away — so it would never close.
/// GTK sees it fine, so it is caught here and forwarded.
fn watch_pointer_leaving(app: &AppHandle, gw: &gtk::ApplicationWindow) {
    gw.add_events(EventMask::LEAVE_NOTIFY_MASK);
    let handle = app.clone();
    gw.connect_leave_notify_event(move |_, event| {
        // `Inferior` is the pointer moving into a child widget, not out of the window.
        if event.detail() != NotifyType::Inferior {
            let _ = handle.emit_to(WINDOW_LABEL, "pointer-left", ());
        }
        Propagation::Proceed
    });
}

/// Places and sizes the window. `collapsed` picks the wake strip instead of the
/// panel. Safe from any thread.
pub fn apply_geometry(app: &AppHandle, edge: Edge, collapsed: bool) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || apply_on_main(&handle, edge, collapsed));
}

fn apply_on_main(app: &AppHandle, edge: Edge, collapsed: bool) {
    let Some(win) = window(app) else { return };
    let Ok(gw) = win.gtk_window() else { return };
    let island = &app.state::<crate::Shared>().island;
    island.collapsed.store(collapsed, Ordering::Relaxed);

    let (lw, lh) = edge.window_size(collapsed);
    if island.is_layer() {
        // One edge anchored, the other three free: the compositor centres the
        // surface along the anchored edge.
        for e in [LayerEdge::Top, LayerEdge::Bottom, LayerEdge::Left, LayerEdge::Right] {
            gw.set_anchor(e, e == edge.layer_edge());
            gw.set_layer_shell_margin(e, 0);
        }
        gw.set_size_request(lw as i32, lh as i32);
        gw.set_default_size(lw as i32, lh as i32);
        gw.resize(lw as i32, lh as i32);
    } else {
        place_on_primary(&win, edge, lw, lh);
    }
    apply_input_region(app);
}

/// X11 fallback: centre the window on the primary monitor's chosen edge.
fn place_on_primary(win: &WebviewWindow, edge: Edge, lw: f64, lh: f64) {
    let Some(m) = win.primary_monitor().ok().flatten().or_else(|| win.current_monitor().ok().flatten())
    else {
        return;
    };
    let scale = m.scale_factor();
    let (mp, ms) = (*m.position(), *m.size());
    let pw = (lw * scale).round().max(1.0) as i32;
    let ph = (lh * scale).round().max(1.0) as i32;
    let (x, y) = match edge {
        Edge::Top => (mp.x + (ms.width as i32 - pw) / 2, mp.y),
        Edge::Bottom => (mp.x + (ms.width as i32 - pw) / 2, mp.y + ms.height as i32 - ph),
        Edge::Left => (mp.x, mp.y + (ms.height as i32 - ph) / 2),
        Edge::Right => (mp.x + ms.width as i32 - pw, mp.y + (ms.height as i32 - ph) / 2),
    };
    let _ = win.set_size(PhysicalSize::new(pw as u32, ph as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.set_size(PhysicalSize::new(pw as u32, ph as u32));
    let _ = win.set_always_on_top(true);
}

/// The front end pushes the island shape; we turn it into the input region.
/// Safe from any thread.
pub fn set_rect(app: &AppHandle, rect: IslandRect) {
    {
        let island = &app.state::<crate::Shared>().island;
        let mut current = island.rect.lock().unwrap();
        if *current == rect {
            return;
        }
        *current = rect;
    }
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || apply_input_region(&handle));
}

/// What "no input at all" is: GTK treats a truly empty region as "no restriction",
/// so the island's hidden state keeps a single pixel in a corner of the window.
fn nothing() -> Region {
    Region::create_rectangle(&RectangleInt::new(0, 0, 1, 1))
}

/// Click-through. The compositor hands the mouse to the island's shape (plus a
/// small margin, so a moving pointer is already "in" by the time it reaches a
/// button) and to nothing else.
fn apply_input_region(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    let Ok(gw) = win.gtk_window() else { return };
    let island = &app.state::<crate::Shared>().island;

    if island.collapsed.load(Ordering::Relaxed) {
        // The strip is the whole window and must take the mouse everywhere.
        gw.input_shape_combine_region(None);
        return;
    }

    let r = *island.rect.lock().unwrap();
    let region = if r.w < 0.5 || r.h < 0.5 {
        // A hidden island has no shape: nothing may catch the mouse.
        nothing()
    } else {
        let x0 = (r.x - HIT_MARGIN).max(0.0);
        let y0 = (r.y - HIT_MARGIN).max(0.0);
        let x1 = (r.x + r.w + HIT_MARGIN).min(PANEL_W);
        let y1 = (r.y + r.h + HIT_MARGIN).min(PANEL_H);
        Region::create_rectangle(&RectangleInt::new(
            x0.floor() as i32,
            y0.floor() as i32,
            (x1 - x0).ceil() as i32,
            (y1 - y0).ceil() as i32,
        ))
    };
    gw.input_shape_combine_region(Some(&region));
}

/// The chat field is the only thing in the island that needs the keyboard, so
/// that is the only time the island may take it. Safe from any thread.
pub fn set_keyboard(app: &AppHandle, wanted: bool) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(win) = window(&handle) else { return };
        let Ok(gw) = win.gtk_window() else { return };
        if handle.state::<crate::Shared>().island.is_layer() {
            gw.set_keyboard_mode(if wanted { KeyboardMode::OnDemand } else { KeyboardMode::None });
        } else {
            gw.set_accept_focus(wanted);
            if wanted {
                let _ = win.set_focus();
            }
        }
    });
}

/// Logical rect of the primary monitor, for the page's information only.
pub fn screen_info(app: &AppHandle) -> ScreenInfo {
    match app.primary_monitor().ok().flatten() {
        Some(m) => {
            let scale = m.scale_factor();
            let (p, s) = (*m.position(), *m.size());
            ScreenInfo {
                x: p.x as f64 / scale,
                y: p.y as f64 / scale,
                width: s.width as f64 / scale,
                height: s.height as f64 / scale,
                scale,
            }
        }
        None => ScreenInfo { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0, scale: 1.0 },
    }
}
