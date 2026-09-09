//! Small, independently hit-tested native surfaces. No network content runs in these WebViews.
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Context as _;
use cliswitch::{activity, i18n::AppLocale};
use serde::{Deserialize, Serialize};
use tao::dpi::LogicalSize;
#[cfg(not(target_os = "macos"))]
use tao::dpi::PhysicalPosition;
use tao::event::WindowEvent;
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::monitor::MonitorHandle;
use tao::window::{Window, WindowBuilder, WindowId};
use wry::{WebView, WebViewBuilder};

use super::UserEvent;

const ACTIVE_TICK: Duration = Duration::from_millis(50);
const IDLE_TICK: Duration = Duration::from_millis(500);
const TOAST_DURATION: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Surface {
    Pet,
    Panel,
    Toast,
}

impl Surface {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pet => "pet",
            Self::Panel => "panel",
            Self::Toast => "toast",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(super) enum Command {
    Ready,
    ToggleList,
    OpenActivities,
    Dismiss,
    Hide,
    DragStart { x: f64, y: f64 },
    DragMove { x: f64, y: f64 },
    DragEnd,
    HitRegions { rects: Vec<HitRect> },
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct HitRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl HitRect {
    fn valid(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.x >= 0.0
            && self.y >= 0.0
            && self.width > 0.0
            && self.height > 0.0
            && self.x + self.width <= 64.0
            && self.y + self.height <= 64.0
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Dock {
    #[default]
    None,
    Left,
    Right,
}

impl Dock {
    fn size(self) -> (f64, f64) {
        if self == Self::None {
            (64.0, 64.0)
        } else {
            (28.0, 40.0)
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct Placement {
    monitor: Option<String>,
    x: f64,
    y: f64,
    dock: Dock,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Bounds {
    fn clamp(self, position: (f64, f64), size: (f64, f64)) -> (f64, f64) {
        (
            position
                .0
                .clamp(self.x, (self.x + self.width - size.0).max(self.x)),
            position
                .1
                .clamp(self.y, (self.y + self.height - size.1).max(self.y)),
        )
    }
}

struct NativeSurface {
    // Drop WebView before its owning native window.
    view: WebView,
    window: Window,
    ready: bool,
}

impl NativeSurface {
    fn create(
        target: &EventLoopWindowTarget<UserEvent>,
        proxy: &EventLoopProxy<UserEvent>,
        surface: Surface,
    ) -> anyhow::Result<Self> {
        let (width, height) = match surface {
            Surface::Pet => (64.0, 64.0),
            Surface::Panel => (190.0, 136.0),
            Surface::Toast => (190.0, 34.0),
        };
        let builder = WindowBuilder::new()
            .with_title("CliSwitch Pet")
            .with_inner_size(LogicalSize::new(width, height))
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top(true)
            .with_resizable(false)
            .with_maximizable(false)
            .with_minimizable(false)
            .with_focused(false)
            .with_focusable(surface == Surface::Panel)
            .with_visible(false);
        #[cfg(target_os = "macos")]
        let builder = {
            use tao::platform::macos::WindowBuilderExtMacOS;
            builder.with_automatic_window_tabbing(false)
        };
        #[cfg(target_os = "windows")]
        let builder = {
            use tao::platform::windows::WindowBuilderExtWindows;
            builder.with_skip_taskbar(true)
        };
        #[cfg(target_os = "linux")]
        let builder = {
            use tao::platform::unix::WindowBuilderExtUnix;
            builder.with_skip_taskbar(true)
        };
        let window = builder
            .build(target)
            .context("create desktop pet surface")?;
        #[cfg(target_os = "macos")]
        {
            use tao::platform::macos::WindowExtMacOS;
            window.set_has_shadow(false);
            // SAFETY: tao owns the window, and surface creation runs on the main thread.
            let ns = unsafe { &*window.ns_window().cast::<objc2_app_kit::NSWindow>() };
            ns.setOpaque(false);
            ns.setBackgroundColor(None);
            ns.setCollectionBehavior(
                objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllSpaces
                    | objc2_app_kit::NSWindowCollectionBehavior::FullScreenAuxiliary,
            );
        }
        if surface == Surface::Toast {
            let _ = window.set_ignore_cursor_events(true);
        }
        let window_id = window.id();
        let proxy = proxy.clone();
        let html = include_str!("desktop_pet/index.html")
            .replace("/*__PET_CSS__*/", include_str!("desktop_pet/style.css"))
            .replace("/*__PET_SPRITE__*/", include_str!("desktop_pet/sprite.js"))
            .replace("/*__PET_JS__*/", include_str!("desktop_pet/pet.js"));
        let view = WebViewBuilder::new()
            .with_transparent(true)
            .with_focused(false)
            .with_accept_first_mouse(true)
            .with_initialization_script(format!("window.__PET_SURFACE__ = {:?};", surface.as_str()))
            .with_html(html)
            .with_navigation_handler(|url| url == "about:blank")
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_ipc_handler(move |req| {
                // These windows load only bundled content; never accept IPC from an external URL.
                if req.body().len() > 65536 {
                    return;
                }
                if let Ok(command) = serde_json::from_str::<Command>(req.body()) {
                    let _ = proxy.send_event(UserEvent::Pet {
                        window_id,
                        surface,
                        command,
                    });
                }
            })
            .build(&window)
            .context("create desktop pet WebView")?;
        Ok(Self {
            view,
            window,
            ready: false,
        })
    }

    fn show(&self, focus: bool) {
        if !self.ready {
            return;
        }
        // Reapply the level on every show: some platform window managers reset it
        // after a hidden window is shown again.
        self.window.set_always_on_top(true);
        // orderFront does not activate another app's background task on macOS.
        #[cfg(target_os = "macos")]
        if !focus {
            use tao::platform::macos::WindowExtMacOS;
            // SAFETY: tao owns this NSWindow and all callers execute on the main event loop.
            unsafe {
                (&*self.window.ns_window().cast::<objc2_app_kit::NSWindow>()).orderFront(None);
            }
            return;
        }
        self.window.set_visible(true);
        if focus {
            self.window.set_focus();
        }
    }

    fn render(&self, payload: &str) {
        if !self.ready {
            return;
        }
        // Serialize into a JS string so user-controlled titles cannot become executable markup.
        let Ok(encoded) = serde_json::to_string(payload) else {
            return;
        };
        if let Err(err) = self
            .view
            .evaluate_script(&format!("window.renderPetState(JSON.parse({encoded}));"))
        {
            tracing::warn!(err = %err, "render desktop pet failed");
        }
    }
}

#[derive(Clone, Serialize)]
struct Notification {
    title: String,
    count: usize,
}

struct Drag {
    start: (f64, f64),
    origin: (f64, f64),
    scale: f64,
}

pub(super) struct DesktopPet {
    pet: NativeSurface,
    panel: NativeSurface,
    toast: NativeSurface,
    placement_path: PathBuf,
    dock: Dock,
    drag: Option<Drag>,
    panel_open: bool,
    locale: AppLocale,
    notification: Option<Notification>,
    notification_until: Option<Instant>,
    seen_finished: HashSet<String>,
    finished_order: VecDeque<String>,
    born_at_ms: i64,
    hit_regions: Vec<HitRect>,
    ignoring_cursor: bool,
    pub(super) next_tick: Instant,
    last_monitor_check: Instant,
    bounds: Bounds,
    last_activity_revision: u64,
}

pub(super) enum Action {
    None,
    Hide,
    OpenActivities,
}

impl DesktopPet {
    pub(super) fn new(
        target: &EventLoopWindowTarget<UserEvent>,
        proxy: &EventLoopProxy<UserEvent>,
        data_dir: &Path,
        locale: AppLocale,
    ) -> anyhow::Result<Self> {
        let pet = NativeSurface::create(target, proxy, Surface::Pet)?;
        let panel = NativeSurface::create(target, proxy, Surface::Panel)?;
        let toast = NativeSurface::create(target, proxy, Surface::Toast)?;
        let snapshot = activity::snapshot();
        let mut this = Self {
            pet,
            panel,
            toast,
            placement_path: data_dir.join("desktop-pet-position.json"),
            dock: Dock::None,
            drag: None,
            panel_open: false,
            locale,
            notification: None,
            notification_until: None,
            seen_finished: snapshot.entries.iter().filter_map(completion_key).collect(),
            finished_order: snapshot.entries.iter().filter_map(completion_key).collect(),
            born_at_ms: cliswitch::storage::now_ms(),
            hit_regions: vec![HitRect {
                x: 8.0,
                y: 8.0,
                width: 52.0,
                height: 52.0,
            }],
            ignoring_cursor: false,
            next_tick: Instant::now(),
            last_monitor_check: Instant::now(),
            bounds: Bounds {
                x: 0.0,
                y: 0.0,
                width: 1024.0,
                height: 768.0,
            },
            last_activity_revision: snapshot.revision,
        };
        let placement = std::fs::read(&this.placement_path)
            .ok()
            .filter(|b| b.len() < 8192)
            .and_then(|bytes| serde_json::from_slice::<Placement>(&bytes).ok())
            .filter(|p| p.x.is_finite() && p.y.is_finite());
        this.restore(placement);
        Ok(this)
    }

    fn restore(&mut self, placement: Option<Placement>) {
        let monitors: Vec<_> = self.pet.window.available_monitors().collect();
        let monitor = placement
            .as_ref()
            .and_then(|p| monitors.iter().find(|m| m.name() == p.monitor))
            .cloned()
            .or_else(|| self.pet.window.primary_monitor())
            .or_else(|| monitors.first().cloned());
        let Some(monitor) = monitor else {
            return;
        };
        let scale = monitor.scale_factor();
        let rect = work_area(&self.pet.window, &monitor);
        self.bounds = rect;
        self.dock = placement.as_ref().map(|p| p.dock).unwrap_or_default();
        self.resize_pet();
        let size = self.dock.size();
        let pos = match placement {
            Some(p) => (
                rect.x + p.x.clamp(0.0, 1.0) * (rect.width - size.0 * scale).max(0.0),
                rect.y + p.y.clamp(0.0, 1.0) * (rect.height - size.1 * scale).max(0.0),
            ),
            None => (
                rect.x + rect.width - 88.0 * scale,
                rect.y + rect.height - 88.0 * scale,
            ),
        };
        self.move_clamped(pos, rect, scale);
    }

    fn resize_pet(&self) {
        let (w, h) = self.dock.size();
        self.pet.window.set_inner_size(LogicalSize::new(w, h));
    }

    fn move_clamped(&self, mut pos: (f64, f64), rect: Bounds, scale: f64) {
        let (w, h) = self.dock.size();
        if self.dock == Dock::Left {
            pos.0 = rect.x;
        } else if self.dock == Dock::Right {
            pos.0 = rect.x + rect.width - w * scale;
        }
        let (x, y) = rect.clamp(pos, (w * scale, h * scale));
        position_window(&self.pet.window, x, y, scale);
    }

    fn save_position(&self) {
        let (Ok(pos), Some(monitor)) = (
            self.pet.window.outer_position(),
            self.pet.window.current_monitor(),
        ) else {
            return;
        };
        let scale = monitor.scale_factor();
        let rect = work_area(&self.pet.window, &monitor);
        let (w, h) = self.dock.size();
        let placement = Placement {
            monitor: monitor.name(),
            x: (f64::from(pos.x) - rect.x) / (rect.width - w * scale).max(1.0),
            y: (f64::from(pos.y) - rect.y) / (rect.height - h * scale).max(1.0),
            dock: self.dock,
        };
        if let Ok(bytes) = serde_json::to_vec(&placement)
            && let Err(err) = std::fs::write(&self.placement_path, bytes)
        {
            tracing::warn!(err=%err,"save desktop pet placement failed");
        }
    }

    fn render(&self) {
        let snapshot = activity::snapshot();
        let payload = serde_json::json!({"locale": self.locale, "dock":self.dock,"snapshot":snapshot,"celebrating":self.notification.is_some(),"notification":self.notification});
        let Ok(payload) = serde_json::to_string(&payload) else {
            return;
        };
        self.pet.render(&payload);
        self.panel.render(&payload);
        self.toast.render(&payload);
    }

    pub(super) fn set_locale(&mut self, locale: AppLocale) {
        if self.locale != locale {
            self.locale = locale;
            self.render();
        }
    }

    pub(super) fn activities_changed(&mut self) {
        self.render();
        self.last_activity_revision = activity::status_summary().0;
    }

    pub(super) fn completed(&mut self, entry: &activity::ActivityEntry) {
        let Some(key) = completion_key(entry) else {
            return;
        };
        if entry.finished_at_ms.is_none_or(|at| at < self.born_at_ms)
            || !self.seen_finished.insert(key.clone())
        {
            return;
        }
        self.finished_order.push_back(key);
        while self.finished_order.len() > 512 {
            if let Some(key) = self.finished_order.pop_front() {
                self.seen_finished.remove(&key);
            }
        }
        if !self.panel_open {
            let title = entry
                .title
                .clone()
                .unwrap_or_else(|| match &entry.thread_id {
                    Some(id) => format!("Codex · {}", id.chars().take(8).collect::<String>()),
                    None => entry.source.clone(),
                });
            let count = 1 + self.notification.as_ref().map(|n| n.count).unwrap_or(0);
            self.notification = Some(Notification { title, count });
            self.notification_until = Some(Instant::now() + TOAST_DURATION);
            self.position_auxiliary(Surface::Toast);
            self.toast.show(false);
        }
        self.render();
    }

    fn position_auxiliary(&self, surface: Surface) {
        let (Ok(pos), Some(monitor)) = (
            self.pet.window.outer_position(),
            self.pet.window.current_monitor(),
        ) else {
            return;
        };
        let scale = monitor.scale_factor();
        let rect = work_area(&self.pet.window, &monitor);
        let aux = if surface == Surface::Panel {
            &self.panel
        } else {
            &self.toast
        };
        let width = 190.0 * scale;
        let height = if surface == Surface::Panel {
            136.0 * scale
        } else {
            34.0 * scale
        };
        let (pw, ph) = self.dock.size();
        let x = f64::from(pos.x) + pw * scale / 2.0 - width / 2.0;
        let above = f64::from(pos.y) - height - 6.0 * scale;
        let y = if above >= rect.y {
            above
        } else {
            f64::from(pos.y) + (ph + 6.0) * scale
        };
        let (x, y) = rect.clamp((x, y), (width, height));
        position_window(&aux.window, x, y, scale);
    }

    fn dismiss(&mut self) {
        self.panel_open = false;
        self.panel.window.set_visible(false);
    }

    pub(super) fn command(
        &mut self,
        window_id: WindowId,
        surface: Surface,
        command: Command,
    ) -> Action {
        let expected = match surface {
            Surface::Pet => self.pet.window.id(),
            Surface::Panel => self.panel.window.id(),
            Surface::Toast => self.toast.window.id(),
        };
        if window_id != expected {
            return Action::None;
        }
        match command {
            Command::Ready => {
                match surface {
                    Surface::Pet => self.pet.ready = true,
                    Surface::Panel => self.panel.ready = true,
                    Surface::Toast => self.toast.ready = true,
                }
                self.render();
                if surface == Surface::Pet {
                    self.pet.show(false);
                }
                if surface == Surface::Panel && self.panel_open {
                    self.panel.show(true);
                }
                if surface == Surface::Toast && self.notification.is_some() {
                    self.toast.show(false);
                }
            }
            Command::ToggleList if surface == Surface::Pet => {
                if self.panel_open {
                    self.dismiss();
                } else {
                    self.notification = None;
                    self.notification_until = None;
                    self.toast.window.set_visible(false);
                    self.panel_open = true;
                    self.position_auxiliary(Surface::Panel);
                    self.render();
                    self.panel.show(true);
                }
            }
            Command::OpenActivities if surface == Surface::Panel => {
                self.dismiss();
                return Action::OpenActivities;
            }
            Command::Hide if surface != Surface::Toast => {
                self.save_position();
                return Action::Hide;
            }
            Command::Dismiss => self.dismiss(),
            Command::DragStart { x, y }
                if surface == Surface::Pet && x.is_finite() && y.is_finite() =>
            {
                self.dismiss();
                self.notification = None;
                self.notification_until = None;
                self.toast.window.set_visible(false);
                if let Ok(pos) = self.pet.window.outer_position() {
                    let scale = self.pet.window.scale_factor();
                    self.drag = Some(Drag {
                        start: (x, y),
                        origin: (f64::from(pos.x), f64::from(pos.y)),
                        scale,
                    });
                    self.dock = Dock::None;
                    self.resize_pet();
                    self.render();
                }
            }
            Command::DragMove { x, y }
                if surface == Surface::Pet && x.is_finite() && y.is_finite() =>
            {
                if let Some(drag) = &self.drag {
                    let px = drag.origin.0 + (x - drag.start.0) * drag.scale;
                    let py = drag.origin.1 + (y - drag.start.1) * drag.scale;
                    position_window(&self.pet.window, px, py, drag.scale);
                }
            }
            Command::DragEnd if surface == Surface::Pet => {
                if self.drag.take().is_some() {
                    self.snap();
                    self.save_position();
                    self.render();
                }
            }
            Command::HitRegions { rects }
                if surface == Surface::Pet
                    && rects.len() <= 512
                    && rects.iter().all(HitRect::valid) =>
            {
                self.hit_regions = rects;
            }
            _ => {}
        }
        Action::None
    }

    fn snap(&mut self) {
        let (Ok(pos), Some(monitor)) = (
            self.pet.window.outer_position(),
            self.pet.window.current_monitor(),
        ) else {
            return;
        };
        let scale = monitor.scale_factor();
        let rect = work_area(&self.pet.window, &monitor);
        let center_y = f64::from(pos.y) + 32.0 * scale;
        let monitors: Vec<_> = self
            .pet
            .window
            .available_monitors()
            .map(|m| {
                let bounds = monitor_bounds(&m);
                #[cfg(target_os = "macos")]
                {
                    let ratio = scale / m.scale_factor();
                    Bounds {
                        x: bounds.x * ratio,
                        y: bounds.y * ratio,
                        width: bounds.width * ratio,
                        height: bounds.height * ratio,
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    bounds
                }
            })
            .collect();
        let frame = monitor_bounds(&monitor);
        let left_open = !monitors.iter().any(|b| {
            b.x + b.width <= frame.x + 1.0
                && b.x + b.width >= frame.x - 1.0
                && center_y >= b.y
                && center_y < b.y + b.height
        });
        let right_open = !monitors.iter().any(|b| {
            (b.x - (frame.x + frame.width)).abs() <= 1.0
                && center_y >= b.y
                && center_y < b.y + b.height
        });
        self.dock = choose_dock(f64::from(pos.x), rect, scale, left_open, right_open);
        self.resize_pet();
        self.move_clamped((f64::from(pos.x), f64::from(pos.y)), rect, scale);
        self.bounds = rect;
    }

    pub(super) fn window_event(&mut self, id: WindowId, event: &WindowEvent<'_>) -> bool {
        if id == self.panel.window.id() {
            if matches!(
                event,
                WindowEvent::Focused(false) | WindowEvent::CloseRequested
            ) {
                self.dismiss();
            }
            return true;
        }
        if id == self.toast.window.id() {
            return true;
        }
        if id == self.pet.window.id() {
            if matches!(event, WindowEvent::ScaleFactorChanged { .. }) {
                self.resize_pet();
                self.render();
            }
            return true;
        }
        false
    }

    pub(super) fn tick(&mut self) {
        let now = Instant::now();
        if now < self.next_tick {
            return;
        }
        let (revision, running) = activity::status_summary();
        if revision != self.last_activity_revision {
            self.last_activity_revision = revision;
            self.render();
        }
        self.next_tick = now
            + if self.drag.is_some() || self.notification.is_some() || running {
                ACTIVE_TICK
            } else {
                IDLE_TICK
            };
        if self
            .notification_until
            .is_some_and(|deadline| now >= deadline)
        {
            self.notification_until = None;
            self.notification = None;
            self.toast.window.set_visible(false);
            self.render();
        }
        // Query only pointer coordinates. No keyboard capture or inspection of other apps.
        if let Some((x, y)) = cursor_in_window(&self.pet.window) {
            let ignore = self.drag.is_none() && !self.hit_regions.iter().any(|r| r.contains(x, y));
            if ignore != self.ignoring_cursor
                && self.pet.window.set_ignore_cursor_events(ignore).is_ok()
            {
                self.ignoring_cursor = ignore;
            }
        }
        if self.drag.is_none()
            && now.duration_since(self.last_monitor_check) >= Duration::from_secs(2)
        {
            self.last_monitor_check = now;
            if let Some(monitor) = self
                .pet
                .window
                .current_monitor()
                .or_else(|| self.pet.window.primary_monitor())
            {
                let rect = work_area(&self.pet.window, &monitor);
                if rect != self.bounds {
                    self.bounds = rect;
                    if let Ok(pos) = self.pet.window.outer_position() {
                        self.move_clamped(
                            (f64::from(pos.x), f64::from(pos.y)),
                            rect,
                            monitor.scale_factor(),
                        );
                    }
                    self.dismiss();
                }
            }
        }
    }
}

fn completion_key(entry: &activity::ActivityEntry) -> Option<String> {
    if entry.status != activity::ActivityStatus::Completed {
        return None;
    }
    match (&entry.thread_id, &entry.completed_turn_id) {
        (Some(thread), Some(turn)) => Some(format!("codex:{thread}:{turn}")),
        _ => Some(entry.id.clone()),
    }
}

fn cursor_in_window(window: &Window) -> Option<(f64, f64)> {
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::WindowExtMacOS;
        // NSScreen points share a coordinate space even across monitors with different DPI.
        // SAFETY: the borrowed NSWindow is live, and this function is called on the main thread.
        let frame = unsafe { &*window.ns_window().cast::<objc2_app_kit::NSWindow>() }.frame();
        let cursor = objc2_app_kit::NSEvent::mouseLocation();
        Some((
            cursor.x - frame.origin.x,
            frame.origin.y + frame.size.height - cursor.y,
        ))
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Wayland intentionally provides no global pointer position; retain the tiny hit area.
        #[cfg(target_os = "linux")]
        if std::env::var_os("WAYLAND_DISPLAY").is_some()
            && std::env::var("GDK_BACKEND").as_deref() != Ok("x11")
        {
            return None;
        }
        let cursor = window.cursor_position().ok()?;
        let origin = window.outer_position().ok()?;
        let scale = window.scale_factor();
        Some((
            (cursor.x - f64::from(origin.x)) / scale,
            (cursor.y - f64::from(origin.y)) / scale,
        ))
    }
}

fn choose_dock(x: f64, rect: Bounds, scale: f64, left_open: bool, right_open: bool) -> Dock {
    if left_open && x <= rect.x + 16.0 * scale {
        Dock::Left
    } else if right_open && x + 64.0 * scale >= rect.x + rect.width - 16.0 * scale {
        Dock::Right
    } else {
        Dock::None
    }
}

fn position_window(window: &Window, x: f64, y: f64, scale: f64) {
    #[cfg(target_os = "macos")]
    window.set_outer_position(tao::dpi::LogicalPosition::new(x / scale, y / scale));
    #[cfg(not(target_os = "macos"))]
    {
        let _ = scale;
        window.set_outer_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    }
}

fn monitor_bounds(monitor: &MonitorHandle) -> Bounds {
    let pos = monitor.position();
    let size = monitor.size();
    Bounds {
        x: f64::from(pos.x),
        y: f64::from(pos.y),
        width: f64::from(size.width),
        height: f64::from(size.height),
    }
}

fn work_area(window: &Window, monitor: &MonitorHandle) -> Bounds {
    let frame = monitor_bounds(monitor);
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::MonitorHandleExtMacOS;
        if let Some(raw) = monitor.ns_screen() {
            // tao transfers a retained NSScreen here; balance its ownership on every query.
            // SAFETY: MonitorHandleExtMacOS::ns_screen uses Retained::into_raw for this object.
            let screen =
                unsafe { objc2::rc::Retained::<objc2_app_kit::NSScreen>::from_raw(raw.cast()) };
            let Some(screen) = screen else {
                return frame;
            };
            let full = screen.frame();
            let visible = screen.visibleFrame();
            let scale = monitor.scale_factor();
            return Bounds {
                x: frame.x + (visible.origin.x - full.origin.x) * scale,
                y: frame.y
                    + (full.origin.y + full.size.height - visible.origin.y - visible.size.height)
                        * scale,
                width: visible.size.width * scale,
                height: visible.size.height * scale,
            };
        }
    }
    #[cfg(target_os = "windows")]
    {
        use tao::platform::windows::MonitorHandleExtWindows;
        use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO};
        // SAFETY: initialized buffer with the documented structure size and a live HMONITOR.
        let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if unsafe { GetMonitorInfoW(monitor.hmonitor() as _, &mut info) } != 0 {
            return Bounds {
                x: f64::from(info.rcWork.left),
                y: f64::from(info.rcWork.top),
                width: f64::from(info.rcWork.right - info.rcWork.left),
                height: f64::from(info.rcWork.bottom - info.rcWork.top),
            };
        }
    }
    #[cfg(target_os = "linux")]
    {
        use gtk::prelude::MonitorExt;
        use tao::platform::unix::MonitorHandleExtUnix;
        let area = monitor.gdk_monitor().workarea();
        let scale = monitor.scale_factor();
        if area.width() > 0 && area.height() > 0 {
            return Bounds {
                x: f64::from(area.x()) * scale,
                y: f64::from(area.y()) * scale,
                width: f64::from(area.width()) * scale,
                height: f64::from(area.height()) * scale,
            };
        }
    }
    // On platforms without a portable work-area API, keep clear of the usual system bars.
    let _ = window;
    let inset = 40.0 * monitor.scale_factor();
    Bounds {
        y: frame.y + inset,
        height: (frame.height - inset * 3.0).max(64.0),
        ..frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn responses_do_not_consume_future_turn_notifications() {
        let mut entry = activity::ActivityEntry {
            id: "codex:thread".into(),
            kind: activity::ActivityKind::ProxyRequest,
            status: activity::ActivityStatus::ResponseFinished,
            title: None,
            thread_id: Some("thread".into()),
            observed_turn_id: Some("turn-1".into()),
            completed_turn_id: None,
            source: "codex".into(),
            protocol: Some("openai".into()),
            model: None,
            project: None,
            started_at_ms: 1,
            updated_at_ms: 2,
            finished_at_ms: Some(2),
        };
        assert_eq!(completion_key(&entry), None);
        entry.status = activity::ActivityStatus::Completed;
        entry.completed_turn_id = Some("turn-1".into());
        let first = completion_key(&entry);
        entry.completed_turn_id = Some("turn-2".into());
        assert_ne!(first, completion_key(&entry));
    }
    #[test]
    fn docking_respects_scale_and_shared_edges() {
        let rect = Bounds {
            x: -1920.0,
            y: 40.0,
            width: 1920.0,
            height: 1000.0,
        };
        assert_eq!(choose_dock(-1910.0, rect, 1.0, true, true), Dock::Left);
        assert_eq!(choose_dock(-1910.0, rect, 1.0, false, true), Dock::None);
        assert_eq!(choose_dock(-70.0, rect, 1.0, true, true), Dock::Right);
        assert_eq!(choose_dock(-500.0, rect, 2.0, true, true), Dock::None);
    }
    #[test]
    fn popup_bounds_never_leave_small_monitor() {
        let rect = Bounds {
            x: 100.0,
            y: 30.0,
            width: 160.0,
            height: 100.0,
        };
        assert_eq!(rect.clamp((-50.0, 999.0), (190.0, 136.0)), (100.0, 30.0));
    }
    #[test]
    fn hit_regions_reject_invalid_ipc_coordinates() {
        let rect = HitRect {
            x: f64::NAN,
            y: 0.0,
            width: 64.0,
            height: 64.0,
        };
        assert!(!rect.valid());
        let rect = HitRect {
            x: 0.0,
            y: 0.0,
            width: 28.0,
            height: 40.0,
        };
        assert!(rect.valid());
        assert!(rect.contains(27.0, 39.0));
        assert!(!rect.contains(28.0, 40.0));
    }
}
