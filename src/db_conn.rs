use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use std::env;
use std::ops::Deref;
use rocket::http::Status;
use rocket::request::{FromRequest, Request, Outcome};

pub type Conn = diesel::SqliteConnection;
pub type DbPool = Pool<ConnectionManager<Conn>>;
pub struct DbConn(pub PooledConnection<ConnectionManager<Conn>>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for DbConn {
    type Error = ();
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let pool = request.rocket().state::<DbPool>().unwrap();
        match pool.get() {
            Ok(conn) => Outcome::Success(DbConn(conn)),
            Err(_) => Outcome::Failure((Status::ServiceUnavailable, ())),
        }
    }
}

// For the convenience of using an &DbConn as an &Connection.
impl Deref for DbConn {
    type Target = Conn;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn init_pool() -> DbPool {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<Conn>::new(database_url);
    Pool::builder()
        .build(manager)
        .expect("database poll init fail")
}
