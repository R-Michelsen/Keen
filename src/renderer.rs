use crate::{
    settings,
    buffer::TextBuffer,
    theme::Theme,
    language_support::SemanticTokenTypes
};

use std::{
    collections::HashMap,
    ptr::null_mut,
    mem::MaybeUninit,
    ffi::OsStr,
    iter::once,
    os::windows::ffi::OsStrExt
};
use winapi::{
    ctypes::c_void,
    Interface,
    um::{
        dcommon::{D2D1_ALPHA_MODE_UNKNOWN, D2D1_PIXEL_FORMAT},
        dwrite::{
            DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, 
            IDWriteTextLayout, DWRITE_WORD_WRAPPING_NO_WRAP,
            DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
            DWRITE_TEXT_RANGE, DWRITE_HIT_TEST_METRICS
        },
        d2d1::{
            ID2D1Factory, ID2D1HwndRenderTarget, D2D1CreateFactory,
            ID2D1Brush, 
            D2D1_PRESENT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
            D2D1_POINT_2F, D2D1_MATRIX_3X2_F, D2D1_SIZE_U, D2D1_RECT_F,
            D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FEATURE_LEVEL_DEFAULT,
            D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_USAGE_NONE,
            D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
            D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_ANTIALIAS_MODE_ALIASED,
            D2D1_ANTIALIAS_MODE_PER_PRIMITIVE
        },
        unknwnbase::IUnknown,
        winuser::{SystemParametersInfoW, SPI_GETCARETWIDTH, GetClientRect, GetDpiForWindow}
    },
    shared::{
        dxgiformat::DXGI_FORMAT_UNKNOWN,
        windef::{RECT, HWND}
    }
};

#[macro_export]
#[cfg(debug_assertions)]
macro_rules! hr_ok {
    ($e:expr) => {
        assert!($e == 0, "Call returned error code HRESULT: 0x{:x}", $e as u32)
    }
}

#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! hr_ok {
    ($e:expr) => {
        std::convert::identity($e)
    }
}

const IDENTITY_MATRIX: D2D1_MATRIX_3X2_F = D2D1_MATRIX_3X2_F { matrix: [[1.0, 0.0], [0.0, 1.0], [0.0, 0.0]] };

pub struct TextLayout {
    origin: (f32, f32),
    extents: (f32, f32),
    layout: *mut IDWriteTextLayout
}

pub struct TextRenderer {
    dpi_scale: f32,
    pub pixel_size: D2D1_SIZE_U,
    pub font_size: f32,
    pub font_height: f32,
    pub font_width: f32,
    font_name: Vec<u16>,
    font_locale: Vec<u16>,

    caret_width: usize,

    theme: Theme,

    write_factory: *mut IDWriteFactory,
    text_format: *mut IDWriteTextFormat,
    
    factory: *mut ID2D1Factory,
    target: *mut ID2D1HwndRenderTarget,

    buffer_layouts: HashMap<String, TextLayout>,
    buffer_line_number_layouts: HashMap<String, TextLayout>
}

