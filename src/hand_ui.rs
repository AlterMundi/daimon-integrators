//! Full-output transparent layer-shell surface; empty input region, no keyboard focus.
use crate::hand_feedback::{self, Frame};
use std::{
    ffi::{c_char, c_void, CString},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
type W = *mut c_void;
#[link(name = "gtk-3")]
extern "C" {
    fn gtk_init_check(a: *mut i32, b: *mut *mut *mut c_char) -> i32;
    fn gtk_window_new(kind: i32) -> W;
    fn gtk_drawing_area_new() -> W;
    fn gtk_box_new(orientation: i32, spacing: i32) -> W;
    fn gtk_box_pack_start(w: W, child: W, expand: i32, fill: i32, padding: u32);
    fn gtk_widget_set_sensitive(w: W, v: i32);
    fn gtk_button_new_with_label(s: *const c_char) -> W;
    fn gtk_container_add(w: W, child: W);
    fn gtk_widget_set_app_paintable(w: W, v: i32);
    fn gtk_widget_get_screen(w: W) -> W;
    fn gtk_widget_set_visual(w: W, v: W);
    fn gtk_widget_get_window(w: W) -> W;
    fn gtk_widget_get_allocated_width(w: W) -> i32;
    fn gtk_widget_get_allocated_height(w: W) -> i32;
    fn gtk_widget_show_all(w: W);
    fn gtk_widget_queue_draw(w: W);
    fn gtk_main();
    fn gtk_main_quit();
    fn gtk_widget_destroy(w: W);
    fn gtk_widget_get_style_context(w: W) -> W;
    fn gtk_css_provider_new() -> W;
    fn gtk_css_provider_load_from_data(w: W, s: *const c_char, len: isize, error: *mut W) -> i32;
    fn gtk_style_context_add_provider(ctx: W, provider: W, priority: u32);
}
#[link(name = "gdk-3")]
extern "C" {
    fn gdk_screen_get_rgba_visual(w: W) -> W;
    fn gdk_window_input_shape_combine_region(w: W, region: W, x: i32, y: i32);
}
#[link(name = "gtk-layer-shell")]
extern "C" {
    fn gtk_layer_is_supported() -> i32;
    fn gtk_layer_init_for_window(w: W);
    fn gtk_layer_set_layer(w: W, v: i32);
    fn gtk_layer_set_anchor(w: W, edge: i32, v: i32);
    fn gtk_layer_set_keyboard_mode(w: W, v: i32);
    fn gtk_layer_set_exclusive_zone(w: W, v: i32);
    fn gtk_layer_set_namespace(w: W, s: *const c_char);
    fn gtk_layer_set_margin(w: W, edge: i32, margin: i32);
}
#[link(name = "gobject-2.0")]
extern "C" {
    fn g_signal_connect_data(
        w: W,
        s: *const c_char,
        cb: Option<unsafe extern "C" fn()>,
        data: W,
        destroy: W,
        flags: i32,
    ) -> u64;
}
#[link(name = "glib-2.0")]
extern "C" {
    fn g_timeout_add(ms: u32, cb: Option<unsafe extern "C" fn(W) -> i32>, data: W) -> u32;
    fn g_source_remove(id: u32) -> i32;
}
#[link(name = "cairo")]
extern "C" {
    fn cairo_region_create() -> W;
    fn cairo_new_path(w: W);
    fn cairo_region_destroy(w: W);
    fn cairo_set_operator(w: W, op: i32);
    fn cairo_set_source_rgba(w: W, r: f64, g: f64, b: f64, a: f64);
    fn cairo_paint(w: W);
    fn cairo_set_line_width(w: W, v: f64);
    fn cairo_move_to(w: W, x: f64, y: f64);
    fn cairo_line_to(w: W, x: f64, y: f64);
    fn cairo_stroke(w: W);
    fn cairo_arc(w: W, x: f64, y: f64, r: f64, a: f64, b: f64);
    fn cairo_fill(w: W);
    fn cairo_rectangle(w: W, x: f64, y: f64, width: f64, height: f64);
    fn cairo_select_font_face(w: W, s: *const c_char, slant: i32, weight: i32);
    fn cairo_set_font_size(w: W, size: f64);
    fn cairo_show_text(w: W, s: *const c_char);
}
fn c(s: &str) -> CString {
    CString::new(s.replace('\0', " ")).unwrap()
}
struct Ui {
    window: W,
    area: W,
    shared: Arc<Mutex<Option<(Frame, Instant)>>>,
    painted: Arc<AtomicBool>,
    until: Instant,
    demo: bool,
    stop_requested: Arc<AtomicBool>,
    command: Arc<Mutex<Option<&'static str>>>,
    cal_buttons: Vec<(W, &'static str)>,
    actual_output: Arc<Mutex<[u32; 2]>>,
}
unsafe fn text(cr: W, x: f64, y: f64, s: &str, size: f64) {
    cairo_set_font_size(cr, size);
    cairo_move_to(cr, x, y);
    cairo_show_text(cr, c(s).as_ptr());
}
unsafe extern "C" fn draw(area: W, cr: W, data: W) -> i32 {
    let ui = &*(data as *const Ui);
    cairo_set_operator(cr, 0);
    cairo_paint(cr);
    cairo_set_operator(cr, 2);
    let width = gtk_widget_get_allocated_width(area) as f64;
    let height = gtk_widget_get_allocated_height(area) as f64;
    *ui.actual_output.lock().unwrap() = [width as u32, height as u32];
    cairo_select_font_face(cr, c("Sans").as_ptr(), 0, 1);
    let now = Instant::now();
    let snapshot = ui.shared.lock().unwrap().clone();
    let remaining = snapshot.as_ref().and_then(|(f, at)| {
        f.remaining_ms
            .map(|ms| ms.saturating_sub(now.saturating_duration_since(*at).as_millis() as u64))
    });
    let testing = snapshot
        .as_ref()
        .is_some_and(|(f, _)| f.test_phase == "testing")
        && remaining.is_some_and(|v| v > 0);
    cairo_set_source_rgba(cr, 0.035, 0.055, 0.09, 0.92);
    cairo_rectangle(cr, 24.0, 24.0, 660.0, 90.0);
    cairo_fill(cr);
    cairo_set_source_rgba(cr, 0.96, 0.76, 0.51, 1.0);
    cairo_rectangle(cr, 24.0, 24.0, 12.0, 90.0);
    cairo_fill(cr);
    if testing && !ui.demo {
        cairo_set_source_rgba(cr, 1.0, 0.3, 0.4, 1.0);
    } else {
        cairo_set_source_rgba(cr, 0.67, 0.72, 0.79, 1.0);
    }
    cairo_new_path(cr);
    cairo_arc(cr, 62.0, 58.0, 10.0, 0.0, std::f64::consts::TAU);
    cairo_fill(cr);
    cairo_set_source_rgba(cr, 0.96, 0.94, 0.90, 1.0);
    let label = if ui.demo {
        "DEMO // NO INPUT"
    } else if testing
        && snapshot.as_ref().is_some_and(|(frame, _)| {
            frame
                .calibration
                .as_ref()
                .is_some_and(|c| c.stage != "active")
        })
    {
        "CALIBRATING // NO CLICKS"
    } else if testing {
        "TESTING // NATIVE INPUT"
    } else if remaining == Some(0) {
        "TEST ENDED"
    } else {
        "PREPARING // LOCAL HANDS"
    };
    text(cr, 86.0, 69.0, label, 26.0);
    let seconds = remaining.map(|ms| ms.div_ceil(1000));
    text(
        cr,
        550.0,
        69.0,
        &seconds
            .map(|s| format!("{:02}:{:02}", s / 60, s % 60))
            .unwrap_or_else(|| "--:--".into()),
        30.0,
    );
    text(
        cr,
        48.0,
        100.0,
        "Local hand feedback // calibrated target when active",
        18.0,
    );
    if let Some((frame, received)) = snapshot {
        let fresh = frame.visible(received, now);
        let pressed = fresh
            && frame.pinch_state == "pressed"
            && frame
                .calibration
                .as_ref()
                .is_none_or(|c| c.stage == "active");
        // Reserve the existing top-right mini panel; status remains outside it.
        let x = (width - 750.0).max(24.0);
        let y = 140.0;
        cairo_set_source_rgba(cr, 0.035, 0.055, 0.09, 0.92);
        cairo_rectangle(cr, x, y, 580.0, 130.0);
        cairo_fill(cr);
        cairo_set_source_rgba(cr, 0.86, 0.70, 0.76, 1.0);
        cairo_rectangle(cr, x, y, 10.0, 130.0);
        cairo_fill(cr);
        cairo_set_source_rgba(cr, 0.53, 0.84, 0.93, 1.0);
        let phase = if !fresh {
            "TRACKING STALE / UNKNOWN"
        } else if pressed {
            "PINCH PRESSED"
        } else {
            &frame.pinch_state
        };
        text(cr, x + 24.0, y + 36.0, phase, 26.0);
        text(
            cr,
            x + 24.0,
            y + 71.0,
            &format!("Reason: {}", frame.blocked_reason),
            20.0,
        );
        text(
            cr,
            x + 24.0,
            y + 105.0,
            &format!(
                "Press intents {} // frame {:.0} ms",
                frame.presses,
                frame.age_ms + now.saturating_duration_since(received).as_secs_f64() * 1000.0
            ),
            20.0,
        );
        if let Some(cal) = &frame.calibration {
            cairo_set_source_rgba(cr, 0.035, 0.055, 0.09, 0.96);
            cairo_rectangle(cr, 24.0, 140.0, 1100.0, 80.0);
            cairo_fill(cr);
            cairo_set_source_rgba(cr, 0.96, 0.76, 0.51, 1.0);
            let guide = if cal.output != [width as u32, height as u32] {
                "DISPLAY SIZE MISMATCH // STOP INPUT"
            } else {
                match cal.stage.as_str() {
                    "left" => "Move to comfortable LEFT; hold, then NEXT",
                    "right" => "Move to comfortable RIGHT; hold, then NEXT",
                    "top" => "Move to comfortable TOP; hold, then NEXT",
                    "bottom" => "Move to comfortable BOTTOM; hold, then NEXT",
                    "review" if cal.corners_verified => {
                        "Corners verified // choose ACCEPT or CANCEL"
                    }
                    "review" => "Hold the highlighted corner // clicks are disabled",
                    "active" => "PROFILE ACTIVE // same target drives cursor and hand",
                    _ => "Choose CALIBRATE // input stays neutral",
                }
            };
            text(cr, 44.0, 172.0, guide, 24.0);
            text(
                cr,
                44.0,
                203.0,
                &format!(
                    "Samples {} // {}",
                    cal.samples,
                    cal.message.chars().take(65).collect::<String>()
                ),
                19.0,
            );
            if let Some(p) = cal.corner_target {
                cairo_set_source_rgba(cr, 0.96, 0.76, 0.51, 1.0);
                cairo_set_line_width(cr, 12.0);
                cairo_new_path(cr);
                cairo_arc(
                    cr,
                    p[0] * width,
                    p[1] * height,
                    48.0,
                    0.0,
                    std::f64::consts::TAU,
                );
                cairo_stroke(cr);
            }
        }
        if frame.renderable(received, now) && remaining != Some(0) {
            if let Some(points) = &frame.landmarks {
                let center = [
                    (points[5][0] + points[17][0]) / 2.0,
                    (points[5][1] + points[17][1]) / 2.0,
                ];
                let anchor = frame
                    .screen_target
                    .filter(|_| {
                        frame
                            .calibration
                            .as_ref()
                            .is_none_or(|c| matches!(c.stage.as_str(), "active" | "review"))
                    })
                    .map(|p| [p[0] * width, p[1] * height])
                    .unwrap_or_else(|| frame.screen_point(center, width, height));
                if pressed {
                    cairo_set_source_rgba(cr, 0.66, 0.86, 0.78, 1.0);
                } else if fresh {
                    cairo_set_source_rgba(cr, 0.96, 0.76, 0.51, 1.0);
                } else {
                    cairo_set_source_rgba(cr, 0.96, 0.76, 0.51, 0.45);
                }
                cairo_set_line_width(cr, 3.0);
                cairo_new_path(cr);
                cairo_arc(cr, anchor[0], anchor[1], 14.0, 0.0, std::f64::consts::TAU);
                cairo_stroke(cr);
                // Camera-pixel bone lengths avoid an oversized full-screen hand.
                let points: Vec<_> = points
                    .iter()
                    .map(|p| [anchor[0] - (p[0] - center[0]), anchor[1] + p[1] - center[1]])
                    .collect();
                if fresh {
                    cairo_set_source_rgba(cr, 0.53, 0.84, 0.93, 0.9);
                } else {
                    cairo_set_source_rgba(cr, 0.96, 0.76, 0.51, 0.45);
                }
                cairo_set_line_width(cr, 4.0);
                for (a, b) in [
                    (0, 1),
                    (1, 2),
                    (2, 3),
                    (3, 4),
                    (0, 5),
                    (5, 6),
                    (6, 7),
                    (7, 8),
                    (5, 9),
                    (9, 10),
                    (10, 11),
                    (11, 12),
                    (9, 13),
                    (13, 14),
                    (14, 15),
                    (15, 16),
                    (13, 17),
                    (0, 17),
                    (17, 18),
                    (18, 19),
                    (19, 20),
                ] {
                    cairo_move_to(cr, points[a][0], points[a][1]);
                    cairo_line_to(cr, points[b][0], points[b][1]);
                    cairo_stroke(cr);
                }
                for p in &points {
                    cairo_new_path(cr);
                    cairo_arc(cr, p[0], p[1], 5.0, 0.0, std::f64::consts::TAU);
                    cairo_fill(cr);
                }
                if pressed {
                    cairo_set_source_rgba(cr, 0.66, 0.86, 0.78, 1.0);
                } else {
                    cairo_set_source_rgba(cr, 0.86, 0.70, 0.76, 1.0);
                }
                cairo_set_line_width(cr, 7.0);
                cairo_move_to(cr, points[4][0], points[4][1]);
                cairo_line_to(cr, points[8][0], points[8][1]);
                cairo_stroke(cr);
                if pressed {
                    text(cr, points[8][0] + 16.0, points[8][1] - 16.0, "PINCH", 24.0);
                }
            }
        }
        ui.painted.store(true, Ordering::SeqCst);
    }
    1
}
unsafe extern "C" fn tick(data: W) -> i32 {
    let ui = &*(data as *const Ui);
    // Reassert the empty input region after allocation/map changes.
    let region = cairo_region_create();
    gdk_window_input_shape_combine_region(gtk_widget_get_window(ui.window), region, 0, 0);
    cairo_region_destroy(region);
    if let Some((frame, at)) = ui.shared.lock().unwrap().as_ref() {
        for (button, action) in &ui.cal_buttons {
            let enabled = frame.calibration.as_ref().is_some_and(|c| {
                let recent = Instant::now().saturating_duration_since(*at) < Duration::from_secs(2);
                recent
                    && frame.remaining_ms.is_none_or(|n| {
                        n > Instant::now().saturating_duration_since(*at).as_millis() as u64
                    })
                    && match *action {
                        "next" => {
                            matches!(c.stage.as_str(), "left" | "right" | "top" | "bottom")
                                && c.samples >= 10
                        }
                        "accept" => c.corners_verified,
                        "cancel" => !matches!(c.stage.as_str(), "active" | "uncalibrated"),
                        _ => true,
                    }
            });
            gtk_widget_set_sensitive(*button, i32::from(enabled));
        }
    }
    gtk_widget_queue_draw(ui.area);
    if Instant::now() >= ui.until {
        gtk_main_quit();
        return 1;
    }
    1
}
unsafe extern "C" fn realize(window: W, _data: W) {
    let region = cairo_region_create();
    gdk_window_input_shape_combine_region(gtk_widget_get_window(window), region, 0, 0);
    cairo_region_destroy(region);
}
unsafe extern "C" fn request_stop(_button: W, data: W) {
    let ui = &*(data as *const Ui);
    ui.stop_requested.store(true, Ordering::SeqCst);
}
macro_rules! calibration_button {
    ($name:ident,$path:literal) => {
        unsafe extern "C" fn $name(_button: W, data: W) {
            let ui = &*(data as *const Ui);
            let mut command = ui.command.lock().unwrap();
            if command.is_none() {
                *command = Some($path);
            }
        }
    };
}
calibration_button!(start_calibration, "/calibration/start");
calibration_button!(next_calibration, "/calibration/next");
calibration_button!(cancel_calibration, "/calibration/cancel");
calibration_button!(accept_calibration, "/calibration/accept");
calibration_button!(rollback_calibration, "/calibration/rollback");
pub fn run(port: u16, seconds: u32, demo: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !(1..=120).contains(&seconds) || (!demo && port == 0) {
        return Err("hand overlay requires a port and 1..120 seconds".into());
    }
    unsafe {
        if gtk_init_check(ptr::null_mut(), ptr::null_mut()) == 0 || gtk_layer_is_supported() == 0 {
            return Err("hand overlay requires GTK Wayland layer-shell".into());
        }
        let window = gtk_window_new(0);
        gtk_widget_set_app_paintable(window, 1);
        let visual = gdk_screen_get_rgba_visual(gtk_widget_get_screen(window));
        if visual.is_null() {
            return Err("transparent visual unavailable".into());
        }
        gtk_widget_set_visual(window, visual);
        gtk_layer_init_for_window(window);
        gtk_layer_set_namespace(window, c("wigglecontrol-hands").as_ptr());
        gtk_layer_set_layer(window, 3);
        for edge in 0..4 {
            gtk_layer_set_anchor(window, edge, 1);
        }
        gtk_layer_set_keyboard_mode(window, 0);
        gtk_layer_set_exclusive_zone(window, -1);
        let css = gtk_css_provider_new();
        gtk_css_provider_load_from_data(css,c("window { background-color: transparent; background-image: none; border: none; box-shadow: none; }").as_ptr(),-1,ptr::null_mut());
        gtk_style_context_add_provider(gtk_widget_get_style_context(window), css, 800);
        g_signal_connect_data(
            window,
            c("realize").as_ptr(),
            Some(std::mem::transmute::<
                unsafe extern "C" fn(W, W),
                unsafe extern "C" fn(),
            >(realize)),
            ptr::null_mut(),
            ptr::null_mut(),
            0,
        );
        let area = gtk_drawing_area_new();
        gtk_container_add(window, area);
        let shared = Arc::new(Mutex::new(None));
        let painted = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let stop_requested = Arc::new(AtomicBool::new(false));
        let until = Instant::now() + Duration::from_secs(seconds as u64);
        let command = Arc::new(Mutex::new(None));
        let actual_output = Arc::new(Mutex::new([0, 0]));
        let data = Box::into_raw(Box::new(Ui {
            window,
            area,
            shared: shared.clone(),
            painted: painted.clone(),
            until,
            demo,
            stop_requested: stop_requested.clone(),
            command: command.clone(),
            cal_buttons: Vec::new(),
            actual_output: actual_output.clone(),
        }));
        g_signal_connect_data(
            area,
            c("draw").as_ptr(),
            Some(std::mem::transmute::<
                unsafe extern "C" fn(W, W, W) -> i32,
                unsafe extern "C" fn(),
            >(draw)),
            data as W,
            ptr::null_mut(),
            0,
        );
        let control = gtk_window_new(0);
        gtk_layer_init_for_window(control);
        gtk_layer_set_namespace(control, c("wigglecontrol-stop").as_ptr());
        gtk_layer_set_layer(control, 3);
        gtk_layer_set_anchor(control, 0, 1);
        gtk_layer_set_anchor(control, 2, 1);
        gtk_layer_set_margin(control, 0, 704);
        gtk_layer_set_margin(control, 2, 24);
        gtk_layer_set_keyboard_mode(control, 0);
        gtk_layer_set_exclusive_zone(control, -1);
        let control_css = gtk_css_provider_new();
        gtk_css_provider_load_from_data(control_css, c("button { background: #f4c283; color: #090e17; border: none; border-radius: 20px; font: bold 24px Sans; padding: 24px; }").as_ptr(), -1, ptr::null_mut());
        let button = gtk_button_new_with_label(c("STOP INPUT").as_ptr());
        gtk_style_context_add_provider(gtk_widget_get_style_context(button), control_css, 800);
        let control_box = gtk_box_new(0, 6);
        gtk_container_add(control, control_box);
        gtk_box_pack_start(control_box, button, 0, 0, 0);
        for (label, action, callback) in [
            (
                "CALIBRATE",
                "start",
                start_calibration as unsafe extern "C" fn(W, W),
            ),
            ("NEXT", "next", next_calibration),
            ("CANCEL", "cancel", cancel_calibration),
            ("ACCEPT", "accept", accept_calibration),
            ("ROLLBACK", "rollback", rollback_calibration),
        ] {
            let b = gtk_button_new_with_label(c(label).as_ptr());
            gtk_style_context_add_provider(gtk_widget_get_style_context(b), control_css, 800);
            gtk_box_pack_start(control_box, b, 0, 0, 0);
            gtk_widget_set_sensitive(b, 0);
            g_signal_connect_data(
                b,
                c("clicked").as_ptr(),
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(W, W),
                    unsafe extern "C" fn(),
                >(callback)),
                data as W,
                ptr::null_mut(),
                0,
            );
            (*data).cal_buttons.push((b, action));
        }
        g_signal_connect_data(
            button,
            c("clicked").as_ptr(),
            Some(std::mem::transmute::<
                unsafe extern "C" fn(W, W),
                unsafe extern "C" fn(),
            >(request_stop)),
            data as W,
            ptr::null_mut(),
            0,
        );
        gtk_widget_show_all(window);
        gtk_widget_show_all(control);
        let worker_stop = stop.clone();
        let worker = thread::spawn(move || {
            let mut acknowledged = false;
            let mut stop_accepted = false;
            while Instant::now() < until && !worker_stop.load(Ordering::SeqCst) {
                let pending = command.lock().unwrap().take();
                if let Some(path) = pending {
                    let _ = hand_feedback::request(port, "POST", path);
                }
                if stop_requested.load(Ordering::SeqCst) && !stop_accepted {
                    stop_accepted = demo || hand_feedback::request(port, "POST", "/stop").is_ok();
                }
                if stop_accepted {
                    if let Some((frame, at)) = shared.lock().unwrap().as_mut() {
                        frame.remaining_ms = Some(0);
                        frame.landmarks = None;
                        *at = Instant::now();
                    }
                    thread::sleep(Duration::from_millis(30));
                    continue;
                }
                let frame = if demo {
                    let points = vec![
                        [320.0, 370.0],
                        [290.0, 340.0],
                        [265.0, 310.0],
                        [240.0, 280.0],
                        [220.0, 250.0],
                        [290.0, 280.0],
                        [280.0, 230.0],
                        [280.0, 190.0],
                        [280.0, 150.0],
                        [320.0, 270.0],
                        [320.0, 210.0],
                        [320.0, 165.0],
                        [320.0, 120.0],
                        [350.0, 280.0],
                        [360.0, 225.0],
                        [365.0, 190.0],
                        [370.0, 150.0],
                        [375.0, 300.0],
                        [395.0, 265.0],
                        [410.0, 240.0],
                        [420.0, 210.0],
                    ];
                    Some(Frame {
                        sequence: 0,
                        screen_target: None,
                        calibration: None,
                        width: 640.0,
                        height: 480.0,
                        landmarks: Some(points),
                        fresh: true,
                        age_ms: 0.0,
                        pinch_state: "open".into(),
                        blocked_reason: "fictional demo".into(),
                        presses: 0,
                        test_phase: "testing".into(),
                        remaining_ms: Some(
                            until.saturating_duration_since(Instant::now()).as_millis() as u64,
                        ),
                    })
                } else {
                    hand_feedback::request(port, "GET", "/state")
                        .and_then(|bytes| Frame::parse(&bytes))
                        .ok()
                };
                if let Some(mut frame) = frame {
                    let received = Instant::now();
                    let mut slot = shared.lock().unwrap();
                    if !demo {
                        if let Some((old, at)) = slot.as_ref() {
                            if old.sequence == frame.sequence {
                                frame.age_ms = frame.age_ms.max(
                                    old.age_ms
                                        + received.saturating_duration_since(*at).as_secs_f64()
                                            * 1000.0,
                                );
                            }
                        }
                    }
                    *slot = Some((frame, received));
                    drop(slot);
                    if !demo && !acknowledged && painted.load(Ordering::SeqCst) {
                        let dimensions_ok =
                            shared.lock().unwrap().as_ref().is_some_and(|(frame, _)| {
                                frame
                                    .calibration
                                    .as_ref()
                                    .is_none_or(|c| c.output == *actual_output.lock().unwrap())
                            });
                        if dimensions_ok {
                            acknowledged = hand_feedback::request(port, "POST", "/ready").is_ok();
                        }
                    }
                }
                thread::sleep(Duration::from_millis(30));
            }
        });
        // Bound presentation scheduling to 30 Hz; inference remains independently paced.
        let timer = g_timeout_add(33, Some(tick), data as W);
        gtk_main();
        g_source_remove(timer);
        stop.store(true, Ordering::SeqCst);
        let _ = worker.join();
        gtk_widget_destroy(control);
        gtk_widget_destroy(window);
        drop(Box::from_raw(data));
    }
    Ok(())
}
