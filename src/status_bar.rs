use crate::renderer::{TextRenderer, RenderableTextRegion};
use crate::text_utils;
use crate::dx_ok;

use std::{
    cell::RefCell,
    rc::Rc, 
    ptr::null_mut
};

use winapi::um::{
    dwrite::IDWriteTextLayout,
    d2d1::D2D1_RECT_F
};

pub struct StatusBar {
    pub origin: (f32, f32),
    pub extents: (f32, f32),
    renderer: Rc<RefCell<TextRenderer>>,
    text_layout: *mut IDWriteTextLayout,
}

impl RenderableTextRegion for StatusBar {
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

            let status_string = text_utils::to_os_str("Text");

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

    fn resize(&mut self, origin: (f32, f32), extents: (f32, f32)) {
        self.origin = origin;
        self.extents = extents;
    }
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


}