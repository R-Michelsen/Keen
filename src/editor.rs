use std::{
    collections::HashMap,
    str,
    rc::Rc, 
    cell::RefCell,
    path::Path
};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN, VK_TAB, VK_RETURN, VK_DELETE, VK_BACK};

use crate::settings::{SCROLL_LINES_PER_MOUSEMOVE, SCROLL_LINES_PER_ROLL, 
    NUMBER_OF_SPACES_PER_TAB, SCROLL_ZOOM_FACTOR};
use crate::renderer::TextRenderer;
use crate::lsp_client::{LSPClient, LSPRequestType};
use crate::lsp_structs::{GenericNotification, GenericRequest, GenericResponse, 
    DidChangeNotification, ResponseError, SemanticTokenResult, ErrorCodes};
use crate::language_support::{CPP_FILE_EXTENSIONS, CPP_LSP_SERVER, CPP_LANGUAGE_IDENTIFIER, 
    RUST_LSP_SERVER, RUST_FILE_EXTENSIONS, RUST_LANGUAGE_IDENTIFIER};
use crate::buffer::{TextBuffer, SelectionMode, MouseSelectionMode};

type MousePos = (f32, f32);
type ShiftDown = bool;
type CtrlDown = bool;

#[derive(PartialEq)]
pub enum EditorCommand {
    CaretVisible,
    CaretInvisible,
    ScrollUp(CtrlDown),
    ScrollDown(CtrlDown),
    LeftClick(MousePos, ShiftDown),
    LeftDoubleClick(MousePos),
    LeftRelease,
    MouseMove(MousePos),
    KeyPressed(i32, ShiftDown, CtrlDown),
    CharInsert(u16),
    LSPClientCrash(&'static str)
}

pub struct Editor {
    hwnd: HWND,
    renderer: Rc<RefCell<TextRenderer>>,
    lsp_client: Option<LSPClient>,
    buffers: HashMap<String, TextBuffer>,
    current_buffer: String,

    force_visible_caret_timer: u32,
    caret_is_visible: bool
}

impl Editor {
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            renderer: Rc::new(RefCell::new(TextRenderer::new(hwnd, "Fira Code Retina", 20.0))),
            lsp_client: None,
            buffers: HashMap::new(),
            current_buffer: "".to_owned(),

            force_visible_caret_timer: 0,
            caret_is_visible: true
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
            file_prefix.clone() + path,
            TextBuffer::new(
                path,
                language_identifier,
                (0.0, 0.0), 
                (self.renderer.borrow().pixel_size.width as f32, self.renderer.borrow().pixel_size.height as f32), 
                self.renderer.clone()
            )
        );
        self.current_buffer = file_prefix.clone() + path;

        // If the LSP Client is not yet running, create an instance
        // we then return since we will handle the open file request
        // once the LSP Client is actually initialized.
        match &self.lsp_client {
            None if CPP_FILE_EXTENSIONS.contains(&extension) => {
                self.lsp_client = Some(LSPClient::new(self.hwnd, CPP_LSP_SERVER));
                self.lsp_client.as_mut().unwrap().send_initialize_request(path.to_owned());
                return;
            },
            None if RUST_FILE_EXTENSIONS.contains(&extension) => {
                self.lsp_client = Some(LSPClient::new(self.hwnd, RUST_LSP_SERVER));
                self.lsp_client.as_mut().unwrap().send_initialize_request(path.to_owned());
                return;
            },
            _ => {}
        }

