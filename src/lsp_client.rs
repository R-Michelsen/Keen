use crate::language_support::{CPP_LSP_SERVER, RUST_LSP_SERVER};
use crate::lsp_structs::{ClangdInitializationOptions, InitializeRequest, InitializeParams, 
    ClientInfo, ClientCapabilities, TextDocumentClientCapabilities, SemanticTokensRequest, 
    DidOpenNotification, InitializeNotification, DidChangeNotification};
use crate::WM_LSP_RESPONSE;
use crate::WM_LSP_CRASH;
use crate::settings::MAX_LSP_RESPONSE_SIZE;

use std::{
    alloc::{alloc, Layout},
    io::{Read, Write},
    process::{ChildStdin, Command, Stdio},
    thread,
    thread::JoinHandle,
};
use winapi::{shared::windef::HWND, um::winuser::SendMessageW};
use serde_json::to_value;

#[derive(Clone, Debug)]
pub enum LSPRequestType {
    InitializationRequest(String),
    SemanticTokenRequest(String)
}

#[derive(Debug)]
pub struct LSPClient {
    client_name: &'static str,
    request_id: i64,
    pub request_types: Vec<LSPRequestType>,
    stdin: ChildStdin,
    thread: JoinHandle<()>
}

impl LSPClient {
    pub fn new(hwnd: HWND, client_name: &'static str) -> Self {
        // Spawn an instance of the language server
        let mut lsp = Command::new(client_name)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let mut stdout = lsp.stdout.take().unwrap();
        let hwnd_clone = hwnd as u64;

        Self {
            client_name,
            request_id: 0,
            request_types: Vec::new(),
            stdin: lsp.stdin.take().unwrap(),
            thread: thread::spawn(move || {
                unsafe {
                    loop {
                        let layout = Layout::from_size_align(MAX_LSP_RESPONSE_SIZE, 8).unwrap();
                        let allocation = alloc(layout);

                        let header_size = 64;
                        let header: &mut [u8] = core::slice::from_raw_parts_mut(allocation, header_size);

                        let mut content_length_bytes = 0;
                        let mut content_length = 0;
                        let mut remaining_length = 0;
                        if let Ok(()) = stdout.read_exact(header) {
                            if header.starts_with(b"Content-Length: ") {
                                // Parse the header to get the length of the content following
                                // The header ends when the second "\r\n" is encountered
                                let mut number_string = String::new();
                                let mut crlf_count = 0;
                                for chr in header.iter() {
                                    if (*chr as char).is_ascii_digit() {
                                        number_string.push(*chr as char);
                                    }
                                    if (*chr as char) == '\r' {
                                        content_length = number_string.as_str().parse::<usize>().unwrap();
                                        crlf_count += 1;
                                        if crlf_count == 2 {
                                            content_length_bytes += 2;
                                            break;
                                        }
                                    }
                                    content_length_bytes += 1;
                                }
                                remaining_length = content_length - (header_size - content_length_bytes);
                            }
                            else {
                                // If stdout read fails, send LSP crash message
                                // with the client string and length as params
                                SendMessageW(hwnd_clone as HWND, WM_LSP_CRASH, (client_name.as_ptr()) as usize, client_name.len() as isize); 
                                return;
                            }
                        }

                        let content: &mut [u8] = core::slice::from_raw_parts_mut(allocation.add(header_size), remaining_length);
                        if let Ok(()) = stdout.read_exact(content) {
                            let range = (content_length_bytes as i32, content_length as i32);
                            SendMessageW(hwnd_clone as HWND, WM_LSP_RESPONSE, allocation as usize, std::mem::transmute::<(i32, i32), isize>(range));
                        }
                    }
                }
            })
        }
    }

    pub fn send_request(&mut self, request: &str, request_type: LSPRequestType) {
        let message = format!("Content-Length: {}\r\n\r\n{}", request.len(), request);

        // TODO: Handle IO errors
        self.stdin.write_all(message.as_bytes()).unwrap();

        self.request_types.push(request_type);
        self.request_id += 1;
    }

    pub fn send_notification(&mut self, notification: &str) {
        let message = format!("Content-Length: {}\r\n\r\n{}", notification.len(), notification);

        // TODO: Handle IO errors
        self.stdin.write_all(message.as_bytes()).unwrap();
    }

    pub fn send_did_change_notification(&mut self, did_change_notification: &DidChangeNotification) {
        let serialized_did_change_notification = serde_json::to_string(&did_change_notification).unwrap();
        self.send_notification(serialized_did_change_notification.as_str());
    }

    pub fn send_initialized_notification(&mut self) {
        let init_notification = InitializeNotification::new();

        let serialized_init_notification = serde_json::to_string(&init_notification).unwrap();
        self.send_notification(serialized_init_notification.as_str());
    }
    
    pub fn send_open_file_notification(&mut self, uri: String, language_id: String, text: String) {
        let open_file_notification = DidOpenNotification::new(uri, language_id, text);
        let serialized_open_file_notification = serde_json::to_string(&open_file_notification).unwrap();

        self.send_notification(serialized_open_file_notification.as_str());
    }

    pub fn send_semantic_token_request(&mut self, uri: String) {
        let semantic_token_request = SemanticTokensRequest::new(self.request_id, uri.clone());

        let serialized_semantic_token_request = serde_json::to_string(&semantic_token_request).unwrap();
        self.send_request(serialized_semantic_token_request.as_str(), LSPRequestType::SemanticTokenRequest(uri));
    }

    pub fn send_initialize_request(&mut self, path: String) {
        let initialization_options;
        match self.client_name {
            CPP_LSP_SERVER => {
                initialization_options = Some(to_value(ClangdInitializationOptions {
                    clangd_file_status: Some(true)
                }).unwrap());
            },
            RUST_LSP_SERVER => {
                initialization_options = None
            },
            _ => initialization_options = None
        }

        let init_request = InitializeRequest {
            id: self.request_id,
            jsonrpc: "2.0".to_owned(),
            method: "initialize".to_owned(),
            params: InitializeParams {
                process_id: 0,
                client_info: ClientInfo {
                    name: "Keen".to_owned(),
                    version: None,
                },
                root_path: None,
                root_uri: None,

                initialization_options,

                capabilities: ClientCapabilities {
                    workspace: None,
                    text_document: Some(TextDocumentClientCapabilities {
                        synchronization: None,
                        completion: None,
                        hover: None,
                        signature_help: None,
                        declaration: None,
                        definition: None,
                        type_definition: None,
                        implementation: None,
                        references: None,
                        document_highlight: None,
                        document_symbol: None,
                        code_action: None,
                        code_lens: None,
                        document_link: None,
                        color_provider: None,
                        formatting: None,
                        range_formatting: None,
                        on_type_formatting: None,
                        rename: None,
                        publish_diagnostics: None,
                        folding_range: None,
                        selection_range: None,
                        semantic_tokens: None
                    }),
                    window: None,
                    experimental: None
                },

                trace: None,
                workspace_folders: None,
            },
        };

        let serialized_init_request = serde_json::to_string(&init_request).unwrap();
        self.send_request(serialized_init_request.as_str(), LSPRequestType::InitializationRequest(path));
    }
}
