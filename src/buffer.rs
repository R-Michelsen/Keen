use crate::{
    settings::{NUMBER_OF_SPACES_PER_TAB, AUTOCOMPLETE_BRACKETS},
    language_support::{LexicalHighlights, highlight_text},
    text_utils
};

use std::{
    char,
    cmp::{min, max},
    fs::File,
    mem::swap,
    ptr::copy_nonoverlapping,
    str
};
use winapi::{
    um::{
        winbase::{GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock, GlobalSize, GMEM_DDESHARE, GMEM_ZEROINIT},
        winuser::{OpenClipboard, CloseClipboard, EmptyClipboard, GetClipboardData, SetClipboardData, CF_TEXT,
                  VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN, VK_TAB, VK_RETURN, VK_DELETE, VK_BACK}
    },
    shared::windef::HWND
};

use ropey::Rope;

#[derive(Clone, PartialEq)]
pub enum SelectionMode {
    Left,
    Right,
    Down,
    Up
}

#[derive(Clone, PartialEq)]
pub enum CharSearchDirection {
    Forward,
    Backward
}

#[derive(Clone, PartialEq)]
pub struct TextRange {
    pub start: u32,
    pub length: u32
}

type TextPos = usize;
type ShiftDown = bool;
type CtrlDown = bool;

#[derive(PartialEq)]
pub enum BufferCommand {
    ScrollUp(usize),
    ScrollDown(usize),
    ScrollLeft(usize),
    ScrollRight(usize),
    LeftClick(TextPos, ShiftDown),
    LeftDoubleClick(TextPos),
    LeftRelease,
    SetMouseSelection(TextPos),
    KeyPressed(i32, ShiftDown, CtrlDown, HWND),
    CharInsert(u16)
}

#[derive(Clone, PartialEq)]
pub struct BufferState {
    rope: Rope,

    caret_char_anchor: usize,
    caret_char_pos: usize,
    caret_trailing: i32,

    char_absolute_pos_start: usize,
    char_absolute_pos_end: usize,
}

// TODO: undo_states should probably just be some fixed array 
// perhaps a ringbuffer to store the last N states
pub struct TextBuffer {
    pub path: String,

    rope: Rope,
    caret_char_anchor: usize,
    caret_char_pos: usize,
    caret_trailing: i32,
    char_absolute_pos_start: usize,
    char_absolute_pos_end: usize,

    pub undo_states: Vec<BufferState>,

    pub view_dirty: bool,

    max_rows: usize,
    line_offset: usize,
    max_columns: usize,

    pub margin_column_count: usize,
    pub column_offset: usize,

    // The selection state of the buffer should be public
    // for the editor to use
    pub currently_selecting: bool,

    // The language of the text buffer as
    // identified by its extension
    pub language_identifier: &'static str,

    cached_column_offset: u32
}

impl TextBuffer {
    pub fn new(path: &str, language_identifier: &'static str, max_rows: usize, max_columns: usize) -> Self {
        let file = File::open(path).unwrap();
        let mut text_buffer = Self {
            path: String::from(path),

            rope: Rope::from_reader(file).unwrap(),
            caret_char_anchor: 0,
            caret_char_pos: 0,
            caret_trailing: 0,
            char_absolute_pos_start: 0,
            char_absolute_pos_end: 0,

            undo_states: Vec::new(),

            view_dirty: true,

            max_rows,
            line_offset: 0,
            max_columns,
            margin_column_count: 0,
            column_offset: 0,

            currently_selecting: false,

            language_identifier,

            cached_column_offset: 0,
        };

        text_buffer.push_undo_state();
        text_buffer.refresh_metrics(max_rows, max_columns);
        text_buffer
    }

    #[inline(always)]
    fn get_last_line(&self) -> usize {
        self.line_offset + self.max_rows - 1
    }

