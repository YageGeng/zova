use pdf::TextExtractDevice;

use hayro_interpret::hayro_syntax::Pdf;
use hayro_interpret::util::PageExt;
use hayro_interpret::{Context, InterpreterSettings, interpret_page};
use kurbo::Rect;

use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);

    let Some(path) = path else {
        eprintln!("Usage: cargo run -p pdf --example extract_chars -- <file.pdf>");
        std::process::exit(2);
    };

    let data = std::fs::read(&path).unwrap_or_else(|e| {
        eprintln!("Failed to read {}: {e}", path.display());
        std::process::exit(1);
    });

    let pdf = Pdf::new(Arc::new(data)).unwrap_or_else(|e| {
        eprintln!("Failed to parse PDF {}: {e:?}", path.display());
        std::process::exit(1);
    });

    let settings = InterpreterSettings::default();

    for (page_idx, page) in pdf.pages().iter().enumerate() {
        let (w, h) = page.render_dimensions();
        let bbox = Rect::new(0.0, 0.0, w as f64, h as f64);

        let mut ctx = Context::new(
            page.initial_transform(true),
            bbox,
            page.xref(),
            settings.clone(),
        );
        let mut device = TextExtractDevice::default();

        interpret_page(page, &mut ctx, &mut device);

        println!("=== Page {} ===", page_idx + 1);
        println!("{}", device.to_string_infer_spaces());
    }
}
