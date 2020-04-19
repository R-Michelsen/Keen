use core::ops::RangeBounds;
use std::{
    cmp::min,
    fs::File,
    ffi::OsStr,
    iter::once,
    os::windows::ffi::OsStrExt,
    ptr::null_mut,
    mem::{swap, MaybeUninit},
    str
};
use winapi::{
    um::{
        dwrite::{IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout, DWRITE_HIT_TEST_METRICS, DWRITE_TEXT_RANGE},
        d2d1::D2D1_RECT_F,
        winuser::{SystemParametersInfoW, SPI_GETCARETWIDTH}
    },
    ctypes::c_void
};

use ropey::Rope;

use crate::dx_ok;

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
    absolute_char_pos_start: usize,
    absolute_char_pos_end: usize,

    pub currently_selecting: bool,
    caret_char_anchor: usize,
    caret_char_pos: usize,
    caret_is_trailing: i32,
    caret_width: u32,
    half_caret_width: u32,

    cached_mouse_width: f32,

    text_layout: *mut IDWriteTextLayout,

    write_factory: *mut IDWriteFactory,
    text_format: *mut IDWriteTextFormat
}

impl TextBuffer {
    pub fn new(path: &str, write_factory: *mut IDWriteFactory, text_format: *mut IDWriteTextFormat) -> TextBuffer {
        let file = File::open(path).unwrap();
        let buffer = Rope::from_reader(file).unwrap();
        let absolute_char_pos_start = buffer.line_to_char(0);
        let absolute_char_pos_end = buffer.line_to_char(100);

        let mut caret_width: u32 = 0;
        unsafe {
            // We'll increase the width from the system width slightly
            SystemParametersInfoW(SPI_GETCARETWIDTH, 0, (&mut caret_width as *mut _) as *mut c_void, 0);
            caret_width *= 2;
        }

        TextBuffer {
            buffer,
            top_line: 0,
            absolute_char_pos_start,
            absolute_char_pos_end,

            currently_selecting: false,
            caret_char_anchor: 0,
            caret_char_pos: 0,
            caret_is_trailing: 0,
            caret_width,
            half_caret_width: caret_width / 2,

            cached_mouse_width: 0.0,

            text_layout: null_mut(),
            write_factory,
            text_format
        }
    }

    pub fn get_caret_absolute_pos(&self) -> usize {
        return self.caret_char_pos + (self.caret_is_trailing as usize);
    }

    pub fn scroll_down(&mut self, delta: usize) {
        self.top_line += delta;
        self.absolute_char_pos_start = self.buffer.line_to_char(self.top_line);
        self.absolute_char_pos_end = self.buffer.line_to_char(self.top_line + 100);
    }

    pub fn scroll_up(&mut self, delta: usize) {
        if self.top_line >= delta {
            self.top_line -= delta;
        }
        self.absolute_char_pos_start = self.buffer.line_to_char(self.top_line);
        self.absolute_char_pos_end = self.buffer.line_to_char(self.top_line + 100);
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

    pub fn set_selection(&mut self, mode: SelectionMode, count: u32, extend_current_selection: bool) {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        match mode {
            SelectionMode::Left | SelectionMode::Right => {
                self.caret_char_pos = caret_absolute_pos;
                if self.caret_char_pos > 0 {
                    if mode == SelectionMode::Left {
                        self.caret_char_pos -= 1;
                    }
                    else {
                        self.caret_char_pos += 1;
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
                    dx_ok!(((*self.text_layout).HitTestTextPosition(
                        (self.caret_char_pos - self.absolute_char_pos_start) as u32,
                        self.caret_is_trailing,
                        &mut caret_pos.0,
                        &mut caret_pos.1,
                        metrics_uninit.as_mut_ptr()
                    )));

                    let metrics = metrics_uninit.assume_init();

                    if caret_pos.0 < self.cached_mouse_width {
                        caret_pos.0 = self.cached_mouse_width;
                    }
                    else {
                        self.cached_mouse_width = caret_pos.0;
                    }

                    if mode == SelectionMode::Down {
                        caret_pos.1 += metrics.height;
                    }
                    else {
                        caret_pos.1 -= metrics.height;
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
        match mode {
            MouseSelectionMode::Click => {
                let mut is_inside = 0;
                let mut metrics_uninit = MaybeUninit::<DWRITE_HIT_TEST_METRICS>::uninit();

                unsafe {
                    dx_ok!(
                        (*self.text_layout).HitTestPoint(
                            mouse_pos.0,
                            mouse_pos.1,
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
                                mouse_pos.0,
                                mouse_pos.1,
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

    pub fn get_caret_rect(&mut self) -> Option<D2D1_RECT_F> {
        if self.caret_char_pos < self.absolute_char_pos_start {
            return None
        }

        let mut caret_pos: (f32, f32) = (0.0, 0.0);
        let mut metrics_uninit = MaybeUninit::<DWRITE_HIT_TEST_METRICS>::uninit();

        unsafe {
            dx_ok!(((*self.text_layout).HitTestTextPosition(
                (self.caret_char_pos - self.absolute_char_pos_start) as u32,
                self.caret_is_trailing,
                &mut caret_pos.0,
                &mut caret_pos.1,
                metrics_uninit.as_mut_ptr()
            )));

            let metrics = metrics_uninit.assume_init();

            let rect = D2D1_RECT_F {
                left: caret_pos.0 - self.half_caret_width as f32,
                top: caret_pos.1,
                right: caret_pos.0 + (self.caret_width - self.half_caret_width) as f32,
                bottom: caret_pos.1 + metrics.height
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

        return Some(range);
    }

    pub fn get_layout(&mut self, layout_box: (f32, f32)) -> *mut IDWriteTextLayout {
        let lines = self.get_current_lines();

        unsafe {
            if !self.text_layout.is_null() {
                (*self.text_layout).Release();
            }

            dx_ok!((*self.write_factory).CreateTextLayout(
                lines.as_ptr(),
                lines.len() as u32,
                self.text_format,
                layout_box.0,
                layout_box.1,
                &mut self.text_layout as *mut *mut _
            ));
        }

        return self.text_layout;
    }

    pub fn get_current_lines(&self) -> Vec<u16> {
        return self.text_range(self.absolute_char_pos_start..self.absolute_char_pos_end);
    }

    fn text_range<R>(&self, char_range: R) -> Vec<u16> where R: RangeBounds<usize> {
        let rope_slice = self.buffer.slice(char_range);
        let chars : Vec<u8> = rope_slice.bytes().collect();
        return OsStr::new(str::from_utf8(chars.as_ref()).unwrap()).encode_wide().chain(once(0)).collect();
    }
}
