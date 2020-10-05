use std::{
    collections::HashMap,
    str,
    rc::Rc, 
    cell::RefCell,
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
        winuser::{VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN, VK_TAB, VK_RETURN, VK_DELETE, VK_BACK, InvalidateRect, SendMessageW}
    }
};

use crate::{
    WM_REGION_CHANGED,
    settings::{SCROLL_LINES_PER_MOUSEMOVE, SCROLL_LINES_PER_ROLL, 
     NUMBER_OF_SPACES_PER_TAB, SCROLL_ZOOM_DELTA, RESIZABLE_BORDER_WIDTH},
    renderer::{TextRenderer, RenderableTextRegion},
    language_support::{CPP_FILE_EXTENSIONS, CPP_LANGUAGE_IDENTIFIER, RUST_FILE_EXTENSIONS, RUST_LANGUAGE_IDENTIFIER},
    buffer::{TextBuffer, SelectionMode, MouseSelectionMode},
    status_bar::StatusBar,
    file_tree::FileTree,
    hr_ok
};

type MousePos = (f32, f32);
type ShiftDown = bool;
type CtrlDown = bool;

#[derive(Debug, PartialEq)]
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
    CharInsert(u16)
}

#[derive(Copy, Clone, Debug)]
pub struct EditorLayout {
    pub layout_origin: (f32, f32),
    pub layout_extents: (f32, f32),
    pub buffer_origin: (f32, f32),
    pub buffer_extents: (f32, f32),
    pub status_bar_origin: (f32, f32),
    pub status_bar_extents: (f32, f32),
    pub file_tree_origin: (f32, f32),
    pub file_tree_extents: (f32, f32),
    pub resizable_border_origin: (f32, f32),
    pub resizable_border_extents: (f32, f32)
}
impl Default for EditorLayout {
    fn default() -> Self {
        Self {
            layout_origin: (0.0, 0.0),
            layout_extents: (0.0, 0.0),
            buffer_origin: (0.0, 0.0),
            buffer_extents: (0.0, 0.0),
            status_bar_origin: (0.0, 0.0),
            status_bar_extents: (0.0, 0.0),
            file_tree_origin: (0.0, 0.0),
            file_tree_extents: (0.0, 0.0),
            resizable_border_origin: (0.0, 0.0),
            resizable_border_extents: (0.0, 0.0)
        }
    }
}
impl EditorLayout {
    pub fn new(width: f32, height: f32, file_tree_width: f32, font_height: f32) -> Self {
        Self {
            layout_origin: (0.0, 0.0),
            layout_extents: (width, height),
            buffer_origin: (file_tree_width, 0.0),
            buffer_extents: (width - file_tree_width, height - font_height),
            status_bar_origin: (0.0, height - font_height),
            status_bar_extents: (width, font_height),
            file_tree_origin: (0.0, 0.0),
            file_tree_extents: (file_tree_width, height - font_height),
            resizable_border_origin: (file_tree_width - (RESIZABLE_BORDER_WIDTH / 2.0), 0.0),
            resizable_border_extents: (RESIZABLE_BORDER_WIDTH, height - font_height)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RegionType {
    FileTree = 0,
    TextBuffer = 1,
    ResizableBorder = 2,
    StatusBar = 3,
    Unknown = 4
}

impl RegionType {
    pub fn from_usize(uint: usize) -> Self {
        match uint {
            0 => Self::FileTree,
            1 => Self::TextBuffer,
            2 => Self::ResizableBorder,
            3 => Self::StatusBar,
            _ => Self::Unknown
        }
    }

    pub fn to_usize(region_type: Self) -> usize {
        match region_type {
            Self::FileTree => 0,
            Self::TextBuffer => 1,
            Self::ResizableBorder => 2,
            Self::StatusBar => 3,
            Self::Unknown => 4
        }
    }
}

pub struct Editor {
    hwnd: HWND,
    renderer: Rc<RefCell<TextRenderer>>,
    layout: EditorLayout,

    status_bar: StatusBar,
    file_tree: FileTree,

    buffers: HashMap<String, TextBuffer>,
    current_buffer: String,

    region_type: RegionType,

    resizing_file_tree: bool,
    resizing_file_tree_offset: f32,

    mouse_pos: (f32, f32),
    mouse_pos_captured: bool,
    force_visible_caret_timer: u32,
    caret_is_visible: bool,

    pub timer: std::time::Instant
}

impl Editor {
    pub fn new(hwnd: HWND) -> Self {
        let renderer = Rc::new(RefCell::new(TextRenderer::new(hwnd, "Fira Code Retina", 20.0)));

        let layout = EditorLayout::new(
            renderer.borrow().pixel_size.width as f32,
            renderer.borrow().pixel_size.height as f32,
            renderer.borrow().pixel_size.width as f32 / 7.5,
            renderer.borrow().font_height);

        Self {
            hwnd,
            renderer: renderer.clone(),
            layout,

            status_bar: StatusBar::new(layout.status_bar_origin, layout.status_bar_extents, renderer.clone()),
            file_tree: FileTree::new("", layout.file_tree_origin, layout.file_tree_extents, renderer.clone()),

            buffers: HashMap::new(),
            current_buffer: "".to_owned(),

            region_type: RegionType::Unknown,

            resizing_file_tree: false,
            resizing_file_tree_offset: 0.0,

            mouse_pos: (0.0, 0.0),
            mouse_pos_captured: false,
            force_visible_caret_timer: 0,
            caret_is_visible: true,

            timer: std::time::Instant::now()
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
                self.layout.buffer_origin, 
                self.layout.buffer_extents, 
                self.renderer.clone()
            )
        );
        self.current_buffer = file_prefix.clone() + path;
    }

    pub fn draw(&mut self) {
        let buffer = self.buffers.get_mut(&self.current_buffer);
        self.renderer.borrow().draw(buffer, &mut self.status_bar, &mut self.file_tree, self.caret_is_visible);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.borrow_mut().resize(width, height);

        self.layout = EditorLayout::new(
            self.renderer.borrow().pixel_size.width as f32,
            self.renderer.borrow().pixel_size.height as f32,
            self.layout.file_tree_extents.0,
            self.renderer.borrow().font_height);

        self.status_bar.resize(self.layout.status_bar_origin, self.layout.status_bar_extents);
        self.file_tree.resize(self.layout.file_tree_origin, self.layout.file_tree_extents);

        for buffer in self.buffers.values_mut() {
            buffer.on_refresh_metrics(
                self.layout.buffer_origin,
                self.layout.buffer_extents
            );
        }
    }

    // Resizes the file tree left or right according
    // to the delta parameter
    pub fn resize_file_tree(&mut self, new_width: f32) {
        self.layout = EditorLayout::new(
            self.renderer.borrow().pixel_size.width as f32,
            self.renderer.borrow().pixel_size.height as f32,
            new_width,
            self.renderer.borrow().font_height);

        self.status_bar.resize(self.layout.status_bar_origin, self.layout.status_bar_extents);
        self.file_tree.resize(self.layout.file_tree_origin, self.layout.file_tree_extents);

        for buffer in self.buffers.values_mut() {
            buffer.on_refresh_metrics(
                self.layout.buffer_origin,
                self.layout.buffer_extents
            );
        }
    }

    pub fn capture_mouse(&mut self) {
        self.mouse_pos_captured = true;
    }

    pub fn release_mouse(&mut self) {
        self.mouse_pos_captured = false;
    }

    pub fn selection_active(&self) -> bool {
        if let Some(buffer) = self.buffers.get(&self.current_buffer) {
            return buffer.currently_selecting;
        }
        false
    }

    pub fn mouse_left_window(&mut self) {
        self.region_type = RegionType::Unknown;
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
            self.file_tree.set_workspace_root(String::from_utf16_lossy(slice));

            (*shell_item).Release();
            (*file_dialog).Release();
        }
    }

    fn force_caret_visible(caret_is_visible: &mut bool, caret_timer: &mut u32) {
        if *caret_is_visible {
            *caret_timer = 1;
        }
        else {
            *caret_is_visible = true;
            *caret_timer = 2;
        }
    }

    fn change_font_size(zoom_delta: f32, layout: &mut EditorLayout, renderer: &mut TextRenderer) {
        renderer.update_text_format(zoom_delta);

        *layout = EditorLayout::new(
            renderer.pixel_size.width as f32,
            renderer.pixel_size.height as f32,
            layout.file_tree_extents.0,
            renderer.font_height);
    }

    fn inside_region(pos: (f32, f32), origin: (f32, f32), extents: (f32, f32)) -> bool {
        let horizontal_range = origin.0..(origin.0 + extents.0);
        let vertical_range = origin.1..(origin.1 + extents.1);
        horizontal_range.contains(&pos.0) && vertical_range.contains(&pos.1)
    }

    fn execute_buffer_command(&mut self, cmd: &EditorCommand) {
        if let Some(buffer) = self.buffers.get_mut(&self.current_buffer) {
            match *cmd {
                EditorCommand::CaretVisible | EditorCommand::CaretInvisible if self.force_visible_caret_timer > 0 => {
                    self.force_visible_caret_timer = self.force_visible_caret_timer.saturating_sub(1);
                    self.caret_is_visible = true;
                }
                EditorCommand::CaretVisible => self.caret_is_visible = true,
                EditorCommand::CaretInvisible => self.caret_is_visible = false,
                EditorCommand::ScrollUp(ctrl_down) => {
                    match ctrl_down {
                        true => {
                            Self::change_font_size(SCROLL_ZOOM_DELTA, &mut self.layout, &mut *self.renderer.borrow_mut());
                            buffer.on_refresh_metrics(
                                self.layout.buffer_origin,
                                self.layout.buffer_extents
                            );
                        },
                        false => buffer.scroll_up(SCROLL_LINES_PER_ROLL)
                    }
                }
                EditorCommand::ScrollDown(ctrl_down) => {
                    match ctrl_down {
                        true => {
                            Self::change_font_size(-SCROLL_ZOOM_DELTA, &mut self.layout, &mut *self.renderer.borrow_mut());
                            buffer.on_refresh_metrics(
                                self.layout.buffer_origin,
                                self.layout.buffer_extents
                            );
                        }
                        false => buffer.scroll_down(SCROLL_LINES_PER_ROLL)
                    }
                }
                EditorCommand::LeftClick(mouse_pos, shift_down) => {
                    buffer.left_click(mouse_pos, shift_down);
                    Self::force_caret_visible(&mut self.caret_is_visible, &mut self.force_visible_caret_timer);
                }
                EditorCommand::LeftDoubleClick(mouse_pos) => {
                    buffer.left_double_click(mouse_pos);
                    Self::force_caret_visible(&mut self.caret_is_visible, &mut self.force_visible_caret_timer);
                }
                EditorCommand::LeftRelease => buffer.left_release(),
                EditorCommand::MouseMove(mouse_pos) => {
                    if mouse_pos.1 > (self.layout.layout_origin.1 + self.layout.layout_extents.1) {
                        buffer.scroll_down(SCROLL_LINES_PER_MOUSEMOVE);
                    }
                    else if mouse_pos.1 < self.layout.layout_origin.1 {
                        buffer.scroll_up(SCROLL_LINES_PER_MOUSEMOVE);
                    }
                    if mouse_pos.0 > (self.layout.layout_origin.0 + self.layout.layout_extents.0) {
                        buffer.scroll_right(SCROLL_LINES_PER_MOUSEMOVE);
                    }
                    else if mouse_pos.0 < self.layout.layout_origin.0 {
                        buffer.scroll_left(SCROLL_LINES_PER_MOUSEMOVE);
                    }
                    buffer.set_mouse_selection(MouseSelectionMode::Move, mouse_pos);
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
                    Self::force_caret_visible(&mut self.caret_is_visible, &mut self.force_visible_caret_timer);
                }
                EditorCommand::CharInsert(character) => {
                    buffer.insert_char(character);
                    Self::force_caret_visible(&mut self.caret_is_visible, &mut self.force_visible_caret_timer);
                }
            }

            buffer.on_editor_action();
        }
    }

    fn execute_resizable_border_command(&mut self, cmd: &EditorCommand) {
        match *cmd {
            EditorCommand::LeftClick(mouse_pos, _) => {
                self.resizing_file_tree = true;
                self.resizing_file_tree_offset = (self.layout.file_tree_origin.0 + self.layout.file_tree_extents.0) - mouse_pos.0;
            }
            EditorCommand::LeftRelease => self.resizing_file_tree = false,
            EditorCommand::MouseMove(mouse_pos) if self.resizing_file_tree => {
                self.resize_file_tree(mouse_pos.0 + self.resizing_file_tree_offset);
                self.draw();
            }
            _ => {}
        }
    }

    fn execute_status_bar_command(&mut self, _cmd: &EditorCommand) {
        unimplemented!();
    }

    fn execute_file_tree_command(&mut self, cmd: &EditorCommand) {
        match *cmd {
            EditorCommand::LeftClick(_, _) => {
                if let Some(path) = self.file_tree.get_hovered_item() {
                    let path_string = "file:///".to_owned() + path.to_str().unwrap();
                    
                    if self.buffers.contains_key(&path_string) {
                        self.current_buffer = path_string;
                    }
                    else {
                        self.open_file(path.to_str().unwrap());
                    }
                }
            }
            EditorCommand::MouseMove(mouse_pos) => {
                if self.file_tree.update_hover_item(mouse_pos) {
                    unsafe { InvalidateRect(self.hwnd, null_mut(), false as i32); }
                }
            }
            _ => {}
        }
    }

    fn get_region(&mut self) -> RegionType {
        // The resizable border has to be the first check since it slightly
        // overlaps the file tree and text buffer regions
        if Self::inside_region(self.mouse_pos, self.layout.resizable_border_origin, self.layout.resizable_border_extents) {
            RegionType::ResizableBorder
        }
        else if Self::inside_region(self.mouse_pos, self.layout.buffer_origin, self.layout.buffer_extents) {
            if self.current_buffer.is_empty() {
                RegionType::FileTree
            }
            else {
                RegionType::TextBuffer
            }
        }
        else if Self::inside_region(self.mouse_pos, self.layout.status_bar_origin, self.layout.status_bar_extents) {
            RegionType::StatusBar
        }
        else if Self::inside_region(self.mouse_pos, self.layout.file_tree_origin, self.layout.file_tree_extents) {
            RegionType::FileTree
        }
        else {
            RegionType::Unknown
        }
    }

    fn update_region_type(&mut self) {
        match self.get_region() {
            region if region != self.region_type => {
                if self.region_type == RegionType::FileTree {
                    self.file_tree.clear_hover();
                }
                unsafe { SendMessageW(self.hwnd, WM_REGION_CHANGED, RegionType::to_usize(region), 0); }
                self.region_type = region;
            }
            _ => {}
        }
    }

    pub fn execute_command(&mut self, cmd: &EditorCommand) {
        match *cmd {
            EditorCommand::MouseMove(mouse_pos) if !self.mouse_pos_captured => {
                self.mouse_pos = mouse_pos;
                self.update_region_type();
            }
            EditorCommand::KeyPressed(key, _, ctrl_down) => { 
                match (key, ctrl_down) {
                    (0x4F, true) => self.open_workspace(),
                    _ => {}
                }
            }
            _ => {}
        }

        if Self::inside_region(self.mouse_pos, self.layout.resizable_border_origin, self.layout.resizable_border_extents) || self.resizing_file_tree {
            self.execute_resizable_border_command(cmd);
            return;
        }
        if Self::inside_region(self.mouse_pos, self.layout.buffer_origin, self.layout.buffer_extents) {
            self.execute_buffer_command(cmd);
            return;
        }
        else if Self::inside_region(self.mouse_pos, self.layout.status_bar_origin, self.layout.status_bar_extents) {
            self.execute_status_bar_command(cmd);
            return;
        }
        else if Self::inside_region(self.mouse_pos, self.layout.file_tree_origin, self.layout.file_tree_extents) {
            self.execute_file_tree_command(cmd);
            return;
        }
    }
}
