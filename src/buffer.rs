use core::ops::RangeBounds;
use std::{
    cell::RefCell,
    cmp::min,
    fs::File,
    ffi::OsStr,
    iter::once,
    os::windows::ffi::OsStrExt,
    ptr::null_mut,
    mem::{swap, MaybeUninit},
    rc::Rc,
    str
};
use winapi::{
    um::{
        dwrite::{IDWriteTextLayout, DWRITE_HIT_TEST_METRICS, DWRITE_TEXT_RANGE},
        d2d1::{D2D1_RECT_F, D2D1_LAYER_PARAMETERS},
        winuser::{SystemParametersInfoW, SPI_GETCARETWIDTH}
    },
    ctypes::c_void
};
use ropey::Rope;

use crate::dx_ok;
use crate::renderer::TextRenderer;

#[derive(PartialEq)]
pub enum SelectionMode {
    Left,
    Right,
    Down,
    Up
}

pub enum MouseSelectionMode {
    Click,
    Move
}

pub struct TextBuffer {
    buffer: Rope,

    top_line: usize,
    bot_line: usize,

    origin: (u32, u32),
    pub text_origin: (u32, u32),
    text_extents: (u32, u32),
    text_visible_line_count: usize,
    numbers_origin: (u32, u32),
    numbers_extents: (u32, u32),
    numbers_visible_digits_count: usize,

    absolute_char_pos_start: usize,
    absolute_char_pos_end: usize,

    pub currently_selecting: bool,
    caret_char_anchor: usize,
    caret_char_pos: usize,
    caret_is_trailing: i32,
    caret_width: u32,
    half_caret_width: u32,

    cached_mouse_width: f32,

    pub text_layer_params: D2D1_LAYER_PARAMETERS,
    text_layout: *mut IDWriteTextLayout,

    pub numbers_layer_params: D2D1_LAYER_PARAMETERS,
    numbers_layout: *mut IDWriteTextLayout,

    renderer: Rc<RefCell<TextRenderer>>
}

impl TextBuffer {
    pub fn new(path: &str, origin: (u32, u32), pixel_size: (u32, u32), renderer: Rc<RefCell<TextRenderer>>) -> TextBuffer {
        let file = File::open(path).unwrap();
        let buffer = Rope::from_reader(file).unwrap();

        let mut caret_width: u32 = 0;
        unsafe {
            // We'll increase the width from the system width slightly
            SystemParametersInfoW(SPI_GETCARETWIDTH, 0, (&mut caret_width as *mut _) as *mut c_void, 0);
            caret_width *= 2;
        }

        let mut text_buffer = TextBuffer {
            buffer,

            top_line: 0,
            bot_line: 0,

            origin: (0, 0),
            text_origin: (0, 0),
            text_extents: (0, 0),
            text_visible_line_count: 0,
            numbers_origin: (0, 0),
            numbers_extents: (0, 0),
            numbers_visible_digits_count: 0,

            absolute_char_pos_start: 0,
            absolute_char_pos_end: 0,

            currently_selecting: false,
            caret_char_anchor: 0,
            caret_char_pos: 0,
            caret_is_trailing: 0,
            caret_width,
            half_caret_width: caret_width / 2,

            cached_mouse_width: 0.0,

            text_layer_params: unsafe { MaybeUninit::<D2D1_LAYER_PARAMETERS>::zeroed().assume_init() },
            text_layout: null_mut(),

            numbers_layer_params: unsafe { MaybeUninit::<D2D1_LAYER_PARAMETERS>::zeroed().assume_init() },
            numbers_layout: null_mut(),

            renderer
        };

        text_buffer.update_metrics(origin, pixel_size);
        text_buffer
    }

    pub fn get_caret_absolute_pos(&self) -> usize {
        self.caret_char_pos + (self.caret_is_trailing as usize)
    }

    pub fn scroll_down(&mut self, delta: usize) {
        self.top_line += delta;

        // In case the number column is not wide enough
        self.update_absolute_char_positions();
    }

