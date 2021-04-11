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
    settings::{SCROLL_LINES_PER_ROLL, SCROLL_LINES_PER_DRAG, SCROLL_ZOOM_DELTA},
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

pub struct TextView {
    pub line_offset: usize,
    pub column_offset: usize
}

pub struct TextDocument {
    pub buffer: TextBuffer,
    pub view: TextView
}

fn scroll_view_up(text_document: &mut TextDocument, lines_per_roll: usize) {
    if text_document.view.line_offset >= lines_per_roll {
        text_document.view.line_offset -= lines_per_roll;
    }
    else {
        text_document.view.line_offset = 0;
    }
}

fn scroll_view_down(text_document: &mut TextDocument, lines_per_roll: usize) {
    let new_top = text_document.view.line_offset + lines_per_roll;
    let number_of_lines = text_document.buffer.get_number_of_lines();

    if new_top >= number_of_lines {
        text_document.view.line_offset = number_of_lines - 1;
    }
    else {
        text_document.view.line_offset = new_top;
    }
}

pub fn scroll_view_left(text_document: &mut TextDocument, lines_per_roll: usize) {
    if text_document.view.column_offset >= lines_per_roll {
        text_document.view.column_offset -= lines_per_roll;
    }
    else {
        text_document.view.column_offset = 0;
    }
}

pub fn scroll_view_right(text_document: &mut TextDocument, lines_per_roll: usize) {
    let new_column = text_document.view.column_offset + lines_per_roll;
    let line_length = text_document.buffer.get_current_line_length();

    if new_column > line_length {
        text_document.view.column_offset = line_length - 1;
    }
    else {
        text_document.view.column_offset = new_column;
    }
}
pub struct Editor {
    hwnd: HWND,
    renderer: TextRenderer,

    documents: HashMap<String, TextDocument>,
    current_document: String,
}

impl Editor {
    pub fn new(hwnd: HWND) -> Result<Self> {
        Ok(Self {
            hwnd,
            renderer: TextRenderer::new(hwnd, "Consolas", 20.0)?,
            documents: HashMap::new(),
            current_document: "".to_owned(),
        })
    }

    pub fn open_file(&mut self, path: &str) {
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

        self.documents.insert(
            path.to_string(),
            TextDocument {
                buffer: TextBuffer::new(path, language_identifier),
                view: TextView {
                    line_offset: 0,
                    column_offset: 0 
                }
            }
        );
        self.current_document = path.to_string();
    }

    pub fn draw(&mut self) {
        if let Some(document) = self.documents.get_mut(&self.current_document) {
            unwrap_hresult(self.renderer.update_buffer_layout(document));
            unwrap_hresult(self.renderer.draw(document));
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        unwrap_hresult(self.renderer.resize(width, height));
    }

    pub fn get_current_selection(&self) -> Option<TextRange> {
        if let Some(document) = self.documents.get(&self.current_document) {
            return document.buffer.get_selection_range(
                document.view.line_offset, 
                document.view.line_offset + self.renderer.get_max_rows()
            );
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
        unwrap_hresult(text_renderer.update_text_format(zoom_delta));
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
        if let Some(document) = self.documents.get_mut(&self.current_document) {
            match *cmd {
                EditorCommand::ScrollUp(ctrl_down) => {
                    match ctrl_down {
                        true => Self::change_font_size(SCROLL_ZOOM_DELTA, &mut self.renderer),
                        false => scroll_view_up(document, SCROLL_LINES_PER_ROLL)
                    }
                }
                EditorCommand::ScrollDown(ctrl_down) => {
                    match ctrl_down {
                        true => Self::change_font_size(-SCROLL_ZOOM_DELTA, &mut self.renderer),
                        false => scroll_view_down(document, SCROLL_LINES_PER_ROLL)
                    }
                }
                EditorCommand::LeftClick(mouse_pos, shift_down) => {
                    let text_pos = unwrap_hresult(self.renderer.mouse_pos_to_text_pos(document, mouse_pos));
                    document.buffer.execute_command(&BufferCommand::LeftClick(text_pos, shift_down));
                }
                EditorCommand::LeftDoubleClick(mouse_pos) => {
                    let text_pos = unwrap_hresult(self.renderer.mouse_pos_to_text_pos(document, mouse_pos));
                    document.buffer.execute_command(&BufferCommand::LeftDoubleClick(text_pos));
                }
                EditorCommand::LeftRelease => document.buffer.execute_command(&BufferCommand::LeftRelease),
                EditorCommand::MouseMove(mouse_pos) => {
                    let extents = self.renderer.get_extents();
                    if mouse_pos.1 > (TEXT_ORIGIN.1 + extents.1) {
                        scroll_view_down(document, SCROLL_LINES_PER_DRAG);
                    }
                    else if mouse_pos.1 < TEXT_ORIGIN.1 {
                        scroll_view_up(document, SCROLL_LINES_PER_DRAG);
                    }
                    if mouse_pos.0 > (TEXT_ORIGIN.0 + extents.0) {
                        scroll_view_right(document, SCROLL_LINES_PER_DRAG);
                    }
                    else if mouse_pos.0 < TEXT_ORIGIN.0 {
                        scroll_view_left(document, SCROLL_LINES_PER_DRAG);
                    }
                    if document.buffer.currently_selecting {
                        let text_pos = unwrap_hresult(self.renderer.mouse_pos_to_text_pos(document, mouse_pos));
                        document.buffer.execute_command(&BufferCommand::SetMouseSelection(text_pos));
                    }
                }
                EditorCommand::KeyPressed(key, shift_down, ctrl_down) => document.buffer.execute_command(&BufferCommand::KeyPressed(key, shift_down, ctrl_down, self.hwnd)),
                EditorCommand::CharInsert(character) => document.buffer.execute_command(&BufferCommand::CharInsert(character))
            }
        }
    }
}
