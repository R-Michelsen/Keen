use serde::{Deserialize, Serialize};
use serde_json::Value;

type DocumentUri = String;
type MarkupKind = String;
type CodeActionKind = String;
type SymbolKind = i64;
type CompletionItemKind = i64;
type CompletionItemTag = i64;
type ErrorCode = i64;
type TextDocumentSymbolKind = i64;
type SemanticTokenType = String;
type SemanticTokenModifier = String;

#[allow(dead_code)]
pub enum CompletionItemKinds {
	Text = 1,
	Method = 2,
	Function = 3,
	Constructor = 4,
	Field = 5,
	Variable = 6,
	Class = 7,
	Interface = 8,
	Module = 9,
	Property = 10,
	Unit = 11,
	Value = 12,
	Enum = 13,
	Keyword = 14,
	Snippet = 15,
	Color = 16,
	File = 17,
	Reference = 18,
	Folder = 19,
	EnumMember = 20,
	Constant = 21,
	Struct = 22,
	Event = 23,
	Operator = 24,
	TypeParameter = 25
}

#[allow(dead_code)]
pub enum SymbolKinds {
	File = 1,
	Module = 2,
	Namespace = 3,
	Package = 4,
	Class = 5,
	Method = 6,
	Property = 7,
	Field = 8,
	Constructor = 9,
	Enum = 10,
	Interface = 11,
	Function = 12,
	Variable = 13,
	Constant = 14,
	String = 15,
	Number = 16,
	Boolean = 17,
	Array = 18,
	Object = 19,
	Key = 20,
	Null = 21,
	EnumMember = 22,
	Struct = 23,
	Event = 24,
	Operator = 25,
	TypeParameter = 26
}

#[allow(dead_code)]
pub enum CompletionItemTags {
    Deprecrated = 1
}

#[allow(dead_code)]
#[derive(PartialEq)]
pub enum ErrorCodes {
	ParseError = -32700,
	InvalidRequest = -32600,
	MethodNotFound = -32601,
	InvalidParams = -32602,
	InternalError = -32603,
	ServerErrorStart = -32099,
	ServerErrorEnd = -32000,
	ServerNotInitialized = -32002,
	UnknownErrorCode = -32001,
	RequestCancelled = -32800,
    ContentModified = -32801,
    Unknown = 0
}

impl ErrorCodes {
    pub fn from_i64(int: i64) -> Self {
        match int {
            -32700 => Self::ParseError,
            -32600 => Self::InvalidRequest,
            -32601 => Self::MethodNotFound,
            -32602 => Self::InvalidParams,
            -32603 => Self::InternalError,
            -32099 => Self::ServerErrorStart,
            -32000 => Self::ServerErrorEnd,
            -32002 => Self::ServerNotInitialized,
            -32001 => Self::UnknownErrorCode,
            -32800 => Self::RequestCancelled,
            -32801 => Self::ContentModified,
            _      => Self::Unknown
        }
    }
}

#[allow(dead_code)]
pub enum TextDocumentSymbolKinds {
    None = 0,
    Full = 1,
    Incremental = 2
}

#[derive(Debug, PartialEq)]
pub enum SemanticTokenTypes {
    None,
    Variable,
    Function,
    Method,
    Class,
    Enum,
    Comment,
    Keyword,
    Literal,
    Macro,
    Preprocessor,
    Primitive
}

#[derive(Debug)]
pub enum CppSemanticTokenTypes {
	Variable = 0,
	LocalVariable = 1,
	Parameter = 2,
	Function = 3,
	Method = 4,
	StaticMethod = 5,
	Field = 6,
	StaticField = 7,
	Class = 8,
	Enum = 9,
	EnumConstant = 10,
	Typedef = 11,
	DependentType = 12,
	DependentName = 13,
	Namespace = 14,
	TemplateParameter = 15,
	Concept = 16,
	Primitive = 17,
	Macro = 18,
    InactiveCode = 19,
    Unknown = 20
}

