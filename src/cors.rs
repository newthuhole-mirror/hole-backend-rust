use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::{Request, Response};

pub struct CORS {
    pub whitelist: Vec<String>,
}

#[rocket::async_trait]
impl Fairing for CORS {
    fn info(&self) -> Info {
        Info {
            name: "Add CORS headers to responses",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        request
            .headers()
            .get_one("Origin")
            .and_then(|origin| self.whitelist.contains(&origin.to_string()).then(|| origin))
            .and_then(|origin| {
                response.set_header(Header::new("Access-Control-Allow-Origin", origin));
                response.set_header(Header::new("Access-Control-Allow-Methods", "POST, GET"));
                response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
                Some(())
            });
    }
}
