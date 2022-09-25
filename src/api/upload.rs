use super::{CurrentUser, JsonApi};
use rocket::fs::TempFile;
use rocket::serde::json::json;
use std::env::var;

#[post("/upload", data = "<file>")]
pub async fn local_upload(_user: CurrentUser, mut file: TempFile<'_>) -> JsonApi {
    let filename: String = format!(
        "file{}.{}",
        file.path().unwrap().file_name().unwrap().to_str().unwrap(),
        file.content_type()
            .map(|ct| ct.extension().unwrap_or_else(|| ct.sub()).as_str())
            .unwrap_or("unknown")
    );

    file.copy_to(format!("{}/{}", var("UPLOAD_DIR").unwrap(), filename))
        .await?;

    code0!(json!({ "path": filename }))
}
