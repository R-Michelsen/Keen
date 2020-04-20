use std::{str, rc::Rc, cell::RefCell};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN};

use crate::renderer::TextRenderer;
use crate::buffer::{TextBuffer, SelectionMode, MouseSelectionMode};

const MOUSEWHEEL_LINES_PER_ROLL: usize = 3;

#[derive(PartialEq)]
pub enum EditorCommand {
    CaretVisible,
    CaretInvisible,
    ScrollUp,
    ScrollDown,
    LeftClick,
    LeftRelease,
    MouseMove,
    KeyPressed,
    CharInsert
}

pub union EditorCommandData {
    pub dummy: bool,
    pub character: u16,
    pub key_shift_ctrl: (i32, bool, bool),
    pub mouse_pos_shift: ((f32, f32), bool),
}

pub struct Editor {
    renderer: Rc<RefCell<TextRenderer>>,
    buffers: Vec<TextBuffer>,
    buffer_idx: usize,

    force_visible_caret_timer: u32,
    pub caret_is_visible: bool
}

impl Editor {
    pub fn new(hwnd: HWND) -> Editor {
        Editor {
            renderer: Rc::new(RefCell::new(TextRenderer::new(hwnd, "Fira Code Retina", 30.0))),
            buffers: Vec::new(),
            buffer_idx: 0,

            force_visible_caret_timer: 0,
            caret_is_visible: true
        }
    }


    pub fn open_file(&mut self, path: &str) {
        self.buffers.push(TextBuffer::new(
                path, 
                (0, 0), 
                ((*self.renderer.borrow()).pixel_size.width, (*self.renderer.borrow()).pixel_size.height), 
                self.renderer.clone())
            );
    }

    pub fn draw(&mut self) {
        (*self.renderer.borrow()).draw(&mut self.buffers[self.buffer_idx], self.caret_is_visible);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        (*self.renderer.borrow_mut()).resize(width, height);
        for buffer in self.buffers.iter_mut() {
            buffer.resize_layer((0, 0), ((*self.renderer.borrow()).pixel_size.width, (*self.renderer.borrow()).pixel_size.height));
        }
    }

    pub fn selection_active(&self) -> bool {
        return self.buffers[self.buffer_idx].currently_selecting;
    }

    pub fn execute_command(&mut self, cmd: EditorCommand, data: EditorCommandData) {
        match cmd {
            EditorCommand::CaretVisible | EditorCommand::CaretInvisible => {
                if self.force_visible_caret_timer > 0 {
                    self.force_visible_caret_timer = self.force_visible_caret_timer.saturating_sub(1);
                    self.caret_is_visible = true;
                }
                else if cmd == EditorCommand::CaretVisible {
                    self.caret_is_visible = true;
                }
                else {
                    self.caret_is_visible = false;
                }
            },
            EditorCommand::ScrollUp => {
                self.buffers[self.buffer_idx].scroll_up(
                    MOUSEWHEEL_LINES_PER_ROLL, 
                    (*self.renderer.borrow()).line_height
                );
            },
            EditorCommand::ScrollDown => { 
                self.buffers[self.buffer_idx].scroll_down(
                    MOUSEWHEEL_LINES_PER_ROLL, 
                    (*self.renderer.borrow()).line_height
                );
            },
            EditorCommand::LeftClick => {
                unsafe {
                    let (mouse_pos, shift) = data.mouse_pos_shift;
                    self.buffers[self.buffer_idx].left_click(mouse_pos, shift);
                    self.force_visible_caret_timer = 1;
                }
            },
            EditorCommand::LeftRelease => self.buffers[self.buffer_idx].left_release(),
            EditorCommand::MouseMove => {
                unsafe {
                    let (mouse_pos, _) = data.mouse_pos_shift;
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
                    self.force_visible_caret_timer = 1;
                }
            }
            EditorCommand::CharInsert => {
                self.buffers[self.buffer_idx].insert_char(unsafe { data.character });
            }
        }
    }
}
