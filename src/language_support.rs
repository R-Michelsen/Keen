use winapi::um::dwrite::DWRITE_TEXT_RANGE;
use ropey::iter::Chars;
use crate::lsp_structs::SemanticTokenTypes;

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
pub const CPP_LSP_SERVER: &str = "clangd";
pub const CPP_LANGUAGE_IDENTIFIER: &str = "cpp";

pub const RUST_KEYWORDS: [&str; 38] = ["as", "break", "const", "continue", "crate", 
"else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", 
"match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self",  "static", 
"struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", 
"async", "await", "dyn"];
pub const RUST_FILE_EXTENSIONS: [&str; 1] = ["rs"];
pub const RUST_LSP_SERVER: &str = "rust-analyzer";
pub const RUST_LANGUAGE_IDENTIFIER: &str = "rust";

pub fn highlight_text(text: &str, language_identifier: &'static str, mut start_it: Chars) -> Vec<(DWRITE_TEXT_RANGE, SemanticTokenTypes)> {
    let mut highlights = Vec::new();

    // Singleline and multiline comments style
    // can convert to a match statement 
    // once languages with different styles are introduced
    let sl_comment =  "//";
    let ml_comment = ["/*", "*/"];

    let string_literal = '"';
    let escaped_string_literal = "\\\"";

    let mut offset = 0;
    let mut identifier = String::from("");
    while offset < text.len() {
        let slice = unsafe { text.get_unchecked(offset..text.len()) };
        // If we run into a multiline comment ending,
        // we need to look back and find its counterpart
        // if there is one.
        if slice.starts_with(ml_comment[1]) {
            let to_match: Vec<char> = ml_comment[0].chars().rev().collect();
            let length = to_match.len();
            let mut index = 0;
            while let Some(chr) = start_it.prev() {
                if chr == to_match[index] {
                    index += 1;
                    // Found a match, highlight the code
                    if index == length {
                        let range = DWRITE_TEXT_RANGE {
                            startPosition: 0 as u32,
                            length: (offset + 2) as u32
                        };
                        highlights.push((range, SemanticTokenTypes::Comment));
                    }
                }
                else {
                    index = 0;
                }
            }
        }
        else if slice.starts_with(ml_comment[0]) {
            if let Some(mlc_end) = slice.find(ml_comment[1]) {
                let range = DWRITE_TEXT_RANGE {
                    startPosition: offset as u32,
                    length: (mlc_end + 2) as u32
                };
                highlights.push((range, SemanticTokenTypes::Comment));
                offset += mlc_end + 2;
                continue;
            }
            else {
                let range = DWRITE_TEXT_RANGE {
                    startPosition: offset as u32,
                    length: (text.len() - offset) as u32
                };
                highlights.push((range, SemanticTokenTypes::Comment));
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
            let range = DWRITE_TEXT_RANGE {
                startPosition: offset as u32,
                length: (string_offset + 1) as u32
            };
            highlights.push((range, SemanticTokenTypes::Literal));
            offset += string_offset + 1;
        }
        else if slice.starts_with(sl_comment) {
            // Find the number of bytes until the next newline
            if let Some(newline_offset) = slice.find(|c: char| c == '\n' || c == '\r') {
                let range = DWRITE_TEXT_RANGE {
                    startPosition: offset as u32,
                    length: newline_offset as u32
                };
                highlights.push((range, SemanticTokenTypes::Comment));
                offset += newline_offset;
                continue;
            }
            else {
                let range = DWRITE_TEXT_RANGE {
                    startPosition: offset as u32,
                    length: (text.len() - offset) as u32
                };
                highlights.push((range, SemanticTokenTypes::Comment));
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
                let range = DWRITE_TEXT_RANGE {
                    startPosition: (offset - identifier.len()) as u32,
                    length: identifier.len() as u32
                };
                highlights.push((range, SemanticTokenTypes::Keyword));
            }
            else if language_identifier == CPP_LANGUAGE_IDENTIFIER && identifier.starts_with('#') {
                let range = DWRITE_TEXT_RANGE {
                    startPosition: (offset - identifier.len()) as u32,
                    length: identifier.len() as u32
                };
                highlights.push((range, SemanticTokenTypes::Preprocessor));
            }
            identifier = String::from("");
        }
        
        offset += 1;
    }

    highlights
}