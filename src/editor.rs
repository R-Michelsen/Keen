use std::{
    collections::HashMap,
    str,
    rc::Rc, 
    cell::RefCell,
    path::Path
};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{ VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN, VK_TAB, VK_RETURN, VK_DELETE, VK_BACK };

use crate::renderer::TextRenderer;
use crate::lsp_client::LSPClient;
use crate::lsp_client::LSPRequestType;
use crate::lsp_structs::*;
use crate::settings::*;
use crate::buffer::{ TextBuffer, SelectionMode, MouseSelectionMode };

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
    pub fn new(hwnd: HWND) -> Editor {
        Editor {
            hwnd,
            renderer: Rc::new(RefCell::new(TextRenderer::new(hwnd.clone(), "Fira Code Retina", 20.0))),
            lsp_client: None,
            buffers: HashMap::new(),
            current_buffer: "".to_owned(),

            force_visible_caret_timer: 0,
            caret_is_visible: true
        }
    }

    pub fn start_language_server(&mut self, path: &str) {
        let os_path = Path::new(path);
        let extension = os_path.extension().unwrap().to_str().unwrap();

        match &self.lsp_client {
            None if CPP_FILE_EXTENSIONS.contains(&extension) => {
                self.lsp_client = Some(LSPClient::new(self.hwnd.clone(), CPP_LSP_SERVER));
            },
            None if RUST_FILE_EXTENSIONS.contains(&extension) => {
                self.lsp_client = Some(LSPClient::new(self.hwnd.clone(), RUST_LSP_SERVER));
            },
            _ => {} 
        }
    }

    pub fn open_file(&mut self, path: &str) {
        let file_prefix = "file:///".to_owned();
        let lsp_client = self.lsp_client.as_mut().unwrap();
        let os_path = Path::new(path);
        let extension = os_path.extension().unwrap().to_str().unwrap();

        let text = std::fs::read_to_string(os_path).unwrap();

        let mut language_identifier = "";
        if CPP_FILE_EXTENSIONS.contains(&extension) {
            language_identifier = CPP_LANGUAGE_IDENTIFIER;
            lsp_client.send_open_file_notification(file_prefix.clone() + path, language_identifier.to_owned(), text);
        }
        else if RUST_FILE_EXTENSIONS.contains(&extension) {
            language_identifier = RUST_LANGUAGE_IDENTIFIER;
            lsp_client.send_open_file_notification(file_prefix.clone() + path, language_identifier.to_owned(), text);
        }

        self.buffers.insert(
            file_prefix.clone() + path,
            TextBuffer::new(
                path,
                language_identifier,
                (0, 0), 
                (self.renderer.borrow().pixel_size.width, self.renderer.borrow().pixel_size.height), 
                self.renderer.clone()
            )
        );
        self.current_buffer = file_prefix.clone() + path;

        lsp_client.send_semantic_token_request(file_prefix + path);
    }

    pub fn draw(&mut self) {
        if let Some(buffer) = self.buffers.get_mut(&self.current_buffer) {
            self.renderer.borrow().draw(buffer, self.caret_is_visible);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.borrow_mut().resize(width, height);
        for (_, buffer) in self.buffers.iter_mut() {
            buffer.update_metrics((0, 0), (self.renderer.borrow().pixel_size.width, self.renderer.borrow().pixel_size.height));
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

    pub fn get_response_id(&self, response: &str) -> i64 {
        let generic_response: GenericResponse = serde_json::from_str(&response).unwrap();

        match generic_response.id {
            serde_json::Value::Number(x) => x.as_i64().unwrap(),
            serde_json::Value::String(x) => x.parse::<i64>().unwrap(),
            _ => {
                println!("Unrecognized response ID from language server");
                -1
            }
        }
    }

    pub fn process_language_server_response(&mut self, response: &str) {
        // Don't handle requests from server
        
        if response.contains("method") {
            return;
        }

        let response_id = self.get_response_id(response);
        let lsp_client = self.lsp_client.as_mut().unwrap();
        let request_type = &lsp_client.request_types[response_id as usize];

        match request_type {
            LSPRequestType::InitializationRequest => lsp_client.send_initialized_notification(),
            LSPRequestType::SemanticTokenRequest(uri) => {
                // Get the buffer for which the semantic token request was issued
                let buffer = self.buffers.get_mut(uri).unwrap();
                let semantic_tokens: SemanticTokenResponse = serde_json::from_str(response).unwrap();

                // Update the semantic tokens of the buffer if they are updated
                if let Some(result) = semantic_tokens.result {
                    buffer.update_semantic_tokens(result.data);
                }
            }
        }
    }

    pub fn process_document_change(did_change_notification: DidChangeNotification, buffer: &mut TextBuffer, lsp_client: &mut LSPClient) {
        // rust-analyzer only supports full change notifications
        match buffer.language_identifier {
            CPP_LANGUAGE_IDENTIFIER => {
                lsp_client.send_did_change_notification(did_change_notification);
                lsp_client.send_semantic_token_request(buffer.get_uri());
            },
            RUST_LANGUAGE_IDENTIFIER => {
                let full_did_change_notification = buffer.get_full_did_change_notification();
                lsp_client.send_did_change_notification(full_did_change_notification);
                lsp_client.send_semantic_token_request(buffer.get_uri());
            },
            _ => {}
        }
    }

    pub fn execute_command(&mut self, cmd: EditorCommand) {
        let lsp_client = self.lsp_client.as_mut().unwrap();

        if let Some(buffer) = self.buffers.get_mut(&self.current_buffer) {
            match cmd {
                EditorCommand::CaretVisible | EditorCommand::CaretInvisible if self.force_visible_caret_timer > 0 => {
                    self.force_visible_caret_timer = self.force_visible_caret_timer.saturating_sub(1);
                    self.caret_is_visible = true;
                },
                EditorCommand::CaretVisible => self.caret_is_visible = true,
                EditorCommand::CaretInvisible => self.caret_is_visible = false,
                EditorCommand::ScrollUp => {
                    buffer.scroll_up();
                },
                EditorCommand::ScrollDown => { 
                    buffer.scroll_down();
                },
                EditorCommand::LeftClick(mouse_pos, shift_down) => {
                    buffer.left_click(mouse_pos, shift_down);
                    self.force_caret_visible();
                },
                EditorCommand::LeftDoubleClick(mouse_pos) => {
                    buffer.left_double_click(mouse_pos);
                    self.force_caret_visible();
                }
                EditorCommand::LeftRelease => buffer.left_release(),
                EditorCommand::MouseMove(mouse_pos) => {
                    buffer.set_mouse_selection(MouseSelectionMode::Move, mouse_pos);
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
                            Editor::process_document_change(did_change_notification, buffer, lsp_client)
                        },
                        (VK_RETURN, true)  => lsp_client.send_semantic_token_request(buffer.get_uri()),
                        (VK_RETURN, false) => {
                            let did_change_notification = buffer.insert_chars("\r\n");
                            Editor::process_document_change(did_change_notification, buffer, lsp_client)
                        },
                        (VK_DELETE, false) => {
                            let did_change_notification = buffer.delete_right();
                            Editor::process_document_change(did_change_notification, buffer, lsp_client)
                        },
                        (VK_DELETE, true) => {
                            let did_change_notification = buffer.delete_right_by_word();
                            Editor::process_document_change(did_change_notification, buffer, lsp_client)
                        },
                        (VK_BACK, false) => {
                            let did_change_notification = buffer.delete_left();
                            Editor::process_document_change(did_change_notification, buffer, lsp_client)
                        },
                        (VK_BACK, true) => {
                            let did_change_notification = buffer.delete_left_by_word();
                            Editor::process_document_change(did_change_notification, buffer, lsp_client)
                        },
                        _ => {}
                    }
                    self.force_caret_visible();
                }
                EditorCommand::CharInsert(character) => {
                    let did_change_notification = buffer.insert_char(character);
                    Editor::process_document_change(did_change_notification, buffer, lsp_client);
                    self.force_caret_visible();
                }
                EditorCommand::LSPClientCrash(client) => {
                    println!("The {} language server has crashed!", client);
                }
            }
        }
        else {
            match cmd {
                EditorCommand::KeyPressed(key, _, ctrl_down) => { 
                    match (key, ctrl_down) {
                        // (VK_RETURN, true)  => self.open_file("C:/llvm-project/clang/lib/CodeGen/CGBuiltin.cpp"),
                        // (VK_RETURN, true)  => self.open_file("C:/Users/Rasmus/Desktop/Yarr/source/AppEditorLogic.cpp"),
                        (VK_RETURN, true)  => self.open_file("C:/Users/Rasmus/Desktop/keen/src/editor.rs"),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}
