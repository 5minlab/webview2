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

fn run_script0(w: &webview2::WebView) {
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

                    let obj = host_object::FunctionWithStringArgument {
                        sender: sender.clone(),
                    };
                    let message_obj = Box::new(host_object::Variant::from(ManuallyDrop::new(
                        Some(IDispatch::from(obj)),
                    )));

                    host_object::ensure_bind(
                        w.clone(),
                        "functioncall".to_owned(),
                        message_obj,
                        move |w| {
                            run_script0(&w);
                            w.navigate("https://wikipedia.com").expect("navigate");
                        },
                    );

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

        /*
        if let Some(receiver) = receiver.get() {
            if let Ok(out) = receiver.try_recv() {
                println!("out={}", out);
                *control_flow = ControlFlow::Exit;
                return;
            }
        }
        */

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