impl TextRenderer {
    pub fn new(hwnd: HWND, font: &str, font_size: f32) -> Self {
        let mut renderer = Self {
            dpi_scale: 0.0,
            pixel_size: D2D1_SIZE_U {
                width: 0,
                height: 0
            },
            font_size,
            font_height: 0.0,
            font_width: 0.0,
            font_name: Vec::new(),
            font_locale: Vec::new(),

            caret_width: 0,

            theme: Theme::default(),
                
            write_factory: null_mut(),
            text_format: null_mut(),

            factory: null_mut(),
            target: null_mut(),

            buffer_layouts: HashMap::new(),
            buffer_line_number_layouts: HashMap::new()
        };

        unsafe {
            // We'll increase the width from the system width slightly
            SystemParametersInfoW(SPI_GETCARETWIDTH, 0, (&mut renderer.caret_width as *mut _) as *mut c_void, 0);
            renderer.caret_width *= 2;

            hr_ok!(
                D2D1CreateFactory(
                    D2D1_FACTORY_TYPE_SINGLE_THREADED, 
                    &ID2D1Factory::uuidof(), null_mut(), 
                    (&mut renderer.factory as *mut *mut _) as *mut *mut c_void
                )
            );

            let dpi = GetDpiForWindow(hwnd);
            renderer.dpi_scale = dpi as f32 / 96.0;

            // Scale the font size to fit the dpi
            renderer.font_size *= renderer.dpi_scale;

            let mut rect_uninit = MaybeUninit::<RECT>::uninit();
            GetClientRect(hwnd, rect_uninit.as_mut_ptr());
            let rect = rect_uninit.assume_init();
            renderer.pixel_size = D2D1_SIZE_U {
                width: (rect.right - rect.left) as u32,
                height: (rect.bottom - rect.top) as u32
            };

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

            hr_ok!((*renderer.factory).CreateHwndRenderTarget(&target_props, &hwnd_props, &mut renderer.target)); 

            renderer.theme = Theme::new_default(renderer.target);

            hr_ok!(
                DWriteCreateFactory(
                    DWRITE_FACTORY_TYPE_SHARED, 
                    &IDWriteFactory::uuidof(), 
                    (&mut renderer.write_factory as *mut *mut _) as *mut *mut IUnknown
                )
            );

            renderer.font_name = OsStr::new(font).encode_wide().chain(once(0)).collect();
            renderer.font_locale = OsStr::new("en-us").encode_wide().chain(once(0)).collect();
            
            renderer.update_text_format(0.0);
        }

        renderer
    }

