#![cfg(windows)]

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED},
        UI::{
            Shell::{
                ITaskbarList3, TaskbarList, THUMBBUTTON, THUMBBUTTONFLAGS,
                THB_FLAGS, THB_ICON, THB_TOOLTIP,
            },
            WindowsAndMessaging::{
                CallWindowProcW, LoadIconW, RegisterWindowMessageW, SetWindowLongPtrW,
                GWLP_WNDPROC, IDI_APPLICATION, WM_COMMAND, WNDPROC,
            },
        },
    },
};

const BTN_PREV: u32 = 1001;
const BTN_PLAY_PAUSE: u32 = 1002;
const BTN_NEXT: u32 = 1003;
const BTN_EXIT: u32 = 1004;

const THBN_CLICKED: u32 = 0x1800;

static ORIG_WNDPROC: Mutex<WNDPROC> = Mutex::new(None);
static APP_HANDLE: Mutex<Option<AppHandle>> = Mutex::new(None);
static TASKBAR_MSG: AtomicU32 = AtomicU32::new(0);
static BUTTONS_ADDED: AtomicBool = AtomicBool::new(false);

pub fn setup_taskbar_buttons(window: &WebviewWindow) {
    let Ok(raw_hwnd) = window.hwnd() else { return };
    let hwnd = HWND(raw_hwnd.0 as _);

    if let Ok(mut guard) = APP_HANDLE.lock() {
        *guard = Some(window.app_handle().clone());
    }

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        // Register the "TaskbarButtonCreated" message
        let msg_name: Vec<u16> = "TaskbarButtonCreated\0".encode_utf16().collect();
        let msg = RegisterWindowMessageW(windows::core::PCWSTR(msg_name.as_ptr()));
        TASKBAR_MSG.store(msg, Ordering::SeqCst);

        // Hook window procedure so we can intercept button clicks and TaskbarButtonCreated
        let prev_proc = SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            custom_wndproc as *const () as isize,
        );
        if prev_proc != 0 {
            if let Ok(mut guard) = ORIG_WNDPROC.lock() {
                *guard = std::mem::transmute(prev_proc);
            }
        }

        add_buttons(hwnd);
    }
}

unsafe fn add_buttons(hwnd: HWND) {
    if BUTTONS_ADDED.load(Ordering::SeqCst) {
        return;
    }
    unsafe {
        let Ok(taskbar) = CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_INPROC_SERVER) else {
            return;
        };
        if taskbar.HrInit().is_err() {
            return;
        }

        let Ok(default_icon) = LoadIconW(None, IDI_APPLICATION) else {
            return;
        };

        let buttons = [
            create_button(BTN_PREV, "Previous track", default_icon),
            create_button(BTN_PLAY_PAUSE, "Play / Pause", default_icon),
            create_button(BTN_NEXT, "Next track", default_icon),
            create_button(BTN_EXIT, "Exit Slide Up", default_icon),
        ];

        if taskbar.ThumbBarAddButtons(hwnd, &buttons).is_ok() {
            BUTTONS_ADDED.store(true, Ordering::SeqCst);
        }
    }
}

fn create_button(
    id: u32,
    tooltip: &str,
    icon: windows::Win32::UI::WindowsAndMessaging::HICON,
) -> THUMBBUTTON {
    let mut btn = THUMBBUTTON {
        dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
        iId: id,
        iBitmap: 0,
        hIcon: icon,
        szTip: [0; 260],
        dwFlags: THUMBBUTTONFLAGS(0),
    };

    let wide: Vec<u16> = tooltip.encode_utf16().collect();
    let len = wide.len().min(259);
    btn.szTip[..len].copy_from_slice(&wide[..len]);
    btn
}

unsafe extern "system" fn custom_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let taskbar_msg = TASKBAR_MSG.load(Ordering::SeqCst);
    if taskbar_msg != 0 && msg == taskbar_msg {
        unsafe {
            add_buttons(hwnd);
        }
        return LRESULT(0);
    }

    if msg == WM_COMMAND {
        let hiword = ((wparam.0 >> 16) & 0xFFFF) as u32;
        let btn_id = (wparam.0 & 0xFFFF) as u32;

        if hiword == THBN_CLICKED {
            if let Ok(guard) = APP_HANDLE.lock() {
                if let Some(app) = guard.as_ref() {
                    match btn_id {
                        BTN_PREV => {
                            let _ = app.emit("taskbar-action", "previous");
                        }
                        BTN_PLAY_PAUSE => {
                            let _ = app.emit("taskbar-action", "play_pause");
                        }
                        BTN_NEXT => {
                            let _ = app.emit("taskbar-action", "next");
                        }
                        BTN_EXIT => {
                            for (_, window) in app.webview_windows() {
                                let _ = window.destroy();
                            }
                            app.exit(0);
                        }
                        _ => {}
                    }
                }
            }
            return LRESULT(0);
        }
    }

    let orig = {
        if let Ok(guard) = ORIG_WNDPROC.lock() {
            *guard
        } else {
            None
        }
    };
    unsafe { CallWindowProcW(orig, hwnd, msg, wparam, lparam) }
}
