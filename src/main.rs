#![feature(new_uninit)]
#![feature(const_fn)]
#![feature(const_fn_floating_point_arithmetic)]
#![windows_subsystem = "console"]

mod editor;
mod renderer;
mod theme;
mod buffer;
mod settings;
mod language_support;
mod text_utils;
mod util;

use buffer::TextRange;
use editor::{Editor, EditorCommand};
use util::{pwstr_from_str, unwrap_hresult};

use std::{
    mem::MaybeUninit,
    ptr::null_mut
};

use bindings::{
    Windows::Win32::SystemServices::*,
    Windows::Win32::KeyboardAndMouseInput::*,
    Windows::Win32::Controls::*,
    Windows::Win32::WindowsAndMessaging::*,
    Windows::Win32::Debug::*,
    Windows::Win32::Gdi::*,
    Windows::Win32::MenusAndResources::*,
    Windows::Win32::HiDpi::*
};

fn low_word(i: i32) -> i32 {
    i & 0xFFFF
}
fn high_word(i: i32) -> i32 {
    i >> 16
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let editor: *mut Editor;
        if msg == WM_CREATE {
            let create = lparam.0 as *mut CREATESTRUCTW;
            let uninit_editor = (*create).lpCreateParams as *mut Box<MaybeUninit<Editor>>;

            // Create the editor, TODO: Error handle
            (*uninit_editor).as_mut_ptr().write(Editor::new(hwnd).unwrap());
            
            // Set the box to be carried over to subsequent callbacks
            SetWindowLongPtrW(hwnd, WINDOW_LONG_PTR_INDEX::GWLP_USERDATA, (*uninit_editor).as_mut_ptr() as isize);
            editor = (*uninit_editor).as_mut_ptr();

            (*editor).open_file("C:/Users/Rasmus/Desktop/Nimble/src/editor.rs");
            (*editor).draw();
        }
        else {
            editor = GetWindowLongPtrW(hwnd, WINDOW_LONG_PTR_INDEX::GWLP_USERDATA) as *mut Editor;
        }

        let shift_down = (GetKeyState(VK_SHIFT as i32) & 0x80) != 0;
        let ctrl_down = (GetKeyState(VK_CONTROL as i32) & 0x80) != 0;

        static mut MOUSE_FROM_OUTSIDE_WINDOW: bool = false;
        static mut CACHED_SELECTION_RANGE: TextRange = TextRange { start: 0, length: 0 }; 
        match msg {
            WM_PAINT => {
                let mut ps = MaybeUninit::<PAINTSTRUCT>::uninit();
                BeginPaint(hwnd, ps.as_mut_ptr());
                (*editor).draw();
                EndPaint(hwnd, ps.as_mut_ptr());
                LRESULT(0)
            }
            WM_ERASEBKGND => {
                LRESULT(0)
            }
            WM_SIZE => {
                // If the window is being minimized just return
                if wparam.0 == SIZE_MINIMIZED as usize {
                    return LRESULT(0);
                }
                let width = low_word(lparam.0 as i32);
                let height = high_word(lparam.0 as i32);
                (*editor).resize(width as u32, height as u32);
                InvalidateRect(hwnd, null_mut(), false);
                LRESULT(0)
            }
            WM_DESTROY | WM_NCDESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_CHAR => {
                if wparam.0 >= 0x20 && wparam.0 <= 0x7E {
                    (*editor).execute_command(&EditorCommand::CharInsert(wparam.0 as u16));
                }
                InvalidateRect(hwnd, null_mut(), false);
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                if high_word(wparam.0 as i32) > 0 {
                    (*editor).execute_command(&EditorCommand::ScrollUp(ctrl_down));
                }
                else {
                    (*editor).execute_command(&EditorCommand::ScrollDown(ctrl_down));
                }
                InvalidateRect(hwnd, null_mut(), false);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                SetCapture(hwnd);
                let mouse_pos = (low_word(lparam.0 as i32) as f32, high_word(lparam.0 as i32) as f32);
                (*editor).execute_command(&EditorCommand::LeftClick(mouse_pos, shift_down));
                InvalidateRect(hwnd, null_mut(), false);
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                let mouse_pos = (low_word(lparam.0 as i32) as f32, high_word(lparam.0 as i32) as f32);
                (*editor).execute_command(&EditorCommand::LeftDoubleClick(mouse_pos));
                InvalidateRect(hwnd, null_mut(), false);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                ReleaseCapture();
                (*editor).execute_command(&EditorCommand::LeftRelease);
                InvalidateRect(hwnd, null_mut(), false);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                (*editor).execute_command(&EditorCommand::KeyPressed(wparam.0 as u32, shift_down, ctrl_down));
                InvalidateRect(hwnd, null_mut(), false);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                // If the mouse came from outside the window,
                // track when the mouse leaves the window (and fires the WM_MOUSELEAVE event)
                if MOUSE_FROM_OUTSIDE_WINDOW {
                    let mut mouse_tracker = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TRACKMOUSEEVENT_dwFlags::TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: HOVER_DEFAULT
                    };
                    TrackMouseEvent(&mut mouse_tracker as *mut _);
                    MOUSE_FROM_OUTSIDE_WINDOW = false;
                }

                let mouse_pos = (low_word(lparam.0 as i32) as f32, high_word(lparam.0 as i32) as f32);
                (*editor).execute_command(&EditorCommand::MouseMove(mouse_pos));
                
                // Only invalidate if selection changes for performance reasons
                if let Some(selection) = (*editor).get_current_selection() {
                    if selection != CACHED_SELECTION_RANGE {
                        InvalidateRect(hwnd, null_mut(), false);
                        CACHED_SELECTION_RANGE = selection;
                    }
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                MOUSE_FROM_OUTSIDE_WINDOW = true;
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

fn main() {
    let mut editor = Box::<Editor>::new_uninit();

    unsafe {
        unwrap_hresult(SetProcessDpiAwareness(PROCESS_DPI_AWARENESS::PROCESS_PER_MONITOR_DPI_AWARE).ok());

        let wnd_class = WNDCLASSW {
            style: WNDCLASS_STYLES::CS_HREDRAW | WNDCLASS_STYLES::CS_VREDRAW | WNDCLASS_STYLES::CS_DBLCLKS,
            lpfnWndProc: Some(wnd_proc),
            lpszClassName: pwstr_from_str("Nimble_Class"),
            lpszMenuName: PWSTR::default(),
            hInstance: HINSTANCE(0),
            hIcon: HICON(0),
            hCursor: LoadCursorW(HINSTANCE(0), IDC_ARROW),
            hbrBackground: HBRUSH(GetStockObject(GetStockObject_iFlags::BLACK_BRUSH).0),
            cbClsExtra: 0,
            cbWndExtra: 0
        };

        let hr = RegisterClassW(&wnd_class);
        assert!(hr != 0, "Failed to register the window class, win32 error code: {}", hr);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            "Nimble_Class",
            "Nimble",
            WINDOW_STYLE::WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND(0),
            HMENU(0),
            HINSTANCE(0),
            (&mut editor as *mut _) as _
        );
        assert!(hwnd != HWND(0), "Failed to open window, win32 error code: {}", GetLastError());
        ShowWindow(hwnd, SHOW_WINDOW_CMD::SW_SHOW);

        let mut mouse_tracker = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TRACKMOUSEEVENT_dwFlags::TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: HOVER_DEFAULT
        };
        TrackMouseEvent(&mut mouse_tracker as *mut _);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND(0), 0, 0).0 > 0 {
            TranslateMessage(&mut msg);
            DispatchMessageW(&mut msg);
        }

        UnregisterClassW(pwstr_from_str("Nimble_Class"), HINSTANCE(0));
        DestroyWindow(hwnd);
    }
}
