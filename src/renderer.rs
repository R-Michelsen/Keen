use crate::{
    settings,
    buffer::TextPosition,
    editor::TextDocument,
    editor::TextView,
    theme::Theme,
    language_support::SemanticTokenTypes,
    util::pwstr_from_str
};

use std::{
    collections::HashMap,
    ptr::null_mut
};

use bindings::{
    Windows::Win32::WindowsAndMessaging::*,
    Windows::Win32::HiDpi::*,
    Windows::Win32::Dxgi::*,
    Windows::Win32::DirectWrite::*,
    Windows::Win32::Direct2D::*,
    Windows::Win32::DisplayDevices::*,
    Windows::Win32::SystemServices::*,
    Windows::Foundation::Numerics::*
};
use windows::{Abi, Result, Interface};

fn get_client_size(hwnd: HWND) -> D2D_SIZE_U {
    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect); }
    D2D_SIZE_U {
        width: (rect.right - rect.left) as u32,
        height: (rect.bottom - rect.top) as u32
    }
}

fn create_dwrite_factory() -> Result<IDWriteFactory> {
    let mut write_factory = None;

    unsafe {
        DWriteCreateFactory(
            DWRITE_FACTORY_TYPE::DWRITE_FACTORY_TYPE_SHARED, 
            &IDWriteFactory::IID, 
            write_factory.set_abi() as _
        ).and_some(write_factory)
    }
}

fn create_text_format(font_name: PWSTR, font_locale: PWSTR, font_size: f32, dwrite_factory: &IDWriteFactory) -> Result<IDWriteTextFormat> {
    unsafe {
        let mut text_format = None;
        dwrite_factory.CreateTextFormat(
            font_name,
            None,
            DWRITE_FONT_WEIGHT::DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE::DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH::DWRITE_FONT_STRETCH_NORMAL,
            font_size,
            font_locale,
            &mut text_format
        ).and_some(text_format)
    }
}

fn create_d2d1_factory() -> Result<ID2D1Factory> {
    let mut d2d1_factory = None;
    unsafe {
        D2D1CreateFactory(
            D2D1_FACTORY_TYPE::D2D1_FACTORY_TYPE_SINGLE_THREADED, 
            &ID2D1Factory::IID,
            null_mut(), 
            d2d1_factory.set_abi()
        ).and_some(d2d1_factory)
    }
}

fn create_render_target(d2d1_factory: &ID2D1Factory, hwnd: HWND) -> Result<ID2D1HwndRenderTarget> {
    let target_props = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE::D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT::DXGI_FORMAT_UNKNOWN,
            alphaMode: D2D1_ALPHA_MODE::D2D1_ALPHA_MODE_UNKNOWN
        },
        dpiX: 96.0,
        dpiY: 96.0,
        usage: D2D1_RENDER_TARGET_USAGE::D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL::D2D1_FEATURE_LEVEL_DEFAULT
    };

    let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
        hwnd,
        pixelSize: get_client_size(hwnd),
        presentOptions: D2D1_PRESENT_OPTIONS::D2D1_PRESENT_OPTIONS_NONE
    };

    let mut render_target = None;
    unsafe {
        d2d1_factory.CreateHwndRenderTarget(&target_props, &hwnd_props, &mut render_target).and_some(render_target)
    }
}

fn get_character_spacing(dwrite_factory: &IDWriteFactory, text_format: &IDWriteTextFormat) -> Result<f32> {
    unsafe {
        let mut temp_text_layout = None;
        let text_layout = dwrite_factory.CreateTextLayout(
            pwstr_from_str("M"),
            1,
            text_format,
            0.0,
            0.0,
            &mut temp_text_layout
        ).and_some(temp_text_layout)?;
        
        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
        let mut dummy: (f32, f32) = (0.0, 0.0);
        text_layout.HitTestTextPosition(
            0,
            false,
            &mut dummy.0,
            &mut dummy.1,
            &mut metrics
        ).ok()?;

        Ok(metrics.width)
    }
}