    pub fn scroll_up(&mut self, delta: usize) {
        if self.top_line >= delta {
            self.top_line -= delta;
        }
        self.update_absolute_char_positions();
    }

    pub fn left_click(&mut self, mouse_pos: (f32, f32), extend_current_selection: bool) {
        self.set_mouse_selection(MouseSelectionMode::Click, mouse_pos);
        if !extend_current_selection {
            self.caret_char_anchor = self.get_caret_absolute_pos();
        }
        self.currently_selecting = true;

        // Reset the cached width
        self.cached_mouse_width = 0.0;
    }

    pub fn left_release(&mut self) {
        self.currently_selecting = false;
    }

    pub fn set_selection(&mut self, mode: SelectionMode, count: usize, extend_current_selection: bool) {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        match mode {
            SelectionMode::Left | SelectionMode::Right => {
                self.caret_char_pos = caret_absolute_pos;
                if self.caret_char_pos > 0 {
                    if mode == SelectionMode::Left {
                        self.caret_char_pos -= count;
                    }
                    else {
                        self.caret_char_pos += count;
                    }
                    self.caret_is_trailing = 0;

                    // Reset the cached width
                    self.cached_mouse_width = 0.0;
                }
            },
            SelectionMode::Down | SelectionMode::Up => {
                let mut caret_pos: (f32, f32) = (0.0, 0.0);
                let mut metrics_uninit = MaybeUninit::<DWRITE_HIT_TEST_METRICS>::uninit();

                unsafe {
                    dx_ok!((*self.text_layout).HitTestTextPosition(
                        (self.caret_char_pos - self.absolute_char_pos_start) as u32,
                        self.caret_is_trailing,
                        &mut caret_pos.0,
                        &mut caret_pos.1,
                        metrics_uninit.as_mut_ptr()
                    ));

                    let metrics = metrics_uninit.assume_init();

                    if caret_pos.0 < self.cached_mouse_width {
                        caret_pos.0 = self.cached_mouse_width;
                    }
                    else {
                        self.cached_mouse_width = caret_pos.0;
                    }

                    if mode == SelectionMode::Down {
                        caret_pos.1 += metrics.height * count as f32;
                    }
                    else {
                        caret_pos.1 -= metrics.height * count as f32
                    }
                    
                    self.set_mouse_selection(MouseSelectionMode::Click, caret_pos);
                }
            },
        }

        if !extend_current_selection {
            self.caret_char_anchor = self.get_caret_absolute_pos();
        }

    }

    pub fn set_mouse_selection(&mut self, mode: MouseSelectionMode, mouse_pos: (f32, f32)) {
        if let Some(relative_mouse_pos) = self.translate_mouse_pos_to_text_region(mouse_pos) {
            match mode {
                MouseSelectionMode::Click => {
                    let mut is_inside = 0;
                    let mut metrics_uninit = MaybeUninit::<DWRITE_HIT_TEST_METRICS>::uninit();

                    unsafe {
                        dx_ok!(
                            (*self.text_layout).HitTestPoint(
                                relative_mouse_pos.0,
                                relative_mouse_pos.1,
                                &mut self.caret_is_trailing,
                                &mut is_inside,
                                metrics_uninit.as_mut_ptr()
                            )
                        );

                        let metrics = metrics_uninit.assume_init();
                        let absolute_text_pos = metrics.textPosition as usize;
                        self.caret_char_pos = self.absolute_char_pos_start + absolute_text_pos;
                    }
                },

                MouseSelectionMode::Move => {
                    if self.currently_selecting {
                        let mut is_inside = 0;
                        let mut metrics_uninit = MaybeUninit::<DWRITE_HIT_TEST_METRICS>::uninit();

                        unsafe {
                            dx_ok!(
                                (*self.text_layout).HitTestPoint(
                                    relative_mouse_pos.0,
                                    relative_mouse_pos.1,
                                    &mut self.caret_is_trailing,
                                    &mut is_inside,
                                    metrics_uninit.as_mut_ptr()
                                )
                            );

                            let metrics = metrics_uninit.assume_init();
                            let absolute_text_pos = metrics.textPosition as usize;
                            self.caret_char_pos = self.absolute_char_pos_start + absolute_text_pos;
                        }
                    }
                }
            }
        }

    }

