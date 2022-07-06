use redis::aio::MultiplexedConnection;
use rocket::request::{FromRequest, Outcome, Request};
use std::env;
use std::ops::{Deref, DerefMut};

pub struct RdsConn(pub MultiplexedConnection);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RdsConn {
    type Error = ();
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let rconn = request.rocket().state::<MultiplexedConnection>().unwrap();
        Outcome::Success(RdsConn(rconn.clone()))
    }
}

impl Clone for RdsConn {
    fn clone(&self) -> Self {
        RdsConn(self.0.clone())
    }
}

impl Deref for RdsConn {
    type Target = MultiplexedConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for RdsConn {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub async fn init_rds_client() -> MultiplexedConnection {
    let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set");
    let client = redis::Client::open(redis_url).expect("connect to redis fail");
    client.get_multiplexed_async_connection().await.unwrap()
}
