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

fn set_bounds(controller: Controller, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }

    let width_target = 1920i32;
    let height_target = 1080i32;

    let ratio_w = width as f64 / width_target as f64;
    let ratio_h = height as f64 / height_target as f64;

    let dpi = unsafe {
        winapi::um::winuser::GetDpiForWindow(
            controller.get_parent_window().expect("get_host_window"),
        )
    };
    let dpi = dpi as f64 / 96.0;
    let ratio = ratio_w.min(ratio_h);

    // how much space should left in `left` side to ensure 16:9 contents
    let left = ((width as f64 - 1920.0 * ratio) / 2.0).floor() as i32;
    let top = ((height as f64 - 1080.0 * ratio) / 2.0).floor() as i32;

    eprintln!(
        "width: {}, height: {}, left={}, top={}",
        width, height, left, top
    );

    let rect = RECT {
        left,
        top,
        right: width - left,
        bottom: height - top,
    };

    let zoom = ratio / dpi;
    controller
        .set_bounds_and_zoom_factor(rect, zoom)
        .expect("set_bonds_and_zoom_factor");
}

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("WebView2 - winit")
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
                    });

                    unsafe {
                        let mut rect = mem::zeroed();
                        GetClientRect(hwnd, &mut rect);
                        set_bounds(controller.clone(), rect.right, rect.bottom);

                        /*
                        let controller3 = controller.get_controller3().expect("get_controller3");
                        controller3.put_bounds_mode(BoundsMode::UseRasterizationScale).expect("put_bounds_mode");
                        */
                    }

                    /*
                    w.open_dev_tools_window().expect("open_dev_tools_window");
                    w.navigate("http://10.0.7.140:3000/title")
                        .expect("navigate");
                    */
                    w.navigate("https://whatismyviewport.com").ok();

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
                    set_bounds(
                        controller.get().unwrap().clone(),
                        new_size.width as i32,
                        new_size.height as i32,
                    );
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