    #[inline(always)]
    fn push_undo_state(&mut self) {
        self.undo_states.push(BufferState {
            rope: self.rope.clone(),
            caret_char_anchor: self.caret_char_anchor,
            caret_char_pos: self.caret_char_pos,
            caret_trailing: self.caret_trailing,
            char_absolute_pos_start: self.char_absolute_pos_start,
            char_absolute_pos_end: self.char_absolute_pos_end
        });
    }

    #[inline(always)]
    fn undo(&mut self) {
        if self.undo_states.len() > 1 {
            let state = self.undo_states.pop().unwrap();
            self.rope = state.rope;
            self.caret_char_anchor = state.caret_char_anchor;
            self.caret_char_pos = state.caret_char_pos;
            self.caret_trailing = state.caret_trailing;
            self.char_absolute_pos_start = state.char_absolute_pos_start;
            self.char_absolute_pos_end = state.char_absolute_pos_end;
        }
        else if self.undo_states.len() == 1 {
            let state = self.undo_states.last().unwrap();
            self.rope = state.rope.clone();
            self.caret_char_anchor = state.caret_char_anchor;
            self.caret_char_pos = state.caret_char_pos;
            self.caret_trailing = state.caret_trailing;
            self.char_absolute_pos_start = state.char_absolute_pos_start;
            self.char_absolute_pos_end = state.char_absolute_pos_end;
        }
    }

    #[inline(always)]
    fn get_caret_absolute_pos(&self) -> usize {
        self.caret_char_pos + (self.caret_trailing as usize)
    }

    pub fn scroll_up(&mut self, lines_per_roll: usize) {
        if self.line_offset >= lines_per_roll {
            self.line_offset -= lines_per_roll;
        }
        else {
            self.line_offset = 0;
        }
    }

    pub fn scroll_down(&mut self, lines_per_roll: usize) {
        let new_top = self.line_offset + lines_per_roll;
        if new_top >= self.rope.len_lines() {
            self.line_offset = self.rope.len_lines() - 1;
        }
        else {
            self.line_offset = new_top;
        }
    }

    pub fn scroll_left(&mut self, lines_per_roll: usize) {
        if self.column_offset >= lines_per_roll {
            self.column_offset -= lines_per_roll;
        }
        else {
            self.column_offset = 0;
        }
    }

