use std::ptr::null_mut;
use winapi::{
    um::d2d1::{ ID2D1SolidColorBrush, ID2D1HwndRenderTarget, D2D1_BRUSH_PROPERTIES, D2D1_MATRIX_3X2_F },
    shared::d3d9types::D3DCOLORVALUE
};
use crate::dx_ok;

const IDENTITY_MATRIX: D2D1_MATRIX_3X2_F = D2D1_MATRIX_3X2_F { matrix: [[1.0, 0.0], [0.0, 1.0], [0.0, 0.0]] };

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
    pub text_brush: *mut ID2D1SolidColorBrush,
    pub line_number_brush: *mut ID2D1SolidColorBrush,
    pub caret_brush: *mut ID2D1SolidColorBrush,
    pub selection_brush: *mut ID2D1SolidColorBrush,
    pub variable_brush: *mut ID2D1SolidColorBrush,
    pub function_brush: *mut ID2D1SolidColorBrush,
    pub method_brush: *mut ID2D1SolidColorBrush,
    pub class_brush: *mut ID2D1SolidColorBrush,
    pub enum_brush: *mut ID2D1SolidColorBrush,
    pub test_brush: *mut ID2D1SolidColorBrush
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            background_color: D3DCOLORVALUE { r: 0.0, g: 0.0, b: 0.0, a: 1.0},
            text_brush: null_mut(),
            line_number_brush: null_mut(),
            caret_brush: null_mut(),
            selection_brush: null_mut(),
            variable_brush: null_mut(),
            function_brush: null_mut(),
            method_brush: null_mut(),
            class_brush: null_mut(),
            enum_brush: null_mut(),
            test_brush: null_mut()
        }
    }
}

impl Theme {
    pub fn new_default(target: *mut ID2D1HwndRenderTarget) -> Theme {
        let mut theme = Theme {
            background_color: create_color(0x282828FF),
            text_brush: null_mut(),
            line_number_brush: null_mut(),
            caret_brush: null_mut(),
            selection_brush: null_mut(),
            variable_brush: null_mut(),
            function_brush: null_mut(),
            method_brush: null_mut(),
            class_brush: null_mut(),
            enum_brush: null_mut(),
            test_brush: null_mut()
        };

        let brush_properties = D2D1_BRUSH_PROPERTIES {
            opacity: 1.0,
            transform: IDENTITY_MATRIX
        };

        const TEXT_COLOR: D3DCOLORVALUE = create_color(0xFBF1C7FF);
        const LINE_NUMBER_COLOR: D3DCOLORVALUE = create_color(0xD5C4A1FF);
        const CARET_COLOR: D3DCOLORVALUE = create_color(0xFE8019FF);
        const SELECTION_COLOR: D3DCOLORVALUE = create_color(0xD65D0EFF);
        const VARIABLE_COLOR: D3DCOLORVALUE = create_color(0x8EC07CFF);
        const FUNCTION_COLOR: D3DCOLORVALUE = create_color(0xFABD2FFF);
        const METHOD_COLOR: D3DCOLORVALUE = create_color(0xD79921FF);
        const CLASS_COLOR: D3DCOLORVALUE = create_color(0x83A598FF);
        const ENUM_COLOR: D3DCOLORVALUE = create_color(0x83A598FF);
        const TEST_COLOR: D3DCOLORVALUE = create_color(0x00FF00FF);

        unsafe {
            dx_ok!((*target).CreateSolidColorBrush(&TEXT_COLOR, &brush_properties, &mut theme.text_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&LINE_NUMBER_COLOR, &brush_properties, &mut theme.line_number_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&CARET_COLOR, &brush_properties, &mut theme.caret_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&SELECTION_COLOR, &brush_properties, &mut theme.selection_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&VARIABLE_COLOR, &brush_properties, &mut theme.variable_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&FUNCTION_COLOR, &brush_properties, &mut theme.function_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&METHOD_COLOR, &brush_properties, &mut theme.method_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&CLASS_COLOR, &brush_properties, &mut theme.class_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&ENUM_COLOR, &brush_properties, &mut theme.enum_brush as *mut *mut _));
            dx_ok!((*target).CreateSolidColorBrush(&TEST_COLOR, &brush_properties, &mut theme.test_brush as *mut *mut _));
        }

        theme
    }
}