    fn translate_mouse_pos_to_text_region(&self, mouse_pos: (f32, f32)) -> Option<(f32, f32)> {
        let dx = mouse_pos.0 - self.text_origin.0 as f32;
        let dy = mouse_pos.1 - self.text_origin.1 as f32;
        if (0.0..self.text_extents.0 as f32).contains(&dx) && 
            (0.0..self.text_extents.1 as f32).contains(&dy) {
            Some((dx, dy))
        }
        else {
            None
        }
    }

    pub fn insert_char(&mut self, character: u16) {
        let caret_absolute_pos = self.get_caret_absolute_pos();
        if caret_absolute_pos != self.caret_char_anchor {
            let diff;
            if caret_absolute_pos < self.caret_char_anchor {
                self.buffer.remove(caret_absolute_pos..self.caret_char_anchor);
                diff = self.caret_char_anchor - caret_absolute_pos;
            }
            else {
                self.buffer.remove(self.caret_char_anchor..caret_absolute_pos);
                diff = caret_absolute_pos - self.caret_char_anchor;
            }
            self.caret_char_pos = caret_absolute_pos - diff;
            self.caret_is_trailing = 0;
            self.caret_char_anchor -= diff;
        }

        // Insert 4 spaces in place of <TAB>
        if character == 0x9 {
            self.buffer.insert(self.get_caret_absolute_pos(), "    ");
            self.set_selection(SelectionMode::Right, 4, false);
        }
        else {
            self.buffer.insert_char(self.get_caret_absolute_pos(), (character as u8) as char);
            self.set_selection(SelectionMode::Right, 1, false);
        }

        self.update_absolute_char_positions();
    }

    pub fn get_caret_rect(&mut self) -> Option<D2D1_RECT_F> {
        if self.caret_char_pos < self.absolute_char_pos_start {
            return None;
        }

        let mut caret_pos: (f32, f32) = (0.0, 0.0);
        let mut metrics_uninit = MaybeUninit::<DWRITE_HIT_TEST_METRICS>::uninit();

        unsafe {
            dx_ok!((*self.text_layout).HitTestTextPosition(
                (self.caret_char_pos - self.absolute_char_pos_start) as u32,
                self.caret_is_trailing,
                &mut caret_pos.0,
                &mut caret_pos.1,
                metrics_uninit.as_mut_ptr()
            ));

            let metrics = metrics_uninit.assume_init();

            let rect = D2D1_RECT_F {
                left: self.text_origin.0 as f32 + caret_pos.0 - self.half_caret_width as f32,
                top: self.text_origin.1 as f32 + caret_pos.1,
                right: self.text_origin.0 as f32 + caret_pos.0 + (self.caret_width - self.half_caret_width) as f32,
                bottom: self.text_origin.1 as f32 + caret_pos.1 + metrics.height
            };

            return Some(rect)
        }
    }

    pub fn get_selection_range(&self) -> Option<DWRITE_TEXT_RANGE> {
        let caret_absolute_pos = self.get_caret_absolute_pos();
        if caret_absolute_pos == self.caret_char_anchor {
            return None;
        }
 
        // Saturating sub ensures that the carets don't go below 0
        let mut caret_begin = self.caret_char_anchor.saturating_sub(self.absolute_char_pos_start);
        let mut caret_end = caret_absolute_pos.saturating_sub(self.absolute_char_pos_start);

        if caret_begin > caret_end {
            swap(&mut caret_begin, &mut caret_end);
        }

        caret_begin = min(caret_begin, self.absolute_char_pos_end);
        caret_end = min(caret_end, self.absolute_char_pos_end);

        let range =  DWRITE_TEXT_RANGE {
            startPosition: caret_begin as u32,
            length: (caret_end - caret_begin) as u32
        };

        Some(range)
    }

