use actix::{Addr, MailboxError};
use actix_redis::{Command, Error as ActixError, RedisActor};
use futures::{Future, future, future::{Either, FutureResult}};
use rand::{Rng, RngCore};
use redis_async::{resp::RespValue};
use serde_json;
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub struct Account {
    pub login: String,
    pub secret: Secret,
}

#[derive(Deserialize, Serialize)]
pub struct Secret {
    pub hash: Vec<u8>,
    pub salt: Vec<u8>,
}

impl Secret {
    pub fn encode(rng: &mut RngCore, secret: &str) -> Secret {
        let salt: Vec<u8> = rng.gen::<[u8; 32]>().as_ref().into();

        let mut hasher = Sha256::new();
        hasher.input(secret.as_bytes());
        hasher.input(&salt);

        let hash: Vec<u8> = hasher.result().as_ref().into();

        Secret { hash, salt }
    }

    pub fn contains(&self, secret: &str) -> bool {
        let mut hasher = Sha256::new();
        hasher.input(secret.as_bytes());
        hasher.input(&self.salt);

        self.hash == hasher.result().as_ref()
    }
}

#[derive(Serialize)]
pub enum GetTokenResult {
    InvalidCredentials(String),
    Token(String)
}

#[derive(Debug)]
pub enum GetTokenError {
    ActixError(ActixError),
    DeserializationError(serde_json::Error),
    MailboxError(MailboxError),
    UnexpectedResp(RespValue)
}

impl fmt::Display for GetTokenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GetTokenError::ActixError(actix_err) => actix_err.fmt(f),
            GetTokenError::DeserializationError(serde_err) => serde_err.fmt(f),
            GetTokenError::MailboxError(mb_err) => mb_err.fmt(f),
            GetTokenError::UnexpectedResp(resp) => write!(f, "Unexpected response from Redis: {:?}", resp),
        }
    }
}

#[derive(Debug)]
pub enum AddAccountError {
    ActixError(ActixError),
    SerializationError(serde_json::Error),
    MailboxError(MailboxError),
    UnexpectedResp(RespValue)
}

impl fmt::Display for AddAccountError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AddAccountError::ActixError(actix_err) => actix_err.fmt(f),
            AddAccountError::SerializationError(serde_err) => serde_err.fmt(f),
            AddAccountError::MailboxError(mb_err) => mb_err.fmt(f),
            AddAccountError::UnexpectedResp(resp) => write!(f, "Unexpected response from Redis: {:?}", resp),
        }
    }
}

pub fn try_add_account(redis: Addr<RedisActor>, rng: &mut RngCore, login: &str, secret: &str)
    -> impl Future<Item = bool, Error = AddAccountError> {
    
    let account = Account {
        login: login.into(),
        secret: Secret::encode(rng, &secret),
    };

    let key = get_redis_account_key(login);
    let json_result: FutureResult<String, _> =
        serde_json::to_string(&account)
            .map_err(AddAccountError::SerializationError)
            .into();

    json_result
        .and_then(move |json| {
            let command = Command(resp_array!["SETNX", key, json]);
            redis.send(command).map_err(AddAccountError::MailboxError)
        })
        .and_then(|resp_value|
            match resp_value {
                Ok(RespValue::Integer(1)) => Ok(true),
                Ok(RespValue::Integer(0)) => Ok(false),
                Ok(resp) => Err(AddAccountError::UnexpectedResp(resp)),
                Err(e) => Err(AddAccountError::ActixError(e)),
            })
}

pub fn try_get_token(redis: Addr<RedisActor>, login: &str, secret: &str)
    -> impl Future<Item = GetTokenResult, Error = GetTokenError> {
    
    let account_key = get_redis_account_key(login);
    let command = Command(resp_array!["GET", account_key]);

    let (login, secret) = (login.to_string(), secret.to_string());
    
    redis.send(command)
        .map_err(GetTokenError::MailboxError)
        .and_then(move |resp_value| {
            let fut: Box<Future<Item = _, Error = _>> =
                match resp_value {
                    Ok(RespValue::Nil) => Box::new(
                        future::ok(GetTokenResult::InvalidCredentials(login.into()))
                    ),
                    Ok(RespValue::BulkString(json)) => Box::new(
                        match serde_json::from_slice(&json) {
                            Err(e) => Either::A(
                                future::err(GetTokenError::DeserializationError(e))
                            ),
                            Ok(account) => Either::B(
                                get_or_update_token(redis.clone(), &account, &secret)
                            ),
                        }
                    ),
                    Ok(resp) => Box::new(
                        future::err(GetTokenError::UnexpectedResp(resp))
                    ),
                    Err(e) => Box::new(
                        future::err(GetTokenError::ActixError(e))
                    ),
                };
            fut
        })
}

fn get_or_update_token(redis: Addr<RedisActor>, account: &Account, expected_secret: &str)
    -> impl Future<Item = GetTokenResult, Error = GetTokenError> {
    let Account { login, secret } = account;

    match secret.contains(expected_secret) {
        false => future::Either::A(
            future::ok(GetTokenResult::InvalidCredentials(login.to_string()))
        ),
        true => {
            let token_key = get_redis_token_key(login);
            let token_value = Uuid::new_v4().hyphenated().to_string();
            let command = Command(resp_array!["SETEX", token_key, "60", token_value.clone()]);

            let fut = redis.send(command)
                .map_err(GetTokenError::MailboxError)
                .and_then(move |resp_value|
                    match resp_value {
                        Ok(RespValue::SimpleString(_)) => Ok(GetTokenResult::Token(token_value)),
                        Ok(resp) => Err(GetTokenError::UnexpectedResp(resp)),
                        Err(e) => Err(GetTokenError::ActixError(e)),
                    });
                    
            future::Either::B(fut)
        }
    }
}

fn get_redis_account_key(login: &str) -> String {
    format!("accounts:{}", login)
}

fn get_redis_token_key(login: &str) -> String {
    format!("tokens:{}", login)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{prng::hc128::Hc128Rng, SeedableRng};

    #[test]
    fn test_secret_should_match() {
        let seed = [0; 32];
        let mut rng = Hc128Rng::from_seed(seed);
        
        let password = String::from("hunter2");
        let secret = Secret::encode(&mut rng, &password);
        assert!(secret.contains(&password));
    }

    #[test]
    fn test_secret_should_not_match() {
        let seed = [0; 32];
        let mut rng = Hc128Rng::from_seed(seed);
        
        let password = String::from("hunter2");
        let secret = Secret::encode(&mut rng, &password);
        assert!(!secret.contains("qwerty"));
    }
}