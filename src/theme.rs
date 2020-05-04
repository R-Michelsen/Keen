use crate::dx_ok;

use std::ptr::null_mut;
use winapi::{
    shared::d3d9types::D3DCOLORVALUE,
    um::d2d1::{ID2D1SolidColorBrush, ID2D1HwndRenderTarget, D2D1_BRUSH_PROPERTIES, D2D1_MATRIX_3X2_F}
};

const IDENTITY_MATRIX: D2D1_MATRIX_3X2_F = D2D1_MATRIX_3X2_F { matrix: [[1.0, 0.0], [0.0, 1.0], [0.0, 0.0]] };

const DEFAULT_BACKGROUND_COLOR: D3DCOLORVALUE = create_color(0x282828FF);
const DEFAULT_STATUS_BAR_COLOR: D3DCOLORVALUE = create_color(0x141414FF);
const DEFAULT_BRACKET_COLOR: D3DCOLORVALUE = create_color(0xFFFFFFFF);
const DEFAULT_TEXT_COLOR: D3DCOLORVALUE = create_color(0xFBF1C7FF);
const DEFAULT_LINE_NUMBER_COLOR: D3DCOLORVALUE = create_color(0xD5C4A1FF);
const DEFAULT_CARET_COLOR: D3DCOLORVALUE = create_color(0xFE8019FF);
const DEFAULT_SELECTION_COLOR: D3DCOLORVALUE = create_color(0x464646FF);
const DEFAULT_VARIABLE_COLOR: D3DCOLORVALUE = create_color(0xADD8E6FF);
const DEFAULT_FUNCTION_COLOR: D3DCOLORVALUE = create_color(0xFBD06DFF);
const DEFAULT_METHOD_COLOR: D3DCOLORVALUE = create_color(0xD3869BFF);
const DEFAULT_CLASS_COLOR: D3DCOLORVALUE = create_color(0xA0DB8EFF);
const DEFAULT_ENUM_COLOR: D3DCOLORVALUE = create_color(0xA0DB8EFF);
const DEFAULT_COMMENT_COLOR: D3DCOLORVALUE = create_color(0xB8BB26FF);
const DEFAULT_KEYWORD_COLOR: D3DCOLORVALUE = create_color(0xFB4934FF);
const DEFAULT_LITERAL_COLOR: D3DCOLORVALUE = create_color(0xFE8019FF);
const DEFAULT_MACRO_PREPROCESSOR_COLOR: D3DCOLORVALUE = create_color(0xEE7AE9FF);
const DEFAULT_PRIMITIVE_COLOR: D3DCOLORVALUE = create_color(0xCDF916FF);

const fn create_color(color: u32) -> D3DCOLORVALUE {
    D3DCOLORVALUE {
        r: ((color >> 24) & 0xFF) as f32 / 255.0,
        g: ((color >> 16) & 0xFF) as f32 / 255.0,
        b: ((color >>  8) & 0xFF) as f32 / 255.0,
        a: (color         & 0xFF) as f32 / 255.0
    }
}

pub struct Theme {
    pub background_color: D3DCOLORVALUE,
    pub status_bar_brush: *mut ID2D1SolidColorBrush,
    pub bracket_brush: *mut ID2D1SolidColorBrush,
    pub bracket_rect_width: f32,
    pub text_brush: *mut ID2D1SolidColorBrush,
    pub line_number_brush: *mut ID2D1SolidColorBrush,
    pub caret_brush: *mut ID2D1SolidColorBrush,
    pub selection_brush: *mut ID2D1SolidColorBrush,
    pub variable_brush: *mut ID2D1SolidColorBrush,
    pub function_brush: *mut ID2D1SolidColorBrush,
    pub method_brush: *mut ID2D1SolidColorBrush,
    pub class_brush: *mut ID2D1SolidColorBrush,
    pub enum_brush: *mut ID2D1SolidColorBrush,
    pub comment_brush: *mut ID2D1SolidColorBrush,
    pub keyword_brush: *mut ID2D1SolidColorBrush,
    pub literal_brush: *mut ID2D1SolidColorBrush,
    pub macro_preprocessor_brush: *mut ID2D1SolidColorBrush,
    pub primitive_brush: *mut ID2D1SolidColorBrush

}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background_color: D3DCOLORVALUE { r: 0.0, g: 0.0, b: 0.0, a: 1.0},
            status_bar_brush: null_mut(),
            bracket_brush: null_mut(),
            bracket_rect_width: 0.0,
            text_brush: null_mut(),
            line_number_brush: null_mut(),
            caret_brush: null_mut(),
            selection_brush: null_mut(),
            variable_brush: null_mut(),
            function_brush: null_mut(),
            method_brush: null_mut(),
            class_brush: null_mut(),
            enum_brush: null_mut(),
            comment_brush: null_mut(),
            keyword_brush: null_mut(),
            literal_brush: null_mut(),
            macro_preprocessor_brush: null_mut(),
            primitive_brush: null_mut(),
        }
    }
}

impl Theme {
    pub fn new_default(target: *mut ID2D1HwndRenderTarget) -> Self {
        let mut theme = Self {
            background_color: DEFAULT_BACKGROUND_COLOR,
            status_bar_brush: null_mut(),
            bracket_brush: null_mut(),
            bracket_rect_width: 1.0,
            text_brush: null_mut(),
            line_number_brush: null_mut(),
            caret_brush: null_mut(),
            selection_brush: null_mut(),
            variable_brush: null_mut(),
            function_brush: null_mut(),
            method_brush: null_mut(),
            class_brush: null_mut(),
            enum_brush: null_mut(),
            comment_brush: null_mut(),
            keyword_brush: null_mut(),
            literal_brush: null_mut(),
            macro_preprocessor_brush: null_mut(),
            primitive_brush: null_mut()
        };

        let brush_properties = D2D1_BRUSH_PROPERTIES {
            opacity: 1.0,
            transform: IDENTITY_MATRIX
        };

        unsafe {
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_TEXT_COLOR, &brush_properties, &mut theme.text_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_STATUS_BAR_COLOR, &brush_properties, &mut theme.status_bar_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_BRACKET_COLOR, &brush_properties, &mut theme.bracket_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_LINE_NUMBER_COLOR, &brush_properties, &mut theme.line_number_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_CARET_COLOR, &brush_properties, &mut theme.caret_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_SELECTION_COLOR, &brush_properties, &mut theme.selection_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_VARIABLE_COLOR, &brush_properties, &mut theme.variable_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_FUNCTION_COLOR, &brush_properties, &mut theme.function_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_METHOD_COLOR, &brush_properties, &mut theme.method_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_CLASS_COLOR, &brush_properties, &mut theme.class_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_ENUM_COLOR, &brush_properties, &mut theme.enum_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_COMMENT_COLOR, &brush_properties, &mut theme.comment_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_KEYWORD_COLOR, &brush_properties, &mut theme.keyword_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_LITERAL_COLOR, &brush_properties, &mut theme.literal_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_MACRO_PREPROCESSOR_COLOR, &brush_properties, &mut theme.macro_preprocessor_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&DEFAULT_PRIMITIVE_COLOR, &brush_properties, &mut theme.primitive_brush as *mut *mut _));
        }

        theme
    }
}