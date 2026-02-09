//! PDF text extraction and rendering utilities.
//!
//! This crate provides:
//! - Text extraction via `TextExtractDevice`
//! - PDF rendering via `PageRenderer`
//!
//! Notes:
//! - PDF text is not guaranteed to have a reliable Unicode mapping. `Glyph::as_unicode()` is
//!   best-effort and may return `None`.
//! - For layout reconstruction (words/lines/reading order), prefer keeping the per-glyph bbox
//!   and transforms instead of committing early to a `String`.

pub mod render;

use hayro_interpret::font::Glyph;
use hayro_interpret::{
    BlendMode, ClipPath, Device, GlyphDrawMode, Image, Paint, PathDrawMode, SoftMask,
};
use kurbo::{Affine, Rect, Shape};

/// A single extracted glyph event, optionally mapped to a Unicode character.
#[derive(Debug, Clone)]
pub struct ExtractedChar {
    /// Best-effort Unicode mapping for the glyph.
    pub ch: Option<char>,
    /// Bounding box in *page space* (after applying transforms), if it can be determined.
    ///
    /// For Type3 glyphs, this may be unavailable without interpreting the glyph program.
    pub bbox: Option<Rect>,
    /// The current transformation matrix (CTM) when the glyph is drawn.
    pub ctm: Affine,
    /// The glyph-local transform (text state + font units conversion + displacement).
    pub glyph_transform: Affine,
    /// How the glyph was drawn (fill/stroke/invisible/etc.).
    pub draw_mode: GlyphDrawMode,
}

/// A `hayro-interpret` [`Device`] implementation that records glyph events for text extraction.
///
/// Typical usage:
/// - Run `hayro_interpret::interpret_page(...)` with this device.
/// - Consume [`TextExtractDevice::chars`] for downstream layout reconstruction.
#[derive(Debug, Default)]
pub struct TextExtractDevice {
    /// Glyph events in the order they are emitted by the interpreter.
    pub chars: Vec<ExtractedChar>,
}

/// Options for inferring spaces from glyph bounding boxes.
#[derive(Debug, Clone)]
pub struct SpaceInferenceOptions {
    /// Minimum vertical overlap ratio (relative to the smaller glyph bbox height)
    /// to consider two glyphs on the same line.
    pub same_line_overlap_ratio: f64,
    /// If the horizontal gap between consecutive glyph bboxes is larger than this
    /// ratio times the average bbox height, insert a space.
    ///
    /// Height is used as a font-size proxy because PDFs often omit explicit space
    /// characters and rely on advances/positioning.
    pub gap_to_height_ratio: f64,
}

impl Default for SpaceInferenceOptions {
    fn default() -> Self {
        Self {
            // Conservative: require noticeable overlap to avoid bridging between lines.
            same_line_overlap_ratio: 0.5,
            // A common heuristic: gap greater than ~1/4 of font size implies a word break.
            gap_to_height_ratio: 0.25,
        }
    }
}

impl TextExtractDevice {
    /// Convert the captured stream into a lossy `String`.
    ///
    /// This drops glyphs that cannot be mapped to Unicode.
    pub fn to_string_lossy(&self) -> String {
        self.chars.iter().filter_map(|c| c.ch).collect()
    }

    /// Convert the captured stream into a lossy `String`, inserting spaces based on bbox gaps.
    ///
    /// This is a *heuristic* for PDFs that omit explicit space characters. It currently:
    /// - inserts `' '` when consecutive glyph bboxes have a sufficiently large horizontal gap
    /// - only considers glyphs to be on the same line if their vertical bboxes overlap
    ///
    /// It does **not** infer newlines/reading order; that's intentionally deferred to line
    /// reconstruction.
    pub fn to_string_infer_spaces(&self) -> String {
        self.to_string_infer_spaces_with(&SpaceInferenceOptions::default())
    }

    /// Same as [`TextExtractDevice::to_string_infer_spaces`] but configurable.
    pub fn to_string_infer_spaces_with(&self, opts: &SpaceInferenceOptions) -> String {
        let mut out = String::new();

        // We base spacing on geometry even for unmapped glyphs (ch=None), because missing
        // Unicode should not erase layout information.
        let mut last_bbox: Option<Rect> = None;
        let mut last_emitted_was_space = false;

        for g in &self.chars {
            if let (Some(prev), Some(cur)) = (last_bbox, g.bbox)
                && is_same_line(prev, cur, opts.same_line_overlap_ratio)
            {
                let gap = cur.x0 - prev.x1;
                // Negative/zero gaps happen due to kerning or overlap.
                if gap > 0.0 {
                    let avg_h = 0.5 * (prev.height() + cur.height());
                    let threshold = opts.gap_to_height_ratio * avg_h;

                    // Avoid emitting repeated spaces.
                    if gap > threshold && !out.is_empty() && !last_emitted_was_space {
                        out.push(' ');
                        last_emitted_was_space = true;
                    }
                }
            }

            if let Some(ch) = g.ch {
                out.push(ch);
                last_emitted_was_space = ch == ' ';
            }

            if g.bbox.is_some() {
                last_bbox = g.bbox;
            }
        }

        out
    }
}

