use std::{time::Duration, sync::{Arc, Mutex}, pin::Pin};

use chrono::Utc;
use futures::{Future, TryFutureExt};
use futures::FutureExt;
use sha2::{ Sha256, Digest };
use rand::{distributions::Alphanumeric, Rng};
use sqlx::Connection;

use crate::server::{DatabaseConnection, DatabaseHandle};

pub mod database {
    use sqlx::FromRow;

    #[derive(Clone, FromRow, Debug)]
    pub struct User {
        pub user_id: String,

        pub email: Option<String>,
        pub email_confirmed: bool,
        
        pub timestamp_register: chrono::DateTime<chrono::Utc>,
        pub timestamp_active: chrono::DateTime<chrono::Utc>,

        pub crystals: u32,
        pub experience: u32,
        pub premium: Option<chrono::DateTime<chrono::Utc>>,
    }

    #[derive(Clone, FromRow, Debug)]
    pub struct UserAuthentication {
        pub user_id: String,
        pub login_user: String,
        pub password_hash: String,
        pub password_salt: String,
    }

    #[derive(Clone, FromRow, Debug)]
    pub struct UserAuthenticationToken {
        pub user_id: String,
        pub timestamp_created: chrono::DateTime<chrono::Utc>,
        pub timestamp_last_used: chrono::DateTime<chrono::Utc>,
        pub token: String,
    }
}

#[derive(Debug)]
pub enum AuthenticationResult {
    Success { user_id: String },
    InvalidCredentials,
    InvalidToken,
    /* BanTemporary */
    /* BanPermanent */
}

pub struct UserRegistry {
    database: DatabaseHandle
}

impl UserRegistry {
    pub fn new(database: DatabaseHandle) -> Self {
        Self {
            database
        }
    }
}

fn error_log_fallback<T>(message: &'static str, fallback: T) -> impl FnOnce(anyhow::Result<T>) -> T {
    move |result | {
        match result {
            Ok(result) => result,
            Err(error) => {
                tracing::error!("{}: {}", message, error);
                fallback
            }
        }
    }
}

pub enum Result<T, E> {
    Ok(T),
    Err(E),
}


impl UserRegistry {
    fn query_database<R: 'static, F>(&self, callback: impl FnOnce(&mut DatabaseConnection) -> F) -> impl Future<Output = anyhow::Result<R>>
    where
        F: Future<Output = anyhow::Result<R>>
    {
        let database = self.database.clone();
        async move {
            let mut database = database.lock().await;
            callback(&mut *database).await;
            drop(database);
            anyhow::bail!("XXX");
            // let result = {
            //     (callback)(&mut *database).await
            // };
            // result
        }
    }

    fn hash_password(password: &str, salt: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(password.as_bytes());
        hasher.update(salt.as_bytes());
        hex::encode(&hasher.finalize())
    }

    pub fn authenticate_with_credentials(&self, username: String, password: String) -> impl Future<Output = AuthenticationResult> {
        let database = self.database.clone();
        async move {
            let mut database = database.lock().await;
            let user_authentication = sqlx::query_as::<_, database::UserAuthentication>(
                "SELECT * FROM `user_authentication` WHERE `login_user` = $1"
            )
                .bind(&username)
                .fetch_optional(&mut *database).await?;
            let user_authentication = if let Some(auth) = user_authentication {
                auth
            } else {
                return anyhow::Ok(AuthenticationResult::InvalidCredentials);
            };

            let hashed_password = Self::hash_password(&password, &user_authentication.password_salt);
            anyhow::Ok(if hashed_password == user_authentication.password_hash {
                AuthenticationResult::Success { user_id: username }
            } else {
                AuthenticationResult::InvalidCredentials
            })
        }
        .unwrap_or_else(|err| {
            tracing::error!("failed to authenticate user via credentials: {}", err);
            AuthenticationResult::InvalidCredentials
        })
    }

