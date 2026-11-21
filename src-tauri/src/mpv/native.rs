//! Native Windows video surface.
//!
//! mpv renders into a child HWND. While the player is focused it is kept
//! *below* the webview, whose transparent background lets the video show
//! through while DOM controls, popovers and the context menu stay on top. When
//! the window loses focus the HWND is moved above an opaque webview so the
//! whole WebView2 surface does not have to be alpha-composited every frame.

use crate::error::{AppError, Result};
use std::ffi::c_void;
use std::sync::{Once, OnceLock};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SetWindowPos, ShowWindow,
    HWND_BOTTOM, WNDCLASSW, SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
    SWP_NOSIZE, WS_CHILD, WS_VISIBLE,
};

fn class_name() -> &'static [u16] {
    static NAME: OnceLock<Vec<u16>> = OnceLock::new();
    NAME.get_or_init(|| "SlideUpVideoSurface\0".encode_utf16().collect())
}
static REGISTER: Once = Once::new();

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

pub struct VideoWindow {
    hwnd: HWND,
}

impl VideoWindow {
    pub fn create(parent: isize) -> Result<Self> {
        if parent == 0 {
            return Err(AppError::native("The application window is not available yet."));
        }

        REGISTER.call_once(|| unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name().as_ptr(),
            };
            RegisterClassW(&class);
        });

        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name().as_ptr(),
                std::ptr::null(),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                1,
                1,
                parent as HWND,
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            )
        };

        if hwnd.is_null() {
            return Err(AppError::native(
                "Could not create the native video surface. Video playback may be unavailable.",
            ));
        }

        Ok(Self { hwnd })
    }

    /// Raw HWND value passed to libmpv as the `wid` property.
    pub fn wid(&self) -> isize {
        self.hwnd as isize
    }

    pub fn set_position(&self, x: i32, y: i32, width: i32, height: i32) {
        if width <= 0 || height <= 0 {
            return;
        }
        unsafe {
            // Only change geometry here. Z-order is owned by the frontend's
            // focus handling (set_z_order); passing SWP_NOZORDER prevents a
            // resize from undoing the focused/unfocused layer mode.
            // No window region is applied: a region on a D3D-rendered child
            // window forces an extra DWM composition path and costs CPU/GPU.
            SetWindowPos(
                self.hwnd,
                0 as HWND,
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    /// Move the video surface above (`top`) or below (`bottom`) the webview
    /// without changing its position or size.
    pub fn set_z_order(&self, top: bool) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                if top { 0 as HWND } else { HWND_BOTTOM },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn hwnd_ptr(&self) -> *mut c_void {
        self.hwnd as *mut c_void
    }
}

impl Drop for VideoWindow {
    fn drop(&mut self) {
        if !self.hwnd.is_null() {
            unsafe {
                DestroyWindow(self.hwnd);
            }
        }
    }
}

unsafe impl Send for VideoWindow {}
unsafe impl Sync for VideoWindow {}
