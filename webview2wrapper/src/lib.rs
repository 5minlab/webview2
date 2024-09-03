use std::mem::ManuallyDrop;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use webview2::host_object::IDispatch;
use webview2::*;
use winapi::shared::windef::*;
use winapi::um::winbase::SetEnvironmentStringsA;

type WebView2DataWrapper = Arc<RwLock<Option<WebView2Data>>>;

struct WebView2Data {
    controller: Controller,

    // Callbacks
    #[allow(unused)]
    queue: mpsc::Receiver<String>,
}

fn from_utf16(ptr: *const u16, len: u32) -> Option<String> {
    if ptr.is_null() || len == 0 {
        return None;
    }
    let data: &[u16] = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    String::from_utf16(data).ok()
}

#[no_mangle]
pub unsafe extern "C" fn webview2_open(
    url_ptr: *const u16,
    url_len: u32,
    host_name_ptr: *const u16,
    host_name_len: u32,
    folder_path_ptr: *const u16,
    folder_path_len: u32,
) -> usize {
    unsafe {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--autoplay-policy=no-user-gesture-required",
        );
    }

    let top = 0;
    let left = 0;

    let url_str = from_utf16(url_ptr, url_len).expect("url_str.from_utf16");
    let host_name = from_utf16(host_name_ptr, host_name_len);
    let folder_path = from_utf16(folder_path_ptr, folder_path_len);

    use winapi::um::winuser::*;

    let hwnd = unsafe { GetActiveWindow() };

    let wrapper: WebView2DataWrapper = Arc::new(RwLock::new(None));
    let ptr = WebView2DataWrapper::into_raw(wrapper.clone());

    let _ = Environment::builder().build(move |env| {
        env.expect("env")
            .create_controller(hwnd, move |controller| {
                let controller = controller.expect("create host");
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

                let _ = w.get_settings().map(|settings| {
                    settings.put_is_status_bar_enabled(false).unwrap();
                    settings
                        .put_are_default_context_menus_enabled(false)
                        .unwrap();
                    settings.put_is_zoom_control_enabled(false).unwrap();
                    settings.put_is_built_in_error_page_enabled(false).unwrap();

                    let s3 = settings.get_settings3().expect("get_settings3");
                    s3.put_are_browser_accelerator_keys_enabled(false).unwrap();

                    let s4 = settings.get_settings4().expect("get_settings4");
                    s4.put_is_password_autosave_enabled(false).unwrap();
                    s4.put_is_general_autofill_enabled(false).unwrap();

                    let s5 = settings.get_settings5().expect("get_settings5");
                    s5.put_is_pinch_zoom_enabled(false).unwrap();

                    let s6 = settings.get_settings6().expect("get_settings6");
                    s6.put_is_swipe_navigation_enabled(false).unwrap();
                });

                let r = RECT {
                    left,
                    top,
                    right: left,
                    bottom: top,
                };

                controller.put_bounds(r).expect("put_bounds");

                let editor_obj = Box::new(host_object::Variant::from(1));
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

                w.navigate_to_string(&util::empty("black"))
                    .expect("navigate_to_string");

                let c = controller.clone();
                w.add_navigation_completed(move |w, _| {
                    c.put_is_visible(true).expect("put_is_visible");
                    Ok(())
                });

                host_object::ensure_bind(w.clone(), "editor".to_owned(), editor_obj, move |w| {
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

                {
                    let mut guard = wrapper.write().unwrap();
                    *guard = Some(WebView2Data {
                        controller,

                        queue: receiver,
                    });
                }

                std::mem::forget(wrapper);

                Ok(())
            })
    });

    ptr as usize
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
            let mut utf16_str = s.encode_utf16().collect::<Vec<u16>>();
            utf16_str.push(0);
            let utf16_str = utf16_str.into_boxed_slice();

            *out = utf16_str.as_ptr();
            *len = utf16_str.len() as u32;

            std::mem::forget(utf16_str);
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn webview2_pull_free(data: *mut u16, len: u32) {
    let b: Box<[u16]> = Box::from_raw(std::slice::from_raw_parts_mut(data, len as usize));
    std::mem::drop(b);
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
