pub const MAX_LSP_RESPONSE_SIZE: usize = 1_048_576; // 1MB per message received alloc/dealloc;

pub const SCROLL_LINES_PER_ROLL: usize = 3;
pub const SCROLL_LINES_PER_MOUSEMOVE: usize = 3;
pub const NUMBER_OF_SPACES_PER_TAB: usize = 4;
pub const SCROLL_ZOOM_DELTA: f32 = 3.0;

pub const AUTOCOMPLETE_BRACKETS: [(char, char); 3] = [('{', '}'), ('(', ')'), ('[', ']')];
