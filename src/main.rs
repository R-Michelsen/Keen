#![feature(new_uninit)]

#![windows_subsystem = "console"]

mod editor;
mod renderer;
mod buffer;

use std::{
    ffi::OsStr,
    mem::MaybeUninit,
    os::windows::ffi::OsStrExt,
    iter::once,
    ptr::null_mut,
    time::Instant
};

use winapi::{
    um::{
        winuser::{
            SetWindowLongPtrW, GetWindowLongPtrW,
            UnregisterClassW, DispatchMessageW,
            TranslateMessage, PeekMessageW, SendMessageW, 
            ShowWindow, CreateWindowExW, PM_REMOVE,
            SetProcessDpiAwarenessContext, PostQuitMessage,
            DefWindowProcW, RegisterClassW, LoadCursorW, 
            BeginPaint, EndPaint, GET_WHEEL_DELTA_WPARAM,
            CW_USEDEFAULT, MSG, IDC_ARROW, GetKeyState,
            WM_PAINT, WM_SIZE, WM_DESTROY, WM_CHAR,
            WM_MOUSEWHEEL, WM_LBUTTONDOWN, WM_ERASEBKGND, 
            WM_LBUTTONUP, WM_KEYDOWN, VK_SHIFT, VK_CONTROL,
            WM_CREATE, CREATESTRUCTW, GWLP_USERDATA, 
            WM_MOUSEMOVE, WM_QUIT, SW_SHOW,
            WS_OVERLAPPEDWINDOW, CS_HREDRAW, CS_VREDRAW,
            WNDCLASSW, PAINTSTRUCT, InvalidateRect 
        },
        errhandlingapi::GetLastError,
        wingdi::{GetStockObject, BLACK_BRUSH}
    },
    shared::{
        windef::{
            HWND, HMENU, HBRUSH, HICON,
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
        },
        minwindef::{
            WPARAM, LPARAM, LRESULT, HINSTANCE,
            LOWORD, HIWORD
        },
        ntdef::LPCWSTR,
        windowsx::{GET_X_LPARAM, GET_Y_LPARAM}
    },
    ctypes::c_void
};

use editor::{Editor, EditorCommand, EditorCommandData};

