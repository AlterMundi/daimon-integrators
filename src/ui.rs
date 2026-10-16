//! Narrow GTK3 / layer-shell C ABI surface. System libraries own widgets; no web runtime.
use crate::*;
use std::{
    ffi::{c_char, c_void, CString},
    ptr,
};
type W = *mut c_void;
#[link(name = "gtk-3")]
extern "C" {
    fn gtk_init_check(argc: *mut i32, argv: *mut *mut *mut c_char) -> i32;
    fn gtk_window_new(kind: i32) -> W;
    fn gtk_window_set_title(w: W, s: *const c_char);
    fn gtk_window_set_default_size(w: W, width: i32, height: i32);
    fn gtk_window_set_resizable(w: W, v: i32);
    fn gtk_container_add(w: W, child: W);
    fn gtk_container_set_border_width(w: W, v: u32);
    fn gtk_box_new(orientation: i32, spacing: i32) -> W;
    fn gtk_box_pack_start(w: W, child: W, expand: i32, fill: i32, padding: u32);
    fn gtk_label_new(s: *const c_char) -> W;
    fn gtk_label_set_text(w: W, s: *const c_char);
    fn gtk_label_set_xalign(w: W, x: f32);
    fn gtk_label_set_line_wrap(w: W, v: i32);
    fn gtk_button_new_with_label(s: *const c_char) -> W;
    fn gtk_widget_set_sensitive(w: W, v: i32);
    fn gtk_widget_set_visible(w: W, v: i32);
    fn gtk_widget_set_tooltip_text(w: W, s: *const c_char);
    fn gtk_window_resize(w: W, width: i32, height: i32);
    fn gtk_image_new_from_icon_name(s: *const c_char, size: i32) -> W;
    fn gtk_image_set_pixel_size(w: W, size: i32);
    fn gtk_widget_show_all(w: W);
    fn gtk_widget_get_style_context(w: W) -> W;
    fn gtk_css_provider_new() -> W;
    fn gtk_css_provider_load_from_data(w: W, s: *const c_char, len: isize, error: *mut W) -> i32;
    fn gtk_style_context_add_provider_for_screen(screen: W, provider: W, priority: u32);
    fn gtk_style_context_add_class(ctx: W, s: *const c_char);
    fn gtk_style_context_remove_class(ctx: W, s: *const c_char);
    fn gtk_main();
}
#[link(name = "gdk-3")]
extern "C" {
    fn gdk_screen_get_default() -> W;
}
#[link(name = "gtk-layer-shell")]
extern "C" {
    fn gtk_layer_is_supported() -> i32;
    fn gtk_layer_init_for_window(w: W);
    fn gtk_layer_set_layer(w: W, layer: i32);
    fn gtk_layer_set_anchor(w: W, edge: i32, v: i32);
    fn gtk_layer_set_margin(w: W, edge: i32, margin: i32);
    fn gtk_layer_set_keyboard_mode(w: W, mode: i32);
    fn gtk_layer_set_exclusive_zone(w: W, zone: i32);
    fn gtk_layer_set_namespace(w: W, s: *const c_char);
}
#[link(name = "gobject-2.0")]
extern "C" {
    fn g_signal_connect_data(
        instance: W,
        signal: *const c_char,
        callback: Option<unsafe extern "C" fn()>,
        data: W,
        destroy: W,
        flags: i32,
    ) -> u64;
}
#[link(name = "glib-2.0")]
extern "C" {
    fn g_timeout_add(
        interval: u32,
        callback: Option<unsafe extern "C" fn(W) -> i32>,
        data: W,
    ) -> u32;
}
fn c(s: &str) -> CString {
    CString::new(s.replace('\0', " ")).unwrap()
}
struct Ui {
    engine: Arc<Mutex<Engine>>,
    ready: Arc<AtomicBool>,
    mic: W,
    cam: W,
    managed: W,
    remote: W,
    button: W,
    last: String,
    window: W,
    detail: W,
    compact_box: W,
    compact: bool,
    small_mic: W,
    small_cam: W,
    small_rec: W,
    small_remote: W,
    small_stop: W,
    system: W,
    small_cpu: W,
    small_temperature: W,
    small_power: W,
}
unsafe fn label(s: &str, box_: W) -> W {
    let w = gtk_label_new(c(s).as_ptr());
    gtk_label_set_xalign(w, 0.0);
    gtk_label_set_line_wrap(w, 1);
    gtk_box_pack_start(box_, w, 0, 0, 0);
    w
}
unsafe fn set(w: W, s: &str, active: bool, unknown: bool) {
    gtk_label_set_text(w, c(s).as_ptr());
    let ctx = gtk_widget_get_style_context(w);
    for class in ["active", "unknown"] {
        gtk_style_context_remove_class(ctx, c(class).as_ptr());
    }
    if active {
        gtk_style_context_add_class(ctx, c("active").as_ptr());
    } else if unknown {
        gtk_style_context_add_class(ctx, c("unknown").as_ptr());
    }
}
unsafe extern "C" fn tick(data: W) -> i32 {
    let u = &mut *(data as *mut Ui);
    let s = u.engine.lock().unwrap().snapshot.clone();
    if s.compact != u.compact {
        apply_view(u, s.compact);
    }
    let o = &s.observation;
    let fresh = observation_fresh(o.at, now());
    let mic_active = fresh && (o.microphone_running || s.pipewire_microphone);
    let cam_active = fresh && (o.camera_open || s.pipewire_camera);
    let mic = sensor_state(
        o.microphone_present,
        mic_active,
        o.complete && s.pipewire_ok,
        o.at,
        now(),
    );
    let cam = sensor_state(
        o.camera_present,
        cam_active,
        o.complete && s.pipewire_ok,
        o.at,
        now(),
    );
    let m = &s.managed;
    let managed = if m.activity == "input" {
        match m.phase.as_str() {
            "preparing" => "Preparando control por gestos · inicio pendiente".into(),
            "capturing" => format!(
                "Control por gestos · {} · {} s · sin grabación",
                m.mode.as_deref().unwrap_or("unknown"),
                now().saturating_sub(m.started_at)
            ),
            "stopping" => "Deteniendo control por gestos".into(),
            "completed" => "Sesión de gestos finalizada".into(),
            "stopped" => "Control por gestos detenido".into(),
            "failed" => "Falló el control por gestos · consultar registro local".into(),
            _ => "Sin sesión de gestos".into(),
        }
    } else {
        match m.phase.as_str() {
            "preparing" => format!(
                "Preparando {} · aún sin captura",
                m.mode.as_deref().unwrap_or("")
            ),
            "capturing" | "stopping" => format!(
                "{} {} · {} s",
                if m.phase == "stopping" {
                    "Deteniendo"
                } else {
                    "Grabación gestionada"
                },
                m.mode.as_deref().unwrap_or(""),
                now().saturating_sub(m.started_at)
            ),
            "completed" => "Grabación finalizada".into(),
            "stopped" => "Captura detenida · proceso finalizado".into(),
            "failed" => "Falló la captura · consultar registro local".into(),
            "cancelled_before_capture" => "Captura cancelada antes de comenzar".into(),
            _ => "Sin grabación gestionada".into(),
        }
    };
    let remote = s
        .remote_activity
        .as_ref()
        .map(|v| {
            format!(
                "CompAII · {}",
                v["summary"].as_str().unwrap_or("estado recibido")
            )
        })
        .unwrap_or_else(|| "CompAII · sin feed remoto reciente".into());
    let local_fresh = observation_fresh(s.system.at, now());
    let cpu = if local_fresh {
        s.system.cpu_percent.map(|v| format!("{v:.0}%"))
    } else {
        None
    }
    .unwrap_or_else(|| "?".into());
    let temperature = if local_fresh {
        s.system.temperature_c.map(|v| format!("{v:.0}°"))
    } else {
        None
    }
    .unwrap_or_else(|| "?".into());
    let power = if local_fresh {
        s.system.power_mode.as_str()
    } else {
        "unknown"
    };
    let (short, mode) = match power {
        "silent" => ("Zz", "Silencioso · ventilador apagado"),
        "quiet" => ("ECO", "Silencioso · ventilador automático reducido"),
        "auto" => ("AUTO", "Automático"),
        "fan_off" => ("OFF", "Ventilador apagado · CPU sin perfil silencioso"),
        "custom" => ("CUSTOM", "Configuración personalizada"),
        _ => ("?", "Modo desconocido"),
    };
    let rpm = s
        .system
        .fan_rpm
        .map(|v| format!("{v} rpm"))
        .unwrap_or_else(|| "rpm desconocidas".into());
    let summary = format!("CPU {cpu} · {temperature}C · {mode} · {rpm}");
    let text = format!("{mic}|{cam}|{managed}|{remote}|{summary}");
    if text != u.last {
        set(
            u.mic,
            &format!("MICRÓFONO   {mic}"),
            mic_active,
            mic.starts_with("Desconocido"),
        );
        set(
            u.cam,
            &format!("CÁMARA   {cam}"),
            cam_active,
            cam.starts_with("Desconocido"),
        );
        set(
            u.managed,
            &managed,
            m.phase == "capturing",
            m.phase == "failed",
        );
        set(u.remote, &remote, false, s.remote_activity.is_none());
        gtk_widget_set_sensitive(
            u.button,
            i32::from(["preparing", "capturing", "stopping"].contains(&m.phase.as_str())),
        );
        let marker = |active: bool, state: &str| {
            if active {
                "●"
            } else if state.starts_with("Desconocido") {
                "?"
            } else if state == "Desconectado" {
                "×"
            } else {
                "○"
            }
        };
        set(
            u.small_mic,
            marker(mic_active, mic),
            mic_active,
            mic.starts_with("Desconocido"),
        );
        set(
            u.small_cam,
            marker(cam_active, cam),
            cam_active,
            cam.starts_with("Desconocido"),
        );
        set(
            u.small_rec,
            if m.phase == "preparing" {
                "…"
            } else if ["capturing", "stopping"].contains(&m.phase.as_str()) {
                if m.activity == "input" {
                    "↔"
                } else {
                    "●"
                }
            } else if m.phase == "failed" {
                "!"
            } else {
                "—"
            },
            m.phase == "capturing",
            m.phase == "failed",
        );
        let count = s
            .remote_activity
            .as_ref()
            .and_then(|v| v["active"].as_u64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| "?".into());
        set(u.small_remote, &count, false, s.remote_activity.is_none());
        gtk_widget_set_tooltip_text(u.small_mic, c(&format!("Micrófono: {mic}")).as_ptr());
        gtk_widget_set_tooltip_text(u.small_cam, c(&format!("Cámara: {cam}")).as_ptr());
        gtk_widget_set_tooltip_text(u.small_rec, c(&managed).as_ptr());
        gtk_widget_set_tooltip_text(u.small_remote, c(&remote).as_ptr());
        gtk_widget_set_sensitive(
            u.small_stop,
            i32::from(["preparing", "capturing", "stopping"].contains(&m.phase.as_str())),
        );
        set(
            u.system,
            &summary,
            power == "silent",
            !local_fresh || power == "unknown",
        );
        set(u.small_cpu, &cpu, false, cpu == "?");
        set(
            u.small_temperature,
            &temperature,
            s.system.temperature_c.is_some_and(|v| v >= 80.0),
            temperature == "?",
        );
        set(
            u.small_power,
            short,
            power == "silent" || power == "quiet",
            power == "unknown",
        );
        gtk_widget_set_tooltip_text(
            u.small_cpu,
            c("Uso total de CPU · 0–100% entre todos los núcleos").as_ptr(),
        );
        gtk_widget_set_tooltip_text(
            u.small_temperature,
            c(&format!("Temperatura local · {rpm}")).as_ptr(),
        );
        gtk_widget_set_tooltip_text(
            u.small_power,
            c(&format!(
                "{mode} · límite CPU {} MHz · {rpm}",
                s.system
                    .cpu_max_mhz
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into())
            ))
            .as_ptr(),
        );
        u.last = text;
    }
    if m.phase == "preparing" {
        u.ready.store(true, Ordering::SeqCst);
    }
    1
}
unsafe extern "C" fn stop(_button: W, data: W) {
    let u = &*(data as *mut Ui);
    u.engine.lock().unwrap().request(Request::Stop);
}
fn beta_helper() -> PathBuf {
    PathBuf::from(env::var("HOME").unwrap_or_default()).join(".local/bin/wigglecontrol-beta1")
}
fn launch_beta(action: &'static str) {
    let helper = beta_helper();
    if !helper.is_file() {
        return;
    }
    // Explicit human click only. Reap the helper off the GTK thread.
    std::thread::spawn(move || match Command::new(helper).arg(action).status() {
        Ok(status) if status.success() => (),
        outcome => eprintln!("WiggleControl Beta1 {action} failed: {outcome:?}"),
    });
}
unsafe extern "C" fn open_beta(_button: W, _data: W) {
    launch_beta("open");
}
unsafe extern "C" fn stop_beta(_button: W, _data: W) {
    launch_beta("stop");
}
unsafe fn apply_view(u: &mut Ui, compact: bool) {
    u.compact = compact;
    u.engine.lock().unwrap().snapshot.ui_compact = Some(compact);
    gtk_widget_set_visible(u.detail, i32::from(!compact));
    gtk_widget_set_visible(u.compact_box, i32::from(compact));
    gtk_window_set_default_size(u.window, if compact { 112 } else { 1080 }, 0);
    gtk_window_resize(u.window, if compact { 112 } else { 1080 }, 1);
    gtk_layer_set_margin(u.window, 1, if compact { 0 } else { 24 });
    let path =
        PathBuf::from(env::var("HOME").unwrap_or_default()).join(".config/daimon-integrators");
    if fs::create_dir_all(&path).is_ok() {
        let _ = atomic_json(&path.join("panel.json"), &json!({"compact":compact}));
    }
}
unsafe extern "C" fn toggle(_button: W, data: W) {
    let u = &*(data as *mut Ui);
    let mut e = u.engine.lock().unwrap();
    e.snapshot.compact = !e.snapshot.compact;
}
unsafe fn badge(box_: W, icon: &str, name: &str) -> W {
    let group = gtk_box_new(1, 3);
    gtk_box_pack_start(box_, group, 0, 0, 0);
    if !icon.is_empty() {
        let image = gtk_image_new_from_icon_name(c(icon).as_ptr(), 6);
        gtk_image_set_pixel_size(image, 24);
        gtk_box_pack_start(group, image, 0, 0, 0);
    }
    let title = label(name, group);
    gtk_style_context_add_class(
        gtk_widget_get_style_context(title),
        c("badge-title").as_ptr(),
    );
    let value = label("?", group);
    gtk_label_set_xalign(value, 0.5);
    gtk_style_context_add_class(
        gtk_widget_get_style_context(value),
        c("badge-value").as_ptr(),
    );
    value
}
pub fn run(
    engine: Arc<Mutex<Engine>>,
    ready: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        if gtk_init_check(ptr::null_mut(), ptr::null_mut()) == 0 {
            return Err("GTK could not connect to desktop".into());
        }
        if gtk_layer_is_supported() == 0 {
            return Err(
                "Wayland layer-shell is required; refusing an ordinary hidden window".into(),
            );
        }
        let window = gtk_window_new(0);
        gtk_window_set_title(window, c("CompAII · Dispositivos").as_ptr());
        gtk_window_set_default_size(window, 1080, 0);
        gtk_window_set_resizable(window, 0);
        gtk_layer_init_for_window(window);
        gtk_layer_set_namespace(window, c("daimon-integrators").as_ptr());
        gtk_layer_set_layer(window, 3);
        // Layer-shell edge enumeration: left=0, right=1, top=2, bottom=3.
        gtk_layer_set_anchor(window, 1, 1);
        gtk_layer_set_anchor(window, 2, 1);
        gtk_layer_set_margin(window, 1, 24);
        gtk_layer_set_margin(window, 2, 24);
        gtk_layer_set_keyboard_mode(window, 0);
        gtk_layer_set_exclusive_zone(window, 0);
        let outer = gtk_box_new(1, 0);
        gtk_container_add(window, outer);
        let compact_box = gtk_box_new(1, 6);
        gtk_style_context_add_class(
            gtk_widget_get_style_context(compact_box),
            c("compact").as_ptr(),
        );
        gtk_container_set_border_width(compact_box, 6);
        gtk_box_pack_start(outer, compact_box, 0, 0, 0);
        let expand = gtk_button_new_with_label(c("◀").as_ptr());
        gtk_box_pack_start(compact_box, expand, 0, 0, 0);
        gtk_widget_set_tooltip_text(expand, c("Expandir panel").as_ptr());
        let small_mic = badge(compact_box, "audio-input-microphone-symbolic", "MIC");
        let small_cam = badge(compact_box, "camera-video-symbolic", "CAM");
        let small_rec = badge(compact_box, "applications-system-symbolic", "LOCAL");
        let small_remote = badge(compact_box, "", "AII");
        let small_cpu = badge(compact_box, "", "CPU");
        let small_temperature = badge(compact_box, "", "TEMP");
        let small_power = badge(compact_box, "", "MODE");
        let small_beta = gtk_button_new_with_label(c("WG1").as_ptr());
        gtk_box_pack_start(compact_box, small_beta, 0, 0, 0);
        gtk_widget_set_tooltip_text(
            small_beta,
            c("Open one-hand Beta1; camera stays off until Start").as_ptr(),
        );
        let small_beta_stop = gtk_button_new_with_label(c("■ WG1").as_ptr());
        gtk_box_pack_start(compact_box, small_beta_stop, 0, 0, 0);
        gtk_widget_set_tooltip_text(small_beta_stop, c("Stop only WiggleControl Beta1").as_ptr());
        let small_stop = gtk_button_new_with_label(c("■").as_ptr());
        gtk_box_pack_start(compact_box, small_stop, 0, 0, 0);
        gtk_widget_set_tooltip_text(small_stop, c("Detener sólo mi captura gestionada").as_ptr());
        let box_ = gtk_box_new(1, 14);
        gtk_container_set_border_width(box_, 24);
        gtk_box_pack_start(outer, box_, 0, 0, 0);
        let collapse = gtk_button_new_with_label(c("▶  Modo mínimo").as_ptr());
        gtk_box_pack_start(box_, collapse, 0, 0, 0);
        let title = label("COMPAII // LOCAL SYSTEMS", box_);
        gtk_style_context_add_class(gtk_widget_get_style_context(title), c("title").as_ptr());
        let mic = label("MICRÓFONO   Desconocido", box_);
        let cam = label("CÁMARA   Desconocido", box_);
        let managed = label("Sin grabación gestionada", box_);
        let remote = label("CompAII · sin feed remoto reciente", box_);
        let system = label("CPU ? · temperatura ? · modo desconocido", box_);
        let beta = gtk_button_new_with_label(c("Open WiggleControl Beta1").as_ptr());
        gtk_box_pack_start(box_, beta, 0, 0, 0);
        let beta_stop = gtk_button_new_with_label(c("Stop WiggleControl Beta1").as_ptr());
        gtk_box_pack_start(box_, beta_stop, 0, 0, 0);
        let button = gtk_button_new_with_label(c("Detener mi captura").as_ptr());
        gtk_box_pack_start(box_, button, 0, 0, 0);
        gtk_widget_set_sensitive(button, 0);
        let footer = label("Uso observado ≠ grabación · estado local cada 1 s", box_);
        gtk_style_context_add_class(gtk_widget_get_style_context(footer), c("footer").as_ptr());
        let provider = gtk_css_provider_new();
        let css=c("window { background: #090e17; color: #f5f0e6; border-left: 14px solid #dcb3c3; border-top: 8px solid #f4c283; border-radius: 28px 0 0 28px; } label { font-family: Space Grotesk, Sans; font-size: 32px; color: #f5f0e6; } .title { font-size: 36px; font-weight: bold; color: #090e17; background: #f4c283; border-radius: 32px 0 0 32px; padding: 12px 22px; } .footer { font-size: 26px; color: #abb7c9; } .badge-title { font-size: 22px; color: #87d7ee; } .badge-value { font-size: 38px; color: #a8dac6; } .active { color: #f4c283; font-weight: bold; } .unknown { color: #dcb3c3; } button { font-size: 30px; padding: 10px; background: #f4c283; color: #090e17; border: 0; border-radius: 32px 0 0 32px; } button:disabled { background: #111925; color: #abb7c9; } button:hover { background: #87d7ee; } image { color: #f5f0e6; } .compact label { font-size: 20px; } .compact .badge-title { font-size: 12px; } .compact .badge-value { font-size: 22px; } .compact button { font-size: 18px; padding: 5px; }");
        gtk_css_provider_load_from_data(provider, css.as_ptr(), -1, ptr::null_mut());
        gtk_style_context_add_provider_for_screen(gdk_screen_get_default(), provider, 800);
        let settings_path = PathBuf::from(env::var("HOME").unwrap_or_default())
            .join(".config/daimon-integrators/panel.json");
        let compact = read_json::<Value>(&settings_path)
            .and_then(|v| v["compact"].as_bool())
            .unwrap_or(false);
        engine.lock().unwrap().snapshot.compact = compact;
        let data = Box::into_raw(Box::new(Ui {
            engine,
            ready,
            mic,
            cam,
            managed,
            remote,
            button,
            last: String::new(),
            window,
            detail: box_,
            compact_box,
            compact: !compact,
            small_mic,
            small_cam,
            small_rec,
            small_remote,
            small_stop,
            system,
            small_cpu,
            small_temperature,
            small_power,
        })) as W;
        let callback: unsafe extern "C" fn() =
            std::mem::transmute(stop as unsafe extern "C" fn(W, W));
        g_signal_connect_data(
            button,
            c("clicked").as_ptr(),
            Some(callback),
            data,
            ptr::null_mut(),
            0,
        );
        g_signal_connect_data(
            small_stop,
            c("clicked").as_ptr(),
            Some(callback),
            data,
            ptr::null_mut(),
            0,
        );
        let toggle_callback: unsafe extern "C" fn() =
            std::mem::transmute(toggle as unsafe extern "C" fn(W, W));
        for button in [expand, collapse] {
            g_signal_connect_data(
                button,
                c("clicked").as_ptr(),
                Some(toggle_callback),
                data,
                ptr::null_mut(),
                0,
            );
        }
        let available = i32::from(beta_helper().is_file());
        for (buttons, function) in [
            ([small_beta, beta], open_beta as unsafe extern "C" fn(W, W)),
            (
                [small_beta_stop, beta_stop],
                stop_beta as unsafe extern "C" fn(W, W),
            ),
        ] {
            let callback: unsafe extern "C" fn() = std::mem::transmute(function);
            for button in buttons {
                gtk_widget_set_sensitive(button, available);
                g_signal_connect_data(
                    button,
                    c("clicked").as_ptr(),
                    Some(callback),
                    data,
                    ptr::null_mut(),
                    0,
                );
            }
        }
        gtk_widget_show_all(window);
        apply_view(&mut *(data as *mut Ui), compact);
        g_timeout_add(300, Some(tick), data);
        gtk_main();
    }
    Ok(())
}
