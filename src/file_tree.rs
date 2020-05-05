use crate::renderer::{TextRenderer, RenderableTextRegion};
use std::os::windows::ffi::OsStrExt;
use crate::dx_ok;

use std::{
    cell::RefCell,
    iter::once,
    rc::Rc,
    ptr::null_mut,
    path::Path
};

use winapi::um::{
    dwrite::IDWriteTextLayout,
    d2d1::D2D1_RECT_F
};

pub struct FileTree {
    pub root: String,

    pub origin: (f32, f32),
    pub extents: (f32, f32),
    renderer: Rc<RefCell<TextRenderer>>,
    text_layout: *mut IDWriteTextLayout,
}

impl RenderableTextRegion for FileTree {
    fn get_origin(&self) -> (f32, f32) {
        self.origin
    }

    fn get_rect(&self) -> D2D1_RECT_F {
        D2D1_RECT_F {
            left: self.origin.0,
            top: self.origin.1,
            right: self.origin.0 + self.extents.0,
            bottom: self.origin.1 + self.extents.1,
        }
    }

    fn get_layout(&mut self) -> *mut IDWriteTextLayout {
        unsafe {
            if !self.text_layout.is_null() {
                (*self.text_layout).Release();
            }

            let mut file_tree_str = vec![];
            if !self.root.is_empty() {
                let root_path = Path::new(self.root.as_str());
                
                if let Ok(entries) = root_path.read_dir() {
                    for entry in entries {
                        match entry {
                            Ok(entry) => {
                                if let Ok(file_type) = entry.file_type() {
                                    if file_type.is_dir() {
                                        file_tree_str.push(0xD83D);
                                        file_tree_str.push(0xDCC1);
                                    }
                                    else {
                                        file_tree_str.push(0xD83D);
                                        file_tree_str.push(0xDCDD);
                                    }
                                }
                                file_tree_str.append(&mut entry.file_name().encode_wide().chain(once(0x000A)).collect())
                            }
                            Err(_) => {}
                        }
                    }
                }
            }

            dx_ok!((*self.renderer.borrow().write_factory).CreateTextLayout(
                file_tree_str.as_ptr(),
                file_tree_str.len() as u32,
                self.renderer.borrow().text_format,
                self.extents.0,
                self.extents.1,
                &mut self.text_layout as *mut *mut _
            ));
        }

        self.text_layout
    }

    fn resize(&mut self, origin: (f32, f32), extents: (f32, f32)) {
        self.origin = origin;
        self.extents = extents;
    }
}

impl FileTree {
    pub fn new(root: &str, origin: (f32, f32), extents: (f32, f32), renderer: Rc<RefCell<TextRenderer>>) -> Self {
        Self {
            root: root.to_owned(),
            origin,
            extents,
            renderer,
            text_layout: null_mut()
        }
    }
}