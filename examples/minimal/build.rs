// use std::path::{Path, PathBuf};

// use ugm::{parser::ParseOptions, speedy::Writable, texture::TextureCompression, Model};
// use xshell::Shell;

// fn parse_model(model_path: PathBuf) {
//     let gltf_bytes = std::fs::read(&model_path).unwrap();

//     let model: Model = Model::parse_glb(
//         &gltf_bytes,
//         ParseOptions {
//             texture_compression: Some(TextureCompression::Bc),
//         },
//     )
//     .expect("Failed to parse glTF model.");

//     let ugm_bytes: Vec<u8> = model.write_to_vec().unwrap();
//     std::fs::write(model_path.with_extension("ugm"), ugm_bytes).unwrap();
// }

// fn parse_assets(shell: &Shell, dir: &PathBuf) {
//     let files = shell.read_dir(dir).unwrap();

//     for file in &files {
//         if !file.is_dir() {
//             if let Some(extension) = file.extension() {
//                 if extension.to_str().unwrap() == "glb" {
//                     parse_model(file.to_path_buf());
//                 }
//             }
//         } else {
//             parse_assets(shell, file);
//         }
//     }
// }

// fn main() {
//     println!("cargo::rerun-if-changed=assets");

//     let root_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

//     let shell = Shell::new().unwrap();
//     shell.change_dir(root_dir);
//     std::env::set_current_dir(root_dir).unwrap();

//     shell.create_dir("baked_assets").unwrap();
//     parse_assets(&shell, &root_dir.to_path_buf());
// }

fn main() {}