impl CppSemanticTokenTypes {
    pub fn from_u32(uint: u32) -> Self {
        match uint {
            0  => Self::Variable,
            1  => Self::LocalVariable,
            2  => Self::Parameter,
            3  => Self::Function,
            4  => Self::Method,
            5  => Self::StaticMethod,
            6  => Self::Field,
            7  => Self::StaticField,
            8  => Self::Class,
            9  => Self::Enum,
            10 => Self::EnumConstant,
            11 => Self::Typedef,
            12 => Self::DependentType,
            13 => Self::DependentName,
            14 => Self::Namespace,
            15 => Self::TemplateParameter,
            16 => Self::Concept,
            17 => Self::Primitive,
            18 => Self::Macro,
            19 => Self::InactiveCode,
            _  => Self::Unknown
        }
    }

    pub fn to_semantic_token_type(cpp_token_type: &Self) -> SemanticTokenTypes {
        match cpp_token_type {
            Self::Variable | Self::LocalVariable
                                    => SemanticTokenTypes::Variable, 
            Self::Function          => SemanticTokenTypes::Function,
            Self::Method | Self::StaticMethod
                                    => SemanticTokenTypes::Method,
            Self::Class             => SemanticTokenTypes::Class,
            Self::Enum              => SemanticTokenTypes::Enum,
            Self::Primitive         => SemanticTokenTypes::Primitive,
            Self::Macro             => SemanticTokenTypes::Macro,
            Self::InactiveCode | Self::EnumConstant | Self::Typedef |      
            Self::DependentType | Self::DependentName | Self::Namespace |        
            Self::TemplateParameter | Self::Concept | Self::Field |            
            Self::StaticField | Self::Parameter | Self::Unknown 
                                    => SemanticTokenTypes::None        
        }
    }
}

#[derive(Debug)]
pub enum RustSemanticTokenTypes {
	Comment = 0,
	Keyword = 1,
	String = 2,
	Number = 3,
	Regexp = 4,
	Operator = 5,
	Namespace = 6,
	Type = 7,
	Struct = 8,
	Class = 9,
	Interface = 10,
	Enum = 11,
	TypeParameter = 12,
	Function = 13,
	Member = 14,
	Property = 15,
	Macro = 16,
	Variable = 17,
	Parameter = 18,
    Label = 19,
    Attribute = 20,
    BuiltinType = 21,
    EnumMember = 22,
    Lifetime = 23,
    TypeAlias = 24,
    Union = 25,
    UnresolvedReference = 26,
    Unknown = 27
}

impl RustSemanticTokenTypes {
    pub fn from_u32(uint: u32) -> Self {
        match uint {
            0  => Self::Comment,
            1  => Self::Keyword,
            2  => Self::String,
            3  => Self::Number,
            4  => Self::Regexp,
            5  => Self::Operator,
            6  => Self::Namespace,
            7  => Self::Type,
            8  => Self::Struct,
            9  => Self::Class,
            10 => Self::Interface,
            11 => Self::Enum,
            12 => Self::TypeParameter,
            13 => Self::Function,
            14 => Self::Member,
            15 => Self::Property,
            16 => Self::Macro,
            17 => Self::Variable,
            18 => Self::Parameter,
            19 => Self::Label,
            20 => Self::Attribute,
            21 => Self::BuiltinType,
            22 => Self::EnumMember,
            23 => Self::Lifetime,
            24 => Self::TypeAlias,
            25 => Self::Union,
            26 => Self::UnresolvedReference,
            _  => Self::Unknown
        }
    }

    pub fn to_semantic_token_type(rust_token_type: &Self) -> SemanticTokenTypes {
        match rust_token_type {
            Self::Keyword             => SemanticTokenTypes::Keyword,
            Self::String | Self::Number | Self::Lifetime            
                                      => SemanticTokenTypes::Literal,
            Self::Struct | Self::Class | Self::TypeAlias
                                      => SemanticTokenTypes::Class,
            Self::Enum                => SemanticTokenTypes::Enum,
            Self::Function            => SemanticTokenTypes::Function,
            Self::Macro               => SemanticTokenTypes::Macro,
            Self::Variable            => SemanticTokenTypes::Variable,
            Self::BuiltinType         => SemanticTokenTypes::Primitive,
            Self::Comment | Self::Interface | Self::TypeParameter | Self::Member |  
            Self::Property | Self::Parameter | Self::Label |
            Self::Attribute | Self::EnumMember | Self::Regexp |             
            Self::Operator | Self::Namespace | Self::Type |               
            Self::Union | Self::UnresolvedReference | Self::Unknown 
                                      => SemanticTokenTypes::None
        }
    }
}

