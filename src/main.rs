extern crate actix;
extern crate actix_redis;
extern crate actix_web;
extern crate config;
extern crate env_logger;
extern crate futures;
extern crate rand;
#[macro_use] extern crate redis_async;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate sha2;
#[macro_use] extern crate serde_derive;
extern crate uuid;

mod settings;
mod store;

use actix::{Addr, System};
use actix_redis::{RedisActor};
use actix_web::{
    error::{ErrorInternalServerError},
    http::{Method},
    middleware::{Logger},
    server,
    App,
    AsyncResponder,
    Either,
    FutureResponse,
    HttpRequest,
    HttpResponse,
    Json,
    Path,
    Responder,
};
use futures::Future;
use rand::OsRng;
use settings::Settings;
use store::{
    FbQueryResponse,
    GetTokenResult,
    get_auth_info,
    get_fb_identity,
    try_add_account,
    try_get_token
};

#[derive(Deserialize)]
struct AuthRequest {
    login: String,
    secret: String,
}

#[derive(Serialize)]
struct AuthResponse(GetTokenResult);

#[derive(Deserialize)]
struct SignupRequest {
    login: String,
    secret: String,
}

#[derive(Deserialize)]
struct AuthRequestFb {
    fb_token: String,
}

#[derive(Deserialize)]
struct SignupRequestFb {
    fb_token: String,
}

#[derive(Serialize)]
enum SignupResponse {
    Ok,
    UserAlreadyExists(String),
}

struct AppState {
    redis: Addr<RedisActor>,
    rng: OsRng,
}

fn signin((body, req): (Json<AuthRequest>, HttpRequest<AppState>))
    -> FutureResponse<impl Responder> {
    
    let redis = req.state().redis.clone();

    let AuthRequest { login, secret } = body.into_inner();

    try_get_token(redis, &login, &secret)
        .map(|token_result| Json(AuthResponse(token_result)))
        .map_err(ErrorInternalServerError)
        .responder()
}

fn signup((body, req): (Json<SignupRequest>, HttpRequest<AppState>))
    -> FutureResponse<impl Responder> {

    let redis = req.state().redis.clone();
    let mut rng = req.state().rng.clone();

    let SignupRequest { login, secret } = body.into_inner();

    try_add_account(redis, &mut rng, &login, &secret)
        .map(|was_added|
            match was_added {
                false => Json(SignupResponse::UserAlreadyExists(login)),
                true => Json(SignupResponse::Ok),
            })
        .map_err(ErrorInternalServerError)
        .responder()
}

fn signin_fb((body, req): (Json<AuthRequestFb>, HttpRequest<AppState>))
    -> impl Responder {
    
    let redis = req.state().redis.clone();

    let AuthRequestFb { fb_token } = body.into_inner();
    let fb_id_result = get_fb_identity(&fb_token);

    match fb_id_result {
        None => Either::A(
            HttpResponse::NotFound()
        ),
        Some(FbQueryResponse { id }) => Either::B(
            try_get_token(redis, &id, "")
                .map(|token_result| Json(AuthResponse(token_result)))
                .map_err(ErrorInternalServerError)
                .responder()
        ),
    }
}

fn signup_fb((body, req): (Json<SignupRequestFb>, HttpRequest<AppState>))
    -> impl Responder {

    let redis = req.state().redis.clone();
    let mut rng = req.state().rng.clone();

    let SignupRequestFb { fb_token } = body.into_inner();
    let fb_id_result = get_fb_identity(&fb_token);

    match fb_id_result {
        None => Either::A(
            HttpResponse::NotFound()
        ),
        Some(FbQueryResponse { id }) => Either::B(
            try_add_account(redis, &mut rng, &id, "")
                .map(|was_added|
                    match was_added {
                        false => Json(SignupResponse::UserAlreadyExists(id)),
                        true => Json(SignupResponse::Ok),
                    })
                .map_err(ErrorInternalServerError)
                .responder()
        ),
    }
}

fn test_auth((auth_token, req): (Path<String>, HttpRequest<AppState>))
    -> impl Responder {

    let redis = req.state().redis.clone();

    get_auth_info(redis, &auth_token)
        .map(|info|
            match info {
                None => Either::A(HttpResponse::NotFound()),
                Some(login) => Either::B(login),
            })
        .map_err(|_| ErrorInternalServerError(""))
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
            .resource("/signin", |r| r.method(Method::POST).with(signin))
            .resource("/signup", |r| r.method(Method::POST).with(signup))
            .resource("/fb/signin", |r| r.method(Method::POST).with(signin_fb))
            .resource("/fb/signup", |r| r.method(Method::POST).with(signup_fb))
            .resource("/test_auth/{auth_token}", |r| r.method(Method::GET).with(test_auth))
    })
        .bind(listen_addr)
        .unwrap()
        .start();

    sys.run();
}
