fn main() {
    windows::build!(
        Windows::Win32::SystemServices::{
            GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock, GlobalSize, 
            LRESULT, HINSTANCE, DPI_AWARENESS_CONTEXT, GlobalAlloc_uFlags,
            CLIPBOARD_FORMATS
        },
        Windows::Win32::DataExchange::{
            OpenClipboard, CloseClipboard, EmptyClipboard, GetClipboardData, 
            SetClipboardData
        },
        Windows::Win32::KeyboardAndMouseInput::{
            SetCapture, ReleaseCapture, GetKeyState, TrackMouseEvent,
            TRACKMOUSEEVENT, TRACKMOUSEEVENT_dwFlags
        },
        Windows::Win32::Controls::{
            WM_MOUSELEAVE, HOVER_DEFAULT
        },
        Windows::Win32::WindowsAndMessaging::{
            SetWindowLongPtrW, GetWindowLongPtrW,
            UnregisterClassW, DispatchMessageW,
            TranslateMessage, GetMessageW,
            ShowWindow, CreateWindowExW, PostQuitMessage,
            DefWindowProcW, RegisterClassW, LoadCursorW,
            DestroyWindow, GetClientRect, SystemParametersInfoW,
            CW_USEDEFAULT, MSG, IDC_ARROW,
            WM_PAINT, WM_SIZE, WM_DESTROY, WM_CHAR, HWND,
            WM_MOUSEWHEEL, WM_LBUTTONDOWN, WM_ERASEBKGND,
            WM_LBUTTONUP, WM_KEYDOWN, VK_SHIFT, VK_CONTROL,
            WM_CREATE, CREATESTRUCTW, WINDOW_LONG_PTR_INDEX,
            WM_MOUSEMOVE, WM_NCDESTROY, SHOW_WINDOW_CMD, WM_LBUTTONDBLCLK,
            WINDOW_STYLE, WNDCLASS_STYLES, WNDCLASSW, SIZE_MINIMIZED, 
            WPARAM, LPARAM, SYSTEM_PARAMETERS_INFO_ACTION, VK_LEFT, VK_RIGHT, 
            VK_UP, VK_DOWN, VK_TAB, VK_RETURN, VK_DELETE, VK_BACK
        },
        Windows::Win32::Debug::GetLastError,
        Windows::Win32::Gdi::{
            GetStockObject, BeginPaint, EndPaint, InvalidateRect,
            GetStockObject_iFlags, HBRUSH, PAINTSTRUCT
        },
        Windows::Win32::Dxgi::DXGI_FORMAT,
        Windows::Win32::MenusAndResources::{HMENU, HICON},
        Windows::Win32::HiDpi::{GetDpiForWindow, SetProcessDpiAwareness, PROCESS_DPI_AWARENESS},
        Windows::Win32::SystemServices::{LRESULT, HINSTANCE, PWSTR},
        Windows::Win32::DisplayDevices::RECT,
        Windows::Win32::DirectWrite::{
            DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, 
            IDWriteTextLayout, IDWriteFontCollection, DWRITE_WORD_WRAPPING,
            DWRITE_FACTORY_TYPE, DWRITE_FONT_WEIGHT,
            DWRITE_FONT_STYLE, DWRITE_FONT_STRETCH,
            DWRITE_TEXT_ALIGNMENT, DWRITE_PARAGRAPH_ALIGNMENT,
            DWRITE_TEXT_RANGE, DWRITE_HIT_TEST_METRICS,
            DWRITE_LINE_SPACING
        },
        Windows::Foundation::Numerics::Matrix3x2,
        Windows::Win32::Direct2D::{
            ID2D1Factory, ID2D1HwndRenderTarget, D2D1CreateFactory,
            ID2D1Brush, ID2D1SolidColorBrush, ID2D1StrokeStyle,
            D2D1_COLOR_F, D2D1_PRESENT_OPTIONS, D2D1_ROUNDED_RECT, 
            D2D_POINT_2F, D2D_SIZE_U, D2D_RECT_F, D2D1_DRAW_TEXT_OPTIONS, 
            D2D1_FEATURE_LEVEL, D2D1_BRUSH_PROPERTIES, 
            D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_USAGE,
            D2D1_RENDER_TARGET_TYPE, D2D1_RENDER_TARGET_PROPERTIES,
            D2D1_FACTORY_TYPE, D2D1_ANTIALIAS_MODE, D2D1_ANTIALIAS_MODE
        }
    );
}