    pub fn scroll_right(&mut self, lines_per_roll: usize) {
        let current_line = self.rope.char_to_line(self.get_caret_absolute_pos());
        let line_length = self.rope.line(current_line).len_chars();
        let new_offset = self.column_offset + lines_per_roll;
        if line_length > self.max_columns && new_offset > (line_length - self.max_columns) {
            self.column_offset = line_length - self.max_columns;
        }
        else if line_length > self.max_columns {
            self.column_offset = new_offset;
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

    pub fn left_click(&mut self, relative_text_pos: usize, extend_current_selection: bool) {
        self.set_mouse_selection(relative_text_pos);
        let caret_absolute_pos = self.get_caret_absolute_pos();

        if !extend_current_selection {
            self.caret_char_anchor = caret_absolute_pos;
        }
        self.currently_selecting = true;

        // Reset the cached width
        self.cached_column_offset = 0;

        // Left-click will scroll down once if on the last line
        if self.get_last_line() == self.get_current_line() {
            self.scroll_down(1)
        }
    }

    pub fn left_double_click(&mut self, relative_text_pos: usize) {
        self.set_mouse_selection(relative_text_pos);

        // Find the boundary on each side of the cursor
        let left_count = self.get_boundary_char_count(CharSearchDirection::Backward);
        let right_count = self.get_boundary_char_count(CharSearchDirection::Forward);

        // Set the caret position at the right edge
        self.caret_char_pos += right_count;

        // Set the anchor position at the left edge
        self.caret_char_anchor = self.caret_char_pos - (left_count + right_count);

        // Left-click will scroll down once if on the last line
        if self.get_last_line() == self.get_current_line() {
            self.scroll_down(1)
        }
    }

    pub fn left_release(&mut self) {
        self.currently_selecting = false;
    }

    pub fn set_selection(&mut self, mode: SelectionMode, count: usize, extend_current_selection: bool) {
        match mode {
            SelectionMode::Left | SelectionMode::Right => {
                self.caret_char_pos = self.get_caret_absolute_pos();

                if mode == SelectionMode::Left {
                    if self.caret_char_pos > 0 {
                        self.caret_char_pos -= count;
                    }
                }
                else if (self.caret_char_pos + count) <= self.rope.len_chars() {
                    self.caret_char_pos += count;
                }
                else {
                    self.caret_char_pos = self.rope.len_chars();
                }
                self.caret_trailing = 0;

                // Reset the cached width
                self.cached_column_offset = 0;
            }
            SelectionMode::Up | SelectionMode::Down => {
                let current_line = self.rope.char_to_line(self.get_caret_absolute_pos());
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
                    if current_line == self.rope.len_lines() - 1 {
                        return;
                    }
                    target_line_idx = current_line + 1;
                    self.linebreaks_before_line(target_line_idx)
                };

                let target_line = self.rope.line(target_line_idx);
                let target_line_length = target_line.len_chars().saturating_sub(target_linebreak_count);

                let current_offset = self.get_caret_absolute_pos() - self.rope.line_to_char(current_line);
                let desired_offset = max(self.cached_column_offset, current_offset as u32);
                self.cached_column_offset = desired_offset;

                let new_offset = min(target_line_length, desired_offset as usize);

                self.caret_char_pos = self.rope.line_to_char(target_line_idx) + new_offset;
                self.caret_trailing = 0;

                if target_line_idx >= self.get_last_line() {
                    self.scroll_down(1);
                }
                else if target_line_idx < self.line_offset {
                    self.scroll_up(1);
                }

            }
        }

        if !extend_current_selection {
            self.caret_char_anchor = self.get_caret_absolute_pos();
        }
    }

    pub fn set_mouse_selection(&mut self, relative_text_pos: usize) {
        self.caret_char_pos = min(
            self.char_absolute_pos_start + relative_text_pos, 
            self.rope.len_chars()
        );

        // If we're at the end of the rope, the caret shall not be trailing
        // otherwise we will be inserting out of bounds on the rope
        if self.caret_char_pos == self.rope.len_chars() {
            self.caret_trailing = 0;
        }
    }

    pub fn select_all(&mut self) {
        self.caret_char_anchor = 0;
        self.caret_trailing = 0;
        self.caret_char_pos = self.rope.len_chars();
    }

    pub fn delete_selection(&mut self) {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        let caret_anchor = self.caret_char_anchor;
        if caret_absolute_pos < self.caret_char_anchor {
            self.rope.remove(caret_absolute_pos..caret_anchor);
            self.caret_char_pos = caret_absolute_pos;
            self.caret_char_anchor = self.caret_char_pos;
        }
        else {
            self.rope.remove(caret_anchor..caret_absolute_pos);
            let caret_anchor_delta = caret_absolute_pos - self.caret_char_anchor;
            self.caret_char_pos = caret_absolute_pos - caret_anchor_delta;
        };

        self.caret_trailing = 0;
        self.ensure_caret_visible();
    }

    pub fn insert_newline(&mut self) {
        let offset = self.get_leading_whitespace_offset();

        // Search back for an open bracket, to see if auto indentation might
        // be necessary
        let mut chars = self.rope.chars_at(self.get_caret_absolute_pos());
        while let Some(prev_char) = chars.prev() {
            if let Some(brackets) = text_utils::is_opening_bracket(prev_char) {
                // If we can find a matching bracket separated only by whitespace
                // then we will insert double newlines and insert the cursor
                // in the middle of the new scope
                for next_char in self.rope.chars_at(self.get_caret_absolute_pos()) {
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

    pub fn insert_bracket(&mut self, bracket_pair: (char, char)) {
        // When inserting an opening bracket,
        // we will insert its corresponding closing bracket 
        // next to it.
        self.insert_chars(format!("{}{}", bracket_pair.0, bracket_pair.1).as_str());
        self.set_selection(SelectionMode::Left, 1, false);
    }

    pub fn insert_chars(&mut self, chars: &str) {
        // If we are currently selecting text, 
        // delete text before insertion
        if self.get_caret_absolute_pos() != self.caret_char_anchor {
            self.delete_selection();
        }

        let caret_absolute_pos = self.get_caret_absolute_pos();

        self.rope.insert(caret_absolute_pos, chars);
        self.set_selection(SelectionMode::Right, chars.len(), false);
        self.ensure_caret_visible();
    }

    pub fn insert_char(&mut self, character: u16) {
        let chr = (character as u8) as char;

        // If we are currently selecting text, 
        // delete text before insertion
        if self.get_caret_absolute_pos() != self.caret_char_anchor {
            self.delete_selection();
        }

        let mut caret_absolute_pos = self.get_caret_absolute_pos();
        for brackets in &AUTOCOMPLETE_BRACKETS {
            if chr == brackets.0 {
                self.insert_bracket(*brackets);
                return;
            }
            // Special case when inserting a closing bracket
            // while the caret is next to closing bracket. Simply
            // advance the caret position once
            if chr == brackets.1 {
                if self.rope.char(caret_absolute_pos) == brackets.1 {
                    self.set_selection(SelectionMode::Right, 1, false);
                }
                // Otherwise if possible move the scope indent back once
                else {
                    let offset = self.get_leading_whitespace_offset();
                    let current_char_pos = caret_absolute_pos - self.rope.line_to_char(self.rope.char_to_line(caret_absolute_pos));
                    if offset >= NUMBER_OF_SPACES_PER_TAB && current_char_pos == offset {
                        self.set_selection(SelectionMode::Left, NUMBER_OF_SPACES_PER_TAB, true);
                    }
                }
            }
        }

        caret_absolute_pos = self.get_caret_absolute_pos();

        self.rope.insert_char(caret_absolute_pos, chr);
        self.set_selection(SelectionMode::Right, 1, false);
        self.ensure_caret_visible();
    }

    pub fn delete_right(&mut self) {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            self.delete_selection();
            return;
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

        let next_char_pos = min(caret_absolute_pos + offset, self.rope.len_chars());
        self.rope.remove(caret_absolute_pos..next_char_pos);
    }

    pub fn delete_right_by_word(&mut self) {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            self.delete_selection();
            return;
        }

        let count = self.get_boundary_char_count(CharSearchDirection::Forward);
        self.set_selection(SelectionMode::Right, count, true);
        self.delete_selection();
    }

    pub fn delete_left(&mut self) {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            self.delete_selection();
            return;
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

        self.rope.remove(previous_char_pos..caret_absolute_pos);
        self.set_selection(SelectionMode::Left, offset, false);
    }

    pub fn delete_left_by_word(&mut self) {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        // If we are currently selecting text, 
        // simply delete the selected text
        if caret_absolute_pos != self.caret_char_anchor {
            self.delete_selection();
            return;
        }

        // Start by moving left once, then get the boundary count
        self.set_selection(SelectionMode::Left, 1, true);
        let count = self.get_boundary_char_count(CharSearchDirection::Backward);
        self.set_selection(SelectionMode::Left, count, true);
        self.delete_selection();
    }

    // Parses and creates ranges of highlight information directly
    // from the text buffer displayed on the screen
    pub fn get_lexical_highlights(&mut self) -> LexicalHighlights {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        let text_in_current_view = self.get_text_view_as_string();
        let start_it = self.rope.chars_at(self.char_absolute_pos_start);
        let caret_it = self.rope.chars_at(caret_absolute_pos);

        highlight_text(text_in_current_view.as_str(), self.char_absolute_pos_start, 
                       caret_absolute_pos, self.language_identifier, start_it, caret_it)
    }

    pub fn get_caret_offset(&mut self) -> Option<usize> {
        if self.caret_char_pos < self.char_absolute_pos_start || 
            self.caret_char_pos > self.char_absolute_pos_end {
            return None;
        }
        Some(self.caret_char_pos - self.char_absolute_pos_start)
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

    pub fn cut_selection(&mut self, hwnd: HWND) {
        // Copy the selection
        self.copy_selection(hwnd);

        let caret_absolute_pos = self.get_caret_absolute_pos();
        // If we're selecting text, delete it
        // otherwise delete the current line
        if caret_absolute_pos != self.caret_char_anchor {
            self.delete_selection();
            return;
        }

        let current_line_idx = self.rope.char_to_line(caret_absolute_pos);
        let current_line = self.rope.line(current_line_idx);
        let current_line_chars = self.rope.line_to_char(current_line_idx);
        let current_line_length = current_line.len_chars();

        // Update caret position
        self.caret_char_pos = current_line_chars;
        self.caret_trailing = 0;
        self.caret_char_anchor = self.caret_char_pos;

        self.rope.remove(current_line_chars..current_line_chars + current_line_length);
    }

    pub fn paste(&mut self, hwnd: HWND) {
        unsafe {
            if OpenClipboard(hwnd) > 0 {
                let clipboard_data_ptr = GetClipboardData(CF_TEXT);
                if !clipboard_data_ptr.is_null() {
                    let byte_size = GlobalSize(clipboard_data_ptr);
                    let memory = GlobalLock(clipboard_data_ptr);

                    let slice: &[u8] = core::slice::from_raw_parts_mut(memory as *mut u8, byte_size as usize);

                    // Convert back to &str and trim the trailing null-byte
                    let chars = std::str::from_utf8_unchecked(slice).trim_end_matches('\0');

                    self.insert_chars(chars);
                    GlobalUnlock(clipboard_data_ptr);
                }

                CloseClipboard();
            }
        }
    }

    pub fn get_selection_range(&self) -> Option<TextRange> {
        let caret_absolute_pos = self.get_caret_absolute_pos();
        if caret_absolute_pos == self.caret_char_anchor {
            return None;
        }
 
        // Saturating sub ensures that the carets don't go below 0
        let mut caret_begin = self.caret_char_anchor.saturating_sub(self.char_absolute_pos_start);
        let mut caret_end = caret_absolute_pos.saturating_sub(self.char_absolute_pos_start);

        if caret_begin > caret_end {
            swap(&mut caret_begin, &mut caret_end);
        }

        caret_begin = min(caret_begin, self.char_absolute_pos_end);
        caret_end = min(caret_end, self.char_absolute_pos_end);

        let range =  TextRange {
            start: caret_begin as u32,
            length: (caret_end - caret_begin) as u32
        };

        Some(range)
    }

    pub fn on_change(&mut self) {
        self.update_margin_and_column_offset();
        self.update_absolute_char_positions();
        self.view_dirty = true;
    }

    pub fn refresh_metrics(&mut self, max_rows: usize, max_columns: usize) {
        self.max_rows = max_rows;
        self.max_columns = max_columns;
        self.on_change();
    }

    pub fn get_current_line(&self) -> usize {
        self.rope.char_to_line(self.get_caret_absolute_pos())
    }

    fn linebreaks_before_line(&self, line: usize) -> usize {
        let mut line_start = self.rope.chars_at(self.rope.line_to_char(line));
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
        let mut rope_iterator = self.rope.chars_at(self.get_caret_absolute_pos());
        for chr in string.chars() {
            match rope_iterator.next() {
                Some(x) if x == chr => continue,
                _ => return false
            }
        }
        true
    }

    fn see_prev_chars(&self, string: &str) -> bool {
        let mut rope_iterator = self.rope.chars_at(self.get_caret_absolute_pos());
        for chr in string.chars().rev() {
            match rope_iterator.prev() {
                Some(x) if x == chr => continue,
                _ => return false
            }
        }
        true
    }

    fn get_selection_data(&self) -> String {
        let caret_absolute_pos = self.get_caret_absolute_pos();

        match self.caret_char_anchor {
            anchor if anchor > caret_absolute_pos => {
                self.rope.slice(caret_absolute_pos..min(self.caret_char_anchor, self.rope.len_chars() - 1)).to_string()
            },
            anchor if anchor < caret_absolute_pos => {
                self.rope.slice(self.caret_char_anchor..min(caret_absolute_pos, self.rope.len_chars() - 1)).to_string()
            },
            // If nothing is selected, copy current line
            _ => self.rope.line(self.rope.char_to_line(caret_absolute_pos)).to_string()
        }
    }

    fn update_margin_and_column_offset(&mut self) {
        let max_digits = text_utils::get_digits_in_number(self.get_last_line() as u32) as usize;
        self.margin_column_count = max_digits + 1;

        let caret_absolute_pos = self.get_caret_absolute_pos();
        let current_line_pos = self.rope.line_to_char(self.rope.char_to_line(caret_absolute_pos));
        let current_column = caret_absolute_pos - current_line_pos;

        let text_columns = self.max_columns - self.margin_column_count;
        if current_column > text_columns && self.column_offset < (current_column - text_columns) {
            self.column_offset = current_column - text_columns;
        }
        else if self.column_offset > current_column {
            self.column_offset = current_column;
        }
    }

    fn update_absolute_char_positions(&mut self) {
        // If the line count is less than the line offset
        // the line offset should be set to the actual line count.
        // self.line_offset is 0-indexed thus the +1 (and -1)
        let line_count = self.rope.len_lines();
        if  line_count < (self.line_offset + 1) {
            self.line_offset = line_count - 1;
        }

        self.char_absolute_pos_start = self.rope.line_to_char(self.line_offset);
        let last_line = self.get_last_line();
        if last_line >= self.rope.len_lines() {
            self.char_absolute_pos_end = self.rope.line_to_char(self.rope.len_lines());
        }
        else {
            self.char_absolute_pos_end = self.rope.line_to_char(last_line) + self.rope.line(last_line).len_chars();
        }
    }

    // Ensures that the caret is visible in the current
    // view of the buffer
    fn ensure_caret_visible(&mut self) {
        let current_line = self.rope.char_to_line(self.get_caret_absolute_pos());
        if current_line > self.get_last_line() || current_line < self.line_offset {
            self.line_offset = current_line;
        }
    }

    // Gets the amount of leading whitespace on the current line.
    // To help with auto indentation
    fn get_leading_whitespace_offset(&self) -> usize {
        let line_slice = self.rope.line(self.rope.char_to_line(self.get_caret_absolute_pos())).chars();
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

    // Finds the number of characters until a boundary is hit.
    // A boundary is defined to be punctuation when the
    // current char is inside a word, and alphanumeric otherwise.
    fn get_boundary_char_count(&self, search_direction: CharSearchDirection) -> usize {
        let caret_absolute_pos = self.get_caret_absolute_pos();
        let mut count = 0;

        match search_direction {
            CharSearchDirection::Forward => {
                if caret_absolute_pos == self.rope.len_chars() {
                    return 0;
                }
                let current_char_type = text_utils::get_char_type(self.rope.char(self.caret_char_pos));
                for chr in self.rope.chars_at(self.get_caret_absolute_pos()) {
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
                let current_char_type = text_utils::get_char_type(self.rope.char(self.caret_char_pos));
                let mut chars = self.rope.chars_at(self.caret_char_pos);
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

    pub fn get_line_number_string(&self) -> Vec<u16> {
        let mut nums: String = String::new();
        let number_range_end = min(self.rope.len_lines(), self.get_last_line() + 1);

        for i in self.line_offset..number_range_end {
            nums += (i + 1).to_string().as_str();
            nums += "\r\n";
        }
        text_utils::to_os_str(&nums)
    }

    pub fn get_text_view_as_string(&self) -> String {
        self.rope.slice(self.char_absolute_pos_start..self.char_absolute_pos_end).to_string()
    }

    pub fn get_text_view_as_utf16(&self) -> Vec<u16> {
        let rope_slice = self.rope.slice(self.char_absolute_pos_start..self.char_absolute_pos_end);
        let chars: Vec<u8> = rope_slice.bytes().collect();
        text_utils::to_os_str(str::from_utf8(chars.as_ref()).unwrap())
    }

    pub fn get_caret_trailing(&self) -> i32 {
        self.caret_trailing
    }

    pub fn get_caret_trailing_as_mut_ref(&mut self) -> &mut i32 {
        &mut self.caret_trailing
    }

    pub fn execute_command(&mut self, cmd: &BufferCommand) {
        match *cmd {
            BufferCommand::ScrollUp(num_lines) => self.scroll_up(num_lines),
            BufferCommand::ScrollDown(num_lines) => self.scroll_down(num_lines),
            BufferCommand::ScrollLeft(num_lines) => self.scroll_left(num_lines),
            BufferCommand::ScrollRight(num_lines) => self.scroll_right(num_lines),
            BufferCommand::LeftClick(mouse_pos, shift_down) => self.left_click(mouse_pos, shift_down),
            BufferCommand::LeftDoubleClick(mouse_pos) => self.left_double_click(mouse_pos),
            BufferCommand::LeftRelease => self.left_release(),
            BufferCommand::SetMouseSelection(mouse_pos) => self.set_mouse_selection(mouse_pos),
            BufferCommand::KeyPressed(key, shift_down, ctrl_down, hwnd) => {
                match (key, ctrl_down) {
                    (VK_LEFT, false)   => self.move_left(shift_down),
                    (VK_LEFT, true)    => self.move_left_by_word(shift_down),
                    (VK_RIGHT, false)   => self.move_right(shift_down),
                    (VK_RIGHT, true)    => self.move_right_by_word(shift_down),
                    (VK_DOWN, _)       => self.set_selection(SelectionMode::Down, 1, shift_down),
                    (VK_UP, _)         => self.set_selection(SelectionMode::Up, 1, shift_down),
                    (VK_TAB, _)        => {
                        self.push_undo_state();
                        self.insert_chars(" ".repeat(NUMBER_OF_SPACES_PER_TAB).as_str());
                    },
                    (VK_RETURN, false) => {
                        self.push_undo_state();
                        self.insert_newline();
                    },
                    (VK_DELETE, false) => {
                        self.push_undo_state();
                        self.delete_right();
                    },
                    (VK_DELETE, true) => {
                        self.push_undo_state();
                        self.delete_right_by_word();
                    },
                    (VK_BACK, false) => {
                        self.push_undo_state();
                        self.delete_left();
                    },
                    (VK_BACK, true) => {
                        self.push_undo_state();
                        self.delete_left_by_word();
                    },
                    // CTRL+A (Select all)
                    (0x41, true) => {
                        self.select_all();
                    }
                    // CTRL+C (Copy)
                    (0x43, true) => {
                        self.copy_selection(hwnd);
                    },
                    // CTRL+X (Cut)
                    (0x58, true) => {
                        self.push_undo_state();
                        self.cut_selection(hwnd);
                    },
                    // CTRL+V (Paste)
                    (0x56, true) => {
                        self.push_undo_state();
                        self.paste(hwnd);
                    }
                    // CTRL+Z (Undo)
                    (0x5A, true) => {
                        self.undo();
                    }
                    _ => {}
                }
            }
            BufferCommand::CharInsert(character) => {
                if text_utils::is_whitespace((character as u8) as char) {
                    self.push_undo_state();
                }
                self.insert_char(character);
            }
        }
        self.on_change();
    }
}
