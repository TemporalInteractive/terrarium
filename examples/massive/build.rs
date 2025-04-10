use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use ugm::{parser::ParseOptions, speedy::Writable, texture::TextureCompression, Model};
use xshell::Shell;

fn file_modified_time_in_seconds(path: &PathBuf) -> u64 {
    std::fs::metadata(path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn parse_model(model_path: PathBuf) {
    if std::fs::exists(model_path.with_extension("ugm")).unwrap() {
        let gltf_modified = file_modified_time_in_seconds(&model_path);
        let ugm_modified = file_modified_time_in_seconds(&model_path.with_extension("ugm"));

        if gltf_modified < ugm_modified {
            return;
        }
    }

    let gltf_bytes = std::fs::read(&model_path).unwrap();

    let model: Model = Model::parse_glb(
        &gltf_bytes,
        ParseOptions {
            texture_compression: Some(TextureCompression::Bc),
            generate_mips: true,
        },
    )
    .expect("Failed to parse glTF model.");

    let ugm_bytes: Vec<u8> = model.write_to_vec().unwrap();
    std::fs::write(model_path.with_extension("ugm"), ugm_bytes).unwrap();
}

fn parse_assets(shell: &Shell, dir: &PathBuf) {
    let files = shell.read_dir(dir).unwrap();

    for file in &files {
        if !file.is_dir() {
            if let Some(extension) = file.extension() {
                if extension.to_str().unwrap() == "glb" {
                    parse_model(file.to_path_buf());
                }
            }
        } else {
            parse_assets(shell, file);
        }
    }
}

fn main() {
    println!("cargo::rerun-if-changed=../assets");

    let root_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let shell = Shell::new().unwrap();
    shell.change_dir(root_dir);
    std::env::set_current_dir(root_dir).unwrap();

    parse_assets(&shell, &root_dir.to_path_buf());
}
