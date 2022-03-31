use crate::api::{CurrentUser, JsonAPI};
use chrono::offset::Local;
use rocket::fs::TempFile;
use rocket::serde::json::json;
use std::fs;
use std::path::Path;
use std::process::Command;

#[post("/upload", data = "<file>")]
pub async fn ipfs_upload(user: CurrentUser, mut file: TempFile<'_>) -> JsonAPI {
    let file_dir = Path::new("user_files").join(&user.namehash);
    fs::create_dir_all(&file_dir)?;
    // dbg!(&file);
    let filename = format!(
        "{}-{}.{}",
        Local::now().timestamp(),
        &file.content_type().map_or("unknow", |ct| ct.top().as_str()),
        &file.content_type().map_or("file", |ct| ct.sub().as_str())
    );
    debug!("dir: {}", &file_dir.to_str().unwrap());
    file.persist_to(file_dir.join(&filename)).await?;
    // dbg!(&file_dir);
    // dbg!(file_dir.with_file_name(&filename));
    let output = Command::new("ipfs")
        .args([
            "add",
            "-q",
            "-r",
            "-cid-version=1",
            file_dir.to_str().unwrap(),
        ])
        .output()?;
    // dbg!(&output);
    let hash = std::str::from_utf8(&output.stdout)
        .unwrap()
        .split_terminator("\n")
        .last()
        .unwrap_or_else(|| {
            dbg!(&output.stdout);
            panic!("get ipfs output error");
        });
    code0!(json!({
        "hash": hash,
        "filename": filename,
    }))
}
