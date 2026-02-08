//! PDF rendering to bitmap
//!
//! Uses hayro-interpret to parse PDF and tiny-skia for rasterization

use hayro_interpret::hayro_syntax::Pdf;
use hayro_interpret::util::PageExt;
use hayro_interpret::{Context, InterpreterSettings, interpret_page};
use hayro_interpret::{Device, Paint, GlyphDrawMode, Image as PdfImage, PathDrawMode, ClipPath, SoftMask, BlendMode};
use hayro_interpret::font::Glyph;
use kurbo::Affine;
use image::{ImageBuffer, Rgba};

/// Render PDF page to image
pub struct PageRenderer;

impl PageRenderer {
    /// Render a PDF page to RGBA image
    /// 
    /// Returns ImageBuffer with the rendered page
    pub fn render_page(
        pdf: &Pdf,
        page_idx: usize,
        width: u32,
        height: u32,
    ) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, RenderError> {
        let page = pdf.pages().get(page_idx)
            .ok_or(RenderError::PageNotFound)?;
        
        // Create render device
        let mut device = BitmapDevice::new(width, height);
        
        // Get page dimensions
        let (page_width, page_height) = page.render_dimensions();
        let bbox = kurbo::Rect::new(0.0, 0.0, page_width as f64, page_height as f64);
        
        // Setup context
        let settings = InterpreterSettings::default();
        let mut ctx = Context::new(
            page.initial_transform(true),
            bbox,
            page.xref(),
            settings,
        );
        
        // Interpret page
        interpret_page(page, &mut ctx, &mut device);
        
        Ok(device.into_image())
    }
}

/// Bitmap rendering device
struct BitmapDevice {
    width: u32,
    height: u32,
    buffer: Vec<u8>,
}

impl BitmapDevice {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            buffer: vec![0u8; (width * height * 4) as usize],
        }
    }
    
    fn into_image(self) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        ImageBuffer::from_raw(self.width, self.height, self.buffer)
            .expect("Invalid image dimensions")
    }
}

impl<'a> Device<'a> for BitmapDevice {
    fn draw_glyph(
        &mut self,
        _glyph: &Glyph<'a>,
        _transform: Affine,
        _glyph_transform: Affine,
        _paint: &Paint<'a>,
        _draw_mode: &GlyphDrawMode,
    ) {
        // TODO: Implement glyph rasterization
    }
    
    fn draw_image(&mut self, _image: PdfImage<'a, '_>, _transform: Affine) {
        // TODO: Implement image rendering
    }
    
    fn draw_path(
        &mut self,
        _path: &kurbo::BezPath,
        _transform: Affine,
        _paint: &Paint<'a>,
        _draw_mode: &PathDrawMode,
    ) {
        // TODO: Implement path rendering
    }
    
    fn set_soft_mask(&mut self, _mask: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _blend_mode: BlendMode) {}
    fn push_clip_path(&mut self, _clip_path: &ClipPath) {}
    fn push_transparency_group(
        &mut self, 
        _opacity: f32, 
        _mask: Option<SoftMask<'a>>, 
        _blend_mode: BlendMode
    ) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

#[derive(Debug)]
pub enum RenderError {
    PageNotFound,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::PageNotFound => write!(f, "Page not found"),
        }
    }
}

impl std::error::Error for RenderError {}