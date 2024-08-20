use winapi::shared::windef::RECT;

pub fn empty(color: &str) -> String {
    format!(
        r#"<html>
    <style>
        body {{
            width: 100%;
            height: 100%;
            background: {};
            overflow: hidden;
        }}
    </style>
    <body>
    </body>
</html>"#,
        color
    )
}

pub fn calculate_bounds(
    rect: RECT,
    ref_width: i32,
    ref_height: i32,
    dpi: u32,
) -> Option<(RECT, f64)> {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return None;
    }

    let ratio_w = width as f64 / ref_width as f64;
    let ratio_h = height as f64 / ref_height as f64;

    let ratio = ratio_w.min(ratio_h);

    // how much space should left in `left` side to ensure 16:9 contents
    let margin_left = ((width as f64 - ref_width as f64 * ratio) / 2.0).floor() as i32;
    let margin_top = ((height as f64 - ref_height as f64 * ratio) / 2.0).floor() as i32;

    let rect = RECT {
        left: rect.left + margin_left,
        top: rect.top + margin_top,
        right: rect.left + width - margin_left,
        bottom: rect.top + height - margin_top,
    };

    let dpi = dpi as f64 / 96.0;
    let zoom = ratio / dpi;
    Some((rect, zoom))
}
