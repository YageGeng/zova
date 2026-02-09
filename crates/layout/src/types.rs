use serde::{Deserialize, Serialize};

/// 版面区域
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutRegion {
    pub id: usize,
    /// 归一化坐标 [0-1]，相对于页面
    pub bbox: BoundingBox,
    pub class: LayoutClass,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LayoutClass {
    Title,      // 标题
    Section,    // 章节标题
    Paragraph,  // 正文段落
    Caption,    // 图/表标题
    Image,      // 图片
    Table,      // 表格
    Header,     // 页眉
    Footer,     // 页脚
    PageNumber, // 页码
    Formula,    // 公式
    Code,       // 代码块
    Reference,  // 参考文献
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32, // 左上角 x
    pub y: f32, // 左上角 y
    pub width: f32,
    pub height: f32,
}

impl BoundingBox {
    /// 转换为 PDF 坐标系
    pub fn to_pdf_coords(&self, page_width: f32, page_height: f32) -> (f32, f32, f32, f32) {
        let x1 = self.x * page_width;
        let y1 = self.y * page_height;
        let x2 = x1 + self.width * page_width;
        let y2 = y1 + self.height * page_height;
        (x1, y1, x2, y2)
    }
}
