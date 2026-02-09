use layout::types::LayoutRegion;
use serde::{Deserialize, Serialize};

/// PDF 的完整结构化表示
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfTree {
    pub pages: Vec<Page>,
    pub metadata: PdfMetadata,
}

impl PdfTree {
    pub fn new() -> Self {
        Self {
            pages: Vec::new(),
            metadata: PdfMetadata::default(),
        }
    }
}

impl Default for PdfTree {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub page_num: usize,
    pub width: f32,
    pub height: f32,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    pub region: LayoutRegion,
    pub content: BlockContent,
    pub bbox: (f32, f32, f32, f32), // PDF 坐标
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BlockContent {
    Text(TextBlock),
    Image(ImageBlock),
    Table(TableBlock),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    pub text: String,
    pub font_info: Option<FontInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageBlock {
    pub alt_text: Option<String>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBlock {
    pub rows: Vec<Vec<String>>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontInfo {
    pub size: f32,
    pub is_bold: bool,
    pub is_italic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub page_count: usize,
}