const WM_CARET_VISIBLE: u32 = 0xC000;
const WM_CARET_INVISIBLE: u32 = 0xC001;

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let editor: *mut Editor;
    if msg == WM_CREATE {
        let create = lparam as *mut CREATESTRUCTW;
        let uninit_editor = (*create).lpCreateParams as *mut Box<MaybeUninit<Editor>>;
        
        // Create the editor
        (*uninit_editor).as_mut_ptr().write(Editor::new(hwnd));
        
        // Set the box to be carried over to subsequent callbacks
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*uninit_editor).as_mut_ptr() as isize);
        editor = (*uninit_editor).as_mut_ptr();
        (*editor).open_file("C:/llvm-project/clang/lib/CodeGen/CGBuiltin.cpp");
    }
    else {
        editor = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Editor;
    }

    let shift = (GetKeyState(VK_SHIFT) & 0x80) != 0;
    let ctrl = (GetKeyState(VK_CONTROL) & 0x80) != 0;

    match msg {
        WM_CARET_VISIBLE => {
            (*editor).caret_is_visible = true;
            (*editor).draw();
            return 0;
        },
        WM_CARET_INVISIBLE => {
            (*editor).caret_is_visible = false;
            (*editor).draw();
            return 0;
        },
        WM_PAINT => {
            let mut ps = MaybeUninit::<PAINTSTRUCT>::uninit();
            BeginPaint(hwnd, ps.as_mut_ptr());
            (*editor).draw();
            EndPaint(hwnd, ps.as_mut_ptr());
            return 0;
        },
        WM_ERASEBKGND => {
            return 0;
        }
        WM_SIZE => {
            let width = LOWORD(lparam as u32);
            let height = HIWORD(lparam as u32);
            (*editor).resize(width.into(), height.into());
            return 0;
        },
        WM_DESTROY => {
            PostQuitMessage(0);
            return 0;
        },
        WM_CHAR => {
            let b = [wparam as u16];
            let a = String::from_utf16(&b);
            println!("Received input: {:?}", a);
            return 0;
        },
        WM_MOUSEWHEEL => {
            if GET_WHEEL_DELTA_WPARAM(wparam) > 0 {
                (*editor).execute_command(EditorCommand::ScrollUp, EditorCommandData { dummy: false });
            }
            else {
                (*editor).execute_command(EditorCommand::ScrollDown, EditorCommandData { dummy: false });
            }
            InvalidateRect(hwnd, null_mut(), false as i32);
            return 0;
        },
        WM_LBUTTONDOWN => {
            let mouse_pos = (GET_X_LPARAM(lparam) as f32, GET_Y_LPARAM(lparam) as f32);
            (*editor).execute_command(EditorCommand::LeftClick, EditorCommandData { mouse_pos_shift: (mouse_pos, shift) });
            InvalidateRect(hwnd, null_mut(), false as i32);
            return 0;
        },
        WM_LBUTTONUP => {
            let mouse_pos = (GET_X_LPARAM(lparam) as f32, GET_Y_LPARAM(lparam) as f32);
            (*editor).execute_command(EditorCommand::LeftRelease, EditorCommandData { mouse_pos_shift: (mouse_pos, shift) });
            InvalidateRect(hwnd, null_mut(), false as i32);
            return 0;
        },
        WM_KEYDOWN => {
            (*editor).execute_command(EditorCommand::KeyPressed, EditorCommandData { key_shift_ctrl: (wparam as i32, shift, ctrl) });
            InvalidateRect(hwnd, null_mut(), false as i32);
            return 0;
        },
        WM_MOUSEMOVE => {
            let mouse_pos = (GET_X_LPARAM(lparam) as f32, GET_Y_LPARAM(lparam) as f32);
            (*editor).execute_command(EditorCommand::MouseMove, EditorCommandData { mouse_pos_shift: (mouse_pos, shift) });
            if (*editor).selection_active() {
                InvalidateRect(hwnd, null_mut(), false as i32);
            }
            return 0;
        },
        _ => DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

fn main() {
    let mut editor = Box::<Editor>::new_uninit();

    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let wnd_name: Vec<u16> = OsStr::new("Keen").encode_wide().chain(once(0)).collect();
        let wnd_class_name: Vec<u16> = OsStr::new("Keen_Class").encode_wide().chain(once(0)).collect();

        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            lpszClassName: wnd_class_name.as_ptr(),
            lpszMenuName: 0 as LPCWSTR,
            hInstance: 0 as HINSTANCE,
            hIcon: 0 as HICON,
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: GetStockObject(BLACK_BRUSH as i32) as HBRUSH,
            cbClsExtra: 0,
            cbWndExtra: 0
        };

        let hr = RegisterClassW(&wnd_class);
        assert!(hr != 0, "Failed to register the window class, win32 error code: {}", hr);

        let hwnd = CreateWindowExW(
            0,
            wnd_class_name.as_ptr(),
            wnd_name.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0 as HWND,
            0 as HMENU,
            0 as HINSTANCE,
            (&mut editor as *mut _) as *mut c_void
        );
        assert!(hwnd != (0 as HWND), "Failed to open window, win32 error code: {}", GetLastError());

        ShowWindow(hwnd, SW_SHOW);

        let mut msg = MaybeUninit::<MSG>::uninit();

        let mut start = Instant::now();
        let mut message_idx: u32 = 0;
        
        'outer: loop {
            let current = Instant::now();

            if (current - start).as_millis() > 500 {
                SendMessageW(hwnd, WM_CARET_VISIBLE + message_idx, 0, 0);
                message_idx ^= 1;
                start = current;
            }

            while PeekMessageW(msg.as_mut_ptr(), 0 as HWND, 0, 0, PM_REMOVE) > 0 {
                TranslateMessage(msg.as_mut_ptr());
                DispatchMessageW(msg.as_mut_ptr());

                match (*msg.as_ptr()).message {
                    WM_QUIT => break 'outer,
                    WM_LBUTTONDOWN | WM_KEYDOWN => {
                        SendMessageW(hwnd, WM_CARET_VISIBLE, 0, 0);
                        message_idx = 0;
                        start = current;
                    },
                    _ => {}
                }
            }
        }

        UnregisterClassW(wnd_class_name.as_ptr(), 0 as HINSTANCE);
    }
}
