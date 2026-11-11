use crate::model::Settings;

fn colors(settings: &Settings) -> (u32, u32) {
    if settings.titlebar_color == "system" {
        return (u32::MAX, u32::MAX);
    }
    let hex = if settings.titlebar_color == "theme" {
        if settings.theme == "light" {
            "f1f0fb"
        } else {
            "1b1f29"
        }
    } else {
        settings.titlebar_color.trim_start_matches('#')
    };
    let rgb = u32::from_str_radix(hex, 16).unwrap_or(0x1b1f29);
    let r = rgb >> 16;
    let g = (rgb >> 8) & 255;
    let b = rgb & 255;
    let text = if r * 299 + g * 587 + b * 114 > 145000 {
        0x171c26
    } else {
        0xedf0f6
    };
    let colorref = |rgb: u32| ((rgb & 255) << 16) | (rgb & 0xff00) | (rgb >> 16);
    (colorref(rgb), colorref(text))
}
pub fn apply(window: &tauri::WebviewWindow, settings: &Settings) {
    #[cfg(windows)]
    {
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
        };
        if let Ok(hwnd) = window.hwnd() {
            let (caption, text) = colors(settings);
            // Windows 11 supports per-window colors; older versions keep native defaults.
            unsafe {
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_CAPTION_COLOR,
                    &caption as *const u32 as _,
                    4,
                );
                let _ = DwmSetWindowAttribute(hwnd, DWMWA_TEXT_COLOR, &text as *const u32 as _, 4);
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (window, settings);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn title_colors_use_windows_rgb_order_and_keep_text_readable() {
        let mut s = Settings::default();
        assert_eq!(colors(&s).0, 0x291f1b);
        s.titlebar_color = "#ff0000".into();
        assert_eq!(colors(&s).0, 0x0000ff);
        s.titlebar_color = "#ffffff".into();
        assert_eq!(colors(&s).1, 0x261c17);
        s.titlebar_color = "system".into();
        assert_eq!(colors(&s), (u32::MAX, u32::MAX));
    }
}
