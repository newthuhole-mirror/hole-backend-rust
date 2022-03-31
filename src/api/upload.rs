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
    let file_dir = Path::new("user_files").join(user.namehash);
    fs::create_dir_all(&file_dir)?;
    let file = &mut form.file;
    // dbg!(&file);
    let filename = file.name().unwrap_or("file").to_string()
        + "."
        + &file.content_type().map_or("", |ct| ct.sub().as_str());
    file.persist_to(file_dir.with_file_name(&filename)).await?;
    let output = Command::new("ipfs")
        .args(["add", "-q", "-r", "-cid-version=1", "user_files"])
        .output()?;
    // dbg!(&output);
    let hash = std::str::from_utf8(&output.stdout)
        .unwrap()
        .split_terminator("\n")
        .last()
        .unwrap();
    code0!(json!({
        "hash": hash,
        "filename": filename,
    }))
}
