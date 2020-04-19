use std::str;
use winapi::shared::windef::HWND;
use winapi::um::winuser::{VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN};

use crate::renderer::TextRenderer;
use crate::buffer::{TextBuffer, SelectionMode, MouseSelectionMode};

const MOUSEWHEEL_LINES_PER_ROLL: usize = 3;

pub enum EditorCommand {
    ScrollUp,
    ScrollDown,
    LeftClick,
    LeftRelease,
    MouseMove,
    KeyPressed
}

pub union EditorCommandData {
    pub dummy: bool,
    pub key_shift_ctrl: (i32, bool, bool),
    pub mouse_pos_shift: ((f32, f32), bool),
}

pub struct Editor {
    renderer: TextRenderer,
    buffers: Vec<TextBuffer>,
    buffer_idx: usize,

    pub caret_is_visible: bool
}

impl Editor {
    pub fn new(hwnd: HWND) -> Editor {
        Editor {
            renderer: TextRenderer::new(hwnd),
            buffers: Vec::new(),
            buffer_idx: 0,

            caret_is_visible: true
        }
    }

    pub fn open_file(&mut self, path: &str) {
        self.buffers.push(TextBuffer::new(path, self.renderer.write_factory, self.renderer.text_format));
    }

    pub fn draw(&mut self) {
        self.renderer.draw(&mut self.buffers[self.buffer_idx], self.caret_is_visible);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    pub fn selection_active(&self) -> bool {
        return self.buffers[self.buffer_idx].currently_selecting;
    }

    pub fn execute_command(&mut self, cmd: EditorCommand, data: EditorCommandData) {
        match cmd {
            EditorCommand::ScrollUp => self.buffers[self.buffer_idx].scroll_up(MOUSEWHEEL_LINES_PER_ROLL),
            EditorCommand::ScrollDown => self.buffers[self.buffer_idx].scroll_down(MOUSEWHEEL_LINES_PER_ROLL),
            EditorCommand::LeftClick => {
                unsafe {
                    let (mouse_pos, shift) = data.mouse_pos_shift;
                    self.buffers[self.buffer_idx].left_click(mouse_pos, shift);
                }
            },
            EditorCommand::LeftRelease => self.buffers[self.buffer_idx].left_release(),
            EditorCommand::MouseMove => {
                unsafe {
                    let (mouse_pos, shift) = data.mouse_pos_shift;
                    self.buffers[self.buffer_idx].set_mouse_selection(MouseSelectionMode::Move, mouse_pos);
                }
            },
            EditorCommand::KeyPressed => { 
                unsafe {
                    let (key, shift, ctrl) = data.key_shift_ctrl;
                    match key {
                        VK_LEFT => self.buffers[self.buffer_idx].set_selection(SelectionMode::Left, 1, shift),
                        VK_RIGHT => self.buffers[self.buffer_idx].set_selection(SelectionMode::Right, 1, shift),
                        VK_DOWN => self.buffers[self.buffer_idx].set_selection(SelectionMode::Down, 1, shift),
                        VK_UP => self.buffers[self.buffer_idx].set_selection(SelectionMode::Up, 1, shift),
                        _ => {}
                    }
                }
            }
        }
    }
}
