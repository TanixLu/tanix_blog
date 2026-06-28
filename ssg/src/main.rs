mod aliyun_openapi;
mod html_builder;
mod unix_zip;
mod utils;

use std::{path::Path, process::Command};

use base64::Engine as _;

use crate::{aliyun_openapi::fc_upload, html_builder::build_html, utils::human_path_size};

const SERVER_PATH: &str = "target/x86_64-unknown-linux-musl/release/server";
const PUBLIC_PATH: &str = "public/";
const OUTPUT_ZIP_PATH: &str = "output.zip";

fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;

    build_server()?;
    build_html()?;
    let zip_base64 = zip2base64(&[SERVER_PATH, PUBLIC_PATH])?;
    if std::env::args().nth(1).as_deref() == Some("u") {
        upload_to_aliyun(&zip_base64)?;
    }
    Ok(())
}

fn build_server() -> anyhow::Result<()> {
    Command::new("cargo")
        .args(&[
            "zigbuild",
            "--bin",
            "server",
            "--release",
            "--target",
            "x86_64-unknown-linux-musl",
        ])
        .status()?;
    println!(
        "Server built successfully, {}.",
        human_path_size(SERVER_PATH)?
    );
    Ok(())
}

fn zip2base64(paths: &[impl AsRef<Path>]) -> anyhow::Result<String> {
    let zip_data = unix_zip::unix_zip(paths)?;
    std::fs::write(OUTPUT_ZIP_PATH, &zip_data)?;
    println!(
        "Zip built successfully, {}.",
        human_path_size(OUTPUT_ZIP_PATH)?
    );
    let base64_data = base64::engine::general_purpose::STANDARD.encode(&zip_data);
    Ok(base64_data)
}

fn upload_to_aliyun(zip_base64: &str) -> anyhow::Result<()> {
    let aliyun_id = std::env::var("ALIYUN_ID")?;
    let aliyun_access_key_id = std::env::var("ALIYUN_ACCESS_KEY_ID")?;
    let aliyun_access_key_secret = std::env::var("ALIYUN_ACCESS_KEY_SECRET")?;
    let aliyun_fc_region = std::env::var("ALIYUN_FC_REGION")?;
    let aliyun_fc_name = std::env::var("ALIYUN_FC_NAME")?;
    fc_upload(
        &aliyun_id,
        &aliyun_access_key_id,
        &aliyun_access_key_secret,
        &aliyun_fc_region,
        &aliyun_fc_name,
        zip_base64,
    )?;
    println!("Uploaded to Aliyun successfully.");
    Ok(())
}