pub struct TextRenderer {
    pub pixel_size: D2D_SIZE_U,
    pub font_size: f32,
    line_spacing: f32,
    character_spacing: f32,

    font_name: String,

    caret_width: u32,

    theme: Theme,

    dwrite_factory: IDWriteFactory,
    text_format: IDWriteTextFormat,
    
    render_target: ID2D1HwndRenderTarget,

    buffer_layouts: HashMap<String, IDWriteTextLayout>
}

impl TextRenderer {
    pub fn new(hwnd: HWND, font: &str, font_size: f32) -> Result<Self> {
        unsafe {
            // We'll increase the width from the system width slightly
            let mut caret_width: u32 = 0;
            SystemParametersInfoW(SYSTEM_PARAMETERS_INFO_ACTION::SPI_GETCARETWIDTH, 0, (&mut caret_width as *mut _) as _, SystemParametersInfo_fWinIni(0));
            caret_width *= 2;

            let dpi = GetDpiForWindow(hwnd);
            let dpi_scale = dpi as f32 / 96.0;

            // Scale the font size to fit the dpi
            let scaled_font_size = font_size * dpi_scale;

            let dwrite_factory = create_dwrite_factory()?;

            let text_format = create_text_format(
                pwstr_from_str(font),
                pwstr_from_str("en-us"),
                scaled_font_size,
                &dwrite_factory
            )?;
            text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT::DWRITE_TEXT_ALIGNMENT_LEADING).ok()?;
            text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT::DWRITE_PARAGRAPH_ALIGNMENT_NEAR).ok()?;
            text_format.SetWordWrapping(DWRITE_WORD_WRAPPING::DWRITE_WORD_WRAPPING_NO_WRAP).ok()?;

            let pixel_aligned_line_spacing = f32::ceil(scaled_font_size * settings::LINE_SPACING_FACTOR);
            text_format.SetLineSpacing(
                DWRITE_LINE_SPACING_METHOD::DWRITE_LINE_SPACING_METHOD_UNIFORM, 
                pixel_aligned_line_spacing, 
                pixel_aligned_line_spacing * 0.8
            ).ok()?;

            let character_spacing = get_character_spacing(&dwrite_factory, &text_format)?;
            text_format.SetIncrementalTabStop(character_spacing * settings::NUMBER_OF_SPACES_PER_TAB as f32).ok()?;

            let d2d1_factory = create_d2d1_factory()?;
            let render_target = create_render_target(&d2d1_factory, hwnd)?;
            render_target.SetAntialiasMode(D2D1_ANTIALIAS_MODE::D2D1_ANTIALIAS_MODE_ALIASED);

