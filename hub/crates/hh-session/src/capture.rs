//! Native GDI desktop capture. Handles stay on the capture thread and are
//! released on every error/shutdown. Secure desktops fail instead of reuse.
use std::{ffi::c_void, ptr};
use crate::enc::RgbaFrame;
type Handle = *mut c_void;
#[repr(C)]
struct BitmapInfoHeader { size: u32, width: i32, height: i32, planes: u16, bits: u16, compression: u32, image_size: u32, xppm: i32, yppm: i32, used: u32, important: u32 }
#[repr(C)]
struct BitmapInfo { header: BitmapInfoHeader, color: u32 }
#[link(name = "user32")]
extern "system" {
    fn GetDC(window: Handle) -> Handle;
    fn ReleaseDC(window: Handle, dc: Handle) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn OpenInputDesktop(flags: u32, inherit: i32, access: u32) -> Handle;
    fn CloseDesktop(desktop: Handle) -> i32;
    fn GetUserObjectInformationW(object: Handle, index: i32, info: Handle, len: u32, needed: *mut u32) -> i32;
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(dc: Handle) -> Handle;
    fn CreateCompatibleBitmap(dc: Handle, width: i32, height: i32) -> Handle;
    fn SelectObject(dc: Handle, object: Handle) -> Handle;
    fn DeleteObject(object: Handle) -> i32;
    fn DeleteDC(dc: Handle) -> i32;
    fn StretchBlt(dst: Handle, x: i32, y: i32, w: i32, h: i32, src: Handle, sx: i32, sy: i32, sw: i32, sh: i32, operation: u32) -> i32;
    fn GetDIBits(dc: Handle, bitmap: Handle, start: u32, lines: u32, data: Handle, info: *mut BitmapInfo, usage: u32) -> i32;
    fn SetStretchBltMode(dc: Handle, mode: i32) -> i32;
}
pub(crate) fn desktop_available() -> bool {
    unsafe {
        let desktop = OpenInputDesktop(0, 0, 1); // DESKTOP_READOBJECTS
        if desktop.is_null() { return false; }
        let mut name = [0u16; 256];
        let mut needed = 0;
        let ok = GetUserObjectInformationW(desktop, 2, name.as_mut_ptr().cast(), 512, &mut needed) != 0;
        CloseDesktop(desktop);
        ok && String::from_utf16_lossy(&name[..name.iter().position(|c| *c == 0).unwrap_or(name.len())]).eq_ignore_ascii_case("Default")
    }
}
pub(crate) fn primary_size() -> (usize, usize) {
    unsafe { (GetSystemMetrics(0).clamp(320, 8192) as usize, GetSystemMetrics(1).clamp(240, 8192) as usize) }
}
pub struct DesktopCapture { screen: Handle, memory: Handle, bitmap: Handle, width: usize, height: usize }
impl DesktopCapture {
    pub fn new(max_width: usize, max_height: usize) -> Result<Self, String> {
        if !desktop_available() { return Err("interactive desktop unavailable or locked".into()); }
        unsafe {
            // Primary monitor only; avoid an unbounded multi-monitor canvas.
            let source_width = GetSystemMetrics(0);
            let source_height = GetSystemMetrics(1);
            if source_width < 16 || source_height < 16 { return Err("desktop dimensions unavailable".into()); }
            let ratio = (max_width as f64 / source_width as f64).min(max_height as f64 / source_height as f64).min(1.0);
            let width = ((source_width as f64 * ratio) as usize / 2 * 2).max(16);
            let height = ((source_height as f64 * ratio) as usize / 2 * 2).max(16);
            let screen = GetDC(ptr::null_mut());
            if screen.is_null() { return Err("desktop DC unavailable".into()); }
            let memory = CreateCompatibleDC(screen);
            if memory.is_null() { ReleaseDC(ptr::null_mut(), screen); return Err("capture DC unavailable".into()); }
            let bitmap = CreateCompatibleBitmap(screen, width as i32, height as i32);
            if bitmap.is_null() { DeleteDC(memory); ReleaseDC(ptr::null_mut(), screen); return Err("capture bitmap unavailable".into()); }
            SetStretchBltMode(memory, 4); // HALFTONE
            Ok(Self { screen, memory, bitmap, width, height })
        }
    }
    pub fn frame(&mut self) -> Result<RgbaFrame, String> {
        if !desktop_available() { return Err("desktop locked or unavailable".into()); }
        let mut frame = RgbaFrame::new(self.width, self.height);
        unsafe {
            let original = SelectObject(self.memory, self.bitmap);
            if original.is_null() || original as isize == -1 { return Err("capture selection failed".into()); }
            let copied = StretchBlt(self.memory, 0, 0, self.width as i32, self.height as i32,
                self.screen, 0, 0, GetSystemMetrics(0), GetSystemMetrics(1), 0x40cc0020); // SRCCOPY | CAPTUREBLT
            // GetDIBits requires the bitmap to be deselected.
            SelectObject(self.memory, original);
            if copied == 0 { return Err("desktop capture failed".into()); }
            let mut info = BitmapInfo { header: BitmapInfoHeader { size: 40, width: self.width as i32, height: -(self.height as i32), planes: 1, bits: 32, compression: 0, image_size: 0, xppm: 0, yppm: 0, used: 0, important: 0 }, color: 0 };
            if GetDIBits(self.memory, self.bitmap, 0, self.height as u32, frame.data.as_mut_ptr().cast(), &mut info, 0) != self.height as i32 {
                return Err("desktop pixels unavailable".into());
            }
        }
        for pixel in frame.data.chunks_exact_mut(4) { pixel.swap(0, 2); pixel[3] = 255; }
        Ok(frame)
    }
}
impl Drop for DesktopCapture {
    fn drop(&mut self) { unsafe { DeleteObject(self.bitmap); DeleteDC(self.memory); ReleaseDC(ptr::null_mut(), self.screen); } }
}
