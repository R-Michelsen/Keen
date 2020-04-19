use std::{
    ptr::null_mut,
    mem::MaybeUninit,
    ffi::OsStr,
    iter::once,
    os::windows::ffi::OsStrExt
};
use winapi::{
    um::{
        winuser::{
            GetDpiForWindow, GetClientRect, 
        },
        dcommon::{
            D2D1_ALPHA_MODE_UNKNOWN, D2D1_PIXEL_FORMAT
        },
        dwrite::{
            DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
            DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_WEIGHT_NORMAL, 
            DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
            DWRITE_TEXT_RANGE, DWRITE_HIT_TEST_METRICS
        },
        d2d1::{
            ID2D1Factory, ID2D1HwndRenderTarget, D2D1CreateFactory,
            ID2D1Brush, ID2D1SolidColorBrush, 
            D2D1_BRUSH_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
            D2D1_POINT_2F, D2D1_MATRIX_3X2_F, D2D1_SIZE_U, D2D1_RECT_F,
            D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FEATURE_LEVEL_DEFAULT,
            D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_USAGE_NONE,
            D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
            D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_ANTIALIAS_MODE_ALIASED,
            D2D1_ANTIALIAS_MODE_PER_PRIMITIVE
        },
        unknwnbase::IUnknown
    },
    shared::{
        d3d9types::D3DCOLORVALUE,
        dxgiformat::DXGI_FORMAT_UNKNOWN,
        windef::{
            RECT, HWND
        }
    },
    ctypes::c_void,
    Interface
};

use crate::buffer::TextBuffer;

#[macro_export]
#[cfg(debug_assertions)]
macro_rules! dx_ok {
    ($e:expr) => {
        assert!($e == 0, "DirectX call returned error code: 0x{:x}", $e as u32)
    }
}

#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! dx_ok {
    ($e:expr) => {
        std::convert::identity($e)
    }
}

const IDENTITY_MATRIX: D2D1_MATRIX_3X2_F = D2D1_MATRIX_3X2_F { matrix: [[1.0, 0.0], [0.0, 1.0], [0.0, 0.0]] };
const BACKGROUND_COLOR: D3DCOLORVALUE = D3DCOLORVALUE {
    r: 0.13, g: 0.13, b: 0.13, a: 1.0
};
const TEXT_COLOR: D3DCOLORVALUE = D3DCOLORVALUE {
    r: 1.0, g: 0.95, b: 0.80, a: 1.0
};
const CARET_COLOR: D3DCOLORVALUE = D3DCOLORVALUE {
    r: 1.0, g: 0.95, b: 0.89, a: 1.0
};
const SELECTION_COLOR: D3DCOLORVALUE = D3DCOLORVALUE {
    r: 0.45, g: 0.76, b: 0.98, a: 1.0
};

struct Brushes {
    text: *mut ID2D1SolidColorBrush,
    caret: *mut ID2D1SolidColorBrush,
    selection: *mut ID2D1SolidColorBrush
}

pub struct TextRenderer {
    brushes: Brushes,

    dpi_scale: f32,
    pixel_size: D2D1_SIZE_U,

    factory: *mut ID2D1Factory,
    target: *mut ID2D1HwndRenderTarget,

    pub write_factory: *mut IDWriteFactory,
    pub text_format: *mut IDWriteTextFormat,
}