        let lsp_client = self.lsp_client.as_mut().unwrap();
        let text = std::fs::read_to_string(os_path).unwrap();
        lsp_client.send_did_open_notification(file_prefix.clone() + path, language_identifier.to_owned(), text);
        lsp_client.send_semantic_token_request(file_prefix + path);
    }

    pub fn draw(&mut self) {
        if let Some(buffer) = self.buffers.get_mut(&self.current_buffer) {
            self.renderer.borrow().draw(buffer, self.caret_is_visible);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.borrow_mut().resize(width, height);
        for buffer in self.buffers.values_mut() {
            buffer.on_window_resize(
                (0.0, 0.0), 
                (self.renderer.borrow().pixel_size.width as f32, self.renderer.borrow().pixel_size.height as f32)
            );
        }
    }

    pub fn selection_active(&self) -> bool {
        if let Some(buffer) = self.buffers.get(&self.current_buffer) {
            return buffer.currently_selecting;
        }
        false
    }

    fn force_caret_visible(&mut self) {
        if self.caret_is_visible {
            self.force_visible_caret_timer = 1;
        }
        else {
            self.caret_is_visible = true;
            self.force_visible_caret_timer = 2;
        }
    }

    fn handle_response_error(&mut self, request_type: LSPRequestType, response_error: &ResponseError) {
        match request_type {
            LSPRequestType::InitializationRequest(_) => {},
            LSPRequestType::SemanticTokensRequest(uri) => {
                // If the semantic token request fails
                // due to content changed, send a new one
                if ErrorCodes::from_i64((*response_error).code) == ErrorCodes::ContentModified {
                    if let Some(lsp_client) = self.lsp_client.as_mut() {
                        lsp_client.send_semantic_token_request(uri);
                    }
                }
            }
        }
    }

    fn handle_response_success(&mut self, request_type: LSPRequestType, result_value: serde_json::Value) {
        if let Some(lsp_client) = self.lsp_client.as_mut() {
            match request_type {
                LSPRequestType::InitializationRequest(path) => {
                    // Send init notification
                    lsp_client.send_initialized_notification();
    
                    // Then open the file that triggered the LSP creation
                    let file_prefix = "file:///".to_owned();
                    let os_path = Path::new(path.as_str());
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
                    let text = std::fs::read_to_string(os_path).unwrap();
                    lsp_client.send_did_open_notification(file_prefix.clone() + path.as_str(), language_identifier.to_owned(), text);
                    lsp_client.send_semantic_token_request(file_prefix + path.as_str());
                },
                LSPRequestType::SemanticTokensRequest(uri) => {
                    // Get the buffer for which the semantic token request was issued
                    let buffer = self.buffers.get_mut(&uri).unwrap();
    
                    // Update the semantic tokens of the buffer if they are updated
                    if let Ok(result) = serde_json::from_value::<SemanticTokenResult>(result_value) {
                        buffer.update_semantic_tokens(result.data);
                    }
                }
            }
        }
    }

    pub fn process_language_server_response(&mut self, message: &str) {
        if let Ok(response) = serde_json::from_str::<GenericResponse>(message) {
            let response_id = match response.id {
                serde_json::Value::Number(x) => x.as_i64().unwrap(),
                serde_json::Value::String(x) => x.parse::<i64>().unwrap(),
                _ => {
                    println!("Unrecognized response ID from language server");
                    -1
                }
            };

            if let Some(lsp_client) = self.lsp_client.as_mut() {
                let request_type = lsp_client.request_types[response_id as usize].clone();

                // Handle any errors
                if let Some(response_error) = response.error {
                    self.handle_response_error(request_type, &response_error)
                }
                // Spec says result is guaranteed to be Some(), when there is no error
                // rust-analyzer doesn't seem to honor this so we have to check it
                else if let Some(response_result) = response.result {
                    self.handle_response_success(request_type, response_result);
                }
            }
        }
        else if let Ok(_) = serde_json::from_str::<GenericNotification>(message) {
            // Atm we don't handle requests
        }
        else if let Ok(_) = serde_json::from_str::<GenericRequest>(message) {
            // Atm we don't handle requests
        }
    }

    pub fn process_document_change(did_change_notification: &DidChangeNotification, buffer: &mut TextBuffer, lsp_client: &mut LSPClient) {
        // rust-analyzer only supports full change notifications
        match buffer.language_identifier {
            CPP_LANGUAGE_IDENTIFIER => {
                lsp_client.send_did_change_notification(did_change_notification);
                lsp_client.send_semantic_token_request(buffer.get_uri());
            },
            RUST_LANGUAGE_IDENTIFIER => {
                let full_did_change_notification = buffer.get_full_did_change_notification();
                lsp_client.send_did_change_notification(&full_did_change_notification);
                lsp_client.send_semantic_token_request(buffer.get_uri());
            },
            _ => {}
        }
    }

    pub fn execute_command(&mut self, cmd: &EditorCommand) {
        if let Some(buffer) = self.buffers.get_mut(&self.current_buffer) {
            match *cmd {
                EditorCommand::CaretVisible | EditorCommand::CaretInvisible if self.force_visible_caret_timer > 0 => {
                    self.force_visible_caret_timer = self.force_visible_caret_timer.saturating_sub(1);
                    self.caret_is_visible = true;
                },
                EditorCommand::CaretVisible => self.caret_is_visible = true,
                EditorCommand::CaretInvisible => self.caret_is_visible = false,
                EditorCommand::ScrollUp(ctrl_down) => {
                    match ctrl_down {
                        true => {
                            self.renderer.borrow_mut().update_text_format(SCROLL_ZOOM_FACTOR);
                            buffer.on_font_change();
                        },
                        false => buffer.scroll_up(SCROLL_LINES_PER_ROLL)
                    }
                    buffer.on_editor_action();
                },
                EditorCommand::ScrollDown(ctrl_down) => {
                    match ctrl_down {
                        true => {
                            self.renderer.borrow_mut().update_text_format(-SCROLL_ZOOM_FACTOR);
                            buffer.on_font_change();
                        },
                        false => buffer.scroll_down(SCROLL_LINES_PER_ROLL)
                    }
                    buffer.on_editor_action();
                },
                EditorCommand::LeftClick(mouse_pos, shift_down) => {
                    buffer.left_click(mouse_pos, shift_down);
                    buffer.on_editor_action();
                    self.force_caret_visible();
                },
                EditorCommand::LeftDoubleClick(mouse_pos) => {
                    buffer.left_double_click(mouse_pos);
                    buffer.on_editor_action();
                    self.force_caret_visible();
                }
                EditorCommand::LeftRelease => buffer.left_release(),
                EditorCommand::MouseMove(mouse_pos) => {
                    if mouse_pos.1 > (buffer.origin.1 + buffer.extents.1) {
                        buffer.scroll_down(SCROLL_LINES_PER_MOUSEMOVE);
                    }
                    else if mouse_pos.1 < buffer.origin.1 {
                        buffer.scroll_up(SCROLL_LINES_PER_MOUSEMOVE);
                    }
                    if mouse_pos.0 > (buffer.origin.0 + buffer.extents.0) {
                        buffer.scroll_right(SCROLL_LINES_PER_MOUSEMOVE);
                    }
                    else if mouse_pos.0 < buffer.origin.0 {
                        buffer.scroll_left(SCROLL_LINES_PER_MOUSEMOVE);
                    }

                    buffer.set_mouse_selection(MouseSelectionMode::Move, mouse_pos);
                    buffer.on_editor_action();
                },
                EditorCommand::KeyPressed(key, shift_down, ctrl_down) => { 
                    match (key, ctrl_down) {
                        (VK_LEFT, false)   => buffer.move_left(shift_down),
                        (VK_LEFT, true)    => buffer.move_left_by_word(shift_down),
                        (VK_RIGHT, false)  => buffer.move_right(shift_down),
                        (VK_RIGHT, true)   => buffer.move_right_by_word(shift_down),
                        (VK_DOWN, _)       => buffer.set_selection(SelectionMode::Down, 1, shift_down),
                        (VK_UP, _)         => buffer.set_selection(SelectionMode::Up, 1, shift_down),
                        (VK_TAB, _)        => {
                            let did_change_notification = buffer.insert_chars(" ".repeat(NUMBER_OF_SPACES_PER_TAB).as_str());
                            if let Some(lsp_client) = self.lsp_client.as_mut() {
                                Self::process_document_change(&did_change_notification, buffer, lsp_client);
                            }
                        },
                        (VK_RETURN, false) => {
                            let did_change_notification = buffer.insert_newline();
                            if let Some(lsp_client) = self.lsp_client.as_mut() {
                                Self::process_document_change(&did_change_notification, buffer, lsp_client);
                            }
                        },
                        (VK_DELETE, false) => {
                            let did_change_notification = buffer.delete_right();
                            if let Some(lsp_client) = self.lsp_client.as_mut() {
                                Self::process_document_change(&did_change_notification, buffer, lsp_client);
                            }
                        },
                        (VK_DELETE, true) => {
                            let did_change_notification = buffer.delete_right_by_word();
                            if let Some(lsp_client) = self.lsp_client.as_mut() {
                                Self::process_document_change(&did_change_notification, buffer, lsp_client);
                            }
                        },
                        (VK_BACK, false) => {
                            let did_change_notification = buffer.delete_left();
                            if let Some(lsp_client) = self.lsp_client.as_mut() {
                                Self::process_document_change(&did_change_notification, buffer, lsp_client);
                            }
                        },
                        (VK_BACK, true) => {
                            let did_change_notification = buffer.delete_left_by_word();
                            if let Some(lsp_client) = self.lsp_client.as_mut() {
                                Self::process_document_change(&did_change_notification, buffer, lsp_client);
                            }
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
                            let did_change_notification = buffer.cut_selection(self.hwnd);
                            if let Some(lsp_client) = self.lsp_client.as_mut() {
                                Self::process_document_change(&did_change_notification, buffer, lsp_client);
                            }
                        },
                        // CTRL+V (Paste)
                        (0x56, true) => {
                            let did_change_notification = buffer.paste(self.hwnd);
                            if let Some(lsp_client) = self.lsp_client.as_mut() {
                                match did_change_notification {
                                    None => {},
                                    Some(notification) => Self::process_document_change(&notification, buffer, lsp_client)
                                }
                            }
                        }
                        _ => {}
                    }
                    buffer.on_editor_action();
                    self.force_caret_visible();
                }
                EditorCommand::CharInsert(character) => {
                    let did_change_notification = buffer.insert_char(character);
                    if let Some(lsp_client) = self.lsp_client.as_mut() {
                        Self::process_document_change(&did_change_notification, buffer, lsp_client);
                    }
                    buffer.on_editor_action();
                    self.force_caret_visible();
                }
                EditorCommand::LSPClientCrash(client) => {
                    println!("The {} language server has crashed!", client);
                }
            }
        }
    }
}
