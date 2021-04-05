use std::{
    collections::HashMap,
    str,
    path::Path
};

use bindings::{
    Windows::Win32::WindowsAndMessaging::*,
};
use windows::Result;

use crate::{
    settings::{SCROLL_LINES_PER_MOUSEMOVE, SCROLL_LINES_PER_ROLL, SCROLL_ZOOM_DELTA},
    renderer::TextRenderer,
    language_support::{CPP_FILE_EXTENSIONS, CPP_LANGUAGE_IDENTIFIER, RUST_FILE_EXTENSIONS, RUST_LANGUAGE_IDENTIFIER},
    buffer::{BufferCommand, TextRange, TextBuffer},
    util::unwrap_hresult
};

type MousePos = (f32, f32);
type ShiftDown = bool;
type CtrlDown = bool;

const TEXT_ORIGIN: (f32, f32) = (0.0_f32, 0.0_f32);

#[derive(PartialEq)]
pub enum EditorCommand {
    ScrollUp(CtrlDown),
    ScrollDown(CtrlDown),
    LeftClick(MousePos, ShiftDown),
    LeftDoubleClick(MousePos),
    LeftRelease,
    MouseMove(MousePos),
    KeyPressed(u32, ShiftDown, CtrlDown),
    CharInsert(u16)
}

pub struct Editor {
    hwnd: HWND,
    renderer: TextRenderer,

    buffers: HashMap<String, TextBuffer>,
    current_buffer: String,
}

impl Editor {
    pub fn new(hwnd: HWND) -> Result<Self> {
        Ok(Self {
            hwnd,
            renderer: TextRenderer::new(hwnd, "Consolas", 20.0)?,
            buffers: HashMap::new(),
            current_buffer: "".to_owned(),
        })
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
                unwrap_hresult(self.renderer.update_buffer_layout(TEXT_ORIGIN, self.renderer.get_extents(), buffer));
                unwrap_hresult(self.renderer.update_buffer_line_number_layout(TEXT_ORIGIN, self.renderer.get_extents(), buffer));
                buffer.view_dirty = false;
            }
            unwrap_hresult(self.renderer.draw(buffer));
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        unwrap_hresult(self.renderer.resize(width, height));

        for buffer in self.buffers.values_mut() {
            buffer.refresh_metrics(self.renderer.get_max_rows(), self.renderer.get_max_columns());
            buffer.view_dirty = true;
        }
    }

    pub fn get_current_selection(&self) -> Option<TextRange> {
        if let Some(buffer) = self.buffers.get(&self.current_buffer) {
            return buffer.get_selection_range();
        }
        None
    }

    fn open_workspace(&mut self) {
        // let mut file_dialog: *mut IFileOpenDialog = null_mut();

        // unsafe {
        //     hr_ok!(
        //         CoCreateInstance(
        //             &FileOpenDialog::uuidof(),
        //             null_mut(), 
        //             CLSCTX_ALL, 
        //             &IFileOpenDialog::uuidof(),
        //             (&mut file_dialog as *mut *mut _) as *mut *mut c_void
        //         )
        //     );

        //     hr_ok!((*file_dialog).SetOptions(FOS_PICKFOLDERS));
        //     hr_ok!((*file_dialog).Show(null_mut()));

        //     let mut shell_item: *mut IShellItem = null_mut();
        //     hr_ok!((*file_dialog).GetResult(&mut shell_item));

        //     let mut folder_path: *mut u16 = null_mut();
        //     hr_ok!((*shell_item).GetDisplayName(SIGDN_FILESYSPATH, &mut folder_path)); 

        //     // We need to get the length of the folder path manually...
        //     let mut length = 0;
        //     while (*folder_path.add(length)) != 0x0000 {
        //         length += 1;
        //     }

        //     let slice = from_raw_parts(folder_path, length);

        //     (*shell_item).Release();
        //     (*file_dialog).Release();
        // }
    }

    fn change_font_size(zoom_delta: f32, text_renderer: &mut TextRenderer) {
        unwrap_hresult(text_renderer.update_text_format());
    }

    pub fn execute_command(&mut self, cmd: &EditorCommand) {
        match *cmd {
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

    fn execute_buffer_command(&mut self, cmd: &EditorCommand) {
        if let Some(buffer) = self.buffers.get_mut(&self.current_buffer) {
            match *cmd {
                EditorCommand::ScrollUp(ctrl_down) => {
                    match ctrl_down {
                        true => {
                            Self::change_font_size(SCROLL_ZOOM_DELTA, &mut self.renderer);
                            buffer.refresh_metrics(self.renderer.get_max_rows(), self.renderer.get_max_columns());
                        },
                        false => { buffer.execute_command(&BufferCommand::ScrollUp(SCROLL_LINES_PER_ROLL)); }
                    }
                }
                EditorCommand::ScrollDown(ctrl_down) => {
                    match ctrl_down {
                        true => {
                            Self::change_font_size(-SCROLL_ZOOM_DELTA, &mut self.renderer);
                            buffer.refresh_metrics(self.renderer.get_max_rows(), self.renderer.get_max_columns());
                        }
                        false => { buffer.execute_command(&BufferCommand::ScrollDown(SCROLL_LINES_PER_ROLL)); }
                    }
                }
                EditorCommand::LeftClick(mouse_pos, shift_down) => {
                    let text_pos = unwrap_hresult(self.renderer.mouse_pos_to_text_pos(buffer, mouse_pos));
                    buffer.execute_command(&BufferCommand::LeftClick(text_pos, shift_down));
                }
                EditorCommand::LeftDoubleClick(mouse_pos) => {
                    let text_pos = unwrap_hresult(self.renderer.mouse_pos_to_text_pos(buffer, mouse_pos));
                    buffer.execute_command(&BufferCommand::LeftDoubleClick(text_pos));
                }
                EditorCommand::LeftRelease => buffer.execute_command(&BufferCommand::LeftRelease),
                EditorCommand::MouseMove(mouse_pos) => {
                    let extents = self.renderer.get_extents();
                    if mouse_pos.1 > (TEXT_ORIGIN.1 + extents.1) {
                        buffer.execute_command(&BufferCommand::ScrollDown(SCROLL_LINES_PER_MOUSEMOVE));
                    }
                    else if mouse_pos.1 < TEXT_ORIGIN.1 {
                        buffer.execute_command(&BufferCommand::ScrollUp(SCROLL_LINES_PER_MOUSEMOVE));
                    }
                    if mouse_pos.0 > (TEXT_ORIGIN.0 + extents.0) {
                        buffer.execute_command(&BufferCommand::ScrollRight(SCROLL_LINES_PER_MOUSEMOVE));
                    }
                    else if mouse_pos.0 < TEXT_ORIGIN.0 {
                        buffer.execute_command(&BufferCommand::ScrollLeft(SCROLL_LINES_PER_MOUSEMOVE));
                    }
                    if buffer.currently_selecting {
                        let text_pos = unwrap_hresult(self.renderer.mouse_pos_to_text_pos(buffer, mouse_pos));
                        buffer.execute_command(&BufferCommand::SetMouseSelection(text_pos));
                    }
                }
                EditorCommand::KeyPressed(key, shift_down, ctrl_down) => buffer.execute_command(&BufferCommand::KeyPressed(key, shift_down, ctrl_down, self.hwnd)),
                EditorCommand::CharInsert(character) => buffer.execute_command(&BufferCommand::CharInsert(character))
            }
        }
    }
}
