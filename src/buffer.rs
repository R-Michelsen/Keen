use crate::dx_ok;
use crate::settings::{NUMBER_OF_SPACES_PER_TAB, AUTOCOMPLETE_BRACKETS};
use crate::lsp_structs::{DidChangeNotification, TextDocumentContentChangeEvent, 
                    VersionedTextDocumentIdentifier, SemanticTokenTypes, CppSemanticTokenTypes, 
                    RustSemanticTokenTypes, RustSemanticTokenModifiers};
use crate::language_support::{CPP_LANGUAGE_IDENTIFIER, RUST_LANGUAGE_IDENTIFIER, LexicalHighlights, highlight_text};
use crate::renderer::TextRenderer;
use crate::text_utils;

use std::{
    cell::RefCell,
    char,
    cmp::{min, max},
    ffi::OsStr,
    fs::File,
    iter::once,
    mem::{MaybeUninit, swap},
    os::windows::ffi::OsStrExt,
    ptr::{copy_nonoverlapping, null_mut},
    rc::Rc,
    str
};
use winapi::{
    ctypes::c_void,
    um::{
        dwrite::{IDWriteTextLayout, DWRITE_HIT_TEST_METRICS, DWRITE_TEXT_RANGE},
        d2d1::{D2D1_RECT_F, D2D1_LAYER_PARAMETERS},
        winbase::{GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock, GlobalSize, GMEM_DDESHARE, GMEM_ZEROINIT},
        winuser::{SystemParametersInfoW, SPI_GETCARETWIDTH, OpenClipboard, CloseClipboard,
            EmptyClipboard, GetClipboardData, SetClipboardData, CF_TEXT}
    },
    shared::windef::HWND
};

use ropey::Rope;

#[derive(Clone, Copy, PartialEq)]
pub enum SelectionMode {
    Left,
    Right,
    Down,
    Up
}

#[derive(Clone, Copy, PartialEq)]
pub enum MouseSelectionMode {
    Click,
    Move
}

#[derive(Clone, Copy, PartialEq)]
pub enum CharSearchDirection {
    Forward,
    Backward
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
    pub line_numbers_max_digits: u32,

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
    semantic_tokens: Vec<u32>
}

impl TextBuffer {
    pub fn new(path: &str, language_identifier: &'static str, origin: (f32, f32), extents: (f32, f32), renderer: Rc<RefCell<TextRenderer>>) -> Self {
        let file = File::open(path).unwrap();
        let buffer = Rope::from_reader(file).unwrap();

        let mut caret_width: u32 = 0;
        unsafe {
            // We'll increase the width from the system width slightly
            SystemParametersInfoW(SPI_GETCARETWIDTH, 0, (&mut caret_width as *mut _) as *mut c_void, 0);
            caret_width *= 3;
        }

        let mut text_buffer = Self {
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
            line_numbers_max_digits: 0,

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
                version: 1
            },
            semantic_tokens: Vec::new()
        };

