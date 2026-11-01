//! The buttons that carry the normal flow forward, painted light green so
//! the eye finds them. A themed push button ignores the colours its parent
//! offers, so these are drawn whole from the button's `NM_CUSTOMDRAW`.

use windows_sys::Win32::Graphics::Gdi as gdi;
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

const FILL: u32 = rgb(204, 240, 204);
const FILL_HOT: u32 = rgb(182, 230, 182);
const FILL_PRESSED: u32 = rgb(156, 214, 156);
const BORDER: u32 = rgb(46, 139, 87);

/// Paints a forward button and answers what its parent returns for the
/// notification. A disabled one is left to the theme: grey is what "not
/// yet" looks like.
pub unsafe fn paint_forward_button(draw: &ctl::NMCUSTOMDRAW) -> isize {
    let state = draw.uItemState;
    if draw.dwDrawStage != ctl::CDDS_PREPAINT || state & ctl::CDIS_DISABLED != 0 {
        return ctl::CDRF_DODEFAULT as isize;
    }
    let fill = if state & ctl::CDIS_SELECTED != 0 {
        FILL_PRESSED
    } else if state & ctl::CDIS_HOT != 0 {
        FILL_HOT
    } else {
        FILL
    };
    let mut rect = draw.rc;
    let brush = gdi::CreateSolidBrush(fill);
    gdi::FillRect(draw.hdc, &rect, brush);
    gdi::DeleteObject(brush);

    // Focus thickens the border, as the theme's own focused button does.
    let border = gdi::CreateSolidBrush(BORDER);
    gdi::FrameRect(draw.hdc, &rect, border);
    if state & ctl::CDIS_FOCUS != 0 {
        gdi::InflateRect(&mut rect, -1, -1);
        gdi::FrameRect(draw.hdc, &rect, border);
    }
    gdi::DeleteObject(border);

    let cues = state & ctl::CDIS_SHOWKEYBOARDCUES != 0;
    draw_label(draw.hdr.hwndFrom, draw.hdc, rect, cues);
    if cues && state & ctl::CDIS_FOCUS != 0 {
        gdi::InflateRect(&mut rect, -2, -2);
        gdi::DrawFocusRect(draw.hdc, &rect);
    }
    ctl::CDRF_SKIPDEFAULT as isize
}

/// The button's own text in its own font, centred; the `&` mnemonic is
/// underlined only while keyboard cues show, as the theme would.
unsafe fn draw_label(
    button: windows_sys::Win32::Foundation::HWND,
    dc: gdi::HDC,
    mut rect: windows_sys::Win32::Foundation::RECT,
    cues: bool,
) {
    let mut text = [0u16; 64];
    let length = wm::GetWindowTextW(button, text.as_mut_ptr(), text.len() as i32);
    let font = wm::SendMessageW(button, wm::WM_GETFONT, 0, 0);
    let previous = gdi::SelectObject(dc, font as gdi::HGDIOBJ);
    gdi::SetBkMode(dc, gdi::TRANSPARENT as i32);
    gdi::SetTextColor(dc, gdi::GetSysColor(gdi::COLOR_BTNTEXT));
    let mut format = gdi::DT_CENTER | gdi::DT_VCENTER | gdi::DT_SINGLELINE;
    if !cues {
        format |= gdi::DT_HIDEPREFIX;
    }
    gdi::DrawTextW(dc, text.as_ptr(), length, &mut rect, format);
    gdi::SelectObject(dc, previous);
}

const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | (green as u32) << 8 | (blue as u32) << 16
}