impl TextRenderer {
    pub fn new(hwnd: HWND) -> TextRenderer {
        let mut renderer = TextRenderer {
            factory: null_mut(),
            target: null_mut(),

            write_factory: null_mut(),
            text_format: null_mut(),

            brushes: Brushes {
                text: null_mut(),
                caret: null_mut(),
                selection: null_mut()
            },

            dpi_scale: 0.0,
            pixel_size: D2D1_SIZE_U {
                width: 0,
                height: 0
            }
        };

        unsafe {
            dx_ok!(
                D2D1CreateFactory(
                    D2D1_FACTORY_TYPE_SINGLE_THREADED, 
                    &ID2D1Factory::uuidof(), null_mut(), 
                    (&mut renderer.factory as *mut *mut _) as *mut *mut c_void
                )
            );

            let mut rect_uninit = MaybeUninit::<RECT>::uninit();
            GetClientRect(hwnd, rect_uninit.as_mut_ptr());
            let rect = rect_uninit.assume_init();
            renderer.pixel_size = D2D1_SIZE_U {
                width: (rect.right - rect.left) as u32,
                height: (rect.bottom - rect.top) as u32
            };

            let dpi = GetDpiForWindow(hwnd);
            renderer.dpi_scale = (dpi as f32) / 96.0;

            let target_props = D2D1_RENDER_TARGET_PROPERTIES {
                _type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_UNKNOWN,
                    alphaMode: D2D1_ALPHA_MODE_UNKNOWN
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT
            };

            let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: renderer.pixel_size,
                presentOptions: D2D1_PRESENT_OPTIONS_NONE
            };

            dx_ok!((*renderer.factory).CreateHwndRenderTarget(&target_props, &hwnd_props, &mut renderer.target as *mut *mut _)); 

            let brush_properties = D2D1_BRUSH_PROPERTIES {
                opacity: 1.0,
                transform: IDENTITY_MATRIX
            };

            dx_ok!((*renderer.target).CreateSolidColorBrush(&TEXT_COLOR, &brush_properties, &mut renderer.brushes.text as *mut *mut _));
            dx_ok!((*renderer.target).CreateSolidColorBrush(&CARET_COLOR, &brush_properties, &mut renderer.brushes.caret as *mut *mut _));
            dx_ok!((*renderer.target).CreateSolidColorBrush(&SELECTION_COLOR, &brush_properties, &mut renderer.brushes.selection as *mut *mut _));


            dx_ok!(
                DWriteCreateFactory(
                    DWRITE_FACTORY_TYPE_SHARED, 
                    &IDWriteFactory::uuidof(), 
                    (&mut renderer.write_factory as *mut *mut _) as *mut *mut IUnknown
                )
            );

            let font_name: Vec<u16> = OsStr::new("Fira Code Retina").encode_wide().chain(once(0)).collect();
            let locale: Vec<u16> = OsStr::new("en-dk").encode_wide().chain(once(0)).collect();

            dx_ok!((*renderer.write_factory).CreateTextFormat(
                    font_name.as_ptr(),
                    null_mut(),
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    30.0,
                    locale.as_ptr(),
                    &mut renderer.text_format as *mut *mut _
            ));

            dx_ok!((*renderer.text_format).SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING));
            dx_ok!((*renderer.text_format).SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR));
        }

        renderer
    }

    fn draw_selection_range(&self, text_layout: *mut IDWriteTextLayout, range: DWRITE_TEXT_RANGE) {
        let mut hit_test_count = 0;

        unsafe {
            let hr: i32 = (*text_layout).HitTestTextRange(
                        range.startPosition, 
                        range.length,
                        0.0,
                        0.0,
                        null_mut(),
                        0,
                        &mut hit_test_count
                    );
            assert!((hr as u32) == 0x8007007A, "HRESULT in this case is expected to error with \"ERROR_INSUFFICIENT_BUFFER\""); 

            let mut hit_tests : Vec<DWRITE_HIT_TEST_METRICS> = Vec::with_capacity(hit_test_count as usize);
            hit_tests.set_len(hit_test_count as usize);

            dx_ok!(
                (*text_layout).HitTestTextRange(
                    range.startPosition,
                    range.length,
                    0.0,
                    0.0,
                    hit_tests.as_mut_ptr(),
                    hit_tests.len() as u32,
                    &mut hit_test_count
                )
            );

            (*self.target).SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
            hit_tests.iter().for_each(|metrics| {

                let highlight_rect = D2D1_RECT_F {
                    left: metrics.left,
                    top: metrics.top,
                    right: metrics.left + metrics.width,
                    bottom: metrics.top + metrics.height
                };

                (*self.target).FillRectangle(&highlight_rect, self.brushes.selection as *mut ID2D1Brush);

            });
            (*self.target).SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);

        }

    }

    pub fn draw(&self, text_buffer: &mut TextBuffer, draw_caret: bool) {
        unsafe {
            (*self.target).BeginDraw();

            (*self.target).SetTransform(&IDENTITY_MATRIX);
            (*self.target).Clear(&BACKGROUND_COLOR);

            let text_layout: *mut IDWriteTextLayout = 
                text_buffer.get_layout((self.pixel_size.width as f32, self.pixel_size.height as f32));

            if let Some(selection_range) = text_buffer.get_selection_range() {
                self.draw_selection_range(text_layout, selection_range);
            }

            (*self.target).DrawTextLayout(
                D2D1_POINT_2F { x: 0.0, y: 0.0 },
                text_layout,
                self.brushes.text as *mut ID2D1Brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE
            );

            if draw_caret {
                if let Some(rect) = text_buffer.get_caret_rect() {
                    (*self.target).SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
                    (*self.target).FillRectangle(&rect, self.brushes.caret as *mut ID2D1Brush);
                    (*self.target).SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                }
            }

            (*self.target).EndDraw(null_mut(), null_mut());
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.pixel_size.width = width;
        self.pixel_size.height = height;
        unsafe {
            (*self.target).Resize(&self.pixel_size);
        }
    }
}
