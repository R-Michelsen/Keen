use crate::settings::AUTOCOMPLETE_BRACKETS;

use std::{
    ffi::OsStr,
    iter::once,
    os::windows::ffi::OsStrExt
};

#[derive(Clone, PartialEq)]
pub enum CharType {
    Word,
    Punctuation,
    Linebreak
}

pub fn to_os_str(chars: &str) -> Vec<u16> {
    OsStr::new(chars).encode_wide().chain(once(0)).collect()
}

pub fn get_char_type(chr: char) -> CharType {
    match chr {
        x if is_word(x) => CharType::Word,
        x if is_linebreak(x) => CharType::Linebreak,
        _ => CharType::Punctuation
    }
}

// Underscore is treated as part of a word to make movement
// programming in snake_case easier
pub fn is_word(chr: char) -> bool {
    chr.is_alphanumeric() || chr == '_'
}

pub fn is_whitespace(chr: char) -> bool {
    chr == ' ' || chr == '\t'
}

pub fn is_linebreak(chr: char) -> bool {
    chr == '\n' || chr == '\r' || chr == '\u{000B}' || chr == '\u{000C}' || 
    chr == '\u{0085}' || chr == '\u{2028}' || chr == '\u{2029}'
}

pub fn is_opening_bracket(chr: char) -> Option<(char, char)> {
    for bracket in &AUTOCOMPLETE_BRACKETS {
        if chr == bracket.0 {
            return Some(*bracket);
        }
    }
    None
}

pub fn is_closing_bracket(chr: char) -> Option<(char, char)> {
    for bracket in &AUTOCOMPLETE_BRACKETS {
        if chr == bracket.1 {
            return Some(*bracket);
        }
    }
    None
}
