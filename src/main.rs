extern crate actix;
extern crate actix_redis;
extern crate actix_web;
extern crate config;
extern crate env_logger;
extern crate futures;
extern crate rand;
#[macro_use] extern crate redis_async;
extern crate serde;
extern crate serde_json;
extern crate sha2;
#[macro_use] extern crate serde_derive;

mod settings;
mod store;

use actix::{Addr, System};
use actix_redis::{Command, RedisActor};
use actix_web::{
    error::{ErrorInternalServerError},
    http::{Method},
    middleware::{Logger},
    server,
    App,
    AsyncResponder,
    FutureResponse,
    HttpRequest,
    Json,
    Responder,
};
use futures::{Future, future::FutureResult};
use rand::OsRng;
use redis_async::resp::RespValue;
use settings::Settings;
use store::{Account, Secret};

#[derive(Deserialize)]
struct SignupRequest {
    email: String,
    password: String
}

#[derive(Serialize)]
enum SignupResponse {
    Ok,
    UserAlreadyExists(String)
}

struct AppState {
    redis: Addr<RedisActor>,
    rng: OsRng,
}

fn auth(_req: HttpRequest<AppState>) -> impl Responder {
    ""
}

fn signup((body, req): (Json<SignupRequest>, HttpRequest<AppState>))
    -> FutureResponse<impl Responder> {
    let redis = req.state().redis.clone();
    let mut rng = req.state().rng.clone();

    let SignupRequest { email, password } = body.into_inner();

    let account = Account {
        email: email.clone(),
        secret: Secret::encode(&mut rng, &password),
    };

    let key = format!("users:{}", email);
    let value_result: FutureResult<String, _> =
        serde_json::to_string(&account)
            .map_err(ErrorInternalServerError)
            .into();

    let was_inserted_result = value_result
        .and_then(move |value|
            redis.send(Command(resp_array!["SETNX", key, value]))
                 .map_err(ErrorInternalServerError))
        .and_then(|resp_value|
            match resp_value {
                Ok(RespValue::Integer(1)) => Ok(true),
                Ok(RespValue::Integer(0)) => Ok(false),
                Ok(_) => Ok(false),
                Err(_) => Err(ErrorInternalServerError("")),
            });

    was_inserted_result
        .map(|was_inserted|
            match was_inserted {
                false => Json(SignupResponse::UserAlreadyExists(email)),
                true => Json(SignupResponse::Ok),
            })
        .responder()
}

fn main() {
    let Settings {
        listen_addr,
        logging,
        redis_addr,
    } = Settings::new().expect("Failed to load settings");

    std::env::set_var("RUST_LOG", logging);
    env_logger::init();

    let sys = System::new("authsrv");

    server::new(move || {
        let redis = RedisActor::start(redis_addr.clone());
        let rng = OsRng::new().expect("Failed to initialize RNG");

        let app_state = AppState { redis, rng };

        App::with_state(app_state)
            .middleware(Logger::default())
            .resource("/auth", |r| r.method(Method::GET).with(auth))
            .resource("/signup", |r| r.method(Method::POST).with(signup))
    })
        .bind(listen_addr)
        .unwrap()
        .start();

    sys.run();
}
