use crate::tree::*;
use burn::prelude::*;
use hayro_interpret::hayro_syntax::Pdf;
use layout::LayoutAnalyzer;
use pdf::render::PageRenderer;

/// PDF 处理管道
pub struct ProcessingPipeline<B: Backend> {
    layout_analyzer: LayoutAnalyzer<B>,
}

impl<B: Backend> ProcessingPipeline<B> {
    /// 创建新的处理管道
    pub fn new(device: B::Device) -> Self {
        Self {
            layout_analyzer: LayoutAnalyzer::new(device),
        }
    }

    /// 处理 PDF 文件
    pub fn process(&self, pdf_data: &[u8]) -> Result<PdfTree, Box<dyn std::error::Error>> {
        let mut tree = PdfTree::new();

        // 加载 PDF
        let pdf = Pdf::new(std::sync::Arc::new(pdf_data.to_vec()))
            .map_err(|e| format!("Failed to parse PDF: {:?}", e))?;

        let total_pages = pdf.pages().len();
        tree.metadata.page_count = total_pages;

        // 处理每一页
        for page_idx in 0..total_pages {
            let page = pdf.pages().get(page_idx).ok_or("Page not found")?;

            let (width, height) = page.render_dimensions();

            // 渲染页面为图片
            let image =
                PageRenderer::render_page(&pdf, page_idx, 1024, (1024.0 * height / width) as u32)?;

            // 版面分析
            let dynamic_image = image::DynamicImage::ImageRgba8(image);
            let regions = self.layout_analyzer.analyze(&dynamic_image);

            // 构建 blocks
            let mut blocks = Vec::new();
            for (idx, region) in regions.iter().enumerate() {
                let block = Block {
                    id: format!("p{}-b{}", page_idx, idx),
                    region: region.clone(),
                    content: BlockContent::Text(TextBlock {
                        text: String::new(),
                        font_info: None,
                    }),
                    bbox: region.bbox.to_pdf_coords(width as f32, height as f32),
                };
                blocks.push(block);
            }

            tree.pages.push(Page {
                page_num: page_idx,
                width: width as f32,
                height: height as f32,
                blocks,
            });
        }

        Ok(tree)
    }

    /// 导出为 JSON
    pub fn export_json(tree: &PdfTree) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(tree)
    }
}
