use crate::api::{CurrentUser, JsonApi};
use rocket::fs::TempFile;
use rocket::serde::json::json;
use std::process::Command;

#[post("/upload", data = "<file>")]
pub async fn ipfs_upload(_user: CurrentUser, file: TempFile<'_>) -> JsonApi {
    // dbg!(&file);

    // dbg!(&file.path());
    if let Some(filepath) = file.path() {
        let output = Command::new("ipfs")
            .args([
                "add",
                "-q",
                "-r",
                "-cid-version=1",
                filepath.to_str().unwrap(),
            ])
            .output()?;
        // dbg!(&output);
        let hash = std::str::from_utf8(&output.stdout)
            .unwrap()
            .split_terminator('\n')
            .last()
            .unwrap_or_else(|| {
                dbg!(&output);
                dbg!(&file.path());
                panic!("get ipfs output error");
            });
        code0!(json!({
            "hash": hash,
        }))
    } else {
        code1!("文件丢失")
    }
}
