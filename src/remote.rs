use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};

use crate::model::{ObjectId, RemoteConfig, SnapRecord};
use crate::store::LocalStore;

mod http_client;
use self::http_client::with_retries;

mod types;
pub use self::types::*;
mod fetch;
mod identity;
mod operations;
mod transfer;

pub struct RemoteClient {
    remote: RemoteConfig,
    token: String,
    client: reqwest::blocking::Client,
}

impl RemoteClient {
    pub fn new(remote: RemoteConfig, token: String) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("converge")
            .build()
            .context("build reqwest client")?;
        Ok(Self {
            remote,
            token,
            client,
        })
    }

    pub fn remote(&self) -> &RemoteConfig {
        &self.remote
    }
}
