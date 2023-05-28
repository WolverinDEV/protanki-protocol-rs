use std::time::Duration;

use futures::Future;

pub struct UserRegistry {

}

impl UserRegistry {
    pub fn new() -> Self {
        Self {}
    }
}

impl UserRegistry {
    pub fn authenticate_with_credentials(&self, username: String, password: String) -> impl Future<Output = bool> {
        async { false }
    }

    pub fn authenticate_with_token(&self, token: String) -> impl Future<Output = bool> {
        async { false }
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