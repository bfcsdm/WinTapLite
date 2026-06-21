use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

#[allow(dead_code)]
pub fn get_cursor_pos() -> Option<(i32, i32)> {
    let mut pt = POINT::default();
    unsafe {
        match GetCursorPos(&mut pt) {
            Ok(()) => Some((pt.x, pt.y)),
            Err(_) => None,
        }
    }
}

pub fn get_screen_size() -> (u32, u32) {
    unsafe {
        let width = GetSystemMetrics(SM_CXSCREEN) as u32;
        let height = GetSystemMetrics(SM_CYSCREEN) as u32;
        (width, height)
    }
}

#[allow(dead_code)]
pub fn validate_coordinate(x: u32, y: u32) -> Result<(), String> {
    let (max_x, max_y) = get_screen_size();
    if x > max_x {
        return Err(format!("X 坐标超出屏幕范围（最大 {}）", max_x));
    }
    if y > max_y {
        return Err(format!("Y 坐标超出屏幕范围（最大 {}）", max_y));
    }
    Ok(())
}

#[allow(dead_code)]
pub fn parse_coordinate(x_str: &str, y_str: &str) -> Result<(u32, u32), String> {
    let x: u32 = x_str
        .trim()
        .parse()
        .map_err(|_| "X 坐标必须为有效数字".to_string())?;
    let y: u32 = y_str
        .trim()
        .parse()
        .map_err(|_| "Y 坐标必须为有效数字".to_string())?;
    validate_coordinate(x, y)?;
    Ok((x, y))
}
