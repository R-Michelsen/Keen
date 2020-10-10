use std::{
    collections::HashMap,
    str,
    path::Path,
    ptr::null_mut,
    slice::from_raw_parts
};
use winapi::{
    Class,
    Interface,
    ctypes::c_void,
    shared::windef::HWND,
    um::{
        combaseapi::{CoCreateInstance, CLSCTX_ALL},
        shobjidl::{IFileOpenDialog, FOS_PICKFOLDERS},
        shobjidl_core::{IShellItem, FileOpenDialog, SIGDN_FILESYSPATH},
        winuser::{VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN, VK_TAB, VK_RETURN, VK_DELETE, VK_BACK} 
    }
};

use crate::{
    settings::{SCROLL_LINES_PER_MOUSEMOVE, SCROLL_LINES_PER_ROLL, 
     NUMBER_OF_SPACES_PER_TAB, SCROLL_ZOOM_DELTA},
    renderer::TextRenderer,
    language_support::{CPP_FILE_EXTENSIONS, CPP_LANGUAGE_IDENTIFIER, RUST_FILE_EXTENSIONS, RUST_LANGUAGE_IDENTIFIER},
    buffer::{TextRange, TextBuffer, SelectionMode},
    hr_ok
};

type MousePos = (f32, f32);
type ShiftDown = bool;
type CtrlDown = bool;

const TEXT_ORIGIN: (f32, f32) = (0.0_f32, 0.0_f32);

#[derive(Debug, PartialEq)]
pub enum EditorCommand {
    ScrollUp(CtrlDown),
    ScrollDown(CtrlDown),
    LeftClick(MousePos, ShiftDown),
    LeftDoubleClick(MousePos),
    LeftRelease,
    MouseMove(MousePos),
    KeyPressed(i32, ShiftDown, CtrlDown),
    CharInsert(u16)
}

pub struct Editor {
    hwnd: HWND,
    renderer: TextRenderer,

    buffers: HashMap<String, TextBuffer>,
    current_buffer: String,

    mouse_pos: (f32, f32),
    mouse_pos_captured: bool
}

impl Editor {
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            renderer: TextRenderer::new(hwnd, "Consolas", 20.0),

            buffers: HashMap::new(),
            current_buffer: "".to_owned(),

