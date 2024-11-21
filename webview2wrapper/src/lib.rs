use std::mem::ManuallyDrop;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use webview2::host_object::IDispatch;
use webview2::*;
use winapi::shared::windef::*;

pub type WebView2DataWrapper = Arc<RwLock<Option<WebView2Data>>>;

pub struct WebView2Data {
    pub controller: Controller,

    // Callbacks
    queue: mpsc::Receiver<String>,
    pull_scratch: Vec<u16>,
}

fn from_utf16(ptr: *const u16, len: u32) -> Option<String> {
    if ptr.is_null() || len == 0 {
        return None;
    }
    let data: &[u16] = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    String::from_utf16(data).ok()
}

pub fn init_env() {
    unsafe {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--autoplay-policy=no-user-gesture-required --unlimited-storage",
        );
    }
}

pub struct InitializeState {
    pub url_str: String,
    pub host_name: Option<String>,
    pub folder_path: Option<String>,

    pub defines: Vec<String>,
}

pub fn setup_controller(controller: Controller) {
    controller.put_is_visible(false).expect("put_is_visible");

    {
        let c2 = controller.get_controller2().expect("get_controller2");
        let c = Color {
            a: 255,
            r: 0,
            g: 0,
            b: 0,
        };
        c2.put_default_background_color(c)
            .expect("put_default_background_color");
    }

    let w = controller.get_webview().expect("get_webview");
    // w.open_dev_tools_window().expect("open_dev_tools_window");

    let _ = w.get_settings().map(|settings| {
        settings.put_is_status_bar_enabled(false).unwrap();
        settings
            .put_are_default_context_menus_enabled(false)
            .unwrap();
        settings.put_is_zoom_control_enabled(false).unwrap();
        settings.put_is_built_in_error_page_enabled(false).unwrap();

        if let Ok(s3) = settings.get_settings3() {
            s3.put_are_browser_accelerator_keys_enabled(false).unwrap();
        }

        if let Ok(s4) = settings.get_settings4() {
            s4.put_is_password_autosave_enabled(false).unwrap();
            s4.put_is_general_autofill_enabled(false).unwrap();
        }

        if let Ok(s5) = settings.get_settings5() {
            s5.put_is_pinch_zoom_enabled(false).unwrap();
        }

        if let Ok(s6) = settings.get_settings6() {
            s6.put_is_swipe_navigation_enabled(false).unwrap();
        }
    });

    // disable all navigation
    w.add_navigation_starting(move |_w, args| {
        if let Ok(args3) = args.get_args3() {
            let kind = args3.get_navigation_kind().expect("get_navigation_kind");
            if kind == NavigationKind::BackOrForward || kind == NavigationKind::Reload {
                args.put_cancel(true).ok();
            }
        }
        Ok(())
    })
    .ok();

    w.navigate_to_string(&util::empty("black"))
        .expect("navigate_to_string");

    let c = controller.clone();
    w.add_navigation_completed(move |_w, _| {
        c.put_is_visible(true).expect("put_is_visible");
        Ok(())
    })
    .ok();
}

pub fn inject_defines<Fn>(w: WebView, mut names: Vec<String>, cb: Fn)
where
    Fn: FnOnce() + 'static,
{
    let name = match names.pop() {
        Some(name) => name,
        None => {
            cb();
            return;
        }
    };

    let obj = Box::new(host_object::Variant::from(1));
    host_object::ensure_bind(w.clone(), name, obj, move |w| {
        inject_defines(w, names, cb);
    });
}

pub fn initialize_controller_nobind(
    controller: Controller,
    state: InitializeState,
) -> Result<WebView2Data> {
    let InitializeState {
        url_str,
        host_name,
        folder_path,
        defines,
    } = state;

    let w = controller.get_webview().expect("get_webview");
    inject_defines(w.clone(), defines, move || {
        if let (Some(host_name), Some(folder_path)) = (host_name.as_ref(), folder_path.as_ref()) {
            w.get_webview_3()
                .expect("get_webview_3")
                .set_virtual_host_name_to_folder_mapping(
                    host_name,
                    folder_path,
                    HostResourceAccessKind::Allow,
                )
                .ok();
        }
        w.navigate(&url_str).expect("navigate");
    });

    let (_sender, receiver) = mpsc::channel();
    Ok(WebView2Data {
        controller,

        queue: receiver,
        pull_scratch: Vec::new(),
    })
}

