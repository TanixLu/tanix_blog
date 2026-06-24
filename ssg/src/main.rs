mod aliyun_openapi;
mod unix_zip;

use std::{path::PathBuf, process::Command};

use base64::Engine as _;

use crate::aliyun_openapi::fc_upload;

fn main() -> anyhow::Result<()> {
    build_server()?;
    println!("Server built successfully.");

    let zip_base64 = zip2base64(&[
        PathBuf::from("target/x86_64-unknown-linux-musl/release/server"),
        PathBuf::from("public"),
    ])?;
    println!("Zip built successfully.");

    if std::env::args().len() == 2 && std::env::args().nth(1).unwrap() == "u" {
        upload_to_aliyun(&zip_base64)?;
        println!("Uploaded to Aliyun successfully.");
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
    Ok(())
}

fn zip2base64(paths: &[PathBuf]) -> anyhow::Result<String> {
    let zip_data = unix_zip::unix_zip(paths)?;
    std::fs::write("output.zip", &zip_data)?;
    let base64_data = base64::engine::general_purpose::STANDARD.encode(&zip_data);
    Ok(base64_data)
}

fn upload_to_aliyun(zip_base64: &str) -> anyhow::Result<()> {
    // Implementation for uploading to Aliyun
    dotenvy::dotenv()?;
    let aliyun_id = std::env::var("ALIYUN_ID")?;
    let aliyun_access_key_id = std::env::var("ALIYUN_ACCESS_KEY_ID")?;
    let aliyun_access_key_secret = std::env::var("ALIYUN_ACCESS_KEY_SECRET")?;
    let aliyun_region = std::env::var("ALIYUN_REGION")?;
    let aliyun_fc_name = std::env::var("ALIYUN_FC_NAME")?;
    fc_upload(
        &aliyun_id,
        &aliyun_access_key_id,
        &aliyun_access_key_secret,
        &aliyun_region,
        &aliyun_fc_name,
        zip_base64,
    )
}