    pub fn authenticate_with_token(&self, token: String) -> impl Future<Output = AuthenticationResult> {
        let database = self.database.clone();
        async move {
            let mut database = database.lock().await;
            let user_token = sqlx::query_as::<_, database::UserAuthenticationToken>("UPDATE `user_authentication_token` SET `timestamp_last_used` = $1 WHERE `token` = $2 RETURNING *;")
                .bind(Utc::now())
                .bind(token)
                .fetch_optional(&mut *database)
                .await?;

            anyhow::Ok(match user_token {
                Some(token) => AuthenticationResult::Success { user_id: token.user_id },
                None => AuthenticationResult::InvalidToken,
            })
        }
        .unwrap_or_else(|err| {
            tracing::error!("failed to authenticate user via token: {}", err);
            AuthenticationResult::InvalidToken
        })
    }

    pub fn create_authentication_token(&self, user: String) -> impl Future<Output = Option<String>> {
        let database = self.database.clone();
        async move {
            let token = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(64)
                .map(char::from)
                .collect::<String>();
            let now = Utc::now();

            let mut database = database.lock().await;
            sqlx::query("INSERT INTO `user_authentication_token`(`user_id`, `timestamp_created`, `timestamp_last_used`, `token`) VALUES ($1, $2, $2, $3)")
                .bind(&user)
                .bind(&now)
                .bind(&token)
                .execute(&mut *database)
                .await?;

            anyhow::Ok(Some(token))
        }
        .unwrap_or_else(|err| {
            tracing::error!("failed to create user token: {}", err);
            None
        })
    }

    pub fn is_username_free(&self, username: String) -> impl Future<Output = bool> {
        let database = self.database.clone();
        async move {
            let mut database = database.lock().await;
            let user_found = sqlx::query("SELECT 1 FROM `user` WHERE `user_id` = $1")
                .bind(&username)
                .fetch_optional(&mut *database)
                .await?
                .is_none();

            anyhow::Ok(user_found)
        }
        .unwrap_or_else(|err| {
            tracing::error!("failed query if username already exists: {}", err);
            false // prevent double user insertion by default
        })
    }

    pub fn register_user(&mut self, username: String, password: String) -> impl Future<Output = bool> {
        let database = self.database.clone();
        async move {
            let password_salt = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(16)
                .map(char::from)
                .collect::<String>();

            let hashed_password = Self::hash_password(&password, &password_salt);

            let mut database = database.lock().await;
            let mut tx = database.begin().await?;

            sqlx::query(
                "INSERT INTO `user`(`user_id`, `timestamp_register`, `timestamp_active`, `crystals`, `experience`, `premium`)
                VALUES ($1, $2, $2, $3, $4, $5)"
            )
                .bind(&username)
                .bind(&Utc::now())
                .bind(0)
                .bind(0)
                .bind(&None::<chrono::DateTime<Utc>>)
                .execute(&mut tx)
                .await?;

            sqlx::query(
                "INSERT INTO `user_authentication`(`user_id`, `login_user`, `password_hash`, `password_salt`)
                VALUES ($1, $2, $3, $4)"
            )
                .bind(&username)
                .bind(&username)
                .bind(&hashed_password)
                .bind(&password_salt)
                .execute(&mut tx)
                .await?;

            tx.commit().await?;
            anyhow::Ok(true)
        }.unwrap_or_else(|err| {
            tracing::error!("failed to create new user: {}", err);
            false
        })
    }

    pub fn find_user(&self, user_id: String) -> impl Future<Output = Option<database::User>> {
        let database = self.database.clone();
        async move {
            let mut database = database.lock().await;
            let result = sqlx::query_as::<_, database::User>("SELECT * FROM `user` WHERE `user_id` = $1")
                .bind(&user_id)
                .fetch_optional(&mut *database)
                .await?;

            anyhow::Ok(result)
        }.unwrap_or_else(|err| {
            tracing::error!("failed to find user: {}", err);
            None
        })
    }
}