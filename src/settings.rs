pub const CPP_FILE_EXTENSIONS: [&str; 5] = ["c", "h", "cpp", "hpp", "cxx"];
pub const CPP_LSP_SERVER: &str = "clangd";
pub const CPP_LANGUAGE_IDENTIFIER: &str = "cpp";
pub const RUST_FILE_EXTENSIONS: [&str; 1] = ["rs"];
pub const RUST_LSP_SERVER: &str = "rust-analyzer";
pub const RUST_LANGUAGE_IDENTIFIER: &str = "rust";

pub const MAX_LSP_RESPONSE_SIZE: usize = 1048576; // 1MB per message received alloc/dealloc;

pub const MOUSEWHEEL_LINES_PER_ROLL: usize = 3;
pub const NUMBER_OF_SPACES_PER_TAB: usize = 4;