        text_buffer.on_window_resize(origin, extents);
        text_buffer
    }

    pub fn get_full_did_change_notification(&mut self) -> DidChangeNotification {
        // Update the file version and return the change notification
        let change_event = TextDocumentContentChangeEvent {
            text: self.buffer.to_string(),
            range: None
        };
        DidChangeNotification::new(self.next_versioned_identifer(), vec![change_event])
    }

    pub fn update_semantic_tokens(&mut self, data: Vec<u32>) {
        self.semantic_tokens = data;
    }

    pub fn get_uri(&self) -> String {
        self.lsp_versioned_identifier.uri.clone()
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
    }

    pub fn scroll_up(&mut self, lines_per_roll: usize) {
        if self.top_line >= lines_per_roll {
            self.top_line -= lines_per_roll;
        }
        else {
            self.top_line = 0;
        }
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

    pub fn move_left(&mut self, shift_down: bool) {
        let count = if self.see_prev_chars("\r\n") { 2 } else { 1 };
        self.set_selection(SelectionMode::Left, count, shift_down);
    }

    pub fn move_left_by_word(&mut self, shift_down: bool) {
        // Start by moving left atleast once, then get the boundary count
        self.set_selection(SelectionMode::Left, 1, shift_down);
        let count = self.get_boundary_char_count(CharSearchDirection::Backward);
        self.set_selection(SelectionMode::Left, count, shift_down);
    }

    pub fn move_right(&mut self, shift_down: bool) {
        let count = if self.see_chars("\r\n") { 2 } else { 1 };
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
                else if (self.caret_char_pos + count) <= self.buffer.len_chars() {
                    self.caret_char_pos += count;
                }
                else {
                    self.caret_char_pos = self.buffer.len_chars();
                }
                self.caret_is_trailing = 0;

                // Reset the cached width
                self.cached_char_offset = 0;
            },
            SelectionMode::Up | SelectionMode::Down => {
                let current_line = self.buffer.char_to_line(caret_absolute_pos);

                let target_line_idx;
                let target_linebreak_count = if mode == SelectionMode::Up {
                    // If we're on the first line, return
                    if current_line == 0 {
                        return;
                    }
                    target_line_idx = current_line - 1;
                    self.linebreaks_before_line(current_line)
                }
                else {
                    // If we're on the last line, return
                    if current_line == self.buffer.len_lines() - 1 {
                        return;
                    }
                    target_line_idx = current_line + 1;
                    self.linebreaks_before_line(target_line_idx)
                };

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
        let char_pos = caret_absolute_pos - self.buffer.line_to_char(line);

        let change_event = if caret_absolute_pos < self.caret_char_anchor {
            let end_line = self.buffer.char_to_line(self.caret_char_anchor);
            let end_char = self.caret_char_anchor - self.buffer.line_to_char(end_line);
            self.buffer.remove(caret_absolute_pos..self.caret_char_anchor);

            self.caret_char_pos = caret_absolute_pos;
            self.caret_char_anchor = self.caret_char_pos;

            TextDocumentContentChangeEvent::new_delete_event(line, char_pos, end_line, end_char)
        }
        else {
            let start_line = self.buffer.char_to_line(self.caret_char_anchor);
            let start_char = self.caret_char_anchor - self.buffer.line_to_char(start_line);
            self.buffer.remove(self.caret_char_anchor..caret_absolute_pos);

            let caret_anchor_delta = caret_absolute_pos - self.caret_char_anchor;
            self.caret_char_pos = caret_absolute_pos - caret_anchor_delta;

            TextDocumentContentChangeEvent::new_delete_event(start_line, start_char, line, char_pos)
        };

        self.preserve_semantic_line_highlights(line, self.get_current_line());
        self.caret_is_trailing = 0;
        self.update_view();

        change_event
    }

    pub fn insert_newline(&mut self) -> DidChangeNotification {
        let offset = self.get_leading_whitespace_offset();

        // Search back for an open bracket, to see if auto indentation might
        // be necessary
        let mut chars = self.buffer.chars_at(self.get_caret_absolute_pos());
        while let Some(prev_char) = chars.prev() {
            if let Some(brackets) = text_utils::is_opening_bracket(prev_char) {
                // If we can find a matching bracket separated only by whitespace
                // then we will insert double newlines and insert the cursor
                // in the middle of the new scope
                for next_char in self.buffer.chars_at(self.get_caret_absolute_pos()) {
                    if next_char == brackets.1 {
                        let change_notification = self.insert_chars(
                            format!("{}{}{}{}{}", 
                                "\r\n", 
                                " ".repeat(offset),
                                " ".repeat(NUMBER_OF_SPACES_PER_TAB),
                                "\r\n",
                                " ".repeat(offset)
                            ).as_str());
                        self.set_selection(SelectionMode::Left, offset + 2, false);
                        return change_notification;
                    }
                    else if text_utils::is_whitespace(next_char) {
                        continue;
                    }
                    break;
                }

                // If no matching bracket is found, simply insert a new line
                // and indent NUMBER_OF_SPACES_PER_TAB extra for the new scope
                let change_notification = self.insert_chars(
                    format!("{}{}{}", "\r\n", " ".repeat(offset), 
                    " ".repeat(NUMBER_OF_SPACES_PER_TAB)).as_str());
                return change_notification;
            }
            if text_utils::is_whitespace(prev_char) {
                continue;
            }
            break;
        }

        self.insert_chars(format!("{}{}", "\r\n", " ".repeat(offset)).as_str())
    }

    pub fn insert_bracket(&mut self, bracket_pair: (char, char)) -> DidChangeNotification {
        // When inserting an opening bracket,
        // we will insert its corresponding closing bracket 
        // next to it.
        let did_change_notification = 
            self.insert_chars(format!("{}{}", bracket_pair.0, bracket_pair.1).as_str());
        self.set_selection(SelectionMode::Left, 1, false);
        did_change_notification
    }

    pub fn insert_chars(&mut self, chars: &str) -> DidChangeNotification {
        let mut changes = Vec::new();

        // If we are currently selecting text, 
        // delete text before insertion
        if self.get_caret_absolute_pos() != self.caret_char_anchor {
            changes.push(self.delete_selection());
        }
        let caret_absolute_pos = self.get_caret_absolute_pos();
        let line = self.buffer.char_to_line(caret_absolute_pos);
        let char_pos = caret_absolute_pos - self.buffer.line_to_char(line);

        self.buffer.insert(caret_absolute_pos, chars);
        self.set_selection(SelectionMode::Right, chars.len(), false);
        self.preserve_semantic_line_highlights(line, self.get_current_line());
        self.update_view();

        let change_event = TextDocumentContentChangeEvent::new_insert_event(chars.to_owned(), line, char_pos, line, char_pos);
        changes.push(change_event);
        DidChangeNotification::new(self.next_versioned_identifer(), changes)
    }

    pub fn insert_char(&mut self, character: u16) -> DidChangeNotification {
        let mut changes = Vec::new();
        let chr = (character as u8) as char;

        // If we are currently selecting text, 
        // delete text before insertion
        if self.get_caret_absolute_pos() != self.caret_char_anchor {
            changes.push(self.delete_selection());
        }

        let mut caret_absolute_pos = self.get_caret_absolute_pos();
        for brackets in &AUTOCOMPLETE_BRACKETS {
            if chr == brackets.0 {
                return self.insert_bracket(*brackets);
            }
            // Special case when inserting a closing bracket
            // while the caret is next to closing bracket. Simply
            // advance the caret position once
            if chr == brackets.1 {
                if self.buffer.char(caret_absolute_pos) == brackets.1 {
                    self.set_selection(SelectionMode::Right, 1, false);
                    return DidChangeNotification::new(self.next_versioned_identifer(), changes);
                }
                // Otherwise if possible move the scope indent back once
                else {
                    let offset = self.get_leading_whitespace_offset();
                    let current_char_pos = caret_absolute_pos - self.buffer.line_to_char(self.buffer.char_to_line(caret_absolute_pos));
                    if offset >= NUMBER_OF_SPACES_PER_TAB && current_char_pos == offset {
                        self.set_selection(SelectionMode::Left, NUMBER_OF_SPACES_PER_TAB, true);
                        changes.push(self.delete_selection());
                    }
                }
            }
        }

        caret_absolute_pos = self.get_caret_absolute_pos();
        let line = self.buffer.char_to_line(caret_absolute_pos);
        let char_pos = caret_absolute_pos - self.buffer.line_to_char(self.buffer.char_to_line(caret_absolute_pos));

        self.buffer.insert_char(caret_absolute_pos, chr);
        self.set_selection(SelectionMode::Right, 1, false);
        self.preserve_semantic_char_highlights(line, char_pos);
        self.update_view();

        let change_event = TextDocumentContentChangeEvent::new_insert_event(chr.to_string(), line, char_pos, line, char_pos);

        changes.push(change_event);
        DidChangeNotification::new(self.next_versioned_identifer(), changes)
    }

    pub fn delete_right(&mut self) -> DidChangeNotification {
        let caret_absolute_pos = self.get_caret_absolute_pos();
        let line = self.buffer.char_to_line(caret_absolute_pos);
        let char_pos = caret_absolute_pos - self.buffer.line_to_char(line);

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            return DidChangeNotification::new(self.next_versioned_identifer(), vec![self.delete_selection()]);
        }

        // In case of a CRLF, delete both characters
        // In case of a <TAB>, delete the corresponding spaces
        let mut offset = 1;
        if self.see_chars("\r\n") { 
            offset = 2 
        }
        else if self.see_chars(" ".repeat(NUMBER_OF_SPACES_PER_TAB).as_str()) {
            offset = NUMBER_OF_SPACES_PER_TAB;
        }

        let next_char_pos = min(caret_absolute_pos + offset, self.buffer.len_chars());
        let new_line = self.buffer.char_to_line(next_char_pos);
        let new_char = next_char_pos - self.buffer.line_to_char(new_line);

        self.buffer.remove(caret_absolute_pos..next_char_pos);
        if new_line > line {
            self.preserve_semantic_line_highlights(line, line - 1);
        }
        
        let change_event = TextDocumentContentChangeEvent::new_delete_event(line, char_pos, new_line, new_char);
        DidChangeNotification::new(self.next_versioned_identifer(), vec![change_event])
    }

    pub fn delete_right_by_word(&mut self) -> DidChangeNotification {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            return DidChangeNotification::new(self.next_versioned_identifer(), vec![self.delete_selection()]);
        }

        let count = self.get_boundary_char_count(CharSearchDirection::Forward);
        self.set_selection(SelectionMode::Right, count, true);

        DidChangeNotification::new(self.next_versioned_identifer(), vec![self.delete_selection()])
    }

    pub fn delete_left(&mut self) -> DidChangeNotification {
        let caret_absolute_pos = self.get_caret_absolute_pos();
        let line = self.buffer.char_to_line(caret_absolute_pos);
        let char_pos = caret_absolute_pos - self.buffer.line_to_char(line);

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            return DidChangeNotification::new(self.next_versioned_identifer(), vec![self.delete_selection()]);
        }

        // In case of a CRLF, delete both characters
        // In case of a <TAB>, delete the corresponding spaces
        let mut offset = 1;
        if self.see_prev_chars("\r\n") { 
            offset = 2 
        }
        else if self.see_prev_chars(" ".repeat(NUMBER_OF_SPACES_PER_TAB).as_str()) {
            offset = NUMBER_OF_SPACES_PER_TAB;
        }

        let previous_char_pos = caret_absolute_pos.saturating_sub(offset);
        self.buffer.remove(previous_char_pos..caret_absolute_pos);
        self.set_selection(SelectionMode::Left, offset, false);
        self.preserve_semantic_line_highlights(line, self.get_current_line());

        let new_line = self.buffer.char_to_line(previous_char_pos);
        let new_char = previous_char_pos - self.buffer.line_to_char(new_line);

        let change_event = TextDocumentContentChangeEvent::new_delete_event(new_line, new_char, line, char_pos);
        DidChangeNotification::new(self.next_versioned_identifer(), vec![change_event])
    }

    pub fn delete_left_by_word(&mut self) -> DidChangeNotification {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            return DidChangeNotification::new(self.next_versioned_identifer(), vec![self.delete_selection()]);
        }

        // Start by moving left once, then get the boundary count
        self.set_selection(SelectionMode::Left, 1, true);
        let count = self.get_boundary_char_count(CharSearchDirection::Backward);
        self.set_selection(SelectionMode::Left, count, true);

        DidChangeNotification::new(self.next_versioned_identifer(), vec![self.delete_selection()])
    }

    // Parses and creates ranges of highlight information directly
    // from the text buffer displayed on the screen
    pub fn get_lexical_highlights(&mut self) -> LexicalHighlights {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        let text_in_current_view = self.get_text_view_as_string();
        let start_it = self.buffer.chars_at(self.absolute_char_pos_start);
        let caret_it = self.buffer.chars_at(caret_absolute_pos);

        highlight_text(text_in_current_view.as_str(), self.absolute_char_pos_start, caret_absolute_pos, self.language_identifier, start_it, caret_it)
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

            match self.language_identifier {
                CPP_LANGUAGE_IDENTIFIER => {
                    let token_type = CppSemanticTokenTypes::to_semantic_token_type(&CppSemanticTokenTypes::from_u32(self.semantic_tokens[i + 3]));
                    let line_absolute_pos = self.buffer.line_to_char(line as usize);
                    let range = DWRITE_TEXT_RANGE {
                        startPosition: ((line_absolute_pos + start as usize) - top_line_absolute_pos) as u32,
                        length
                    };
                    highlights.push((range, token_type));
                },
                RUST_LANGUAGE_IDENTIFIER => {
                    let token_type = RustSemanticTokenTypes::to_semantic_token_type(&RustSemanticTokenTypes::from_u32(self.semantic_tokens[i + 3]));
                    let line_absolute_pos = self.buffer.line_to_char(line as usize);
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
        (
            self.text_origin.0 - self.text_column_offset as f32 * self.renderer.borrow().font_width,
            self.text_origin.1
        )
    }

    pub fn get_caret_rect(&mut self) -> Option<D2D1_RECT_F> {
        if self.caret_char_pos < self.absolute_char_pos_start || self.caret_char_pos > self.absolute_char_pos_end {
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

            Some(rect)
        }
    }

    pub fn copy_selection(&mut self, hwnd: HWND) {
        unsafe {
            if OpenClipboard(hwnd) > 0 {
                if EmptyClipboard() > 0 {
                    let data = self.get_selection_data();
                    if data.is_empty() {
                        CloseClipboard();
                        return;
                    }
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
            return DidChangeNotification::new(self.next_versioned_identifer(), vec![self.delete_selection()]);
        }

        let current_line_idx = self.buffer.char_to_line(caret_absolute_pos);
        let current_line = self.buffer.line(current_line_idx);
        let current_line_chars = self.buffer.line_to_char(current_line_idx);
        let current_line_length = current_line.len_chars();

        // Slight hack to fix the semantic highlighting
        // self.caret_is_trailing = 0;
        // self.caret_char_pos = self.buffer.line_to_char(current_line_idx - 1);
        self.preserve_semantic_line_highlights(current_line_idx, current_line_idx.saturating_sub(1));

        // Update caret position
        self.caret_char_pos = current_line_chars;
        self.caret_is_trailing = 0;
        self.caret_char_anchor = self.caret_char_pos;

        self.buffer.remove(current_line_chars..current_line_chars + current_line_length);

        let change_event = TextDocumentContentChangeEvent::new_delete_event(current_line_idx, 0, current_line_idx + 1, 0);
        DidChangeNotification::new(self.next_versioned_identifer(), vec![change_event])
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
        let lines = self.get_text_view_as_utf16();

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

    pub fn on_editor_action(&mut self) {
        // Full update if number of lines have exceeded a "digit-boundary"
        if self.line_numbers_max_digits != text_utils::get_digits_in_number(self.buffer.len_lines() as u32) {
            self.on_editor_refresh_metrics();
            return;
        }

        // In theory for some actions such as selecting text
        // the absolute char positions need not be updated. However,
        // for readability and code flexibility reasons we will do it 
        // anyway on every editor action
        self.update_text_column_offset();
        self.update_absolute_char_positions();
    }

    pub fn on_editor_refresh_metrics(&mut self) {
        self.update_line_numbers_margin();
        self.update_text_region();
        self.update_numbers_region();
        self.update_text_visible_line_count();
        self.update_text_column_offset();
        self.update_absolute_char_positions();
    }

    pub fn on_window_resize(&mut self, origin: (f32, f32), extents: (f32, f32)) {
        self.origin = origin;
        self.extents = extents;

        self.update_line_numbers_margin();
        self.update_text_region();
        self.update_numbers_region();
        self.update_text_visible_line_count();
        self.update_text_column_offset();
        self.update_absolute_char_positions();
    }

    pub fn get_current_line(&self) -> usize {
        self.buffer.char_to_line(self.get_caret_absolute_pos())
    }

    fn next_versioned_identifer(&mut self) -> VersionedTextDocumentIdentifier {
        self.lsp_versioned_identifier.version += 1;
        self.lsp_versioned_identifier.clone()
    }

    // Adds potential newlines to the temporary edits to preserve
    // semantic highlighting until new highlights are resolved
    // by the language server
    fn preserve_semantic_line_highlights(&mut self, line_before_edit: usize, line_after_edit: usize) {
        // We will insert delta number of lines in between the line
        // before the edit that occured and the new line of the caret
        let delta: i32 = (line_after_edit as i32) - (line_before_edit as i32);

        if delta != 0 {
            let mut i = 0;
            let mut line = 0;
            while i < self.semantic_tokens.len() {
                let delta_line = self.semantic_tokens[i];
                line += delta_line;
    
                if line > (line_before_edit as u32) {
                    if delta > 0 {
                        self.semantic_tokens[i] += delta.abs() as u32;
                    }
                    else {
                        self.semantic_tokens[i] = self.semantic_tokens[i].saturating_sub(delta.abs() as u32);
                    }
                    break;
                }
                i += 5;
            }
        }
    }

    // Adds a character to the temporary edits to preserve
    // semantic highlighting until new highlights are resolved
    // by the language server
    fn preserve_semantic_char_highlights(&mut self, line_pos: usize, char_pos_in_line: usize) {
        let mut i = 0;
        let mut line = 0;
        let mut start = 0;

        while i < self.semantic_tokens.len() {
            let delta_line = self.semantic_tokens[i];
            line += delta_line;

            let delta_start = self.semantic_tokens[i + 1];
            if delta_line == 0 {
                start += delta_start;
            }
            else {
                start = delta_start;
            }
            let length = self.semantic_tokens[i + 2];

            if line == (line_pos as u32) && (start + length) == (char_pos_in_line as u32) {
                self.semantic_tokens[i + 2] += 1;
            }

            i += 5;
        }
    }

    fn linebreaks_before_line(&self, line: usize) -> usize {
        let mut line_start = self.buffer.chars_at(self.buffer.line_to_char(line));
        match line_start.prev() {
            Some('\n') => if line_start.prev() == Some('\r') { 2 } else { 1 }
            // For completeness, we will count all linebreaks
            // that ropey supports
            Some('\u{000B}') | Some('\u{000C}') |
            Some('\u{000D}') | Some('\u{0085}') |
            Some('\u{2028}') | Some('\u{2029}') => 1,
            _ => 0
        }
    }

    fn see_chars(&self, string: &str) -> bool {
        let mut rope_iterator = self.buffer.chars_at(self.get_caret_absolute_pos());
        for chr in string.chars() {
            match rope_iterator.next() {
                Some(x) if x == chr => continue,
                _ => return false,
            }
        }
        true
    }

    fn see_prev_chars(&self, string: &str) -> bool {
        let mut rope_iterator = self.buffer.chars_at(self.get_caret_absolute_pos());
        for chr in string.chars().rev() {
            match rope_iterator.prev() {
                Some(x) if x == chr => continue,
                _ => return false,
            }
        }
        true
    }

    fn get_selection_data(&self) -> String {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        match self.caret_char_anchor {
            anchor if anchor > caret_absolute_pos => {
                self.buffer.slice(caret_absolute_pos..min(self.caret_char_anchor, self.buffer.len_chars() - 1)).to_string()
            },
            anchor if anchor < caret_absolute_pos => {
                self.buffer.slice(self.caret_char_anchor..min(caret_absolute_pos, self.buffer.len_chars() - 1)).to_string()
            },
            // If nothing is selected, copy current line
            _ => self.buffer.line(self.buffer.char_to_line(caret_absolute_pos)).to_string()
        }
    }

    fn update_line_numbers_margin(&mut self) {
        self.line_numbers_max_digits = text_utils::get_digits_in_number(self.buffer.len_lines() as u32);
        let font_width = self.renderer.borrow().font_width;
        self.line_numbers_margin = (self.line_numbers_max_digits as f32).mul_add(font_width, font_width / 2.0)
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

    // Updates the view in case the current line
    // is not within the current view
    fn update_view(&mut self) {
        let current_line = self.buffer.char_to_line(self.get_caret_absolute_pos());
        if current_line > self.bot_line || current_line < self.top_line {
            self.top_line = current_line;
        }
    }

    // Gets the amount of leading whitespace on the current line.
    // To help with auto indentation
    fn get_leading_whitespace_offset(&self) -> usize {
        let line_slice = self.buffer.line(self.buffer.char_to_line(self.get_caret_absolute_pos())).chars();
        let mut offset = 0;
        for chr in line_slice {
            match chr {
                ' ' => offset += 1,
                '\t' => offset += NUMBER_OF_SPACES_PER_TAB,
                _ => break
            }
        }
        offset
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
                let current_char_type = text_utils::get_char_type(self.buffer.char(self.caret_char_pos));
                for chr in self.buffer.chars_at(self.get_caret_absolute_pos()) {
                    if text_utils::get_char_type(chr) != current_char_type {
                        break;
                    }
                    count += 1;
                }
            },
            CharSearchDirection::Backward => {
                if caret_absolute_pos == 0 {
                    return 0;
                }
                let current_char_type = text_utils::get_char_type(self.buffer.char(self.caret_char_pos));
                let mut chars = self.buffer.chars_at(self.caret_char_pos);
                while let Some(chr) = chars.prev() {
                    if text_utils::get_char_type(chr) != current_char_type {
                        break;
                    }
                    count += 1;
                }
            }
        }

        count
    }

    pub fn get_text_view_as_string(&self) -> String {
        self.buffer.slice(self.absolute_char_pos_start..self.absolute_char_pos_end).to_string()
    }

    fn get_text_view_as_utf16(&self) -> Vec<u16> {
        let rope_slice = self.buffer.slice(self.absolute_char_pos_start..self.absolute_char_pos_end);
        let chars: Vec<u8> = rope_slice.bytes().collect();
        OsStr::new(str::from_utf8(chars.as_ref()).unwrap()).encode_wide().chain(once(0)).collect()
    }
}