fn is_same_line(a: Rect, b: Rect, min_overlap_ratio: f64) -> bool {
    let overlap = a.y1.min(b.y1) - a.y0.max(b.y0);
    if overlap <= 0.0 {
        return false;
    }

    let denom = a.height().min(b.height());
    // Degenerate bboxes can happen; fall back to "not same line".
    if denom <= 0.0 {
        return false;
    }

    (overlap / denom) >= min_overlap_ratio
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ec(ch: char, x0: f64, x1: f64, y0: f64, y1: f64) -> ExtractedChar {
        ExtractedChar {
            ch: Some(ch),
            bbox: Some(Rect::new(x0, y0, x1, y1)),
            ctm: Affine::IDENTITY,
            glyph_transform: Affine::IDENTITY,
            draw_mode: GlyphDrawMode::Invisible,
        }
    }

    #[test]
    fn infer_space_on_large_gap_same_line() {
        let mut d = TextExtractDevice::default();
        // Height ~ 10, gap 4 => 0.4 * height => should insert with default 0.25.
        d.chars.push(ec('H', 0.0, 6.0, 0.0, 10.0));
        d.chars.push(ec('i', 10.0, 12.0, 0.0, 10.0));
        assert_eq!(d.to_string_infer_spaces(), "H i");
    }

    #[test]
    fn no_space_on_small_gap_same_line() {
        let mut d = TextExtractDevice::default();
        d.chars.push(ec('H', 0.0, 6.0, 0.0, 10.0));
        d.chars.push(ec('i', 6.8, 8.8, 0.0, 10.0));
        assert_eq!(d.to_string_infer_spaces(), "Hi");
    }

    #[test]
    fn no_space_across_lines() {
        let mut d = TextExtractDevice::default();
        d.chars.push(ec('A', 0.0, 6.0, 0.0, 10.0));
        // Next glyph is on a different line (no vertical overlap).
        d.chars.push(ec('B', 0.0, 6.0, 20.0, 30.0));
        assert_eq!(d.to_string_infer_spaces(), "AB");
    }
}

impl<'a> Device<'a> for TextExtractDevice {
    fn set_soft_mask(&mut self, _mask: Option<SoftMask<'a>>) {
        // Not needed for text extraction.
    }

    fn set_blend_mode(&mut self, _blend_mode: BlendMode) {
        // Not needed for text extraction.
    }

    fn draw_path(
        &mut self,
        _path: &kurbo::BezPath,
        _transform: Affine,
        _paint: &Paint<'a>,
        _draw_mode: &PathDrawMode,
    ) {
        // Ignore non-text paths.
    }

    fn push_clip_path(&mut self, _clip_path: &ClipPath) {
        // Ignore clipping; glyph positions are still captured via transforms.
    }

    fn push_transparency_group(
        &mut self,
        _opacity: f32,
        _mask: Option<SoftMask<'a>>,
        _blend_mode: BlendMode,
    ) {
        // Ignore transparency groups.
    }

    fn draw_glyph(
        &mut self,
        glyph: &Glyph<'a>,
        transform: Affine,
        glyph_transform: Affine,
        _paint: &Paint<'a>,
        draw_mode: &GlyphDrawMode,
    ) {
        let ch = glyph.as_unicode();

        // For outline glyphs, compute bbox by transforming the outline into page space.
        // This preserves rotations/shears that are common in PDFs.
        let bbox = match glyph {
            Glyph::Outline(og) => {
                let path_in_page = transform * (glyph_transform * og.outline());
                Some(path_in_page.bounding_box())
            }
            Glyph::Type3(_) => None,
        };

        self.chars.push(ExtractedChar {
            ch,
            bbox,
            ctm: transform,
            glyph_transform,
            draw_mode: draw_mode.clone(),
        });
    }

    fn draw_image(&mut self, _image: Image<'a, '_>, _transform: Affine) {
        // Ignore images.
    }

    fn pop_clip_path(&mut self) {
        // Ignore.
    }

    fn pop_transparency_group(&mut self) {
        // Ignore.
    }
}
