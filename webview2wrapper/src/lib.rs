use std::ffi::*;
use std::mem::ManuallyDrop;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use webview2::host_object::IDispatch;
use webview2::*;
use winapi::shared::windef::*;

type WebView2DataWrapper = Arc<RwLock<Option<WebView2Data>>>;

struct WebView2Data {
    controller: Controller,

    // Callbacks
    #[allow(unused)]
    queue: mpsc::Receiver<String>,
}

#[no_mangle]
pub unsafe extern "C" fn webview2_open(url_ptr: *const u16, len: u32) -> usize {
    let top = 0;
    let left = 0;

    let url_data: &[u16] = std::slice::from_raw_parts(url_ptr, len as usize);
    let url_str = String::from_utf16_lossy(url_data);

    use winapi::um::winuser::*;

    let hwnd = unsafe { GetActiveWindow() };

    let wrapper: WebView2DataWrapper = Arc::new(RwLock::new(None));
    let ptr = WebView2DataWrapper::into_raw(wrapper.clone());

    let _ = Environment::builder().build(move |env| {
        env.expect("env")
            .create_controller(hwnd, move |controller| {
                let controller = controller.expect("create host");
                let w = controller.get_webview().expect("get_webview");

                let _ = w.get_settings().map(|settings| {
                    let _ = settings.put_is_status_bar_enabled(false);
                    let _ = settings.put_are_default_context_menus_enabled(false);
                    let _ = settings.put_is_zoom_control_enabled(false);
                });

                let r = RECT {
                    left,
                    top,
                    right: left + 500,
                    bottom: top + 500,
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
                    eprintln!("msg={:?}", msg);
                    if let Ok(msg) = msg {
                        sender0.send(msg).expect("mpsc::Sender::send");
                    }
                    Ok(())
                })
                .expect("add_web_message_received");

                host_object::ensure_bind(w.clone(), "editor".to_owned(), editor_obj, move |w| {
                    let url_str = url_str.clone();
                    host_object::ensure_bind(
                        w.clone(),
                        "functioncall".to_owned(),
                        message_obj,
                        move |w| {
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

#[no_mangle]
pub unsafe extern "C" fn webview2_set_visible(ptr: usize, visible: i32) {
    let data: WebView2DataWrapper = Arc::from_raw(ptr as *mut _);

    {
        let mut guard = data.write().unwrap();
        if let Some(data) = guard.as_mut() {
            data.controller
                .put_is_visible(visible != 0)
                .expect("put_is_visible");
        }
    }

    std::mem::forget(data);
}

#[no_mangle]
pub unsafe extern "C" fn webview2_open_dev_tools_window(ptr: usize) {
    let data: WebView2DataWrapper = Arc::from_raw(ptr as *mut _);

    {
        let mut guard = data.write().unwrap();
        if let Some(data) = guard.as_mut() {
            let w = data.controller.get_webview().expect("get_webview");
            w.open_dev_tools_window().expect("open_dev_tools_window");
        }
    }

    std::mem::forget(data);
}


#[no_mangle]
pub unsafe extern "C" fn webview2_update_position(ptr: usize, left: i32, top: i32, w: i32, h: i32) {
    let data: WebView2DataWrapper = Arc::from_raw(ptr as *mut _);
    let r = RECT {
        left,
        top,
        right: w + left,
        bottom: h + top,
    };

    {
        let mut guard = data.write().unwrap();
        if let Some(data) = guard.as_mut() {
            data.controller.put_bounds(r).expect("put_bounds");
        }
    }

    std::mem::forget(data);
}

#[no_mangle]
pub unsafe extern "C" fn webview2_pull(ptr: usize) -> *const c_char {
    use std::ffi::CString;

    let data: WebView2DataWrapper = Arc::from_raw(ptr as *mut _);

    let ret = {
        let mut guard = data.write().unwrap();
        if let Some(data) = guard.as_mut() {
            if let Ok(s) = data.queue.try_recv() {
                let c_str = CString::new(s).unwrap();
                c_str.into_raw()
            } else {
                std::ptr::null()
            }
        } else {
            std::ptr::null()
        }
    };

    std::mem::forget(data);
    ret
}

#[no_mangle]
pub unsafe extern "C" fn webview2_execute_script(ptr: usize, script_ptr: *const u16, len: u32) {
    let data: WebView2DataWrapper = Arc::from_raw(ptr as *mut _);

    let script_data: &[u16] = std::slice::from_raw_parts(script_ptr, len as usize);
    let script_str = String::from_utf16_lossy(script_data);

    {
        let mut guard = data.write().unwrap();
        if let Some(data) = guard.as_mut() {
            if let Ok(webview) = data.controller.get_webview() {
                let _ = webview.execute_script(&script_str, |_| Ok(()));
            }
        }
    }

    std::mem::forget(data);
}

#[no_mangle]
pub unsafe extern "C" fn webview2_close(ptr: usize) {
    let data: WebView2DataWrapper = Arc::from_raw(ptr as *mut _);

    let mut guard = data.write().unwrap();
    if let Some(data) = guard.as_mut() {
        /*
        let w = data.controller.get_webview().expect("get_webview");

        w.remove_host_object_from_script("editor")
            .expect("remove_host_object_from_script");
        w.remove_host_object_from_script("functioncall")
            .expect("remove_host_object_from_script");
        */

        data.controller.close().expect("close");
    }
}
