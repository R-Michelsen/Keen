use bindings::{
    Windows::Foundation::Numerics::*,
    Windows::Win32::Direct2D::*
};
use windows::Result;

const DEFAULT_BACKGROUND_COLOR: D2D1_COLOR_F = create_color(0x282828FF);
const DEFAULT_STATUS_BAR_COLOR: D2D1_COLOR_F = create_color(0x141414FF);
const DEFAULT_BRACKET_COLOR: D2D1_COLOR_F = create_color(0xFFFFFFFF);
const DEFAULT_TEXT_COLOR: D2D1_COLOR_F = create_color(0xFBF1C7FF);
const DEFAULT_LINE_NUMBER_COLOR: D2D1_COLOR_F = create_color(0xD5C4A1FF);
const DEFAULT_CARET_COLOR: D2D1_COLOR_F = create_color(0xFE8019FF);
const DEFAULT_SELECTION_COLOR: D2D1_COLOR_F = create_color(0x464646FF);
const DEFAULT_VARIABLE_COLOR: D2D1_COLOR_F = create_color(0xADD8E6FF);
const DEFAULT_FUNCTION_COLOR: D2D1_COLOR_F = create_color(0xFBD06DFF);
const DEFAULT_METHOD_COLOR: D2D1_COLOR_F = create_color(0xD3869BFF);
const DEFAULT_CLASS_COLOR: D2D1_COLOR_F = create_color(0xA0DB8EFF);
const DEFAULT_ENUM_COLOR: D2D1_COLOR_F = create_color(0xA0DB8EFF);
const DEFAULT_COMMENT_COLOR: D2D1_COLOR_F = create_color(0xB8BB26FF);
const DEFAULT_KEYWORD_COLOR: D2D1_COLOR_F = create_color(0xFB4934FF);
const DEFAULT_LITERAL_COLOR: D2D1_COLOR_F = create_color(0xFE8019FF);
const DEFAULT_MACRO_PREPROCESSOR_COLOR: D2D1_COLOR_F = create_color(0xEE7AE9FF);
const DEFAULT_PRIMITIVE_COLOR: D2D1_COLOR_F = create_color(0xCDF916FF);

const fn create_color(color: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((color >> 24) & 0xFF) as f32 / 255.0,
        g: ((color >> 16) & 0xFF) as f32 / 255.0,
        b: ((color >>  8) & 0xFF) as f32 / 255.0,
        a: (color         & 0xFF) as f32 / 255.0
    }
}

pub struct Theme {
    pub background_color: D2D1_COLOR_F,
    pub status_bar_brush: Option<ID2D1SolidColorBrush>,
    pub bracket_brush: Option<ID2D1SolidColorBrush>,
    pub bracket_rect_width: f32,
    pub text_brush: Option<ID2D1SolidColorBrush>,
    pub line_number_brush: Option<ID2D1SolidColorBrush>,
    pub caret_brush: Option<ID2D1SolidColorBrush>,
    pub selection_brush: Option<ID2D1SolidColorBrush>,
    pub variable_brush: Option<ID2D1SolidColorBrush>,
    pub function_brush: Option<ID2D1SolidColorBrush>,
    pub method_brush: Option<ID2D1SolidColorBrush>,
    pub class_brush: Option<ID2D1SolidColorBrush>,
    pub enum_brush: Option<ID2D1SolidColorBrush>,
    pub comment_brush: Option<ID2D1SolidColorBrush>,
    pub keyword_brush: Option<ID2D1SolidColorBrush>,
    pub literal_brush: Option<ID2D1SolidColorBrush>,
    pub macro_preprocessor_brush: Option<ID2D1SolidColorBrush>,
    pub primitive_brush: Option<ID2D1SolidColorBrush>

}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background_color: D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 1.0},
            status_bar_brush: None,
            bracket_brush: None,
            bracket_rect_width: 0.0,
            text_brush: None,
            line_number_brush: None,
            caret_brush: None,
            selection_brush: None,
            variable_brush: None,
            function_brush: None,
            method_brush: None,
            class_brush: None,
            enum_brush: None,
            comment_brush: None,
            keyword_brush: None,
            literal_brush: None,
            macro_preprocessor_brush: None,
            primitive_brush: None,
        }
    }
}

impl Theme {
    pub fn new_default(render_target: &ID2D1HwndRenderTarget) -> Result<Self> {
        let mut theme = Self {
            background_color: DEFAULT_BACKGROUND_COLOR,
            status_bar_brush: None,
            bracket_brush: None,
            bracket_rect_width: 2.0,
            text_brush: None,
            line_number_brush: None,
            caret_brush: None,
            selection_brush: None,
            variable_brush: None,
            function_brush: None,
            method_brush: None,
            class_brush: None,
            enum_brush: None,
            comment_brush: None,
            keyword_brush: None,
            literal_brush: None,
            macro_preprocessor_brush: None,
            primitive_brush: None
        };

        let brush_properties = D2D1_BRUSH_PROPERTIES {
            opacity: 1.0,
            transform: Matrix3x2::identity()
        };

        unsafe {
            render_target.CreateSolidColorBrush(&DEFAULT_TEXT_COLOR, &brush_properties, &mut theme.text_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_STATUS_BAR_COLOR, &brush_properties, &mut theme.status_bar_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_BRACKET_COLOR, &brush_properties, &mut theme.bracket_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_LINE_NUMBER_COLOR, &brush_properties, &mut theme.line_number_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_CARET_COLOR, &brush_properties, &mut theme.caret_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_SELECTION_COLOR, &brush_properties, &mut theme.selection_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_VARIABLE_COLOR, &brush_properties, &mut theme.variable_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_FUNCTION_COLOR, &brush_properties, &mut theme.function_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_METHOD_COLOR, &brush_properties, &mut theme.method_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_CLASS_COLOR, &brush_properties, &mut theme.class_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_ENUM_COLOR, &brush_properties, &mut theme.enum_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_COMMENT_COLOR, &brush_properties, &mut theme.comment_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_KEYWORD_COLOR, &brush_properties, &mut theme.keyword_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_LITERAL_COLOR, &brush_properties, &mut theme.literal_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_MACRO_PREPROCESSOR_COLOR, &brush_properties, &mut theme.macro_preprocessor_brush).ok()?;
            render_target.CreateSolidColorBrush(&DEFAULT_PRIMITIVE_COLOR, &brush_properties, &mut theme.primitive_brush).ok()?;
        }

        Ok(theme)
    }
}