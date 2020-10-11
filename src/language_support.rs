use crate::text_utils;
use std::collections::HashMap;
use winapi::um::dwrite::DWRITE_TEXT_RANGE;
use ropey::iter::Chars;


pub const CPP_KEYWORDS: [&str; 92] = ["alignas", "alignof", "and", "and_eq", "asm", 
"auto", "bitand", "bitor", "bool", "break", "case", "catch", "char", "char8_t", 
"char16_t", "char32_t", "class", "compl", "concept", "const", "consteval", 
"constexpr", "constinit", "const_cast", "continue", "co_await", "co_return", 
"co_yield", "decltype", "default", "delete", "do", "double", "dynamic_cast", 
"else", "enum", "explicit", "export", "extern", "false", "float", "for", "friend", 
"goto", "if", "inline", "int", "long", "mutable", "namespace", "new", "noexcept", 
"not", "not_eq", "nullptr", "operator", "or", "or_eq", "private", "protected", 
"public", "register", "reinterpret_cast", "requires", "return", "short", "signed", 
"sizeof", "static", "static_assert", "static_cast", "struct", "switch", "template", 
"this", "thread_local", "throw", "true", "try", "typedef", "typeid", "typename", 
"union", "unsigned", "using", "virtual", "void", "volatile", "wchar_t", "while", 
"xor", "xor_eq"];
pub const CPP_FILE_EXTENSIONS: [&str; 5] = ["c", "h", "cpp", "hpp", "cxx"];
pub const CPP_LANGUAGE_IDENTIFIER: &str = "cpp";

pub const RUST_KEYWORDS: [&str; 38] = ["as", "break", "const", "continue", "crate", 
"else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", 
"match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self",  "static", 
"struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", 
"async", "await", "dyn"];
pub const RUST_FILE_EXTENSIONS: [&str; 1] = ["rs"];
pub const RUST_LANGUAGE_IDENTIFIER: &str = "rust";

#[derive(PartialEq)]
pub enum SemanticTokenTypes {
    Comment,
    Keyword,
    Literal,
    Preprocessor,
}

fn new_range(start: usize, length: usize) -> DWRITE_TEXT_RANGE {
    DWRITE_TEXT_RANGE {
        startPosition: start as u32,
        length: length as u32
    }
}

pub struct LexicalHighlights {
    pub highlight_tokens: Vec<(DWRITE_TEXT_RANGE, SemanticTokenTypes)>,
    pub enclosing_brackets: Option<[Option<usize>; 2]>
}

