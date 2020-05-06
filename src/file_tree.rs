use crate::{
    renderer::{TextRenderer, RenderableTextRegion},
    hr_ok
};

use std::{
    cell::RefCell,
    iter::once,
    os::windows::ffi::OsStrExt,
    rc::Rc,
    ptr::null_mut,
    path::Path
};

use winapi::um::{
    dwrite::{IDWriteTextLayout, DWRITE_LINE_METRICS},
    d2d1::D2D1_RECT_F
};

pub struct FileTree {
    pub root: String,
    pub text: Vec<u16>,

    pub origin: (f32, f32),
    pub extents: (f32, f32),

    pub hovered_line_number: Option<usize>,
    pub hovered_line_rect: Option<D2D1_RECT_F>,
    line_metrics: Vec<DWRITE_LINE_METRICS>,

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
        self.text_layout
    }

    fn resize(&mut self, origin: (f32, f32), extents: (f32, f32)) {
        self.origin = origin;
        self.extents = extents;
    }
}

impl FileTree {
    pub fn new(root: &str, origin: (f32, f32), extents: (f32, f32), renderer: Rc<RefCell<TextRenderer>>) -> Self {
        let mut file_tree = Self {
            root: root.to_owned(),
            text: Vec::new(),
            origin,
            extents,

            hovered_line_number: None,
            hovered_line_rect: None,
            line_metrics: Vec::new(),

            renderer,
            text_layout: null_mut()
        };

        file_tree.update_layout();
        file_tree
    }

    pub fn clear_hover(&mut self) {
        self.hovered_line_number = None;
        self.hovered_line_rect = None;
    }

    pub fn update_hover_item(&mut self, mouse_pos: (f32, f32)) -> bool {
        // At this point we already know that the mouse position
        // is within the bounds of the file tree, therefore from
        // here we simply find the line from the line metrics
        let relative_mouse_pos = self.translate_mouse_pos_to_file_tree_region(mouse_pos);

        let length = self.line_metrics.len();
        let mut offset = 0.0;
        for (i, metrics) in self.line_metrics.iter_mut().enumerate() {
            // Skip final line (empty line)
            if i == length - 1 {
                break;
            }

            // Check whether or not the mouse is within the vertical
            // range of the current line. If so, update the hovered
            // rect and line number
            let line_range = offset..(offset + metrics.height);
            if line_range.contains(&relative_mouse_pos.1) {
                let rect = D2D1_RECT_F {
                    left: self.origin.0,
                    right: self.origin.0 + self.extents.0,
                    top: self.origin.1 + offset,
                    bottom: self.origin.1 + (offset + metrics.height)
                };
                match self.hovered_line_rect {
                    Some(current_rect) => {
                        if  current_rect.left   != rect.left ||
                            current_rect.right  != rect.right ||
                            current_rect.top    != rect.top ||
                            current_rect.bottom != rect.bottom {
                            self.hovered_line_number = Some(i);
                            self.hovered_line_rect = Some(rect);
                            return true;
                        }
                        else {
                            return false;
                        }
                    }
                    None => {
                        self.hovered_line_number = Some(i);
                        self.hovered_line_rect = Some(rect);
                        return true;
                    }
                }
            }

            offset += metrics.height;
        }

        false
    }

    pub fn update_layout(&mut self) {
        unsafe {
            if !self.text_layout.is_null() {
                (*self.text_layout).Release();
            }

            hr_ok!((*self.renderer.borrow().write_factory).CreateTextLayout(
                self.text.as_ptr(),
                self.text.len() as u32,
                self.renderer.borrow().text_format,
                self.extents.0,
                self.extents.1,
                &mut self.text_layout as *mut *mut _
            ));

            let mut line_metrics_count = 0;
            let hr: i32 = (*self.text_layout).GetLineMetrics(
                        null_mut(), 
                        0,
                        &mut line_metrics_count
                    );
            assert!((hr as u32) == 0x8007007A, "HRESULT in this case is expected to error with \"ERROR_INSUFFICIENT_BUFFER\""); 

            self.line_metrics.reserve_exact(line_metrics_count as usize);
            self.line_metrics.set_len(line_metrics_count as usize);
            hr_ok!((*self.text_layout).GetLineMetrics(
                    self.line_metrics.as_mut_ptr(), 
                    self.line_metrics.len() as u32,
                    &mut line_metrics_count
            ));
        }
    }

    pub fn set_workspace_root(&mut self, root: String) {
        self.root = root;

        let root_path = Path::new(self.root.as_str());
        
        if let Ok(entries) = root_path.read_dir() {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        if let Ok(file_type) = entry.file_type() {
                            if file_type.is_dir() {
                                self.text.push(0xD83D);
                                self.text.push(0xDCC1);
                            }
                            else {
                                self.text.push(0xD83D);
                                self.text.push(0xDCDD);
                            }
                        }
                        self.text.append(&mut entry.file_name().encode_wide().chain(once(0x000A)).collect())
                    }
                    Err(_) => {}
                }
            }
        }

        self.update_layout();
    }

    fn translate_mouse_pos_to_file_tree_region(&self, mouse_pos: (f32, f32)) -> (f32, f32) {
        let dx = mouse_pos.0 - self.origin.0;
        let dy = mouse_pos.1 - self.origin.1;
        (dx, dy)
    }
}