pub fn initialize_controller(
    controller: Controller,
    state: InitializeState,
) -> Result<WebView2Data> {
    let InitializeState {
        url_str,
        host_name,
        folder_path,
        defines,
    } = state;

    setup_controller(controller.clone());

    let w = controller.get_webview().expect("get_webview");
    let r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };

    controller.put_bounds(r).expect("put_bounds");

    let (sender, receiver) = mpsc::channel();

    let obj = host_object::FunctionWithStringArgument {
        sender: sender.clone(),
    };
    let message_obj = Box::new(host_object::Variant::from(ManuallyDrop::new(Some(
        IDispatch::from(obj),
    ))));

    let sender0 = sender.clone();
    w.add_web_message_received(move |_w, args| {
        let msg = args.try_get_web_message_as_string();
        if let Ok(msg) = msg {
            sender0.send(msg).expect("mpsc::Sender::send");
        }
        Ok(())
    })
    .expect("add_web_message_received");

    inject_defines(w.clone(), defines, move || {
        let url_str = url_str.clone();
        host_object::ensure_bind(
            w.clone(),
            "functioncall".to_owned(),
            message_obj,
            move |w| {
                if let (Some(host_name), Some(folder_path)) =
                    (host_name.as_ref(), folder_path.as_ref())
                {
                    w.get_webview_3()
                        .expect("get_webview_3")
                        .set_virtual_host_name_to_folder_mapping(
                            host_name,
                            folder_path,
                            HostResourceAccessKind::Allow,
                        )
                        .ok();
                }
                w.navigate(&url_str).expect("navigate");
            },
        );
    });

    Ok(WebView2Data {
        controller,

        queue: receiver,
        pull_scratch: Vec::new(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn webview2_check() -> usize {
    if Environment::builder().build(move |_env| Ok(())).is_ok() {
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn webview2_open(
    url_ptr: *const u16,
    url_len: u32,
    host_name_ptr: *const u16,
    host_name_len: u32,
    folder_path_ptr: *const u16,
    folder_path_len: u32,
    defines_ptr: *const u16,
    defines_len: u32,
) -> usize {
    init_env();

    let url_str = from_utf16(url_ptr, url_len).expect("url_str.from_utf16");
    let host_name = from_utf16(host_name_ptr, host_name_len);
    let folder_path = from_utf16(folder_path_ptr, folder_path_len);
    let defines = from_utf16(defines_ptr, defines_len)
        .unwrap_or("".to_owned())
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .collect();

    let state = InitializeState {
        url_str,
        host_name,
        folder_path,
        defines,
    };

    use winapi::um::winuser::*;

    let hwnd = unsafe { GetActiveWindow() };

    let wrapper: WebView2DataWrapper = Arc::new(RwLock::new(None));
    let ptr = WebView2DataWrapper::into_raw(wrapper.clone());

    let res = Environment::builder().build(move |env| {
        let env = env.expect("env");

        env.create_controller(hwnd, move |controller| {
            let controller = controller.expect("create host");
            let data = initialize_controller(controller, state).expect("initialize_controller");

            {
                let mut guard = wrapper.write().unwrap();
                *guard = Some(data);
            }

            std::mem::forget(wrapper);

            Ok(())
        })
    });

    if let Err(e) = res {
        eprintln!("webview2_open: {:?}", e);
        0
    } else {
        ptr as usize
    }
}

fn with_wrapper<F>(ptr: usize, f: F)
where
    F: FnOnce(&mut WebView2Data),
{
    let data: WebView2DataWrapper = unsafe { Arc::from_raw(ptr as *mut _) };
    {
        let mut guard = data.write().unwrap();
        if let Some(data) = guard.as_mut() {
            f(data);
        }
    }
    std::mem::forget(data);
}

#[no_mangle]
pub unsafe extern "C" fn webview2_set_visible(ptr: usize, visible: i32) {
    with_wrapper(ptr, |data| {
        data.controller
            .put_is_visible(visible != 0)
            .expect("put_is_visible");
    });
}

#[no_mangle]
pub unsafe extern "C" fn webview2_open_dev_tools_window(ptr: usize) {
    with_wrapper(ptr, |data| {
        let w = data.controller.get_webview().expect("get_webview");
        w.open_dev_tools_window().expect("open_dev_tools_window");
    });
}

#[no_mangle]
pub unsafe extern "C" fn webview2_update_position(ptr: usize, left: i32, top: i32, w: i32, h: i32) {
    let r = RECT {
        left,
        top,
        right: w + left,
        bottom: h + top,
    };

    with_wrapper(ptr, |data| {
        data.controller.put_bounds(r).expect("put_bounds");
    });
}

#[no_mangle]
pub unsafe extern "C" fn webview2_update_position2(
    ptr: usize,
    left: i32,
    top: i32,
    w: i32,
    h: i32,
    ref_width: i32,
    ref_height: i32,
) {
    let r = RECT {
        left,
        top,
        right: w + left,
        bottom: h + top,
    };

    with_wrapper(ptr, |data| {
        if !data.controller.get_is_visible().unwrap() {
            return;
        }

        let dpi = unsafe {
            winapi::um::winuser::GetDpiForWindow(
                data.controller
                    .get_parent_window()
                    .expect("get_host_window"),
            )
        };

        if let Some((rect, zoom)) = util::calculate_bounds(r, ref_width, ref_height, dpi) {
            data.controller
                .set_bounds_and_zoom_factor(rect, zoom)
                .expect("set_bonds_and_zoom_factor");
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn webview2_pull(ptr: usize, out: *mut *const u16, len: *mut u32) {
    with_wrapper(ptr, |data| {
        if let Ok(s) = data.queue.try_recv() {
            let data = &mut data.pull_scratch;
            data.clear();
            for v in s.encode_utf16() {
                data.push(v);
            }
            data.push(0);

            *out = data.as_ptr();
            *len = data.len() as u32;
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn webview2_pull_free(_data: *mut u16, _len: u32) {
    // noop
}

#[no_mangle]
pub unsafe extern "C" fn webview2_post_web_message_as_json(
    ptr: usize,
    json_ptr: *const u16,
    len: u32,
) {
    let json_str = from_utf16(json_ptr, len).expect("json_str.from_utf16");

    with_wrapper(ptr, |data| {
        if let Ok(webview) = data.controller.get_webview() {
            webview
                .post_web_message_as_json(&json_str)
                .expect("post_web_message_as_json");
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn webview2_execute_script(ptr: usize, script_ptr: *const u16, len: u32) {
    let script_str = from_utf16(script_ptr, len).expect("script_str.from_utf16");

    with_wrapper(ptr, |data| {
        if let Ok(webview) = data.controller.get_webview() {
            let _ = webview.execute_script(&script_str, |_| Ok(()));
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn webview2_close(ptr: usize) {
    with_wrapper(ptr, |data| {
        data.controller.close().expect("close");
    });
}
