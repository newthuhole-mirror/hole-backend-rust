#![allow(clippy::unused_unit)]

use crate::db_conn::Db;
use crate::models::User;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::Redirect;
use rocket::serde::Deserialize;
use std::env;
use url::Url;

pub struct RefHeader(pub String);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RefHeader {
    type Error = ();
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match request.headers().get_one("Referer") {
            Some(h) => Outcome::Success(RefHeader(h.to_string())),
            None => Outcome::Forward(()),
        }
    }
}

#[get("/?p=cs")]
pub fn cs_login(r: RefHeader) -> Redirect {
    let mast_url = env::var("MAST_BASE_URL").unwrap();
    let mast_cli = env::var("MAST_CLIENT").unwrap();
    let mast_scope = env::var("MAST_SCOPE").unwrap();

    let jump_to_url = Url::parse(&r.0).unwrap();

    let mut redirect_url = env::var("AUTH_BACKEND_URL")
        .map(|url| Url::parse(&url).unwrap())
        .unwrap_or_else(|_| jump_to_url.clone());
    redirect_url.set_path("/_login/cs/auth");

    redirect_url = Url::parse_with_params(
        redirect_url.as_str(),
        &[
            ("redirect_url", redirect_url.as_str()),
            ("jump_to_url", jump_to_url.as_str()),
        ],
    )
    .unwrap();

    let url = Url::parse_with_params(
        &format!("{}oauth/authorize", mast_url),
        &[
            ("redirect_uri", redirect_url.as_str()),
            ("client_id", &mast_cli),
            ("scope", &mast_scope),
            ("response_type", "code"),
        ],
    )
    .unwrap();

    Redirect::to(url.to_string())
}

#[derive(Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct Token {
    pub access_token: String,
}

#[derive(Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct Account {
    pub id: String,
}
#[get("/cs/auth?<code>&<redirect_url>&<jump_to_url>")]
pub async fn cs_auth(
    code: String,
    redirect_url: String,
    jump_to_url: String,
    db: Db,
) -> Result<Redirect, &'static str> {
    if !env::var("FRONTEND_WHITELIST")
        .unwrap_or_default()
        .split(',')
        .any(|url| jump_to_url.starts_with(url))
    {
        return Err("前端地址不在白名单内");
    }

    let mast_url = env::var("MAST_BASE_URL").unwrap();
    let mast_cli = env::var("MAST_CLIENT").unwrap();
    let mast_sec = env::var("MAST_SECRET").unwrap();
    let mast_scope = env::var("MAST_SCOPE").unwrap();

    // to keep same
    let redirect_url = Url::parse_with_params(
        redirect_url.as_str(),
        &[
            ("redirect_url", redirect_url.as_str()),
            ("jump_to_url", jump_to_url.as_str()),
        ],
    )
    .unwrap();

    let client = reqwest::Client::new();
    let r = client
        .post(format!("{}oauth/token", &mast_url))
        .form(&[
            ("client_id", mast_cli.as_str()),
            ("client_secret", mast_sec.as_str()),
            ("scope", mast_scope.as_str()),
            ("redirect_uri", redirect_url.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
        ])
        .send()
        .await
        .unwrap();
    //dbg!(&r);

    let token: Token = r.json().await.unwrap();
    //dbg!(&token);

    let client = reqwest::Client::new();
    let account = client
        .get(format!("{}api/v1/accounts/verify_credentials", &mast_url))
        .bearer_auth(token.access_token)
        .send()
        .await
        .unwrap()
        .json::<Account>()
        .await
        .unwrap();

    //dbg!(&account);

    let tk = User::find_or_create_token(&db, &format!("cs_{}", &account.id), false)
        .await
        .unwrap();

    Ok(Redirect::to(format!("{}?token={}", &jump_to_url, &tk)))
}

#[get("/gh")]
pub fn gh_login(r: RefHeader) -> Redirect {
    let gh_url = "https://github.com/login/oauth/authorize";
    let gh_cli = env::var("GH_CLIENT").unwrap();
    let gh_scope = "user:email";

    let jump_to_url = Url::parse(&r.0).unwrap();

    let mut redirect_url = env::var("AUTH_BACKEND_URL")
        .map(|url| Url::parse(&url).unwrap())
        .unwrap_or_else(|_| jump_to_url.clone());
    redirect_url.set_path("/_login/gh/auth");

    redirect_url = Url::parse_with_params(
        redirect_url.as_str(),
        &[("jump_to_url", jump_to_url.as_str())],
    )
    .unwrap();

    let url = Url::parse_with_params(
        gh_url,
        &[
            ("redirect_uri", redirect_url.as_str()),
            ("client_id", &gh_cli),
            ("scope", gh_scope),
        ],
    )
    .unwrap();

    Redirect::to(url.to_string())
}

#[derive(Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct GithubEmail {
    pub email: String,
    pub verified: bool,
}

#[get("/gh/auth?<code>&<jump_to_url>")]
pub async fn gh_auth(code: String, jump_to_url: String, db: Db) -> Result<Redirect, &'static str> {
    if !env::var("FRONTEND_WHITELIST")
        .unwrap_or_default()
        .split(',')
        .any(|url| jump_to_url.starts_with(url))
    {
        return Err("前端地址不在白名单内");
    }

    let gh_cli = env::var("GH_CLIENT").unwrap();
    let gh_sec = env::var("GH_SECRET").unwrap();

    let client = reqwest::Client::new();
    let r = client
        .post("https://github.com/login/oauth/access_token")
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&[
            ("client_id", gh_cli.as_str()),
            ("client_secret", gh_sec.as_str()),
            ("code", code.as_str()),
        ])
        .send()
        .await
        .unwrap();

    //let token: rocket::serde::json::Value = r.json().await.unwrap();
    let token: Token = r.json().await.unwrap();

    dbg!(&token);

    let client = reqwest::Client::new();
    let r = client
        .get("https://api.github.com/user/emails")
        .bearer_auth(token.access_token)
        .header(reqwest::header::USER_AGENT, "hole_thu LoginBot")
        .send()
        .await
        .unwrap();
    // dbg!(&r);
    let emails = r
        .json::<Vec<GithubEmail>>()
        //.json::<rocket::serde::json::Value>()
        .await
        .unwrap();

    //dbg!(&emails);

    for email in emails {
        if let Some(name) = email
            .email
            .strip_suffix("@mails.tsinghua.edu.cn")
            .and_then(|name| email.verified.then_some(name))
        {
            let tk = User::find_or_create_token(&db, &format!("email_{}", name), false)
                .await
                .unwrap();

            return Ok(Redirect::to(format!("{}?token={}", &jump_to_url, &tk)));
        }
    }

    Err("没有找到已验证的清华邮箱")
}