    pub fn get_text_layout(&mut self) -> *mut IDWriteTextLayout {
        let lines = self.get_current_lines();

        unsafe {
            if !self.text_layout.is_null() {
                (*self.text_layout).Release();
            }

            dx_ok!((*self.renderer.borrow().write_factory).CreateTextLayout(
                lines.as_ptr(),
                lines.len() as u32,
                self.renderer.borrow().text_format,
                self.text_extents.0 as f32,
                self.text_extents.1 as f32,
                &mut self.text_layout as *mut *mut _
            ));
        }


        self.text_layout
    }

    pub fn get_number_layout(&mut self) -> *mut IDWriteTextLayout {

        // TODO: Optimize
        let mut nums: String = String::new();
        for i in self.top_line..(self.bot_line + 1) {
            nums += i.to_string().as_str();
            nums += "\r\n";
        }
        let lines: Vec<u16> = OsStr::new(nums.as_str()).encode_wide().chain(once(0)).collect();

        unsafe {
            if !self.numbers_layout.is_null() {
                (*self.numbers_layout).Release();
            }

            dx_ok!((*self.renderer.borrow().write_factory).CreateTextLayout(
                lines.as_ptr(),
                lines.len() as u32,
                self.renderer.borrow().text_format,
                self.numbers_extents.0 as f32,
                self.numbers_extents.1 as f32,
                &mut self.numbers_layout as *mut *mut _
            ));
        }

        self.numbers_layout
    }

    pub fn update_metrics(&mut self, origin: (u32, u32), pixel_size: (u32, u32)) {
        self.update_text_region(origin, pixel_size);
        self.update_numbers_region(origin, pixel_size);
        self.update_text_visible_line_count(pixel_size);
        self.update_absolute_char_positions();
    }

    fn update_numbers_visible_digits_count(&mut self) -> usize {
        0
    }

    fn update_text_region(&mut self, origin: (u32, u32), pixel_size: (u32, u32)) {
        self.text_origin = (origin.0 + self.renderer.borrow().font_width as u32 + 5, origin.1);
        self.text_extents = (pixel_size.0 - self.renderer.borrow().font_width as u32, pixel_size.1);
        self.text_layer_params = TextRenderer::layer_params(self.text_origin, self.text_extents);
    }

    fn update_numbers_region(&mut self, origin: (u32, u32), pixel_size: (u32, u32)) {
        self.numbers_origin = (origin.0, origin.1);
        self.numbers_extents = (self.renderer.borrow().font_width as u32 + 5, pixel_size.1);
        self.numbers_layer_params = TextRenderer::layer_params(self.numbers_origin, self.numbers_extents);
    }

    fn update_text_visible_line_count(&mut self, pixel_size: (u32, u32)) {
        self.text_visible_line_count = pixel_size.1 as usize / self.renderer.borrow().font_height as usize;
    }

    fn update_absolute_char_positions(&mut self) {
        self.bot_line = self.top_line + self.text_visible_line_count;
        self.absolute_char_pos_start = self.buffer.line_to_char(self.top_line);
        self.absolute_char_pos_end = self.buffer.line_to_char(self.bot_line);
    }

    pub fn get_current_lines(&self) -> Vec<u16> {
        self.text_range(self.absolute_char_pos_start..self.absolute_char_pos_end)
    }

    fn text_range<R>(&self, char_range: R) -> Vec<u16> where R: RangeBounds<usize> {
        let rope_slice = self.buffer.slice(char_range);
        let chars: Vec<u8> = rope_slice.bytes().collect();
        OsStr::new(str::from_utf8(chars.as_ref()).unwrap()).encode_wide().chain(once(0)).collect()
    }
}
