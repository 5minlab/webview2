//! A demo using native-windows-gui for window creation and event handling.

use std::mem;
use std::sync::{Arc, RwLock};
use webview2wrapper::*;
use winapi::shared::windef::*;
use winapi::um::winuser::*;
use winit::dpi::Size;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::windows::WindowExtWindows;
use winit::window::WindowBuilder;

fn update_bounds(controller: webview2::Controller, rect: RECT) {
    let dpi = unsafe {
        winapi::um::winuser::GetDpiForWindow(
            controller.get_parent_window().expect("get_host_window"),
        )
    };

    if let Some((rect, zoom)) = webview2::util::calculate_bounds(rect, 1920, 1080, dpi) {
        controller
            .set_bounds_and_zoom_factor(rect, zoom)
            .expect("set_bonds_and_zoom_factor");
    }
}

fn main() {
    init_env();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Private Military Manager")
        .with_inner_size(Size::Logical((1600, 900).into()))
        .build(&event_loop)
        .unwrap();

    let wrapper: WebView2DataWrapper = Arc::new(RwLock::new(None));

    let mut cwd = std::env::current_dir().unwrap();
    cwd.push("public");

    let state = InitializeState {
        url_str: "https://shell.pmc0.pages.dev/index.html".to_owned(),
        host_name: Some("shell.pmc0.pages.dev".to_owned()),
        folder_path: Some(cwd.to_str().unwrap().to_owned()),
        defines: vec!["shellonly".to_owned()],
    };

    let create_result = {
        let wrapper = wrapper.clone();
        let hwnd = window.hwnd() as HWND;

        webview2::Environment::builder().build(move |env| {
            env.expect("env")
                .create_controller(hwnd, move |controller| {
                    let controller = controller.expect("create host");
                    setup_controller(controller.clone());

                    let data = initialize_controller_nobind(controller, state)
                        .expect("initialize_controller");

                    unsafe {
                        let mut rect = mem::zeroed();
                        GetClientRect(hwnd, &mut rect);

                        let hwnd = data
                            .controller
                            .get_parent_window()
                            .expect("get_host_window");
                        let hdc = winapi::um::winuser::GetDC(hwnd);
                        let hbrush = winapi::um::wingdi::CreateSolidBrush(0x00000000);
                        FillRect(hdc, &rect, hbrush);

                        update_bounds(data.controller.clone(), rect);
                    }

                    if false {
                        let w = data.controller.get_webview().expect("get_webview");
                        w.open_dev_tools_window().expect("open_dev_tools_window");
                    }
                    {
                        let mut guard = wrapper.write().unwrap();
                        *guard = Some(data);
                    }

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
        let wrapper = wrapper.clone();
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    let mut guard = wrapper.write().unwrap();
                    if let Some(data) = guard.as_mut() {
                        data.controller.close().expect("close");
                    }

                    *control_flow = ControlFlow::Exit;
                }
                // Notify the webview when the parent window is moved.
                WindowEvent::Moved(_) => {
                    let mut guard = wrapper.write().unwrap();
                    if let Some(data) = guard.as_mut() {
                        let _ = data.controller.notify_parent_window_position_changed();
                    }
                }
                // Update webview bounds when the parent window is resized.
                WindowEvent::Resized(new_size) => {
                    let mut guard = wrapper.write().unwrap();
                    if let Some(data) = guard.as_mut() {
                        let r = RECT {
                            left: 0,
                            top: 0,
                            right: new_size.width as i32,
                            bottom: new_size.height as i32,
                        };
                        update_bounds(data.controller.clone(), r);
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
