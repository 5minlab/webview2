//! A demo using native-windows-gui for window creation and event handling.

use once_cell::unsync::OnceCell;
use std::mem;
use std::rc::Rc;
use webview2::*;
use winapi::shared::windef::*;
use winapi::um::winuser::*;
use winit::dpi::Size;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::windows::WindowExtWindows;
use winit::window::WindowBuilder;

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("WebView2 - virtual host")
        .with_inner_size(Size::Logical((1600, 900).into()))
        .build(&event_loop)
        .unwrap();

    let controller: Rc<OnceCell<Controller>> = Rc::new(OnceCell::new());

    let create_result = {
        let controller_clone = controller.clone();
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

                        settings.put_is_web_message_enabled(true).unwrap();
                    });

                    {
                        let mut cwd = std::env::current_dir().unwrap();
                        cwd.push("public");
                        let w3 = w.get_webview_3().expect("get_webview_3");

                        w3.set_virtual_host_name_to_folder_mapping(
                            "example.com",
                            cwd.to_str().unwrap(),
                            HostResourceAccessKind::Allow,
                        )
                        .expect("set_virtual_host_name_to_folder_mapping");
                    }

                    {
                        let cc = controller.get_composition_controller().expect("get_composition_controller");
                        let cursor = cc.get_cursor().expect("get_cursor");
                        eprintln!("cursor={:?}", cursor);
                    }

                    unsafe {
                        let mut rect = mem::zeroed();
                        GetClientRect(hwnd, &mut rect);
                        controller.put_bounds(rect).expect("put_bounds");
                    }

                    w.add_navigation_completed(|w, _| {
                        w.post_web_message_as_json(r#"{"type":"hello"}"#)
                            .expect("post_web_message_as_json");
                        Ok(())
                    })
                    .expect("add_navigation_completed");

                    w.navigate("https://example.com/index.html")
                        .expect("navigate");

                    controller_clone.set(controller).unwrap();
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