            Ok(Self {
                pixel_size: get_client_size(hwnd),
                font_size: scaled_font_size,
                line_spacing: pixel_aligned_line_spacing,
                character_spacing,
                font_name: String::from(font),
                caret_width,
                theme: Theme::new_default(&render_target)?,
                dwrite_factory,
                text_format,
                render_target,
                buffer_layouts: HashMap::new()
            })
        }
    }

    pub fn update_text_format(&mut self, zoom_delta: f32) -> Result<()> {
        self.font_size = f32::max(1.0, self.font_size + zoom_delta);
        unsafe {
            self.text_format = create_text_format(
                pwstr_from_str(&self.font_name),
                pwstr_from_str("en-us"),
                self.font_size,
                &self.dwrite_factory
            )?;
    
            self.text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT::DWRITE_TEXT_ALIGNMENT_LEADING).ok()?;
            self.text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT::DWRITE_PARAGRAPH_ALIGNMENT_NEAR).ok()?;
            self.text_format.SetWordWrapping(DWRITE_WORD_WRAPPING::DWRITE_WORD_WRAPPING_NO_WRAP).ok()?;
            self.line_spacing = f32::ceil(self.font_size * settings::LINE_SPACING_FACTOR);
            self.text_format.SetLineSpacing(
                DWRITE_LINE_SPACING_METHOD::DWRITE_LINE_SPACING_METHOD_UNIFORM, 
                self.line_spacing, 
                self.line_spacing * 0.8
            ).ok()?;
    
            self.character_spacing = get_character_spacing(&self.dwrite_factory, &self.text_format)?;
            self.text_format.SetIncrementalTabStop(self.character_spacing * settings::NUMBER_OF_SPACES_PER_TAB as f32).ok()?;
        }
        Ok(())
    }

    pub fn get_max_rows(&self) -> usize {
        (self.pixel_size.height as f32 / self.line_spacing).ceil() as usize
    }

    pub fn get_max_columns(&self) -> usize {
        (self.pixel_size.width as f32 / self.character_spacing) as usize
    }

    pub fn get_extents(&self) -> (f32, f32) {
        (self.pixel_size.width as f32, self.pixel_size.height as f32)
    }

    fn adjust_text_view(&self, text_view: &mut TextView, caret_line: usize, caret_column: usize) {
        let current_line_start = text_view.line_offset;
        let current_line_end = current_line_start + self.get_max_rows();
        let current_column_start = text_view.column_offset;
        let current_column_end = current_column_start + self.get_max_columns();
    
        // Check for vertical adjustments
        if !(current_line_start..current_line_end).contains(&caret_line) {
            if caret_line < current_line_start {
                text_view.line_offset -= current_line_start - caret_line;
            }
            else {
                text_view.line_offset += caret_line - current_line_end;
            }
        }
    
        // Check for horizontal adjustments
        if !(current_column_start..current_column_end).contains(&caret_column) {
            if caret_column < current_column_start {
                text_view.column_offset -= current_column_start - caret_column;
            }
            else {
                text_view.column_offset += caret_column - current_column_end;
            }
        }    
    }

    pub fn update_buffer_layout(&mut self, text_document: &mut TextDocument) -> Result<()> {
        let mut lines = text_document.buffer.get_text_view_as_utf16(
            text_document.view.line_offset, 
            text_document.view.line_offset + self.get_max_rows()
        );

        unsafe {
            let mut text_layout = None;
            self.dwrite_factory.CreateTextLayout(
                PWSTR(lines.as_mut_ptr()),
                lines.len() as u32,
                &self.text_format,
                self.pixel_size.width as f32,
                self.pixel_size.height as f32,
                &mut text_layout
            ).ok()?;
            self.buffer_layouts.insert(text_document.buffer.path.to_string(), text_layout.unwrap());
        }
        Ok(())
    }

    pub fn mouse_pos_to_text_pos(&self, text_document: &mut TextDocument, mouse_pos: (f32, f32)) -> Result<TextPosition> {
        let text_layout = self.buffer_layouts.get(&text_document.buffer.path).unwrap();
        let column_offset = text_document.view.column_offset as f32 * self.character_spacing;
        
        let mut is_inside = BOOL::from(false);
        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
        unsafe {
            text_layout.HitTestPoint(
                mouse_pos.0 + column_offset,
                mouse_pos.1,
                text_document.buffer.get_caret_trailing_as_mut_ref(),
                &mut is_inside,
                &mut metrics
            ).ok()?;
        }
        Ok(TextPosition {
            line_offset: text_document.view.line_offset,
            char_offset: metrics.textPosition as usize
        })
    }

    fn draw_selection_range(&self, column_offset: f32, text_layout: &IDWriteTextLayout, range: DWRITE_TEXT_RANGE) -> Result<()> {
        let mut hit_test_count = 0;
        unsafe {
            let error_code = text_layout.HitTestTextRange(
                range.startPosition, 
                range.length,
                -column_offset,
                0.0,
                null_mut(),
                0,
                &mut hit_test_count
            );
            assert!(error_code.0 == 0x8007007A, "HRESULT in this case is expected to error with \"ERROR_INSUFFICIENT_BUFFER\""); 

            let mut hit_tests : Vec<DWRITE_HIT_TEST_METRICS> = Vec::with_capacity(hit_test_count as usize);
            hit_tests.set_len(hit_test_count as usize);

            text_layout.HitTestTextRange(
                range.startPosition,
                range.length,
                -column_offset,
                0.0,
                hit_tests.as_mut_ptr(),
                hit_tests.len() as u32,
                &mut hit_test_count
            ).ok()?;

            hit_tests.iter().for_each(|metrics| {
                let highlight_rect = D2D_RECT_F {
                    left: metrics.left,
                    top: metrics.top,
                    right: metrics.left + metrics.width,
                    bottom: metrics.top + metrics.height
                };

                self.render_target.FillRectangle(&highlight_rect, self.theme.selection_brush.as_ref().unwrap());
            });
        }
        Ok(())
    }

    fn get_rect_from_hit_test(&self, pos: u32, column_offset: f32, text_layout: &IDWriteTextLayout) -> Result<D2D_RECT_F> {
        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
        let mut dummy = (0.0, 0.0);

        unsafe {
            text_layout.HitTestTextPosition(
                pos,
                false,
                &mut dummy.0,
                &mut dummy.1,
                &mut metrics,
            ).ok()?;

            // Offset by +- 1 to ensure rect is drawn within bounds
            Ok(D2D_RECT_F {
                left: metrics.left - column_offset + 1.0,
                top: metrics.top + 1.0,
                right: metrics.left + metrics.width - column_offset - 1.0,
                bottom: metrics.top + metrics.height - 1.0
            })
        }
    }

    fn draw_rect(&self, rect: &D2D_RECT_F) {
        unsafe {
            self.render_target.DrawRectangle(
                rect, 
                self.theme.bracket_brush.as_ref().unwrap(), 
                self.theme.bracket_rect_width, 
                None
            );
        }
    }

    fn draw_enclosing_brackets(&self, column_offset: f32, text_layout: &IDWriteTextLayout, enclosing_bracket_positions: [Option<usize>; 2]) -> Result<()> {
        match &enclosing_bracket_positions {
            [Some(pos1), Some(pos2)] => {
                let rect1 = self.get_rect_from_hit_test(*pos1 as u32, column_offset, &text_layout)?;
                let rect2 = self.get_rect_from_hit_test(*pos2 as u32, column_offset, &text_layout)?;

                // If the brackets are right next to eachother, draw one big rect
                if *pos2 == (*pos1 + 1) {
                    let rect = D2D_RECT_F {
                        left: rect1.left + 1.0,
                        top: rect1.top + 1.0,
                        right: rect2.right - 1.0,
                        bottom: rect2.bottom - 1.0
                    };
                    self.draw_rect(&rect);
                    return Ok(());
                }

                self.draw_rect(&rect1);
                self.draw_rect(&rect2);
            }
            [None, Some(pos)]  | [Some(pos), None] => {
                let rect = self.get_rect_from_hit_test(*pos as u32, column_offset, &text_layout)?;
                self.draw_rect(&rect);
            }
            [None, None] => {}
        }
        Ok(())
    }

    fn draw_text(&self, column_offset: f32, text_document: &mut TextDocument, text_layout: &IDWriteTextLayout) -> Result<()> {
        unsafe {
            let lexical_highlights = text_document.buffer.get_lexical_highlights(text_document.view.line_offset, text_document.view.line_offset + self.get_max_rows());
            // In case of overlap, lexical highlights trump semantic for now.
            // This is to ensure that commenting out big sections of code happen
            // instantaneously
            for (range, token_type) in lexical_highlights.highlight_tokens {
                match token_type {
                    SemanticTokenTypes::Comment      => { text_layout.SetDrawingEffect(self.theme.comment_brush.as_ref().unwrap(), range).ok()?; },
                    SemanticTokenTypes::Keyword      => { text_layout.SetDrawingEffect(self.theme.keyword_brush.as_ref().unwrap(), range).ok()?; },
                    SemanticTokenTypes::Literal      => { text_layout.SetDrawingEffect(self.theme.literal_brush.as_ref().unwrap(), range).ok()?; },
                    SemanticTokenTypes::Preprocessor => { text_layout.SetDrawingEffect(self.theme.macro_preprocessor_brush.as_ref().unwrap(), range).ok()?; },
                }
            }

            if let Some(selection_range) = text_document.buffer.get_selection_range(text_document.view.line_offset, text_document.view.line_offset + self.get_max_rows()) {
                self.draw_selection_range(column_offset, text_layout, DWRITE_TEXT_RANGE { startPosition: selection_range.start, length: selection_range.length })?;
            }
            if let Some(enclosing_bracket_ranges) = lexical_highlights.enclosing_brackets {
                self.draw_enclosing_brackets(column_offset, &text_layout, enclosing_bracket_ranges)?;
            }

            self.render_target.DrawTextLayout(
                D2D_POINT_2F { x: -column_offset, y: 0.0 },
                text_layout,
                self.theme.text_brush.as_ref().unwrap(),
                D2D1_DRAW_TEXT_OPTIONS::D2D1_DRAW_TEXT_OPTIONS_NONE
            );
        }
        Ok(())
    }

    fn draw_caret(&self, column_offset: f32, text_document: &mut TextDocument, text_layout: &IDWriteTextLayout) -> Result<()> {
        if let Some(caret_offset) = text_document.buffer.get_caret_offset(text_document.view.line_offset, text_document.view.line_offset + self.get_max_rows()) {
            let mut caret_pos: (f32, f32) = (0.0, 0.0);
            let mut metrics = DWRITE_HIT_TEST_METRICS::default();
            unsafe {
                text_layout.HitTestTextPosition(
                    caret_offset as u32,
                    text_document.buffer.get_caret_trailing(),
                    &mut caret_pos.0,
                    &mut caret_pos.1,
                    &mut metrics
                ).ok()?;

                let rect = D2D_RECT_F {
                    left: caret_pos.0 - (self.caret_width as f32 / 2.0) - column_offset,
                    top: caret_pos.1,
                    right: caret_pos.0 + (self.caret_width as f32 / 2.0) - column_offset,
                    bottom: caret_pos.1 + metrics.height
                };

                self.render_target.FillRectangle(&rect, self.theme.caret_brush.as_ref().unwrap());
            }
        }
        Ok(())
    }

    pub fn draw(&self, text_document: &mut TextDocument) -> Result<()> {
        unsafe {
            self.render_target.BeginDraw();

            self.render_target.SetTransform(&Matrix3x2::identity());
            self.render_target.Clear(&self.theme.background_color);

            let text_layout = self.buffer_layouts.get(&text_document.buffer.path).unwrap();

            if text_document.buffer.view_dirty {
                let (caret_line, caret_column) = text_document.buffer.get_caret_line_and_column();
                self.adjust_text_view(&mut text_document.view, caret_line, caret_column);
                text_document.buffer.view_dirty = false;
            }

            let column_offset = (text_document.view.column_offset as f32) * self.character_spacing;

            // TODO
            // let clip_rect = D2D_RECT_F {
            //     left: 0.0,
            //     top: 0.0,
            //     right: 0.0,
            //     bottom: 0.0
            // };
            // self.render_target.PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE::D2D1_ANTIALIAS_MODE_ALIASED);

            // Adjust origin to account for column offset
            self.draw_text(column_offset, text_document, &text_layout)?;
            self.draw_caret(column_offset, text_document, &text_layout)?;
            // self.render_target.PopAxisAlignedClip();

            self.render_target.EndDraw(null_mut(), null_mut()).ok()?;
        }
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.pixel_size.width = width;
        self.pixel_size.height = height;
        unsafe {
            self.render_target.Resize(&self.pixel_size).ok()?;
        }
        Ok(())
    }
}
