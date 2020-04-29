use core::ops::RangeBounds;
use std::{
    cell::RefCell,
    cmp::{ min, max },
    fs::File,
    ffi::OsStr,
    iter::once,
    os::windows::ffi::OsStrExt,
    ptr::null_mut,
    mem::{ swap, MaybeUninit },
    rc::Rc,
    char,
    str
};
use winapi::{
    um::{
        dwrite::{ IDWriteTextLayout, DWRITE_HIT_TEST_METRICS, DWRITE_TEXT_RANGE },
        d2d1::{ D2D1_RECT_F, D2D1_LAYER_PARAMETERS },
        winuser::{ SystemParametersInfoW, SPI_GETCARETWIDTH }
    },
    ctypes::c_void
};
use ropey::Rope;

use crate::dx_ok;
use crate::settings::{ NUMBER_OF_SPACES_PER_TAB };
use crate::lsp_structs::*;
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

#[derive(PartialEq)]
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
    pub added: bool,
    pub line_num: usize
}

pub struct TextBuffer {
    buffer: Rope,

    // The layout of the text buffer should be public for
    // the renderer to use
    pub origin: (u32, u32),
    pub extents: (u32, u32),
    pub text_origin: (u32, u32),
    pub text_extents: (u32, u32),
    pub text_visible_line_count: usize,
    pub line_numbers_origin: (u32, u32),
    pub line_numbers_extents: (u32, u32),
    pub line_numbers_margin: u32,

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
    pub fn new(path: &str, language_identifier: &'static str, origin: (u32, u32), extents: (u32, u32), renderer: Rc<RefCell<TextRenderer>>) -> TextBuffer {
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
            text_origin: (0, 0),
            text_extents: (0, 0),
            text_visible_line_count: 0,
            line_numbers_origin: (0, 0),
            line_numbers_extents: (0, 0),
            line_numbers_margin: 0,

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
        let mut chars = self.buffer.chars_at(caret_absolute_pos);
        let mut count = 0;

        match search_direction {
            CharSearchDirection::Forward => {
                if caret_absolute_pos == self.buffer.len_chars() {
                    return 0;
                }
                //let caret_inside_word = self.is_inside_word(self.buffer.char(caret_absolute_pos));
                let current_char_type = self.get_char_type(self.buffer.char(caret_absolute_pos));
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
                //let left_of_current_char_type = self.is_inside_word(self.buffer.char(caret_absolute_pos - 1));
                let left_of_current_char_type = self.get_char_type(self.buffer.char(caret_absolute_pos - 1));
                while let Some(chr) = chars.prev() {
                    if self.get_char_type(chr) != left_of_current_char_type {
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

        // Set the caret position at the left edge
        self.caret_char_pos = self.get_caret_absolute_pos() - left_count;
        self.caret_is_trailing = 0;

        // Set the anchor position at the right edge
        self.caret_char_anchor = self.caret_char_pos + (left_count + right_count);
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
                    if self.caret_char_pos < self.buffer.len_chars() {
                        self.caret_char_pos += count;
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

                self.caret_char_pos = self.absolute_char_pos_start + absolute_text_pos;
            }

            // If we're at the end of the rope, the caret may not be trailing
            // otherwise we will be inserting out of bounds on the rope
            if self.caret_char_pos == self.buffer.len_chars() {
                self.caret_is_trailing = 0;
            }
        }
    }

    fn translate_mouse_pos_to_text_region(&self, mouse_pos: (f32, f32)) -> (f32, f32) {
        let dx = mouse_pos.0 - self.text_origin.0 as f32;
        let dy = mouse_pos.1 - self.text_origin.1 as f32;
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

        let caret_absolute_pos = self.get_caret_absolute_pos();
        let line = self.buffer.char_to_line(caret_absolute_pos);
        let character_position_in_line = caret_absolute_pos - self.buffer.line_to_char(line);

        // Add the newline to the temporary edits to preserve
        // semantic highlighting until new highlights are resolved
        // by the language server
        if chars == "\r\n" {
            self.semantic_tokens_edits.push(TemporaryEdit {
                added: true,
                line_num: line + 1
            })
        }

        // If we are currently selecting text, 
        // delete text before insertion
        if caret_absolute_pos != self.caret_char_anchor {
            changes.push(self.delete_selection());
        }

        self.buffer.insert(caret_absolute_pos, chars);
        self.set_selection(SelectionMode::Right, chars.len(), false);

        self.update_absolute_char_positions();
        
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

        let caret_absolute_pos = self.get_caret_absolute_pos();
        let line = self.buffer.char_to_line(caret_absolute_pos);
        let character_position_in_line = caret_absolute_pos - self.buffer.line_to_char(line);

        // If we are currently selecting text, 
        // delete text before insertion
        if caret_absolute_pos != self.caret_char_anchor {
            changes.push(self.delete_selection());
        }

        self.buffer.insert_char(caret_absolute_pos, (character as u8) as char);
        self.set_selection(SelectionMode::Right, 1, false);

        self.update_absolute_char_positions();

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

        let next_char_pos = min(caret_absolute_pos + offset, self.buffer.len_chars());
        self.buffer.remove(caret_absolute_pos..next_char_pos);
        self.update_absolute_char_positions();

        let new_line = self.buffer.char_to_line(next_char_pos);
        let new_character_position_in_line = next_char_pos - self.buffer.line_to_char(new_line);

        // In case the line has changed,
        // insert temporary edit to preserve semantic highlights
        // until the LSP server resolves the edit
        if len_lines > self.buffer.len_lines() {
            self.semantic_tokens_edits.push(TemporaryEdit {
                added: false,
                line_num: line
            })
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
            self.semantic_tokens_edits.push(TemporaryEdit {
                added: false,
                line_num: line
            })
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

        let count = self.get_boundary_char_count(CharSearchDirection::Backward);
        self.set_selection(SelectionMode::Left, count, true);

        self.lsp_versioned_identifier.version += 1;
        return DidChangeNotification::new(self.lsp_versioned_identifier.clone(), vec![self.delete_selection()]);
    }

    // Parses and creates ranges of highlight information directly
    // from the text buffer displayed on the screen
    pub fn get_lexical_highlights(&mut self) -> Vec<(DWRITE_TEXT_RANGE, SemanticTokenTypes)> {
        match self.language_identifier {
            CPP_LANGUAGE_IDENTIFIER => {
                let mut highlights = Vec::new();
                let top_line_absolute_pos = self.buffer.line_to_char(self.top_line);

                let mut multiline_start_position = 0;
                let mut multiline_active = false;
                for line in self.top_line..min(self.buffer.len_lines(), self.bot_line) {
                    let line_absolute_pos = self.buffer.line_to_char(line);
                    let slice = self.buffer.line(line);

                    let mut identifier = String::from("");
                    let mut char_pos = 0;
                    let mut was_forward_slash = false;
                    let mut was_star = false;
                    let mut string_literal_start_position = std::usize::MAX;
                    for chr in slice.chars() {
                        if chr.is_ascii_digit() {
                            let range = DWRITE_TEXT_RANGE {
                                startPosition: ((line_absolute_pos + char_pos as usize) - top_line_absolute_pos) as u32,
                                length: 1
                            };
                            highlights.push((range, SemanticTokenTypes::Literal));
                            string_literal_start_position = std::usize::MAX;
                            was_star = false;
                            was_forward_slash = false;
                        }
                        else if chr == '"' {
                            if string_literal_start_position != std::usize::MAX {
                                let range = DWRITE_TEXT_RANGE {
                                    startPosition: ((line_absolute_pos + string_literal_start_position as usize) - top_line_absolute_pos) as u32,
                                    length: ((char_pos + 1) - string_literal_start_position) as u32
                                };
                                highlights.push((range, SemanticTokenTypes::Literal));
                                string_literal_start_position = std::usize::MAX;
                            }
                            else {
                                string_literal_start_position = char_pos;
                            }
                            was_star = false;
                            was_forward_slash = false;
                        }
                        else if chr == '/' {
                            if was_forward_slash {
                                let range = DWRITE_TEXT_RANGE {
                                    startPosition: ((line_absolute_pos + (char_pos - 1) as usize) - top_line_absolute_pos) as u32,
                                    length: (slice.len_chars() - char_pos) as u32
                                };
                                highlights.push((range, SemanticTokenTypes::Comment));

                                // Break here since the rest of the line is commented out anyway
                                break;
                            }
                            else if was_star {
                                let position = (line_absolute_pos + (char_pos + 1) as usize) - top_line_absolute_pos;
                                let range = DWRITE_TEXT_RANGE {
                                    startPosition: multiline_start_position as u32,
                                    length: (position - multiline_start_position as usize) as u32
                                };
                                highlights.push((range, SemanticTokenTypes::Comment));
                                multiline_active = false;
                            }
                            was_star = false;
                            was_forward_slash = true;
                        }
                        else if chr == '*' {
                            if was_forward_slash {
                                multiline_active = true;
                                multiline_start_position = (line_absolute_pos + (char_pos - 1) as usize) - top_line_absolute_pos;
                            }
                            was_star = true;
                            was_forward_slash = false;
                        }
                        else if self.is_linebreak(chr) || chr == ' ' || chr == '\t' {
                            if CPP_KEYWORDS.contains(&identifier.as_str()) {
                                let range = DWRITE_TEXT_RANGE {
                                    startPosition: ((line_absolute_pos + (char_pos - identifier.len()) as usize) - top_line_absolute_pos) as u32,
                                    length: identifier.len() as u32
                                };
                                highlights.push((range, SemanticTokenTypes::Keyword));
                                identifier = String::from("");
                            }
                            else if identifier.starts_with("#") {
                                let range = DWRITE_TEXT_RANGE {
                                    startPosition: ((line_absolute_pos + (char_pos - identifier.len()) as usize) - top_line_absolute_pos) as u32,
                                    length: identifier.len() as u32
                                };
                                highlights.push((range, SemanticTokenTypes::Preprocessor));
                                identifier = String::from("");
                            }
                            was_star = false;
                            was_forward_slash = false;
                        }
                        else {
                            was_star = false;
                            was_forward_slash = false;
                            identifier.push(chr);
                        }
                        char_pos += 1;
                    }
                }

                // If there is still a multiline comment active,
                // comment it out
                if multiline_active {
                    let line_num = min(self.buffer.len_lines(), self.bot_line);
                    let position = (self.buffer.line_to_char(line_num) + self.buffer.line(line_num).len_chars()) - top_line_absolute_pos;
                    let range = DWRITE_TEXT_RANGE {
                        startPosition: multiline_start_position as u32,
                        length: (position - multiline_start_position as usize) as u32
                    };
                    highlights.push((range, SemanticTokenTypes::Comment));
                }
        
                highlights
            },
            // For rust we let the language server do all the work
            RUST_LANGUAGE_IDENTIFIER | _ => Vec::new()
        }

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
                if edit.added && line as usize >= edit.line_num {
                    line_offset += 1;
                }
                else if !edit.added && line as usize >= edit.line_num {
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
                self.text_extents.0 as f32,
                self.text_extents.1 as f32,
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
                self.line_numbers_extents.0 as f32,
                self.line_numbers_extents.1 as f32,
                &mut self.line_numbers_layout as *mut *mut _
            ));
        }

        (self.line_numbers_layout, self.line_numbers_layer_params)
    }

    pub fn update_metrics(&mut self, origin: (u32, u32), extents: (u32, u32)) {
        self.origin = origin;
        self.extents = extents;

        self.update_line_numbers_margin();
        self.update_text_region();
        self.update_numbers_region();
        self.update_text_visible_line_count();
        self.update_absolute_char_positions();
    }

    fn update_line_numbers_margin(&mut self) {
        let end_line_max_digits = self.get_digits_in_number(self.buffer.len_lines() as u32);
        let font_width = self.renderer.borrow().font_width;
        self.line_numbers_margin = (end_line_max_digits * font_width as u32) + (font_width / 2.0) as u32;
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
        let max_lines_in_text_region = self.extents.1 as usize / self.renderer.borrow().font_height as usize;
        self.text_visible_line_count = min(self.buffer.len_lines(), max_lines_in_text_region);
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
            self.absolute_char_pos_end = self.buffer.line_to_char(self.bot_line);
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
