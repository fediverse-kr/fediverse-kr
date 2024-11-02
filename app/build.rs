use std::env;
use std::process::Command;
use std::path::Path;

fn main() {
    println!("cargo::rerun-if-changed=../style/**/*");
    println!("cargo::rerun-if-changed=../uno.config.ts");

    // 현재 디렉터리를 한 단계 위로 설정
    let root_dir = Path::new("..");
    if env::set_current_dir(root_dir).is_err() {
        panic!("Failed to set current directory to {:?}", root_dir);
    }

    let style_css_out = "public/style.css";

    let status = Command::new("pnpm")
        .arg("exec")
        .arg("sass")
        .arg("./style/main.scss")
        .arg(style_css_out)
        .status()
        .expect("Failed to execute sass command");

    if !status.success() {
        panic!("sass command failed with status: {:?}", status.code());
    }

    // `unocss` CLI 호출
    let status = Command::new("pnpm")
        .arg("exec")
        .arg("unocss")
        .arg(style_css_out)
        .arg("--write-transformed")
        .status()
        .expect("Failed to execute unocss command");

    if !status.success() {
        panic!("unocss transformation failed with status: {:?}", status.code());
    }

    // `unocss` CLI 호출
    let status = Command::new("pnpm")
        .arg("exec")
        .arg("unocss")
        .arg("./app/src/**/*.rs")
        .arg("-o")
        .arg("public/util.css")
        .status()
        .expect("Failed to execute unocss command");

    if !status.success() {
        panic!("unocss command failed with status: {:?}", status.code());
    }
}
