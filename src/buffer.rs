use core::ops::RangeBounds;
use std::{
    cell::RefCell,
    cmp::{ min, max },
    fs::File,
    ffi::OsStr,
    iter::once,
    os::windows::ffi::OsStrExt,
    ptr::{copy_nonoverlapping, null_mut},
    mem::{ swap, MaybeUninit },
    rc::Rc,
    char,
    str,
};
use winapi::{
    um::{
        dwrite::{ IDWriteTextLayout, DWRITE_HIT_TEST_METRICS, DWRITE_TEXT_RANGE },
        d2d1::{ D2D1_RECT_F, D2D1_LAYER_PARAMETERS },
        winbase::{ GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock, GlobalSize, GMEM_DDESHARE, GMEM_ZEROINIT},
        winuser::{ SystemParametersInfoW, SPI_GETCARETWIDTH, OpenClipboard, CloseClipboard,
            EmptyClipboard, GetClipboardData, SetClipboardData, CF_TEXT}
    },
    shared::windef::HWND,
    ctypes::c_void
};
use ropey::Rope;

use crate::dx_ok;
use crate::settings::NUMBER_OF_SPACES_PER_TAB;
use crate::lsp_structs::*;
use crate::language_support::*;
use crate::renderer::TextRenderer;

#[derive(PartialEq)]
pub enum SelectionMode {
    Left,
    Right,
    Down,
    Up
}

#[derive(PartialEq)]
pub enum MouseSelectionMode {
    Click,
    Move
}

#[derive(Debug, PartialEq)]
pub enum CharType {
    Word,
    Punctuation,
    Linebreak
}

pub enum CharSearchDirection {
    Forward,
    Backward
}

struct TemporaryEdit {
    pub add_line: bool,
    pub line_num: usize
}

pub struct TextBuffer {
    buffer: Rope,

    // The layout of the text buffer should be public for
    // the renderer to use
    pub origin: (f32, f32),
    pub extents: (f32, f32),
    pub text_origin: (f32, f32),
    pub text_extents: (f32, f32),
    pub text_visible_line_count: usize,
    pub text_column_offset: usize,
    pub line_numbers_origin: (f32, f32),
    pub line_numbers_extents: (f32, f32),
    pub line_numbers_margin: f32,

    // The selection state of the buffer should be public
    // for the editor to use
    pub currently_selecting: bool,

    // The language of the text buffer as
    // identified by its extension
    pub language_identifier: &'static str,

    top_line: usize,
    bot_line: usize,
    absolute_char_pos_start: usize,
    absolute_char_pos_end: usize,

    caret_char_anchor: usize,
    caret_char_pos: usize,
    caret_is_trailing: i32,
    caret_width: u32,
    half_caret_width: u32,

    cached_char_offset: u32,

    text_layer_params: D2D1_LAYER_PARAMETERS,
    text_layout: *mut IDWriteTextLayout,

    line_numbers_layer_params: D2D1_LAYER_PARAMETERS,
    line_numbers_layout: *mut IDWriteTextLayout,

    renderer: Rc<RefCell<TextRenderer>>,

    lsp_versioned_identifier: VersionedTextDocumentIdentifier,
    semantic_tokens: Vec<u32>,
    semantic_tokens_edits: Vec<TemporaryEdit>
}

impl TextBuffer {
    pub fn new(path: &str, language_identifier: &'static str, origin: (f32, f32), extents: (f32, f32), renderer: Rc<RefCell<TextRenderer>>) -> TextBuffer {
        let file = File::open(path).unwrap();
        let buffer = Rope::from_reader(file).unwrap();

        let mut caret_width: u32 = 0;
        unsafe {
            // We'll increase the width from the system width slightly
            SystemParametersInfoW(SPI_GETCARETWIDTH, 0, (&mut caret_width as *mut _) as *mut c_void, 0);
            caret_width *= 3;
        }

        let mut text_buffer = TextBuffer {
            buffer,

            origin,
            extents,
            text_origin: (0.0, 0.0),
            text_extents: (0.0, 0.0),
            text_visible_line_count: 0,
            text_column_offset: 0,
            line_numbers_origin: (0.0, 0.0),
            line_numbers_extents: (0.0, 0.0),
            line_numbers_margin: 0.0,

            currently_selecting: false,

            language_identifier,

            top_line: 0,
            bot_line: 0,
            absolute_char_pos_start: 0,
            absolute_char_pos_end: 0,

            caret_char_anchor: 0,
            caret_char_pos: 0,
            caret_is_trailing: 0,
            caret_width,
            half_caret_width: caret_width / 2,

            cached_char_offset: 0,

            text_layer_params: unsafe { MaybeUninit::<D2D1_LAYER_PARAMETERS>::zeroed().assume_init() },
            text_layout: null_mut(),

            line_numbers_layer_params: unsafe { MaybeUninit::<D2D1_LAYER_PARAMETERS>::zeroed().assume_init() },
            line_numbers_layout: null_mut(),

            renderer,

            lsp_versioned_identifier: VersionedTextDocumentIdentifier {
                uri: "file:///".to_owned() + path,
                version: 0
            },
            semantic_tokens: Vec::new(),
            semantic_tokens_edits: Vec::new()
        };

        text_buffer.update_metrics(origin, extents);
        text_buffer
    }