    pub fn update_text_format(&mut self, font_size_delta: f32) {
        if (self.font_size + font_size_delta) > 0.0 {
            self.font_size += font_size_delta;
        }

        unsafe {
            // Release the old text format
            if !self.text_format.is_null() {
                (*self.text_format).Release();
            }

            hr_ok!((*self.write_factory).CreateTextFormat(
                self.font_name.as_ptr(),
                null_mut(),
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                self.font_size,
                self.font_locale.as_ptr(),
                &mut self.text_format
            ));
            hr_ok!((*self.text_format).SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING));
            hr_ok!((*self.text_format).SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR));
            hr_ok!((*self.text_format).SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP));
        }

        self.update_font_metrics();
    }

    fn update_font_metrics(&mut self) {
        static GLYPH_CHAR: u16 = 0x0061;
        unsafe {
            let mut test_text_layout: *mut IDWriteTextLayout = null_mut();
            hr_ok!((*self.write_factory).CreateTextLayout(
                &GLYPH_CHAR,
                1,
                self.text_format,
                0.0,
                0.0,
                &mut test_text_layout
            ));

            let mut metrics_uninit = MaybeUninit::<DWRITE_HIT_TEST_METRICS>::uninit();
            let mut dummy: (f32, f32) = (0.0, 0.0);
            hr_ok!((*test_text_layout).HitTestTextPosition(
                0,
                0,
                &mut dummy.0,
                &mut dummy.1,
                metrics_uninit.as_mut_ptr()
            ));
            let metrics = metrics_uninit.assume_init();
            (*test_text_layout).Release();

            self.font_width = metrics.width;
            self.font_height = metrics.height;

            hr_ok!((*self.text_format).SetIncrementalTabStop(self.font_width * settings::NUMBER_OF_SPACES_PER_TAB as f32));
        }
    }

    pub fn get_max_rows(&self) -> usize {
        (self.pixel_size.height as f32 / self.font_height).ceil() as usize
    }

    pub fn get_max_columns(&self) -> usize {
        (self.pixel_size.width as f32 / self.font_width) as usize
    }

    pub fn get_extents(&self) -> (f32, f32) {
        (self.pixel_size.width as f32, self.pixel_size.height as f32)
    }

    fn get_text_buffer_margin(&self, text_buffer: &mut TextBuffer) -> f32 {
        text_buffer.margin_column_count as f32 * self.font_width
    }

    fn get_text_buffer_column_offset(&self, text_buffer: &mut TextBuffer) -> f32 {
        text_buffer.column_offset as f32 * self.font_width
    }

    fn get_text_buffer_adjusted_origin(&self, text_buffer: &mut TextBuffer) -> (f32, f32) {
        let margin = self.get_text_buffer_margin(text_buffer);
        let column_offset = self.get_text_buffer_column_offset(text_buffer);
        let text_layout = self.buffer_layouts.get(&text_buffer.path).unwrap();
        (text_layout.origin.0 + margin - column_offset, text_layout.origin.1)
    }

    pub fn update_buffer_layout(&mut self, origin: (f32, f32), extents: (f32, f32), text_buffer: &mut TextBuffer) {
        if self.buffer_layouts.contains_key(&text_buffer.path) {
            unsafe {
                (**(self.buffer_layouts.get_mut(&text_buffer.path).unwrap().layout)).Release();
            }
        }

        let lines = text_buffer.get_text_view_as_utf16();
        let margin = self.get_text_buffer_margin(text_buffer);

        unsafe {
            let mut text_layout: *mut IDWriteTextLayout = null_mut();
            hr_ok!((*self.write_factory).CreateTextLayout(
                lines.as_ptr(),
                lines.len() as u32,
                self.text_format,
                self.pixel_size.width as f32 - margin,
                self.pixel_size.height as f32,
                &mut text_layout
            ));
            self.buffer_layouts.insert(text_buffer.path.to_string(), TextLayout { origin, extents, layout: text_layout });
        }
    }

    pub fn update_buffer_line_number_layout(&mut self, origin: (f32, f32), extents: (f32, f32), text_buffer: &mut TextBuffer) {
        if self.buffer_line_number_layouts.contains_key(&text_buffer.path) {
            unsafe {
                (**(self.buffer_line_number_layouts.get_mut(&text_buffer.path).unwrap().layout)).Release();
            }
        }

        let line_number_string = text_buffer.get_line_number_string();
        unsafe {
            let mut text_layout: *mut IDWriteTextLayout = null_mut();
            hr_ok!((*self.write_factory).CreateTextLayout(
                line_number_string.as_ptr(),
                line_number_string.len() as u32,
                self.text_format,
                self.get_text_buffer_margin(text_buffer),
                self.pixel_size.height as f32,
                &mut text_layout
            ));
            self.buffer_line_number_layouts.insert(text_buffer.path.to_string(), TextLayout { origin, extents, layout: text_layout });
        }
    }

    pub fn mouse_pos_to_text_pos(&self, text_buffer: &mut TextBuffer, mouse_pos: (f32, f32)) -> usize {
        let text_layout = self.buffer_layouts.get(&text_buffer.path).unwrap();

        let adjusted_origin = self.get_text_buffer_adjusted_origin(text_buffer);
        
        let mut is_inside = 0;
        let mut metrics_uninit = MaybeUninit::<DWRITE_HIT_TEST_METRICS>::uninit();
        unsafe {
            hr_ok!(
                (*text_layout.layout).HitTestPoint(
                    mouse_pos.0 - adjusted_origin.0,
                    mouse_pos.1 - adjusted_origin.1,
                    text_buffer.get_caret_trailing_as_mut_ref(),
                    &mut is_inside,
                    metrics_uninit.as_mut_ptr()
                )
            );

            let metrics = metrics_uninit.assume_init();
            metrics.textPosition as usize
        }
    }

    fn draw_selection_range(&self, origin: (f32, f32), text_layout: *mut IDWriteTextLayout, range: DWRITE_TEXT_RANGE) {
        let mut hit_test_count = 0;

        unsafe {
            let hr: i32 = (*text_layout).HitTestTextRange(
                        range.startPosition, 
                        range.length,
                        origin.0,
                        origin.1,
                        null_mut(),
                        0,
                        &mut hit_test_count
                    );
            assert!((hr as u32) == 0x8007007A, "HRESULT in this case is expected to error with \"ERROR_INSUFFICIENT_BUFFER\""); 

            let mut hit_tests : Vec<DWRITE_HIT_TEST_METRICS> = Vec::with_capacity(hit_test_count as usize);
            hit_tests.set_len(hit_test_count as usize);

            hr_ok!(
                (*text_layout).HitTestTextRange(
                    range.startPosition,
                    range.length,
                    origin.0,
                    origin.1,
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

                (*self.target).FillRectangle(&highlight_rect, self.theme.selection_brush as *mut ID2D1Brush);

            });
            (*self.target).SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);

        }
    }

    fn get_rect_from_hit_test(&self, pos: u32, origin: (f32, f32), text_layout: *mut IDWriteTextLayout) -> D2D1_RECT_F {
        let mut metrics_uninit = MaybeUninit::<DWRITE_HIT_TEST_METRICS>::uninit();
        let mut dummy = (0.0, 0.0);

        unsafe {
            hr_ok!((*text_layout).HitTestTextPosition(
                pos,
                false as i32,
                &mut dummy.0,
                &mut dummy.1,
                metrics_uninit.as_mut_ptr(),
            ));
            let metrics = metrics_uninit.assume_init();

            D2D1_RECT_F {
                left: origin.0 + metrics.left,
                top: origin.1 + metrics.top,
                right: origin.0 + metrics.left + metrics.width,
                bottom: origin.1 + metrics.top + metrics.height
            }
        }
    }

    fn draw_rounded_rect(&self, rect: &D2D1_RECT_F) {
        let rounded_rect = D2D1_ROUNDED_RECT {
            rect: *rect,
            radiusX: 3.0,
            radiusY: 3.0
        };

        unsafe {
            (*self.target).DrawRoundedRectangle(
                &rounded_rect, 
                self.theme.bracket_brush as *mut ID2D1Brush, 
                self.theme.bracket_rect_width, 
                null_mut()
            );
        }
    }

    fn draw_enclosing_brackets(&self, origin: (f32, f32), text_layout: *mut IDWriteTextLayout, enclosing_bracket_positions: [Option<usize>; 2]) {
        match &enclosing_bracket_positions {
            [Some(pos1), Some(pos2)] => {
                let rect1 = self.get_rect_from_hit_test(*pos1 as u32, origin, text_layout);
                let rect2 = self.get_rect_from_hit_test(*pos2 as u32, origin, text_layout);

                // If the brackets are right next to eachother, draw one big rect
                if *pos2 == (*pos1 + 1) {
                    let rect = D2D1_RECT_F {
                        left: rect1.left,
                        top: rect1.top,
                        right: rect2.right,
                        bottom: rect2.bottom
                    };
                    self.draw_rounded_rect(&rect);
                    return;
                }

                self.draw_rounded_rect(&rect1);
                self.draw_rounded_rect(&rect2);
            }
            [None, Some(pos)]  | [Some(pos), None] => {
                let rect = self.get_rect_from_hit_test(*pos as u32, origin, text_layout);
                self.draw_rounded_rect(&rect);
            }
            [None, None] => {}
        }
    }

    fn draw_line_numbers(&self, text_buffer: &mut TextBuffer) {
        let text_layout = self.buffer_line_number_layouts.get(&text_buffer.path).unwrap();

        unsafe {
            (*self.target).DrawTextLayout(
                D2D1_POINT_2F {
                    x: text_layout.origin.0,
                    y: text_layout.origin.1
                },
                text_layout.layout,
                self.theme.line_number_brush as *mut ID2D1Brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE
            );
        }
    }

    fn draw_text(&self, origin: (f32, f32), text_buffer: &mut TextBuffer, text_layout: *mut IDWriteTextLayout) {
        unsafe {
            let lexical_highlights = text_buffer.get_lexical_highlights();
            // In case of overlap, lexical highlights trump semantic for now.
            // This is to ensure that commenting out big sections of code happen
            // instantaneously
            for (range, token_type) in lexical_highlights.highlight_tokens {
                match token_type {
                    SemanticTokenTypes::Comment           => { hr_ok!((*text_layout).SetDrawingEffect(self.theme.comment_brush as *mut IUnknown, range)); },
                    SemanticTokenTypes::Keyword           => { hr_ok!((*text_layout).SetDrawingEffect(self.theme.keyword_brush as *mut IUnknown, range)); },
                    SemanticTokenTypes::Literal           => { hr_ok!((*text_layout).SetDrawingEffect(self.theme.literal_brush as *mut IUnknown, range)); },
                    SemanticTokenTypes::Preprocessor      => { hr_ok!((*text_layout).SetDrawingEffect(self.theme.macro_preprocessor_brush as *mut IUnknown, range)); },
                }
            }

            if let Some(selection_range) = text_buffer.get_selection_range() {
                self.draw_selection_range(origin, text_layout, DWRITE_TEXT_RANGE { startPosition: selection_range.start, length: selection_range.length });
            }
            if let Some(enclosing_bracket_ranges) = lexical_highlights.enclosing_brackets {
                self.draw_enclosing_brackets(origin, text_layout, enclosing_bracket_ranges);
            }

            (*self.target).DrawTextLayout(
                D2D1_POINT_2F { x: origin.0, y: origin.1 },
                text_layout,
                self.theme.text_brush as *mut ID2D1Brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE
            );
        }
    }

    fn draw_caret(&self, origin: (f32, f32), text_buffer: &mut TextBuffer, text_layout: *mut IDWriteTextLayout) {
        if let Some(caret_offset) = text_buffer.get_caret_offset() {

            let mut caret_pos: (f32, f32) = (0.0, 0.0);
            let mut metrics_uninit = MaybeUninit::<DWRITE_HIT_TEST_METRICS>::uninit();

            unsafe {
                hr_ok!((*text_layout).HitTestTextPosition(
                    caret_offset as u32,
                    text_buffer.get_caret_trailing(),
                    &mut caret_pos.0,
                    &mut caret_pos.1,
                    metrics_uninit.as_mut_ptr()
                ));

                let metrics = metrics_uninit.assume_init();

                let rect = D2D1_RECT_F {
                    left: origin.0 + caret_pos.0 - (self.caret_width as f32 / 2.0),
                    top: origin.1 + caret_pos.1,
                    right: origin.0 + caret_pos.0 + (self.caret_width as f32 / 2.0),
                    bottom: origin.1 + caret_pos.1 + metrics.height
                };

                (*self.target).SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
                (*self.target).FillRectangle(&rect, self.theme.caret_brush as *mut ID2D1Brush);
                (*self.target).SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            }

        }
    }

    pub fn draw(&self, text_buffer: &mut TextBuffer) {
        unsafe {
            (*self.target).BeginDraw();

            (*self.target).SetTransform(&IDENTITY_MATRIX);
            (*self.target).Clear(&self.theme.background_color);

            self.draw_line_numbers(text_buffer);

            let text_layout = self.buffer_layouts.get(&text_buffer.path).unwrap();
            let margin = self.get_text_buffer_margin(text_buffer);
            let column_offset = self.get_text_buffer_column_offset(text_buffer);

            let clip_rect = D2D1_RECT_F {
                left: text_layout.origin.0 + margin,
                top: text_layout.origin.1,
                right: text_layout.origin.0 + text_layout.extents.0,
                bottom: text_layout.origin.1 + text_layout.extents.1
            };
            (*self.target).PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_ALIASED);

            // Adjust origin to account for column offset and margin
            let adjusted_origin = (text_layout.origin.0 + margin - column_offset, text_layout.origin.1);

            self.draw_text(adjusted_origin, text_buffer, text_layout.layout);
            self.draw_caret(adjusted_origin, text_buffer, text_layout.layout);
            (*self.target).PopAxisAlignedClip();

            (*self.target).EndDraw(null_mut(), null_mut());
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.pixel_size.width = width;
        self.pixel_size.height = height;
        unsafe {
            (*self.target).Resize(&self.pixel_size);
        }
        self.update_font_metrics();
    }
}
