//! Foreground native button; remote sessions remain owned by their controller.
use serde_json::{json, Value};
use std::{
    ffi::{c_char, c_void, CString},
    io::{BufRead, BufReader, Write},
    os::unix::process::CommandExt,
    path::Path,
    process::{ChildStdin, Command, Stdio},
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
    fn gtk_window_set_title(w: W, s: *const c_char);
    fn gtk_window_set_default_size(w: W, width: i32, height: i32);
    fn gtk_container_add(w: W, child: W);
    fn gtk_container_set_border_width(w: W, border: u32);
    fn gtk_box_new(orientation: i32, spacing: i32) -> W;
    fn gtk_box_pack_start(w: W, child: W, expand: i32, fill: i32, padding: u32);
    fn gtk_label_new(s: *const c_char) -> W;
    fn gtk_label_set_text(w: W, s: *const c_char);
    fn gtk_label_set_line_wrap(w: W, wrap: i32);
    fn gtk_label_set_max_width_chars(w: W, width: i32);
    fn gtk_button_new_with_label(s: *const c_char) -> W;
    fn gtk_button_set_label(w: W, s: *const c_char);
    fn gtk_widget_set_sensitive(w: W, value: i32);
    fn gtk_widget_grab_focus(w: W);
    fn gtk_widget_show_all(w: W);
    fn gtk_css_provider_new() -> W;
    fn gtk_css_provider_load_from_data(w: W, s: *const c_char, len: isize, error: *mut W) -> i32;
    fn gtk_widget_get_style_context(w: W) -> W;
    fn gtk_style_context_add_provider(w: W, provider: W, priority: u32);
    fn gtk_main();
    fn gtk_main_quit();
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
fn c(value: &str) -> CString {
    CString::new(value.replace('\0', " ")).unwrap()
}
struct Ui {
    view: Arc<Mutex<Value>>,
    input: Arc<Mutex<Option<ChildStdin>>>,
    closed: Arc<AtomicBool>,
    status: W,
    source: W,
    microphone: W,
    button: W,
    pressed: Option<Instant>,
    held: bool,
}
impl Ui {
    fn send(&self, action: &str) {
        if let Some(input) = self.input.lock().unwrap().as_mut() {
            let _ = writeln!(input, "{}", json!({"action":action}));
            let _ = input.flush();
        }
    }
}
unsafe extern "C" fn pressed(_w: W, data: W) {
    let ui = &mut *(data as *mut Ui);
    ui.pressed = Some(Instant::now());
    ui.held = false;
}
unsafe extern "C" fn released(_w: W, data: W) {
    (*(data as *mut Ui)).pressed = None;
}
unsafe extern "C" fn clicked(_w: W, data: W) {
    let ui = &mut *(data as *mut Ui);
    if !ui.held {
        ui.send("button");
    }
    ui.held = false;
    ui.pressed = None;
}
unsafe extern "C" fn destroyed(_w: W, data: W) {
    let ui = &mut *(data as *mut Ui);
    if !ui.closed.swap(true, Ordering::SeqCst) {
        ui.send("close");
    }
    gtk_main_quit();
}
unsafe extern "C" fn tick(data: W) -> i32 {
    let ui = &mut *(data as *mut Ui);
    if ui.closed.load(Ordering::SeqCst) {
        return 0;
    }
    if ui
        .pressed
        .is_some_and(|at| at.elapsed() >= Duration::from_millis(900))
        && !ui.held
    {
        ui.held = true;
        ui.send("discard");
    }
    let view = ui.view.lock().unwrap().clone();
    gtk_label_set_text(
        ui.status,
        c(view["status"].as_str().unwrap_or("Connecting…")).as_ptr(),
    );
    let source = view["source"]
        .as_str()
        .filter(|v| !v.is_empty())
        .or_else(|| view["focus"].as_str().filter(|v| !v.is_empty()))
        .unwrap_or("Your conversation");
    gtk_label_set_text(ui.source, c(source).as_ptr());
    gtk_button_set_label(
        ui.button,
        c(view["button"].as_str().unwrap_or("Talk")).as_ptr(),
    );
    gtk_widget_set_sensitive(ui.button, i32::from(view["enabled"] == true));
    let observation = &view["observation"];
    let at = observation["at"].as_u64().unwrap_or(0);
    let mic = if !daimon_integrators::observation_fresh(at, daimon_integrators::now()) {
        "Unknown"
    } else if observation["microphone_running"] == true {
        "In use"
    } else if observation["microphone_present"] == false {
        "Disconnected"
    } else if observation["complete"] == true {
        "No use detected"
    } else {
        "Unknown"
    };
    gtk_label_set_text(ui.microphone, c(&format!("Microphone · {mic}")).as_ptr());
    1
}
unsafe fn signal(w: W, name: &str, callback: unsafe extern "C" fn(W, W), data: W) {
    g_signal_connect_data(
        w,
        c(name).as_ptr(),
        Some(std::mem::transmute::<
            unsafe extern "C" fn(W, W),
            unsafe extern "C" fn(),
        >(callback)),
        data,
        ptr::null_mut(),
        0,
    );
}
unsafe fn label(box_: W, text: &str) -> W {
    let w = gtk_label_new(c(text).as_ptr());
    gtk_label_set_line_wrap(w, 1);
    gtk_label_set_max_width_chars(w, 45);
    gtk_box_pack_start(box_, w, 0, 0, 0);
    w
}
pub fn run(config: &Path) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        if gtk_init_check(ptr::null_mut(), ptr::null_mut()) == 0 {
            return Err("GTK display unavailable".into());
        }
    }
    let mut child = Command::new("python3")
        .args([
            "-u",
            "-c",
            include_str!("../tools/companion_client.py"),
            "--config",
        ])
        .arg(config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    let input = Arc::new(Mutex::new(child.stdin.take()));
    let stdout = child
        .stdout
        .take()
        .ok_or("Companion state pipe unavailable")?;
    let view = Arc::new(Mutex::new(
        json!({"status":"Connecting to your conversation.","enabled":false}),
    ));
    let reader_view = view.clone();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if line.len() > 262144 {
                break;
            }
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                if value["schema"] == "daimon-integrators.companion-view/v1" {
                    *reader_view.lock().unwrap() = value;
                }
            }
        }
        *reader_view.lock().unwrap() =
            json!({"status":"Companion stopped. Saved inputs are retained.","enabled":false});
    });
    let closed = Arc::new(AtomicBool::new(false));
    unsafe {
        let window = gtk_window_new(0);
        gtk_window_set_title(window, c("Daimon conversation").as_ptr());
        gtk_window_set_default_size(window, 360, 0);
        gtk_container_set_border_width(window, 14);
        let box_ = gtk_box_new(1, 10);
        gtk_container_add(window, box_);
        let source = label(box_, "Your conversation");
        let button = gtk_button_new_with_label(c("Talk").as_ptr());
        gtk_box_pack_start(box_, button, 1, 1, 0);
        let status = label(box_, "Connecting to your conversation.");
        let microphone = label(box_, "Microphone · Unknown");
        label(
            box_,
            "Press to talk or send. Hold to discard a recording. Work continues in its sessions.",
        );
        let css = gtk_css_provider_new();
        let style=c("window { background: #07101a; color: #e8f2f1; font-family: Sans; font-size: 18px; } button { background: #102b31; color: #9ce8cf; border: 2px solid #83d5dc; border-radius: 24px; min-height: 80px; font-size: 30px; padding: 12px; } button:active { background: #214b49; } button:disabled { color: #87969a; }");
        gtk_css_provider_load_from_data(css, style.as_ptr(), -1, ptr::null_mut());
        gtk_style_context_add_provider(gtk_widget_get_style_context(window), css, 800);
        gtk_style_context_add_provider(gtk_widget_get_style_context(button), css, 800);
        let ui = Box::into_raw(Box::new(Ui {
            view,
            input,
            closed: closed.clone(),
            status,
            source,
            microphone,
            button,
            pressed: None,
            held: false,
        }));
        let data = ui as W;
        signal(button, "pressed", pressed, data);
        signal(button, "released", released, data);
        signal(button, "clicked", clicked, data);
        signal(window, "destroy", destroyed, data);
        let timer = g_timeout_add(100, Some(tick), data);
        gtk_widget_show_all(window);
        gtk_widget_grab_focus(button);
        tick(data);
        gtk_main();
        g_source_remove(timer);
        drop(Box::from_raw(ui));
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait()?.is_none() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
    }
    if child.try_wait()?.is_none() {
        child.kill()?;
        child.wait()?;
    }
    let _ = reader.join();
    Ok(())
}