            mouse_pos: (0.0, 0.0),
            mouse_pos_captured: false
        }
    }

    pub fn open_file(&mut self, path: &str) {
        let file_prefix = "file:///".to_owned();
        let os_path = Path::new(path);
        let extension = os_path.extension().unwrap().to_str().unwrap();

        let language_identifier = 
        if CPP_FILE_EXTENSIONS.contains(&extension) {
            CPP_LANGUAGE_IDENTIFIER
        }
        else if RUST_FILE_EXTENSIONS.contains(&extension) {
            RUST_LANGUAGE_IDENTIFIER
        }
        else {
            ""
        };

        self.buffers.insert(
            path.to_string(),
            TextBuffer::new(
                path,
                language_identifier,
                self.renderer.get_max_rows(),
                self.renderer.get_max_columns()
            )
        );
        self.current_buffer = path.to_string();
    }

    pub fn draw(&mut self) {
        let current_buffer = self.buffers.get_mut(&self.current_buffer);
        if let Some(buffer) = current_buffer {
            if buffer.view_dirty {
                self.renderer.update_buffer_layout(TEXT_ORIGIN, self.renderer.get_extents(), buffer);
                self.renderer.update_buffer_line_number_layout(TEXT_ORIGIN, self.renderer.get_extents(), buffer);
                buffer.view_dirty = false;
            }
            self.renderer.draw(buffer);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);

        for buffer in self.buffers.values_mut() {
            buffer.refresh_metrics(self.renderer.get_max_rows(), self.renderer.get_max_columns());
            buffer.view_dirty = true;
        }
    }

    pub fn capture_mouse(&mut self) {
        self.mouse_pos_captured = true;
    }

    pub fn release_mouse(&mut self) {
        self.mouse_pos_captured = false;
    }

    pub fn get_current_selection(&self) -> Option<TextRange> {
        if let Some(buffer) = self.buffers.get(&self.current_buffer) {
            return buffer.get_selection_range();
        }
        None
    }

    fn open_workspace(&mut self) {
        let mut file_dialog: *mut IFileOpenDialog = null_mut();

        unsafe {
            hr_ok!(
                CoCreateInstance(
                    &FileOpenDialog::uuidof(),
                    null_mut(), 
                    CLSCTX_ALL, 
                    &IFileOpenDialog::uuidof(),
                    (&mut file_dialog as *mut *mut _) as *mut *mut c_void
                )
            );

            hr_ok!((*file_dialog).SetOptions(FOS_PICKFOLDERS));
            hr_ok!((*file_dialog).Show(null_mut()));

            let mut shell_item: *mut IShellItem = null_mut();
            hr_ok!((*file_dialog).GetResult(&mut shell_item));

            let mut folder_path: *mut u16 = null_mut();
            hr_ok!((*shell_item).GetDisplayName(SIGDN_FILESYSPATH, &mut folder_path)); 

            // We need to get the length of the folder path manually...
            let mut length = 0;
            while (*folder_path.add(length)) != 0x0000 {
                length += 1;
            }

            let slice = from_raw_parts(folder_path, length);

            (*shell_item).Release();
            (*file_dialog).Release();
        }
    }

    fn change_font_size(zoom_delta: f32, text_renderer: &mut TextRenderer) {
        text_renderer.update_text_format(zoom_delta);
    }

    fn execute_buffer_command(&mut self, cmd: &EditorCommand) {
        if let Some(buffer) = self.buffers.get_mut(&self.current_buffer) {
            match *cmd {
                EditorCommand::ScrollUp(ctrl_down) => {
                    match ctrl_down {
                        true => {
                            Self::change_font_size(SCROLL_ZOOM_DELTA, &mut self.renderer);
                            buffer.refresh_metrics(self.renderer.get_max_rows(), self.renderer.get_max_columns());
                            buffer.view_dirty = true;
                        },
                        false => { buffer.scroll_up(SCROLL_LINES_PER_ROLL); }
                    }
                }
                EditorCommand::ScrollDown(ctrl_down) => {
                    match ctrl_down {
                        true => {
                            Self::change_font_size(-SCROLL_ZOOM_DELTA, &mut self.renderer);
                            buffer.refresh_metrics(self.renderer.get_max_rows(), self.renderer.get_max_columns());
                            buffer.view_dirty = true;
                        }
                        false => { buffer.scroll_down(SCROLL_LINES_PER_ROLL); }
                    }
                }
                EditorCommand::LeftClick(mouse_pos, shift_down) => {
                    let text_pos = self.renderer.mouse_pos_to_text_pos(buffer, mouse_pos);
                    buffer.left_click(text_pos, shift_down);
                }
                EditorCommand::LeftDoubleClick(mouse_pos) => {
                    let text_pos = self.renderer.mouse_pos_to_text_pos(buffer, mouse_pos);
                    buffer.left_double_click(text_pos);
                }
                EditorCommand::LeftRelease => buffer.left_release(),
                EditorCommand::MouseMove(mouse_pos) => {
                    let extents = self.renderer.get_extents();
                    if mouse_pos.1 > (TEXT_ORIGIN.1 + extents.1) {
                        buffer.scroll_down(SCROLL_LINES_PER_MOUSEMOVE);
                    }
                    else if mouse_pos.1 < TEXT_ORIGIN.1 {
                        buffer.scroll_up(SCROLL_LINES_PER_MOUSEMOVE);
                    }
                    if mouse_pos.0 > (TEXT_ORIGIN.0 + extents.0) {
                        buffer.scroll_right(SCROLL_LINES_PER_MOUSEMOVE);
                    }
                    else if mouse_pos.0 < TEXT_ORIGIN.0 {
                        buffer.scroll_left(SCROLL_LINES_PER_MOUSEMOVE);
                    }
                    if buffer.currently_selecting {
                        let text_pos = self.renderer.mouse_pos_to_text_pos(buffer, mouse_pos);
                        buffer.set_mouse_selection(text_pos);
                    }
                }
                EditorCommand::KeyPressed(key, shift_down, ctrl_down) => { 
                    match (key, ctrl_down) {
                        (VK_LEFT, false)   => buffer.move_left(shift_down),
                        (VK_LEFT, true)    => buffer.move_left_by_word(shift_down),
                        (VK_RIGHT, false)  => buffer.move_right(shift_down),
                        (VK_RIGHT, true)   => buffer.move_right_by_word(shift_down),
                        (VK_DOWN, _)       => buffer.set_selection(SelectionMode::Down, 1, shift_down),
                        (VK_UP, _)         => buffer.set_selection(SelectionMode::Up, 1, shift_down),
                        (VK_TAB, _)        => {
                            buffer.insert_chars(" ".repeat(NUMBER_OF_SPACES_PER_TAB).as_str());
                        },
                        (VK_RETURN, false) => {
                            buffer.insert_newline();
                        },
                        (VK_DELETE, false) => {
                            buffer.delete_right();
                        },
                        (VK_DELETE, true) => {
                            buffer.delete_right_by_word();
                        },
                        (VK_BACK, false) => {
                            buffer.delete_left();
                        },
                        (VK_BACK, true) => {
                            buffer.delete_left_by_word();
                        },
                        // CTRL+A (Select all)
                        (0x41, true) => {
                            buffer.select_all();
                        }
                        // CTRL+C (Copy)
                        (0x43, true) => {
                            buffer.copy_selection(self.hwnd);
                        },
                        // CTRL+X (Cut)
                        (0x58, true) => {
                            buffer.cut_selection(self.hwnd);
                        },
                        // CTRL+V (Paste)
                        (0x56, true) => {
                            buffer.paste(self.hwnd);
                        }
                        _ => {}
                    }
                }
                EditorCommand::CharInsert(character) => {
                    buffer.insert_char(character);
                }
            }
        }
    }

    pub fn execute_command(&mut self, cmd: &EditorCommand) {
        match *cmd {
            EditorCommand::MouseMove(mouse_pos) if !self.mouse_pos_captured => {
                self.mouse_pos = mouse_pos;
            }
            EditorCommand::KeyPressed(key, _, ctrl_down) => { 
                match (key, ctrl_down) {
                    (0x4F, true) => self.open_workspace(),
                    _ => {}
                }
            }
            _ => {}
        }

        self.execute_buffer_command(cmd);
    }
}
