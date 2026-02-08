use burn_import::onnx::ModelGen;
use std::path::PathBuf;

fn main() {
    // 使用最终的静态模型
    let model_path = "../../models/doclayout_final.onnx";
    
    // 检查模型是否存在
    if !PathBuf::from(model_path).exists() {
        panic!("Model not found: {}", model_path);
    }
    
    println!("cargo:rerun-if-changed={}", model_path);
    
    // 尝试使用 burn-import 生成模型代码
    // 注意：模型包含动态形状操作（Shape/Gather/Range），
    // burn-import 0.16 可能无法处理
    ModelGen::new()
        .input(model_path)
        .out_dir("model")
        .run_from_script();
    
    println!("cargo:warning=Model code generation completed");
}