pub fn highlight_text(text: &str, start_pos: usize, caret_pos: usize, language_identifier: &'static str, mut start_it: Chars, mut caret_it: Chars) -> LexicalHighlights {
    let mut highlight_tokens = Vec::new();

    // Singleline and multiline comments style
    // can convert to a match statement 
    // once languages with different styles are introduced
    let sl_comment =  "//";
    let ml_comment = ["/*", "*/"];

    let string_literal = '"';
    let escaped_string_literal = "\\\"";

    // Initially we need to look back and see if the first line 
    // already inside a multiline comment
    let mut inside_comment = false;
    let do_match: Vec<char> = ml_comment[0].chars().rev().collect();
    let dont_match: Vec<char> = ml_comment[1].chars().rev().collect();
    let length0 = do_match.len();
    let length1 = dont_match.len();
    let mut index0 = 0;
    let mut index1 = 0;
    while let Some(chr) = start_it.prev() {
        if chr == do_match[index0] {
            index0 += 1;
            // If we found a match, the first line is inside a multiline comment
            if index0 == length0 {
                inside_comment = true;
                break;
            }
        }
        else {
            index0 = 0;
        }
        if chr == dont_match[index1] {
            index1 += 1;
            // If a closing bracket was found first, return
            if index1 == length1 {
                break;
            }
        }
        else {
            index1 = 0;
        }
    }

    let mut offset = 0;
    let mut identifier = String::from("");
    while offset < text.len() {
        let slice = unsafe { text.get_unchecked(offset..text.len()) };
        // If we run into a multiline comment ending,
        // insert a comment if the start of the view 
        // was already inside a multiline comment
        if slice.starts_with(ml_comment[1]) && inside_comment {
            highlight_tokens.push((new_range(0, offset + 2), SemanticTokenTypes::Comment));
            inside_comment = false;
        }
        else if slice.starts_with(ml_comment[0]) {
            if let Some(mlc_end) = slice.find(ml_comment[1]) {
                highlight_tokens.push((new_range(offset, mlc_end + 2), SemanticTokenTypes::Comment));
                offset += mlc_end + 2;
                continue;
            }
            else {
                highlight_tokens.push((new_range(offset, text.len() - offset), SemanticTokenTypes::Comment));
                break;
            }
        }
        else if slice.starts_with(string_literal) {
            let mut string_offset = 1;
            while string_offset < slice.len() {
                let string_slice = unsafe { slice.get_unchecked(string_offset..slice.len()) };
                if string_slice.starts_with(escaped_string_literal) {
                    string_offset += 2;
                    continue;
                }
                if string_slice.starts_with(string_literal) || string_slice.starts_with(|c: char| c == '\n' || c == '\r') {
                    break;
                }
                string_offset += 1;
            }
            highlight_tokens.push((new_range(offset, string_offset + 1), SemanticTokenTypes::Literal));
            offset += string_offset + 1;
        }
        else if slice.starts_with(sl_comment) {
            // Find the number of bytes until the next newline
            if let Some(newline_offset) = slice.find(|c: char| c == '\n' || c == '\r') {
                highlight_tokens.push((new_range(offset, newline_offset), SemanticTokenTypes::Comment));
                offset += newline_offset;
                continue;
            }
            else {
                highlight_tokens.push((new_range(offset, text.len() - offset), SemanticTokenTypes::Comment));
            }
        }
        else if slice.starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '#') {
            identifier.push(slice.chars().next().unwrap());
        }
        else if slice.starts_with(|c: char| c.is_ascii_punctuation() || c.is_ascii_whitespace()) {
            let keyword_match = match language_identifier {
                CPP_LANGUAGE_IDENTIFIER => CPP_KEYWORDS.contains(&identifier.as_str()),
                RUST_LANGUAGE_IDENTIFIER => RUST_KEYWORDS.contains(&identifier.as_str()),
                _ => false
            };
            if keyword_match {
                highlight_tokens.push((new_range(offset - identifier.len(), identifier.len()), SemanticTokenTypes::Keyword));
            }
            else if language_identifier == CPP_LANGUAGE_IDENTIFIER && identifier.starts_with('#') {
                highlight_tokens.push((new_range(offset - identifier.len(), identifier.len()), SemanticTokenTypes::Preprocessor));
            }
            identifier = String::from("");
        }        
        offset += 1;
    }

    // If the first line of the view is inside
    // a comment and no match was found, the entire
    // view is inside a comment
    if inside_comment {
        return LexicalHighlights {
            highlight_tokens: vec![(new_range(0, text.len()), SemanticTokenTypes::Comment)],
            enclosing_brackets: None
        };
    }

    // Closure to figure out if a text offset is inside a comment.
    // Used when searching for matching bracket pairs
    let contained_in_comments = |offset: isize| -> bool {
        for token in &highlight_tokens {
            let range = (token.0.startPosition as isize)..((token.0.startPosition + token.0.length) as isize);
            if token.1 == SemanticTokenTypes::Comment && range.contains(&(offset - 1)) {
                return true;
            }
        }
        false
    };

    // TODO: The following part finds matching bracket pairs that
    // are not inside comments. It searches beyond the visible
    // text buffer range. In the future perhaps it would be better
    // to only search a certain distance in case no bracket match is found

    // Iterate backwards searching for an opening bracket
    let mut closed_map: HashMap<char, usize> = HashMap::new();
    let mut bracket_type = ('\0', '\0');
    let mut backwards_offset = 0;
    while let Some(prev_char) = caret_it.prev() {
        let relative_pos_caret = caret_pos as isize - start_pos as isize;
        let relative_pos = relative_pos_caret - backwards_offset as isize;

        if let Some(brackets) = text_utils::is_opening_bracket(prev_char) {
            if contained_in_comments(relative_pos) {
                backwards_offset += 1;
                continue;
            }
            match closed_map.get_mut(&brackets.1) {
                Some(size) if *size > 0 => {
                    *size -= 1;
                }
                _ => {
                    bracket_type = brackets;
                    backwards_offset += 1;
                    break;
                }
            }
        }
        if let Some(brackets) = text_utils::is_closing_bracket(prev_char) {
            if contained_in_comments(relative_pos) {
                backwards_offset += 1;
                continue;
            }
            *closed_map.entry(brackets.1).or_insert(0) += 1;
        }
        backwards_offset += 1;
    }

    // If no opening bracket was found behind the cursor, just return
    if bracket_type == ('\0', '\0') {
        return LexicalHighlights {
            highlight_tokens,
            enclosing_brackets: None
        };
    }

    // Now search forward from the same iterator to find the matching
    // closing bracket
    let mut closing_brackets_left = 0;
    for (offset, chr) in caret_it.enumerate() {
        // Skip the first char as it is the opening bracket itself
        if offset == 0 { continue; }
        let relative_pos_caret = caret_pos as isize - start_pos as isize;
        let relative_pos = relative_pos_caret - backwards_offset as isize;

        if let Some(brackets) = text_utils::is_closing_bracket(chr) {
            if contained_in_comments(relative_pos) {
                continue;
            }
            if bracket_type == brackets {
                if closing_brackets_left == 0 {
                    // Get the left bracket position relative to the absolute
                    // start position of the current view
                    let right_pos = offset as isize + (caret_pos as isize - start_pos as isize - backwards_offset as isize);
                    let left_pos = caret_pos as isize - start_pos as isize - backwards_offset as isize;

                    // Only include a position if its inside the visible
                    // range of the current text buffer
                    let visible_range = 0..text.len();
                    return LexicalHighlights {
                        highlight_tokens,
                        enclosing_brackets: Some([
                            if visible_range.contains(&(left_pos as usize)) { Some(left_pos  as usize) } else { None },
                            if visible_range.contains(&(right_pos  as usize)) { Some(right_pos  as usize) } else { None }
                        ])
                    }
                }
                else {
                    closing_brackets_left -= 1;
                }
            }
        }
        else if let Some(brackets) = text_utils::is_opening_bracket(chr) {
            if contained_in_comments(relative_pos) {
                continue;
            }
            if bracket_type == brackets {
                closing_brackets_left += 1;
            }
        }
    }

    LexicalHighlights {
        highlight_tokens,
        enclosing_brackets: None
    }
}
