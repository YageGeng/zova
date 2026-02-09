extern crate alloc;

pub mod model;
pub mod types;

use alloc::vec::Vec;
use burn::prelude::*;
use model::doclayout::Model as DocLayoutModel;
use types::*;

pub use types::*;

/// 版面分析器
pub struct LayoutAnalyzer<B: Backend> {
    model: DocLayoutModel<B>,
    device: B::Device,
}

impl<B: Backend> LayoutAnalyzer<B> {
    /// 创建新的分析器
    pub fn new(device: B::Device) -> Self {
        let model = DocLayoutModel::new(&device);
        Self { model, device }
    }

    /// 分析图片，返回版面区域列表
    pub fn analyze(&self, image: &image::DynamicImage) -> Vec<LayoutRegion> {
        // 预处理图片到 1024x1024
        let input_tensor = self.preprocess_image(image);

        // 运行模型推理
        let output = self.model.forward(input_tensor);

        // 解码 YOLO 输出
        self.decode_output(output)
    }

    /// 预处理图片
    fn preprocess_image(&self, image: &image::DynamicImage) -> Tensor<B, 4> {
        // Resize 到 1024x1024
        let resized = image.resize_exact(1024, 1024, image::imageops::FilterType::Lanczos3);

        // 转换为 RGB
        let rgb = resized.to_rgb8();

        // 归一化并转换为 CHW 格式
        let mut data = Vec::with_capacity(3 * 1024 * 1024);
        for pixel in rgb.pixels() {
            data.push(pixel[0] as f32 / 255.0);
        }
        for pixel in rgb.pixels() {
            data.push(pixel[1] as f32 / 255.0);
        }
        for pixel in rgb.pixels() {
            data.push(pixel[2] as f32 / 255.0);
        }

        // 创建 tensor [1, 3, 1024, 1024]
        let tensor_1d: Tensor<B, 1> = Tensor::from_floats(data.as_slice(), &self.device);
        tensor_1d.reshape([1, 3, 1024, 1024])
    }

    /// 解码 YOLO 输出
    fn decode_output(&self, output: Tensor<B, 3>) -> Vec<LayoutRegion> {
        // 输出形状: [1, 300, 6] - [batch, num_boxes, (x, y, w, h, conf, class)]
        let [_batch, num_boxes, _features] = output.dims();

        // 获取输出数据
        let output_data = output.into_data();
        let values: Vec<f32> = output_data.convert::<f32>().to_vec().unwrap_or_default();

        let mut regions = Vec::new();

        // 解析每个检测框
        for i in 0..num_boxes.min(300) {
            let offset = i * 6;
            if offset + 5 >= values.len() {
                break;
            }

            let x = values[offset];
            let y = values[offset + 1];
            let w = values[offset + 2];
            let h = values[offset + 3];
            let conf = values[offset + 4];
            let class_id = values[offset + 5] as i32;

            // 过滤低置信度
            if conf < 0.3 {
                continue;
            }

            let class = match class_id {
                0 => LayoutClass::Title,
                1 => LayoutClass::Section,
                2 => LayoutClass::Paragraph,
                3 => LayoutClass::Caption,
                4 => LayoutClass::Image,
                5 => LayoutClass::Table,
                6 => LayoutClass::Header,
                7 => LayoutClass::Footer,
                8 => LayoutClass::PageNumber,
                9 => LayoutClass::Formula,
                10 => LayoutClass::Code,
                11 => LayoutClass::Reference,
                _ => LayoutClass::Other,
            };

            regions.push(LayoutRegion {
                id: i,
                bbox: BoundingBox {
                    x: (x - w / 2.0) / 1024.0,
                    y: (y - h / 2.0) / 1024.0,
                    width: w / 1024.0,
                    height: h / 1024.0,
                },
                class,
                confidence: conf,
            });
        }

        regions
    }
}