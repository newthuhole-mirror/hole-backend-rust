use crate::api::{CurrentUser, JsonAPI};
use rocket::form::Form;
use rocket::fs::TempFile;
use rocket::serde::json::json;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(FromForm)]
pub struct Upload<'f> {
    file: TempFile<'f>,
}

#[post("/upload", data = "<form>")]
pub async fn ipfs_upload(user: CurrentUser, mut form: Form<Upload<'_>>) -> JsonAPI {
    let file_dir = Path::new("user_files").join(&user.namehash);
    fs::create_dir_all(&file_dir)?;
    let file = &mut form.file;
    // dbg!(&file);
    let filename = file.name().unwrap_or("file").to_string()
        + "."
        + &file.content_type().map_or("", |ct| ct.sub().as_str());
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
