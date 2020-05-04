use crate::renderer::TextRenderer;
use crate::text_utils;
use crate::dx_ok;

use std::{
    cell::RefCell,
    rc::Rc, 
    ptr::null_mut
};

use winapi::um::dwrite::IDWriteTextLayout;

pub struct StatusBar {
    pub origin: (f32, f32),
    pub extents: (f32, f32),
    renderer: Rc<RefCell<TextRenderer>>,
    text_layout: *mut IDWriteTextLayout,
}

impl StatusBar {
    pub fn new(origin: (f32, f32), extents: (f32, f32), renderer: Rc<RefCell<TextRenderer>>) -> Self {
        Self {
            origin, 
            extents,
            renderer,
            text_layout: null_mut()
        }
    }

    pub fn resize(&mut self, origin: (f32, f32), extents: (f32, f32)) {
        self.origin = origin;
        self.extents = extents;
    }

    pub fn get_layout(&mut self, text: &str) -> *mut IDWriteTextLayout {
        unsafe {
            if !self.text_layout.is_null() {
                (*self.text_layout).Release();
            }

            let status_string = text_utils::to_os_str(text);

            dx_ok!((*self.renderer.borrow().write_factory).CreateTextLayout(
                status_string.as_ptr(),
                status_string.len() as u32,
                self.renderer.borrow().text_format,
                self.extents.0,
                self.extents.1,
                &mut self.text_layout as *mut *mut _
            ));
        }

        self.text_layout
    }
}