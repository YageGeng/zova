use burn_onnx::ModelGen;

fn main() {
    // Generate model code from ONNX file
    ModelGen::new()
        .input("src/model/doclayout_yolo.onnx")
        .out_dir("model/")
        .run_from_script();
}
