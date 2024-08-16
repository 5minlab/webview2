//! A demo using native-windows-gui for window creation and event handling.

use once_cell::unsync::OnceCell;
use std::mem::{self, ManuallyDrop};
use std::rc::Rc;
use std::sync::mpsc;
use webview2::host_object::IDispatch;
use webview2::{host_object, Controller};
use winapi::shared::windef::*;
use winapi::um::winuser::*;
use winit::dpi::Size;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::windows::WindowExtWindows;
use winit::window::WindowBuilder;

fn check_loaded(name: &str) -> String {
    // format!("(function () {{ try {{ typeof window.chrome.webview.hostObjects.sync.{}; return 'loaded'; }} catch (e) {{ if(e.message.indexOf('Element out found') === 0) {{ return 'not_loaded'; }} else {{ return 'error' }} }} }})()", name)
    format!("(function () {{ try {{ typeof window.chrome.webview.hostObjects.sync.{}; return 'loaded'; }} catch (e) {{ return 'not_loaded'; }} }})()", name)
}

fn ensure_bind<F>(w: webview2::WebView, name: String, sender: mpsc::Sender<String>, cb: F)
where
    F: FnMut() + 'static,
{
    eprintln!("bind");
    let obj = host_object::FunctionWithStringArgument {
        sender: sender.clone(),
    };
    let mut message_obj = Box::new(host_object::Variant::from(ManuallyDrop::new(Some(
        IDispatch::from(obj),
    ))));
    w.add_host_object_to_script(&name, &mut message_obj.0)
        .expect("add_host_object_to_script");

    check_bind(w.clone(), name.clone(), sender.clone(), cb);

    std::mem::forget(message_obj);
}

fn check_bind<F>(w: webview2::WebView, name: String, sender: mpsc::Sender<String>, mut cb: F)
where
    F: FnMut() + 'static,
{
    eprintln!("check_bind");
    let script = check_loaded(&name);

    let w0 = w.clone();
    let sender1 = sender.clone();
    w.execute_script(&script, move |s| {
        println!("s={:?}", s);
        if s == "\"loaded\"" {
            cb();
        } else if s == "\"not_loaded\"" {
            ensure_bind(w0.clone(), name.clone(), sender1.clone(), cb);
        } else {
            todo!();
        }
        Ok(())
    })
    .expect("execute_script");

    eprintln!("check_bind end");
}

#[allow(unused)]
fn bind(w: webview2::WebView, sender: mpsc::Sender<String>) {
    eprintln!("bind");
    let obj = host_object::FunctionWithStringArgument {
        sender: sender.clone(),
    };
    let mut message_obj = Box::new(host_object::Variant::from(ManuallyDrop::new(Some(
        IDispatch::from(obj),
    ))));
    w.add_host_object_to_script("functioncall", &mut message_obj.0)
        .expect("add_host_object_to_script");
    std::mem::forget(message_obj);
}

#[allow(unused)]
fn run_script(w: webview2::WebView, sender: mpsc::Sender<String>) {
    eprintln!("run_script");
    let script =
        r#"document.write(window.chrome.webview.hostObjects.sync.functioncall("hello")); "hello""#;

    let w0 = w.clone();
    let sender1 = sender.clone();
    w.execute_script(script, move |s| {
        println!("s={:?}", s);
        if s == "null" {
            bind(w0.clone(), sender1.clone());
            run_script(w0.clone(), sender1.clone());
        }
        Ok(())
    })
    .expect("execute_script");
    eprintln!("run_script end");
}

fn run_script0(w: webview2::WebView) {
    eprintln!("run_script0");
    let script =
        r#"document.write(window.chrome.webview.hostObjects.sync.functioncall("hello")); "hello""#;

    w.execute_script(script, move |s| {
        println!("s={:?}", s);
        Ok(())
    })
    .expect("run_script0");
    eprintln!("run_script end");
}


fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("WebView2 - winit")
        .with_inner_size(Size::Logical((1600, 900).into()))
        .build(&event_loop)
        .unwrap();

    let controller: Rc<OnceCell<Controller>> = Rc::new(OnceCell::new());
    let receiver: Rc<OnceCell<mpsc::Receiver<String>>> = Rc::new(OnceCell::new());

    let create_result = {
        let controller_clone = controller.clone();
        let receiver_clone = receiver.clone();
        let hwnd = window.hwnd() as HWND;

        webview2::Environment::builder().build(move |env| {
            env.expect("env")
                .create_controller(hwnd, move |controller| {
                    let controller = controller.expect("create host");
                    let w = controller.get_webview().expect("get_webview");

                    let _ = w.get_settings().map(|settings| {
                        let _ = settings.put_is_status_bar_enabled(false);
                        let _ = settings.put_are_default_context_menus_enabled(false);
                        let _ = settings.put_is_zoom_control_enabled(false);
                    });

                    unsafe {
                        let mut rect = mem::zeroed();
                        GetClientRect(hwnd, &mut rect);
                        controller.put_bounds(rect).expect("put_bounds");
                    }

                    let (sender, receiver) = mpsc::channel();
                    let url = "about:blank";

                    let sender0 = sender.clone();
                    w.add_navigation_starting(move |w, args| {
                        let url_loaded = args.get_uri().unwrap();
                        eprintln!("navigation_starting url={}, url_loaded={}", url, url_loaded);
                        if url == url_loaded {
                            let w0 = w.clone();
                            ensure_bind(w, "functioncall".to_owned(), sender0.clone(), move || {
                                run_script0(w0.clone());
                            });
                        }
                        Ok(())
                    })
                    .unwrap();

                    /*
                    let sender0 = sender.clone();
                    w.add_navigation_completed(move |w, _| {
                        eprintln!("navigation_completed");
                        run_script(w.clone(), sender0.clone());
                        Ok(())
                    })
                    .unwrap();
                    */

                    w.navigate(url).expect("navigate");

                    w.open_dev_tools_window().expect("open_dev_tools_window");

                    controller_clone.set(controller).unwrap();
                    receiver_clone.set(receiver).unwrap();
                    Ok(())
                })
        })
    };

    if let Err(e) = create_result {
        eprintln!(
            "Failed to create webview environment: {}. Is the new edge browser installed?",
            e
        );
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Some(receiver) = receiver.get() {
            if let Ok(out) = receiver.try_recv() {
                println!("out={}", out);
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    if let Some(webview_host) = controller.get() {
                        webview_host.close().expect("close");
                    }
                    *control_flow = ControlFlow::Exit;
                }
                // Notify the webview when the parent window is moved.
                WindowEvent::Moved(_) => {
                    if let Some(webview_host) = controller.get() {
                        let _ = webview_host.notify_parent_window_position_changed();
                    }
                }
                // Update webview bounds when the parent window is resized.
                WindowEvent::Resized(new_size) => {
                    if let Some(webview_host) = controller.get() {
                        let r = RECT {
                            left: 0,
                            top: 0,
                            right: new_size.width as i32,
                            bottom: new_size.height as i32,
                        };
                        webview_host.put_bounds(r).expect("put_bounds");
                    }
                }
                _ => {}
            },
            Event::MainEventsCleared => {
                // Application update code.

                // Queue a RedrawRequested event.
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {}
            _ => (),
        }
    });
}
