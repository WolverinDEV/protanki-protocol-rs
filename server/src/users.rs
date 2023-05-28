use std::time::Duration;

use futures::Future;

#[derive(Debug)]
pub enum AuthenticationResult {
    Success { user_id: String },
    InvalidCredentials,
    InvalidToken,
    /* BanTemporary */
    /* BanPermanent */
}

pub struct UserRegistry { }

impl UserRegistry {
    pub fn new() -> Self {
        Self {}
    }
}

impl UserRegistry {
    pub fn authenticate_with_credentials(&self, username: String, password: String) -> impl Future<Output = AuthenticationResult> {
        async move { 
            if username == "WolverinDEV" && password == "markus" {
                AuthenticationResult::Success { user_id: username }
            } else {
                AuthenticationResult::InvalidCredentials
            }
         }
    }

    pub fn authenticate_with_token(&self, token: String) -> impl Future<Output = AuthenticationResult> {
        async move { 
            if token == "this-is-one-of-a-token-wolverindev" {
                AuthenticationResult::Success { user_id: "WolverinDEV".to_string() }
            } else {
                AuthenticationResult::InvalidToken
            }
         }
    }

    pub fn create_authentication_token(&self, user: String) -> impl Future<Output = String> {
        async {
            "this-is-one-of-a-token".to_string()
        }
    }

    pub fn validate_username(&self, username: String) -> impl Future<Output = bool> {
        async move {
            if username.starts_with("m") {
                true
            } else {
                false
            }
        }
    }

    pub fn register_user(&mut self, username: String, password: String) -> impl Future<Output = bool> {
        async move {
            username.starts_with("ma")
        }
    }
}