#[derive(Debug)]
pub enum RustSemanticTokenModifiers {
	Documentation = 0,
	Declaration = 1,
	Definition = 2,
	Static = 3,
	Abstract = 4,
	Deprecated = 5,
    Readonly = 6,
    Constant = 7,
    Mutable = 8,
    Unsafe = 9,
    ControlFlow = 10,
    Unknown = 11
}

impl RustSemanticTokenModifiers {
    pub fn from_u32(uint: u32) -> Self {
        match uint {
            0  =>     Self::Documentation,
            1  =>     Self::Declaration,
            2  =>     Self::Definition,
            3  =>     Self::Static,
            4  =>     Self::Abstract,
            5  =>     Self::Deprecated,
            6  =>     Self::Readonly,
            7  =>     Self::Constant,
            8  =>     Self::Mutable,
            9  =>     Self::Unsafe,
            10 =>     Self::ControlFlow,
            _ =>      Self::Unknown
        }
    }
}

/**************************************
*********** NOTIFICATIONS *************
***************************************/
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentItem {
    pub uri: String,
    pub language_id: String,
    pub version: i64,
    pub text: String
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidOpenTextDocumentParams {
    pub text_document: TextDocumentItem
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionedTextDocumentIdentifier {
    pub uri: String,
    pub version: i64
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub line: i64,
    pub character: i64
}

impl Position {
    pub const fn new(line: i64, character: i64) -> Self {
        Self {
            line,
            character
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start: Position,
    pub end: Position
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentContentChangeEvent {
    pub range: Option<Range>,
    pub text: String
}

impl TextDocumentContentChangeEvent {
    pub fn new_delete_event(start_line: usize, start_char: usize, end_line: usize, end_char: usize) -> Self {
        Self {
            text: "".to_owned(),
            range: Some(Range {
                start: Position::new(start_line as i64, start_char as i64),
                end: Position::new(end_line as i64, end_char as i64),
            })
        }
    }
    pub fn new_insert_event(text: String, start_line: usize, start_char: usize, end_line: usize, end_char: usize) -> Self {
        Self {
            text,
            range: Some(Range {
                start: Position::new(start_line as i64, start_char as i64),
                end: Position::new(end_line as i64, end_char as i64),
            })
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeTextDocumentParams {
    pub text_document: VersionedTextDocumentIdentifier,
    pub content_changes: Vec<TextDocumentContentChangeEvent>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidOpenNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: DidOpenTextDocumentParams
}

impl DidOpenNotification {
    pub fn new(uri: String, language_id: String, text: String) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            method: "textDocument/didOpen".to_owned(),
            params: DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id,
                    version: 0,
                    text
                }
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: DidChangeTextDocumentParams
}

impl DidChangeNotification {
    pub fn new(text_document: VersionedTextDocumentIdentifier, content_changes: Vec<TextDocumentContentChangeEvent>) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            method: "textDocument/didChange".to_owned(),
            params: DidChangeTextDocumentParams {
                text_document,
                content_changes
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>
}

impl InitializeNotification {
    pub fn new() -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            method: "initialized".to_owned(),
            params: None
        }
    }
}

/**************************************
************** REQUESTS ***************
***************************************/
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericRequest {
    pub jsonrpc: String,
    pub id: i64,
    pub method: String,
    pub params: Option<Value>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    pub jsonrpc: String,
    pub id: i64,
    pub method: String,
    pub params: InitializeParams
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentIdentifier {
    pub uri: String
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensParams {
    pub text_document: TextDocumentIdentifier
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensRequest {
    pub jsonrpc: String,
    pub id: i64,
    pub method: String,
    pub params: SemanticTokensParams
}

impl SemanticTokensRequest {
    pub fn new(id: i64, uri: String) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            method: "textDocument/semanticTokens".to_owned(),
            params: SemanticTokensParams {
                text_document: TextDocumentIdentifier {
                    uri
                }
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClangdInitializationOptions {
    pub clangd_file_status: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub process_id: i64,
    pub client_info: ClientInfo,
    pub root_path: Option<String>,
    pub root_uri: Option<DocumentUri>,

    pub initialization_options: Option<Value>,

    pub capabilities: ClientCapabilities,
    pub trace: Option<String>,

    // No current support for workspace folders
    pub workspace_folders: Option<Value>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub version: Option<String>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    pub workspace: Option<Value>,
    pub text_document: Option<TextDocumentClientCapabilities>,
    pub window: Option<Value>,
    pub experimental: Option<Value>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentClientCapabilities {
    pub synchronization:     Option<TextDocumentSyncClientCapabilities>,
    pub completion:          Option<CompletionClientCapabilities>,
    pub hover:               Option<HoverClientCapabilities>,
    pub signature_help:      Option<SignatureHelpClientCapabilities>,
    pub declaration:         Option<DeclarationClientCapabilities>,
    pub definition:          Option<DefinitionClientCapabilities>,
    pub type_definition:     Option<TypeDefinitionClientCapabilities>,
    pub implementation:      Option<ImplementationClientCapabilities>,
    pub references:          Option<ReferenceClientCapabilities>,
    pub document_highlight:  Option<DocumentHighlightClientCapabilities>,
    pub document_symbol:     Option<DocumentSymbolClientCapabilities>,
    pub code_action:         Option<CodeActionClientCapabilities>,
    pub code_lens:           Option<CodeLensClientCapabilities>,
    pub document_link:       Option<DocumentLinkClientCapabilities>,
    pub color_provider:      Option<DocumentColorClientCapabilities>,
    pub formatting:          Option<DocumentFormattingClientCapabilities>,
    pub range_formatting:    Option<DocumentRangeFormattingClientCapabilities>,
    pub on_type_formatting:  Option<DocumentOnTypeFormattingClientCapabilities>,
    pub rename:              Option<RenameClientCapabilities>,
    pub publish_diagnostics: Option<PublishDiagnosticsClientCapabilities>,
    pub folding_range:       Option<FoldingRangeClientCapabilities>,
    pub selection_range:     Option<SelectionRangeClientCapabilities>,
    pub semantic_tokens:     Option<SemanticTokensClientCapabilities>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub token_types: Option<Vec<SemanticTokenType>>,
    pub token_modifiers: Option<Vec<SemanticTokenModifier>>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentSyncClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub will_save: Option<bool>,
    pub will_save_wait_until: Option<bool>,
    pub did_save: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSupport {
    pub value_set: Option<CompletionItemTag>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub snippet_support: Option<bool>,
    pub commit_characters_cupport: Option<bool>,
    pub documentation_format: Option<Vec<MarkupKind>>,
    pub deprecated_support: Option<bool>,
    pub preselect_support: Option<bool>,
    pub tag_support: Option<TagSupport>,
    pub context_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub completion_item: Option<CompletionItem>,
    pub completion_item_kind: Option<CompletionItemKind>,
    pub context_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HoverClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub content_format: Option<Vec<MarkupKind>>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterInformation {
    pub label_offset_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureInformation {
    pub documentation_format: Option<Vec<MarkupKind>>,
    pub parameter_information: Option<ParameterInformation>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub signature_information: Option<SignatureInformation>,
    pub context_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclarationClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub link_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub link_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDefinitionClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub link_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub link_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceClientCapabilities {
    pub dynamic_registration: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentHighlightClientCapabilities {
    pub dynamic_registration: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolKindValues {
    pub value_set: Option<Vec<SymbolKind>>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbolClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub symbol_kind: Option<SymbolKindValues>,
    pub hierarchical_document_symbol_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionKindValues {
    pub value_set: Option<CodeActionKind>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionLiteralSupport {
    pub code_action_kind: Option<CodeActionKindValues>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub code_action_literal_support: Option<CodeActionLiteralSupport>,
    pub is_preferred_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensClientCapabilities {
    pub dynamic_registration: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentLinkClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub tooltip_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentColorClientCapabilities {
    pub dynamic_registration: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFormattingClientCapabilities {
    pub dynamic_registration: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRangeFormattingClientCapabilities {
    pub dynamic_registration: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentOnTypeFormattingClientCapabilities {
    pub dynamic_registration: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub prepare_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishDiagnosticsClientCapabilities {
    pub related_information: Option<bool>,
    pub tag_support: Option<TagSupport>,
    pub version_support: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingRangeClientCapabilities {
    pub dynamic_registration: Option<bool>,
    pub range_limit: Option<i64>,
    pub line_folding_only: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRangeClientCapabilities {
    pub dynamic_registration: Option<bool>
}

/**************************************
************** RESPONSES **************
***************************************/
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseError {
    pub code: ErrorCode,
    pub message: String,
    pub data: Option<Value>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Option<Value>,
    pub error: Option<ResponseError>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokenResult {
    pub result_id: Option<String>,
    pub data: Vec<u32>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokenResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Option<SemanticTokenResult>,
    pub error: Option<ResponseError>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Option<InitializeResult>,
    pub error: Option<ResponseError>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
    pub server_info: Option<ServerInfo>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: Option<String>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum TextDocumentSync {
    TextDocumentSyncOptions(TextDocumentSyncOptions),
    Number(i64)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum HoverProvider {
    Boolean(bool),
    HoverOptions(HoverOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum DeclarationProvider {
    Boolean(bool),
    DeclarationOptions(DeclarationOptions),
    DeclarationRegistrationOptions(DeclarationRegistrationOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum DefinitionProvider {
    Boolean(bool),
    DefinitionOptions(DefinitionOptions),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum TypeDefinitionProvider {
    Boolean(bool),
    TypeDefinitionOptions(TypeDefinitionOptions),
    TypeDefinitionRegistrationOptions(TypeDefinitionRegistrationOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum ImplementationProvider {
    Boolean(bool),
    ImplementationOptions(ImplementationOptions),
    ImplementationRegistrationOptions(ImplementationRegistrationOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum ReferencesProvider {
    Boolean(bool),
    ReferenceOptions(ReferenceOptions),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum DocumentHighlightProvider {
    Boolean(bool),
    DocumentHighlightOptions(DocumentHighlightOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum DocumentSymbolProvider {
    Boolean(bool),
    DocumentSymbolOptions(DocumentSymbolOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum CodeActionProvider {
    Boolean(bool),
    CodeActionOptions(CodeActionOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum ColorProvider {
    Boolean(bool),
    DocumentColorOptions(DocumentColorOptions),
    DocumentColorRegistrationOptions(DocumentColorRegistrationOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum DocumentFormattingProvider {
    Boolean(bool),
    DocumentFormattingOptions(DocumentFormattingOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum DocumentRangeFormattingProvider {
    Boolean(bool),
    DocumentRangeFormattingOptions(DocumentRangeFormattingOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum RenameProvider {
    Boolean(bool),
    RenameOptions(RenameOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum FoldingRangeProvider {
    Boolean(bool),
    FoldingRangeOptions(FoldingRangeOptions),
    FoldingRangeRegistrationOptions(FoldingRangeRegistrationOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum SelectionRangeProvider {
    Boolean(bool),
    SelectionRangeOptions(SelectionRangeOptions),
    SelectionRangeRegistrationOptions(SelectionRangeRegistrationOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SemanticTokensProvider {
    SemanticTokensOptions(SemanticTokensOptions),
    SemanticTokensRegistrationOptions(SemanticTokensRegistrationOptions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum DocumentSelector {
    Null,
    DocumentSelector(Vec<DocumentFilter>)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum DocumentProvider {
    Boolean(bool),
    DocumentProvider(DocumentProviderEdits)
}

type WorkDoneProgressOptions = Option<bool>;
type StaticRegistrationOptions = Option<String>;
type TextDocumentRegistrationOptions = DocumentSelector;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentProviderEdits {
    pub edits: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFilter {
    pub language: Option<String>,
    pub scheme: Option<String>,
    pub pattern: Option<String>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentSyncOptions {
    pub open_close: Option<bool>,
    pub change: Option<TextDocumentSymbolKind>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionOptions {
    pub work_done_progress: WorkDoneProgressOptions,
    pub trigger_characters: Option<Vec<String>>,
    pub all_commit_characters: Option<Vec<String>>,
    pub resolve_provider: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoverOptions {
    pub work_done_progress: WorkDoneProgressOptions,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpOptions {
    pub work_done_progress: WorkDoneProgressOptions,
    pub trigger_characters: Option<Vec<String>>,
    pub retrigger_characters: Option<Vec<String>>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclarationOptions {
    pub work_done_progress: WorkDoneProgressOptions,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclarationRegistrationOptions {
    pub work_done_progress: Option<bool>,
    pub document_selector: TextDocumentRegistrationOptions,
    pub id: StaticRegistrationOptions
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionOptions {
    pub work_done_progress: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDefinitionOptions {
    pub work_done_progress: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDefinitionRegistrationOptions {
    pub work_done_progress: Option<bool>,
    pub document_selector: TextDocumentRegistrationOptions,
    pub id: StaticRegistrationOptions
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationOptions {
    pub work_done_progress: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationRegistrationOptions {
    pub work_done_progress: Option<bool>,
    pub document_selector: TextDocumentRegistrationOptions,
    pub id: StaticRegistrationOptions
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceOptions {
    pub work_done_progress: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentHighlightOptions {
    pub work_done_progress: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbolOptions {
    pub work_done_progress: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionOptions {
    pub work_done_progress: Option<bool>,
    pub code_action_kinds: Option<Vec<CodeActionKind>>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensOptions {
    pub work_done_progress: Option<bool>,
    pub resolve_provider: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentLinkOptions {
    pub work_done_progress: Option<bool>,
    pub resolve_provider: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentColorOptions {
    pub work_done_progress: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentColorRegistrationOptions {
    pub work_done_progress: Option<bool>,
    pub document_selector: TextDocumentRegistrationOptions,
    pub id: StaticRegistrationOptions
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFormattingOptions {
    pub work_done_progress: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRangeFormattingOptions {
    pub work_done_progress: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentOnTypeFormattingOptions {
    pub first_trigger_character: String,
    pub more_trigger_character: Option<Vec<String>>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameOptions {
    pub work_done_progress: Option<bool>,
    pub prepare_provider: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingRangeOptions {
    pub work_done_progress: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingRangeRegistrationOptions {
    pub work_done_progress: Option<bool>,
    pub document_selector: TextDocumentRegistrationOptions,
    pub id: StaticRegistrationOptions
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteCommandOptions {
    pub work_done_progress: Option<bool>,
    pub commands: Vec<String>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRangeOptions {
    pub work_done_progress: Option<bool>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRangeRegistrationOptions {
    pub work_done_progress: Option<bool>,
    pub document_selector: TextDocumentRegistrationOptions,
    pub id: StaticRegistrationOptions
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFolderServerCapabilities {
    pub supported: Option<bool>,
    pub change_notifications: Option<Value>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub workspace_folders: Option<WorkspaceFolderServerCapabilities>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensLegend {
    pub token_types: Vec<String>,
    pub token_modifiers: Vec<String>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensOptions {
    pub workspace_folders: Option<WorkspaceFolderServerCapabilities>,
    pub legend: SemanticTokensLegend,
    pub range_provider: Option<bool>,
    pub document_provider: Option<DocumentProvider>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensRegistrationOptions {
    pub workspace_folders: Option<WorkspaceFolderServerCapabilities>,
    pub legend: SemanticTokensLegend,
    pub range_provider: Option<bool>,
    pub document_provider: Option<DocumentProvider>,
    pub document_selector: TextDocumentRegistrationOptions,
    pub id: StaticRegistrationOptions
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ServerCapabilities {
     text_document_sync: Option<TextDocumentSync>,
     completion_provider: Option<CompletionOptions>,
     hover_provider: Option<HoverProvider>,
     signature_help_provider: Option<SignatureHelpOptions>,
     declaration_provider: Option<DeclarationProvider>,
     definition_provider: Option<DefinitionProvider>,
     type_definition_provider: Option<TypeDefinitionProvider>,
     implementation_provider: Option<ImplementationProvider>,
     references_provider: Option<ReferencesProvider>,
     document_highlight_provider: Option<DocumentHighlightProvider>,
     document_symbol_provider: Option<DocumentSymbolProvider>,
     code_action_provider: Option<CodeActionProvider>,
     code_lens_provider: Option<CodeLensOptions>,
     document_link_provider: Option<DocumentLinkOptions>,
     color_provider: Option<ColorProvider>,
     document_formatting_provider: Option<DocumentFormattingProvider>,
     document_range_formatting_provider: Option<DocumentRangeFormattingProvider>,
     document_on_type_formatting_provider: Option<DocumentOnTypeFormattingOptions>,
     rename_provider: Option<RenameProvider>,
     folding_range_provider: Option<FoldingRangeProvider>,
     execute_command_provider: Option<ExecuteCommandOptions>,
     selection_range_provider: Option<SelectionRangeProvider>,
     workspace_symbol_provider: Option<bool>,
     workspace: Option<Workspace>,
     experimental: Option<Value>,
     semantic_tokens_provider: Option<SemanticTokensProvider>
}
