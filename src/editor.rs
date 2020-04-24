use std::{
    str, 
    rc::Rc, 
    cell::RefCell 
};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{ VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN, VK_TAB, VK_RETURN, VK_DELETE, VK_BACK };

use crate::renderer::TextRenderer;
use crate::buffer::{ TextBuffer, SelectionMode, MouseSelectionMode };
use crate::settings;

type MousePos = (f32, f32);
type ShiftDown = bool;
type CtrlDown = bool;

#[derive(PartialEq)]
pub enum EditorCommand {
    CaretVisible,
    CaretInvisible,
    ScrollUp,
    ScrollDown,
    LeftClick(MousePos, ShiftDown),
    LeftRelease,
    MouseMove(MousePos),
    KeyPressed(i32, ShiftDown, CtrlDown),
    CharInsert(u16)
}

pub struct Editor {
    renderer: Rc<RefCell<TextRenderer>>,
    buffers: Vec<TextBuffer>,
    buffer_idx: usize,

    force_visible_caret_timer: u32,
    caret_is_visible: bool
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
        self.buffers.push(
            TextBuffer::new(
                path, 
                (0, 0), 
                (self.renderer.borrow().pixel_size.width, self.renderer.borrow().pixel_size.height), 
                self.renderer.clone()
            )
        );
    }

    pub fn draw(&mut self) {
        self.renderer.borrow().draw(&mut self.buffers[self.buffer_idx], self.caret_is_visible);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.borrow_mut().resize(width, height);
        for buffer in self.buffers.iter_mut() {
            buffer.update_metrics((0, 0), (self.renderer.borrow().pixel_size.width, self.renderer.borrow().pixel_size.height));
        }
    }

    pub fn selection_active(&self) -> bool {
        self.buffers[self.buffer_idx].currently_selecting
    }

    pub fn execute_command(&mut self, cmd: EditorCommand) {
        match cmd {
            EditorCommand::CaretVisible | EditorCommand::CaretInvisible if self.force_visible_caret_timer > 0 => {
                    self.force_visible_caret_timer = self.force_visible_caret_timer.saturating_sub(1);
                    self.caret_is_visible = true;
            },
            EditorCommand::CaretVisible => self.caret_is_visible = true,
            EditorCommand::CaretInvisible => self.caret_is_visible = false,
            EditorCommand::ScrollUp => {
                self.buffers[self.buffer_idx].scroll_up();
            },
            EditorCommand::ScrollDown => { 
                self.buffers[self.buffer_idx].scroll_down();
            },
            EditorCommand::LeftClick(mouse_pos, shift_down) => {
                self.buffers[self.buffer_idx].left_click(mouse_pos, shift_down);
                self.force_visible_caret_timer = 1;
            },
            EditorCommand::LeftRelease => self.buffers[self.buffer_idx].left_release(),
            EditorCommand::MouseMove(mouse_pos) => {
                self.buffers[self.buffer_idx].set_mouse_selection(MouseSelectionMode::Move, mouse_pos);
            },
            EditorCommand::KeyPressed(key, shift_down, ctrl_down) => { 
                match key {
                    VK_LEFT => self.buffers[self.buffer_idx].move_left(shift_down),
                    VK_RIGHT => self.buffers[self.buffer_idx].move_right(shift_down),
                    VK_DOWN => self.buffers[self.buffer_idx].set_selection(SelectionMode::Down, 1, shift_down),
                    VK_UP => self.buffers[self.buffer_idx].set_selection(SelectionMode::Up, 1, shift_down),
                    VK_TAB => self.buffers[self.buffer_idx].insert_chars(" ".repeat(settings::NUMBER_OF_SPACES_PER_TAB).as_str()),
                    VK_RETURN => self.buffers[self.buffer_idx].insert_chars("\r\n"),
                    VK_DELETE => self.buffers[self.buffer_idx].delete_char(),
                    VK_BACK => self.buffers[self.buffer_idx].delete_previous_char(),
                    _ => {}
                }
                self.force_visible_caret_timer = 1;
            }
            EditorCommand::CharInsert(character) => {
                self.buffers[self.buffer_idx].insert_char(character);
                self.force_visible_caret_timer = 1;
            }
        }
    }
}
