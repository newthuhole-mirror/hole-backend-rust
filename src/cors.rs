use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::{Request, Response};
use std::path::PathBuf;

pub struct Cors {
    pub whitelist: Vec<String>,
}

#[rocket::async_trait]
impl Fairing for Cors {
    fn info(&self) -> Info {
        Info {
            name: "Add CORS headers to responses",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        if let Some(origin) = request
            .headers()
            .get_one("Origin")
            .and_then(|origin| self.whitelist.contains(&origin.to_string()).then(|| origin))
        {
            response.set_header(Header::new("Access-Control-Allow-Origin", origin));
            response.set_header(Header::new(
                "Access-Control-Allow-Methods",
                "POST, GET, OPTIONS",
            ));
            response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
            response.set_header(Header::new(
                "Access-Control-Allow-Headers",
                "User-Token, Content-Type",
            ));
        }
    }
}

#[options("/<_path..>")]
pub async fn options_handler(_path: PathBuf) {}