    pub fn get_full_did_change_notification(&mut self) -> DidChangeNotification {
        // Update the file version and return the change notification
        self.lsp_versioned_identifier.version += 1;
        let change_event = TextDocumentContentChangeEvent {
            text: self.buffer.to_string(),
            range: None
        };
        DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![change_event])
    }

    pub fn update_semantic_tokens(&mut self, data: Vec<u32>) {
        self.semantic_tokens = data;
        self.semantic_tokens_edits.clear();
    }

    pub fn get_uri(&self) -> String {
        return self.lsp_versioned_identifier.uri.clone();
    }

    pub fn get_caret_absolute_pos(&self) -> usize {
        self.caret_char_pos + (self.caret_is_trailing as usize)
    }

    pub fn scroll_down(&mut self, lines_per_roll: usize) {
        let new_top = self.top_line + lines_per_roll;
        if new_top >= self.buffer.len_lines() {
            self.top_line = self.buffer.len_lines() - 1;
        }
        else {
            self.top_line = new_top;
        }
        self.update_absolute_char_positions();
    }

    pub fn scroll_up(&mut self, lines_per_roll: usize) {
        if self.top_line >= lines_per_roll {
            self.top_line -= lines_per_roll;
        }
        else {
            self.top_line = 0;
        }
        self.update_absolute_char_positions();
    }

    pub fn scroll_left(&mut self, lines_per_roll: usize) {
        if self.text_column_offset >= lines_per_roll {
            self.text_column_offset -= lines_per_roll;
        }
        else {
            self.text_column_offset = 0;
        }
    }

    pub fn scroll_right(&mut self, lines_per_roll: usize) {
        let max_columns_in_text_region = (self.text_extents.0 / self.renderer.borrow().font_width) as usize;
        let current_line = self.buffer.char_to_line(self.get_caret_absolute_pos());
        let line_length = self.buffer.line(current_line).len_chars();
        let new_offset = self.text_column_offset + lines_per_roll;
        if line_length > max_columns_in_text_region && new_offset > (line_length - max_columns_in_text_region) {
            self.text_column_offset = line_length - max_columns_in_text_region;
        }
        else if line_length > max_columns_in_text_region{
            self.text_column_offset = new_offset;
        }
    }


    fn is_linebreak(&self, chr: char) -> bool {
        return chr == '\n'
            || chr == '\r'
            || chr == '\u{000B}'
            || chr == '\u{000C}'
            || chr == '\u{000D}'
            || chr == '\u{0085}'
            || chr == '\u{2028}'
            || chr == '\u{2029}';
    }

    // Underscore is treated as part of a word to make movement
    // programming in snake_case easier.
    fn is_word(&self, chr: char) -> bool {
        return chr.is_alphanumeric() || chr == '_';
    }

    fn get_char_type(&self, chr: char) -> CharType {
        match chr {
            x if self.is_word(x) => CharType::Word,
            x if self.is_linebreak(x) => CharType::Linebreak,
            _ => CharType::Punctuation
        }
    }

    // Finds the number of characters until a boundary
    // A boundary is defined to be punctuation when the
    // current char is inside a word, and alphanumeric otherwise
    // bool specifies the direction to search in,
    // true for right false for left
    fn get_boundary_char_count(&self, search_direction: CharSearchDirection) -> usize {
        let caret_absolute_pos = self.get_caret_absolute_pos();
        let mut count = 0;

        match search_direction {
            CharSearchDirection::Forward => {
                if caret_absolute_pos == self.buffer.len_chars() {
                    return 0;
                }
                let current_char_type = self.get_char_type(self.buffer.char(self.caret_char_pos));
                let mut chars = self.buffer.chars_at(self.get_caret_absolute_pos());
                while let Some(chr) = chars.next() {
                    if self.get_char_type(chr) != current_char_type {
                        break;
                    }
                    count += 1;
                }
            },
            CharSearchDirection::Backward => {
                if caret_absolute_pos == 0 {
                    return 0;
                }
                let current_char_type = self.get_char_type(self.buffer.char(self.caret_char_pos));
                let mut chars = self.buffer.chars_at(self.caret_char_pos);
                while let Some(chr) = chars.prev() {
                    if self.get_char_type(chr) != current_char_type {
                        break;
                    }
                    count += 1;
                }
            }
        }

        count
    }

    pub fn move_left(&mut self, shift_down: bool) {
        let mut count = 1;
        if self.see_prev_chars("\r\n") {
            count = 2;
        }
        self.set_selection(SelectionMode::Left, count, shift_down);
    }

    pub fn move_left_by_word(&mut self, shift_down: bool) {
        // Start by moving left atleast once, then get the boundary count
        self.set_selection(SelectionMode::Left, 1, shift_down);
        let count = self.get_boundary_char_count(CharSearchDirection::Backward);
        self.set_selection(SelectionMode::Left, count, shift_down);
    }

    pub fn move_right(&mut self, shift_down: bool) {
        let mut count = 1;
        if self.see_chars("\r\n") {
            count = 2;
        }
        self.set_selection(SelectionMode::Right, count, shift_down);
    }

    pub fn move_right_by_word(&mut self, shift_down: bool) {
        let count = self.get_boundary_char_count(CharSearchDirection::Forward);
        self.set_selection(SelectionMode::Right, count, shift_down);
    }

    pub fn left_click(&mut self, mouse_pos: (f32, f32), extend_current_selection: bool) {
        self.set_mouse_selection(MouseSelectionMode::Click, mouse_pos);
        if !extend_current_selection {
            self.caret_char_anchor = self.get_caret_absolute_pos();
        }
        self.currently_selecting = true;

        // Reset the cached width
        self.cached_char_offset = 0;
    }

    pub fn left_double_click(&mut self, mouse_pos: (f32, f32)) {
        self.set_mouse_selection(MouseSelectionMode::Click, mouse_pos);

        // Find the boundary on each side of the cursor
        let left_count = self.get_boundary_char_count(CharSearchDirection::Backward);
        let right_count = self.get_boundary_char_count(CharSearchDirection::Forward);

        // Set the caret position at the right edge
        self.caret_char_pos += right_count;

        // Set the anchor position at the left edge
        self.caret_char_anchor = self.caret_char_pos - (left_count + right_count);
    }

    pub fn left_release(&mut self) {
        self.currently_selecting = false;
    }

    fn linebreaks_before_line(&self, line: usize) -> usize {
        let mut line_start = self.buffer.chars_at(self.buffer.line_to_char(line));
        match line_start.prev() {
            Some('\n') => {
                if line_start.prev() == Some('\r') {
                    return 2;
                }
                else {
                    return 1;
                }
            },

            // For completeness, we will count all linebreaks
            // that ropey supports
            Some('\u{000B}') => 1,
            Some('\u{000C}') => 1,
            Some('\u{000D}') => 1,
            Some('\u{0085}') => 1,
            Some('\u{2028}') => 1,
            Some('\u{2029}') => 1,
            _ => 0
        }
    }

    pub fn set_selection(&mut self, mode: SelectionMode, count: usize, extend_current_selection: bool) {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        match mode {
            SelectionMode::Left | SelectionMode::Right => {
                self.caret_char_pos = caret_absolute_pos;

                if mode == SelectionMode::Left {
                    if self.caret_char_pos > 0 {
                        self.caret_char_pos -= count;
                    }
                }
                else {
                    if (self.caret_char_pos + count) <= self.buffer.len_chars() {
                        self.caret_char_pos += count;
                    }
                    else {
                        self.caret_char_pos = self.buffer.len_chars();
                    }
                }
                self.caret_is_trailing = 0;

                // Reset the cached width
                self.cached_char_offset = 0;
            },
            SelectionMode::Up | SelectionMode::Down => {
                let current_line = self.buffer.char_to_line(caret_absolute_pos);

                let target_line_idx;
                let target_linebreak_count;
                if mode == SelectionMode::Up {
                    // If we're on the first line, return
                    if current_line == 0 {
                        return;
                    }
                    target_line_idx = current_line - 1;
                    target_linebreak_count = self.linebreaks_before_line(current_line);
                }
                else {
                    // If we're on the last line, return
                    if current_line == self.buffer.len_lines() - 1 {
                        return;
                    }
                    target_line_idx = current_line + 1;
                    target_linebreak_count = self.linebreaks_before_line(target_line_idx);
                }

                let target_line = self.buffer.line(target_line_idx);
                let target_line_length = target_line.len_chars().saturating_sub(target_linebreak_count);

                let current_offset = caret_absolute_pos - self.buffer.line_to_char(current_line);
                let desired_offset = max(self.cached_char_offset, current_offset as u32);
                self.cached_char_offset = desired_offset;

                let new_offset = min(target_line_length, desired_offset as usize);

                self.caret_char_pos = self.buffer.line_to_char(target_line_idx) + new_offset;
                self.caret_is_trailing = 0;

                if target_line_idx >= self.bot_line {
                    self.scroll_down(1);
                }
                else if target_line_idx < self.top_line {
                    self.scroll_up(1);
                }
            },
        }

        if !extend_current_selection {
            self.caret_char_anchor = self.get_caret_absolute_pos();
        }

        self.update_text_column_offset();
    }

    pub fn set_mouse_selection(&mut self, mode: MouseSelectionMode, mouse_pos: (f32, f32)) {
        let relative_mouse_pos = self.translate_mouse_pos_to_text_region(mouse_pos);

        if mode == MouseSelectionMode::Click || (mode == MouseSelectionMode::Move && self.currently_selecting) {
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

                self.caret_char_pos = min(self.absolute_char_pos_start + absolute_text_pos, self.buffer.len_chars());
            }

            // If we're at the end of the rope, the caret may not be trailing
            // otherwise we will be inserting out of bounds on the rope
            if self.caret_char_pos == self.buffer.len_chars() {
                self.caret_is_trailing = 0;
            }
        }
    }

    pub fn select_all(&mut self) {
        self.caret_char_anchor = 0;
        self.caret_is_trailing = 0;
        self.caret_char_pos = self.buffer.len_chars();
        self.update_text_column_offset();
    }

    fn translate_mouse_pos_to_text_region(&self, mouse_pos: (f32, f32)) -> (f32, f32) {
        let view_origin = self.get_view_origin();

        let dx = mouse_pos.0 - view_origin.0;
        let dy = mouse_pos.1 - view_origin.1;
        (dx, dy)
    }

    pub fn delete_selection(&mut self) -> TextDocumentContentChangeEvent {
        let caret_absolute_pos = self.get_caret_absolute_pos();
        let line = self.buffer.char_to_line(caret_absolute_pos);
        let character_position_in_line = caret_absolute_pos - self.buffer.line_to_char(line);

        let change_event;
        if caret_absolute_pos < self.caret_char_anchor {
            let end_line = self.buffer.char_to_line(self.caret_char_anchor);
            let character_position_in_end_line = self.caret_char_anchor - self.buffer.line_to_char(end_line);
            self.buffer.remove(caret_absolute_pos..self.caret_char_anchor);

            self.caret_char_pos = caret_absolute_pos;
            self.caret_char_anchor = self.caret_char_pos;

            change_event = TextDocumentContentChangeEvent {
                text: "".to_owned(),
                range: Some(Range {
                    start: Position::new(line as i64, character_position_in_line as i64),
                    end: Position::new(end_line as i64, character_position_in_end_line as i64),
                })
            };
        }
        else {
            let start_line = self.buffer.char_to_line(self.caret_char_anchor);
            let character_position_in_start_line = self.caret_char_anchor - self.buffer.line_to_char(start_line);
            self.buffer.remove(self.caret_char_anchor..caret_absolute_pos);

            let caret_anchor_delta = caret_absolute_pos - self.caret_char_anchor;
            self.caret_char_pos = caret_absolute_pos - caret_anchor_delta;

            change_event = TextDocumentContentChangeEvent {
                text: "".to_owned(),
                range: Some(Range {
                    start: Position::new(start_line as i64, character_position_in_start_line as i64),
                    end: Position::new(line as i64, character_position_in_line as i64),
                })
            };
        }
        self.caret_is_trailing = 0;
        self.update_absolute_char_positions();

        // Return the change event
        return change_event;
    }

    pub fn insert_chars(&mut self, chars: &str) -> DidChangeNotification {
        let mut changes = Vec::new();

        let mut caret_absolute_pos = self.get_caret_absolute_pos();
        // If we are currently selecting text, 
        // delete text before insertion
        if self.get_caret_absolute_pos() != self.caret_char_anchor {
            changes.push(self.delete_selection());
            caret_absolute_pos = self.get_caret_absolute_pos();
        }

        let line = self.buffer.char_to_line(caret_absolute_pos);
        let character_position_in_line = caret_absolute_pos - self.buffer.line_to_char(line);

        // Add the newline to the temporary edits to preserve
        // semantic highlighting until new highlights are resolved
        // by the language server
        if chars == "\r\n" {
            self.semantic_tokens_edits.push(TemporaryEdit {
                add_line: true,
                line_num: line + 1
            })
        }

        self.buffer.insert(caret_absolute_pos, chars);
        self.set_selection(SelectionMode::Right, chars.len(), false);

        self.update_absolute_char_positions();
        self.update_text_column_offset();
        
        // Update the file version and return the change notification
        self.lsp_versioned_identifier.version += 1;
        let change_event = TextDocumentContentChangeEvent {
            text: chars.to_owned(),
            range: Some(Range {
                start: Position::new(line as i64, character_position_in_line as i64),
                end: Position::new(line as i64, character_position_in_line as i64),
            })
        };

        changes.push(change_event);
        DidChangeNotification::new(self.lsp_versioned_identifier.clone(), changes)
    }

    pub fn insert_char(&mut self, character: u16) -> DidChangeNotification {
        let mut changes = Vec::new();
        let mut caret_absolute_pos = self.get_caret_absolute_pos();

        // If we are currently selecting text, 
        // delete text before insertion
        if self.get_caret_absolute_pos() != self.caret_char_anchor {
            changes.push(self.delete_selection());
            caret_absolute_pos = self.get_caret_absolute_pos();
        }

        let line = self.buffer.char_to_line(caret_absolute_pos);
        let character_position_in_line = caret_absolute_pos - self.buffer.line_to_char(line);

        self.buffer.insert_char(caret_absolute_pos, (character as u8) as char);
        self.set_selection(SelectionMode::Right, 1, false);

        self.update_absolute_char_positions();
        self.update_text_column_offset();

        // Update the file version and return the change notification
        self.lsp_versioned_identifier.version += 1;
        let change_event = TextDocumentContentChangeEvent {
            text: ((character as u8) as char).to_string(),
            range: Some(Range {
                start: Position::new(line as i64, character_position_in_line as i64),
                end: Position::new(line as i64, character_position_in_line as i64),
            })
        };

        changes.push(change_event);
        DidChangeNotification::new(self.lsp_versioned_identifier.clone(), changes)
    }

    fn see_chars(&mut self, string: &str) -> bool {
        let mut rope_iterator = self.buffer.chars_at(self.get_caret_absolute_pos());
        for chr in string.chars() {
            match rope_iterator.next() {
                Some(x) if x == chr => continue,
                _ => return false,
            }
        }
        true
    }

    fn see_prev_chars(&mut self, string: &str) -> bool {
        let mut rope_iterator = self.buffer.chars_at(self.get_caret_absolute_pos());
        for chr in string.chars().rev() {
            match rope_iterator.prev() {
                Some(x) if x == chr => continue,
                _ => return false,
            }
        }
        true
    }

    pub fn delete_right(&mut self) -> DidChangeNotification {
        let caret_absolute_pos = self.get_caret_absolute_pos();
        let line = self.buffer.char_to_line(caret_absolute_pos);
        let character_position_in_line = caret_absolute_pos - self.buffer.line_to_char(line);
        let len_lines = self.buffer.len_lines();

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            self.lsp_versioned_identifier.version += 1;
            return DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![self.delete_selection()]);
        }

        // In case of a CRLF, delete both characters
        let mut offset = 1;
        if self.see_chars("\r\n") {
            offset = 2;
        }
        // In case of a <TAB>, delete the corresponding
        // number of spaces
        else if self.see_chars(" ".repeat(NUMBER_OF_SPACES_PER_TAB).as_str()) {
            offset = NUMBER_OF_SPACES_PER_TAB;
        }

        // In case the next line is also empty, we'll effectively
        // be deleting the next line instead of the current one
        let double_crlf = self.see_chars("\r\n\r\n");

        let next_char_pos = min(caret_absolute_pos + offset, self.buffer.len_chars());
        self.buffer.remove(caret_absolute_pos..next_char_pos);
        self.update_absolute_char_positions();

        let new_line = self.buffer.char_to_line(next_char_pos);
        let new_character_position_in_line = next_char_pos - self.buffer.line_to_char(new_line);
        
        // In case the line has changed,
        // insert temporary edit to preserve semantic highlights
        // until the LSP server resolves the edit
        if len_lines > self.buffer.len_lines() {
            if double_crlf {
                self.semantic_tokens_edits.push(TemporaryEdit {
                    add_line: false,
                    line_num: line + 1
                })
            }
            else {
                self.semantic_tokens_edits.push(TemporaryEdit {
                    add_line: false,
                    line_num: line
                })
            }
        }

        // Update the file version and return the change event
        self.lsp_versioned_identifier.version += 1;
        let change_event = TextDocumentContentChangeEvent {
            text: "".to_owned(),
            range: Some(Range {
                start: Position::new(line as i64, character_position_in_line as i64),
                end: Position::new(new_line as i64, new_character_position_in_line as i64),
            })
        };
        DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![change_event])
    }

    pub fn delete_right_by_word(&mut self) -> DidChangeNotification {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            self.lsp_versioned_identifier.version += 1;
            return DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![self.delete_selection()]);
        }

        let count = self.get_boundary_char_count(CharSearchDirection::Forward);
        self.set_selection(SelectionMode::Right, count, true);

        self.lsp_versioned_identifier.version += 1;
        return DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![self.delete_selection()]);
    }

    pub fn delete_left(&mut self) -> DidChangeNotification {
        let caret_absolute_pos = self.get_caret_absolute_pos();
        let line = self.buffer.char_to_line(caret_absolute_pos);
        let character_position_in_line = caret_absolute_pos - self.buffer.line_to_char(line);
        let len_lines = self.buffer.len_lines();

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            self.lsp_versioned_identifier.version += 1;
            return DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![self.delete_selection()]);
        }

        // In case of a CRLF, delete both characters
        // Also insert a temporary edit
        let mut offset = 1;
        if self.see_prev_chars("\r\n") {
            offset = 2;
        }
        // In case of a <TAB>, delete the corresponding
        // number of spaces
        else if self.see_prev_chars(" ".repeat(NUMBER_OF_SPACES_PER_TAB).as_str()) {
            offset = NUMBER_OF_SPACES_PER_TAB;
        }

        // In case the previous line is also empty, we'll effectively
        // be deleting the next line instead of the current one
        let double_crlf = self.see_prev_chars("\r\n\r\n");

        let previous_char_pos = caret_absolute_pos.saturating_sub(offset);
        self.buffer.remove(previous_char_pos..caret_absolute_pos);
        self.set_selection(SelectionMode::Left, offset, false);
        self.update_absolute_char_positions();

        let new_line = self.buffer.char_to_line(previous_char_pos);
        let new_character_position_in_line = previous_char_pos - self.buffer.line_to_char(new_line);

        // In case the line has changed,
        // insert temporary edit to preserve semantic highlights
        // until the LSP server resolves the edit
        if len_lines > self.buffer.len_lines() {
            if double_crlf {
                self.semantic_tokens_edits.push(TemporaryEdit {
                    add_line: false,
                    line_num: line - 1
                })
            }
            else {
                self.semantic_tokens_edits.push(TemporaryEdit {
                    add_line: false,
                    line_num: line
                })
            }
        }

        // Update the file version and return the change event
        self.lsp_versioned_identifier.version += 1;
        let change_event = TextDocumentContentChangeEvent {
            text: "".to_owned(),
            range: Some(Range {
                start: Position::new(new_line as i64, new_character_position_in_line as i64),
                end: Position::new(line as i64, character_position_in_line as i64),
            })
        };
        DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![change_event])
    }

    pub fn delete_left_by_word(&mut self) -> DidChangeNotification {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            self.lsp_versioned_identifier.version += 1;
            return DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![self.delete_selection()]);
        }

        // Start by moving left atleast once, then get the boundary count
        self.set_selection(SelectionMode::Left, 1, true);
        let count = self.get_boundary_char_count(CharSearchDirection::Backward);
        self.set_selection(SelectionMode::Left, count, true);

        self.lsp_versioned_identifier.version += 1;
        return DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![self.delete_selection()]);
    }

    // Parses and creates ranges of highlight information directly
    // from the text buffer displayed on the screen
    pub fn get_lexical_highlights(&mut self) -> Vec<(DWRITE_TEXT_RANGE, SemanticTokenTypes)> {
        let text_in_current_view = self.buffer.slice(self.absolute_char_pos_start..self.absolute_char_pos_end).to_string();
        highlight_text(text_in_current_view.as_str(), self.language_identifier)
    }

    // Processes the semantic tokens received from the language server
    pub fn get_semantic_highlights(&mut self) -> Vec<(DWRITE_TEXT_RANGE, SemanticTokenTypes)> {
        let top_line_absolute_pos = self.buffer.line_to_char(self.top_line);
        let mut highlights = Vec::new();

        // This is only safe because the semantic token data
        // always comes in multiples of 5
        let mut i = 0;
        let mut line = 0;
        let mut start = 0;

        while i < self.semantic_tokens.len() {
            let delta_line = self.semantic_tokens[i];
            line += delta_line;

            // Early continue if line is above the current view
            // Early break if line is below the current view
            if line < self.top_line as u32 {
                i += 5;
                continue;
            }
            else if line > min(self.buffer.len_lines(), self.bot_line) as u32 {
                break;
            }

            let delta_start = self.semantic_tokens[i + 1];
            if delta_line == 0 {
                start += delta_start;
            }
            else {
                start = delta_start;
            }
            let length = self.semantic_tokens[i + 2];

            let mut line_offset: i32 = 0;
            for edit in &self.semantic_tokens_edits {
                if edit.add_line && line as usize >= edit.line_num {
                    line_offset += 1;
                }
                else if !edit.add_line && line as usize >= edit.line_num {
                    line_offset -= 1;
                }
            }

            match self.language_identifier {
                CPP_LANGUAGE_IDENTIFIER => {
                    let token_type = CppSemanticTokenTypes::to_semantic_token_type(CppSemanticTokenTypes::from_u32(self.semantic_tokens[i + 3]));
                    let line_absolute_pos = self.buffer.line_to_char((line as i32 + line_offset) as usize);
                    let range = DWRITE_TEXT_RANGE {
                        startPosition: ((line_absolute_pos + start as usize) - top_line_absolute_pos) as u32,
                        length
                    };
                    highlights.push((range, token_type));
                },
                RUST_LANGUAGE_IDENTIFIER => {
                    let token_type = RustSemanticTokenTypes::to_semantic_token_type(RustSemanticTokenTypes::from_u32(self.semantic_tokens[i + 3]));
                    let line_absolute_pos = self.buffer.line_to_char((line as i32 + line_offset) as usize);
                    let range = DWRITE_TEXT_RANGE {
                        startPosition: ((line_absolute_pos + start as usize) - top_line_absolute_pos) as u32,
                        length
                    };
                    highlights.push((range, token_type));

                    // We don't currently use the modifiers for highlighting
                    let _  = RustSemanticTokenModifiers::from_u32(self.semantic_tokens[i + 4]);
                },
                _ => return Vec::new()
            }

            i += 5;
        }

        highlights
    }

    pub fn get_view_origin(&self) -> (f32, f32) {
        return (
            self.text_origin.0 - self.text_column_offset as f32 * self.renderer.borrow().font_width,
            self.text_origin.1
        );
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

            let column_offset = self.text_column_offset as f32 * self.renderer.borrow().font_width;
            let rect = D2D1_RECT_F {
                left: self.text_origin.0 - column_offset + caret_pos.0 - self.half_caret_width as f32,
                top: self.text_origin.1 + caret_pos.1,
                right: self.text_origin.0 - column_offset + caret_pos.0 + (self.caret_width - self.half_caret_width) as f32,
                bottom: self.text_origin.1 + caret_pos.1 + metrics.height
            };

            return Some(rect)
        }
    }

    fn get_selection_data(&self) -> String {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        if self.caret_char_anchor > caret_absolute_pos {
            self.buffer.slice(caret_absolute_pos..min(self.caret_char_anchor, self.buffer.len_chars() - 1)).to_string()
        }
        else if self.caret_char_anchor < caret_absolute_pos {
            self.buffer.slice(self.caret_char_anchor..min(caret_absolute_pos, self.buffer.len_chars() - 1)).to_string()
        }
        // If nothing is selected, copy current line
        else {
            self.buffer.line(self.buffer.char_to_line(caret_absolute_pos)).to_string()
        }
    }

    pub fn copy_selection(&mut self, hwnd: HWND) {
        unsafe {
            if OpenClipboard(hwnd) > 0 {
                if EmptyClipboard() > 0 {
                    let data = self.get_selection_data();
                    // +1 since str.len() returns the length minus the null-byte
                    let byte_size = data.len() + 1;
                    let clipboard_data_ptr = GlobalAlloc(GMEM_DDESHARE | GMEM_ZEROINIT, byte_size);
                    if !clipboard_data_ptr.is_null() {
                        let memory = GlobalLock(clipboard_data_ptr);
                        if !memory.is_null() {
                            copy_nonoverlapping(data.as_ptr(), memory as *mut u8, byte_size);
                            GlobalUnlock(clipboard_data_ptr);

                            // If setting the clipboard data fails, free it
                            // otherwise its now owned by the system
                            if SetClipboardData(CF_TEXT, clipboard_data_ptr).is_null() {
                                GlobalFree(clipboard_data_ptr);
                            }
                        }
                        else {
                            GlobalFree(clipboard_data_ptr);
                        }
                    }
                }
                CloseClipboard();
            }
        }
    }

    pub fn cut_selection(&mut self, hwnd: HWND) -> DidChangeNotification {
        // Copy the selection
        self.copy_selection(hwnd);

        let caret_absolute_pos = self.get_caret_absolute_pos();
        // If we're selecting text, delete it
        // otherwise delete the current line
        if caret_absolute_pos != self.caret_char_anchor {
            self.lsp_versioned_identifier.version += 1;
            return DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![self.delete_selection()]);
        }

        let current_line_idx = self.buffer.char_to_line(caret_absolute_pos);
        let current_line = self.buffer.line(current_line_idx);
        let current_line_chars = self.buffer.line_to_char(current_line_idx);
        let current_line_length = current_line.len_chars();

        // Update caret position
        self.caret_char_pos = current_line_chars;
        self.caret_is_trailing = 0;
        self.caret_char_anchor = self.caret_char_pos;

        self.buffer.remove(current_line_chars..current_line_chars + current_line_length);
        self.update_absolute_char_positions();

        // Add a temporary semantic highlight edit
        self.semantic_tokens_edits.push(TemporaryEdit {
            add_line: false,
            line_num: min(current_line_idx + 1, self.buffer.len_lines() - 1)
        });

        let change_event = TextDocumentContentChangeEvent {
            text: "".to_owned(),
            range: Some(Range {
                start: Position::new(current_line_idx as i64, 0 as i64),
                end: Position::new((current_line_idx + 1) as i64, 0 as i64),
            })
        };
        self.lsp_versioned_identifier.version += 1;
        return DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![change_event]);
    }

    pub fn paste(&mut self, hwnd: HWND) -> Option<DidChangeNotification> {
        let mut did_change_notification: Option<DidChangeNotification> = None;
        unsafe {
            if OpenClipboard(hwnd) > 0 {
                let clipboard_data_ptr = GetClipboardData(CF_TEXT);
                if !clipboard_data_ptr.is_null() {
                    let byte_size = GlobalSize(clipboard_data_ptr);
                    let memory = GlobalLock(clipboard_data_ptr);

                    let slice: &[u8] = core::slice::from_raw_parts_mut(memory as *mut u8, byte_size as usize);

                    // Convert back to &str and trim the trailing null-byte
                    let chars = std::str::from_utf8_unchecked(slice).trim_end_matches('\0');

                    did_change_notification = Some(self.insert_chars(chars));
                    GlobalUnlock(clipboard_data_ptr);
                }

                CloseClipboard();
            }
        }

        did_change_notification
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

    pub fn get_text_layout(&mut self) -> (*mut IDWriteTextLayout, D2D1_LAYER_PARAMETERS) {
        let lines = self.get_current_lines();

        unsafe {
            if !self.text_layout.is_null() {
                (*self.text_layout).Release();
            }

            dx_ok!((*self.renderer.borrow().write_factory).CreateTextLayout(
                lines.as_ptr(),
                lines.len() as u32,
                self.renderer.borrow().text_format,
                self.text_extents.0,
                self.text_extents.1,
                &mut self.text_layout as *mut *mut _
            ));
        }

        (self.text_layout, self.text_layer_params)
    }

    pub fn get_line_numbers_layout(&mut self) -> (*mut IDWriteTextLayout, D2D1_LAYER_PARAMETERS) {
        let mut nums: String = String::new();
        let number_range_end = min(self.buffer.len_lines() - 1, self.bot_line);

        for i in self.top_line..=number_range_end {
            nums += (i + 1).to_string().as_str();
            nums += "\r\n";
        }
        let lines: Vec<u16> = OsStr::new(nums.as_str()).encode_wide().chain(once(0)).collect();

        unsafe {
            if !self.line_numbers_layout.is_null() {
                (*self.line_numbers_layout).Release();
            }

            dx_ok!((*self.renderer.borrow().write_factory).CreateTextLayout(
                lines.as_ptr(),
                lines.len() as u32,
                self.renderer.borrow().text_format,
                self.line_numbers_extents.0,
                self.line_numbers_extents.1,
                &mut self.line_numbers_layout as *mut *mut _
            ));
        }

        (self.line_numbers_layout, self.line_numbers_layer_params)
    }

    pub fn update_metrics(&mut self, origin: (f32, f32), extents: (f32, f32)) {
        self.origin = origin;
        self.extents = extents;

        self.update_line_numbers_margin();
        self.update_text_region();
        self.update_numbers_region();
        self.update_text_visible_line_count();
        self.update_text_column_offset();
        self.update_absolute_char_positions();
    }

    fn update_line_numbers_margin(&mut self) {
        let end_line_max_digits = self.get_digits_in_number(self.buffer.len_lines() as u32);
        let font_width = self.renderer.borrow().font_width;
        self.line_numbers_margin = (end_line_max_digits as f32 * font_width) + (font_width / 2.0);
    }

    fn update_text_region(&mut self) {
        self.text_origin = (
            self.origin.0 + self.line_numbers_margin,
            self.origin.1
        );
        self.text_extents = (
            self.extents.0 - self.line_numbers_margin,
            self.extents.1
        );
        self.text_layer_params = TextRenderer::layer_params(self.text_origin, self.text_extents);
    }

    fn update_numbers_region(&mut self) {
        self.line_numbers_origin = (self.origin.0, self.origin.1);
        self.line_numbers_extents = (
            self.line_numbers_margin,
            self.extents.1
        );
        self.line_numbers_layer_params = TextRenderer::layer_params(
            self.line_numbers_origin, 
            self.line_numbers_extents
        );
    }

    fn update_text_visible_line_count(&mut self) {
        // Here we explicitly do the line calculation properly
        // then floor it, if we want to display "half" lines at the bottom
        // simply floor the font_height before dividing instead
        let max_lines_in_text_region = (self.text_extents.1 / self.renderer.borrow().font_height) as usize;
        self.text_visible_line_count = min(self.buffer.len_lines(), max_lines_in_text_region);
    }

    fn update_text_column_offset(&mut self) {
        let max_columns_in_text_region = (self.text_extents.0 / self.renderer.borrow().font_width) as usize;
        let caret_absolute_pos = self.get_caret_absolute_pos();
        let current_line_pos = self.buffer.line_to_char(self.buffer.char_to_line(caret_absolute_pos));
        let current_column = caret_absolute_pos - current_line_pos;
        if current_column > max_columns_in_text_region && self.text_column_offset < (current_column - max_columns_in_text_region) {
            self.text_column_offset = current_column - max_columns_in_text_region;
        }
        else if self.text_column_offset > current_column {
            self.text_column_offset = current_column;
        }
    }

    fn update_absolute_char_positions(&mut self) {
        // If the line count is less than the top line
        // the top line should be set to the actual line count.
        // self.top_line is 0-indexed thus the +1 (and -1)
        let line_count = self.buffer.len_lines();
        if  line_count < (self.top_line + 1) {
            self.top_line = line_count - 1;
        }
        self.bot_line = self.top_line + (self.text_visible_line_count - 1);
        self.absolute_char_pos_start = self.buffer.line_to_char(self.top_line);
        if self.bot_line >= self.buffer.len_lines() {
            self.absolute_char_pos_end = self.buffer.line_to_char(self.buffer.len_lines());
        }
        else {
            self.absolute_char_pos_end = self.buffer.line_to_char(self.bot_line) + self.buffer.line(self.bot_line).len_chars();
        }
    }

    pub fn get_current_lines(&self) -> Vec<u16> {
        self.text_range(self.absolute_char_pos_start..self.absolute_char_pos_end)
    }

    fn text_range<R>(&self, char_range: R) -> Vec<u16> where R: RangeBounds<usize> {
        let rope_slice = self.buffer.slice(char_range);
        let chars: Vec<u8> = rope_slice.bytes().collect();
        OsStr::new(str::from_utf8(chars.as_ref()).unwrap()).encode_wide().chain(once(0)).collect()
    }

    fn get_digits_in_number(&self, number: u32) -> u32 {
        match number {
            0..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            10000..=99999 => 5,
            100000..=999999 => 6,
            1000000..=9999999 => 7,
            10000000..=99999999 => 8,
            100000000..=999999999 => 9,
            1000000000..=4294967295 => 10
        }
    }
}
