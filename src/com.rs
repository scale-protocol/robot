use std::rc::Rc;

use anchor_client::solana_sdk::pubkey::Pubkey;
use thiserror::Error;

use crate::config;
use anchor_client::solana_sdk::commitment_config::CommitmentConfig;
use anchor_client::solana_sdk::signature;
use std::io::Cursor;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Unknown error: {0}")]
    Unknown(String),
    #[error("Can not load config file from local:{0}")]
    LoadConfigFileError(String),
    #[error("Subscription account failed: {0}")]
    SubscriptionAccountFailed(String),
    #[error("Can not create tokio runtime: {0}")]
    TokioRuntimeCreateField(String),
    #[error("Can not create local db:{0}")]
    DBError(String),
    #[error("Error in json parsing:{0}")]
    JsonError(String),
    #[error("deserialize error:{0}")]
    DeserializeError(String),
    #[error("get price error{0}")]
    PriceError(String),
    #[error("new wallet keypair error:{0}")]
    KeypairError(String),
    #[error("Http server error:{0}")]
    HttpServerError(String),
}
pub fn id() -> Pubkey {
    Pubkey::try_from("ECte5vr5zJkRVnEPY9XPkgq3JFfFkthrMKxLk6gfa7v4").unwrap()
}
pub struct Context<'a> {
    pub config: &'a config::Config,
    pub client: &'a anchor_client::Client,
}

impl<'a> Context<'a> {
    pub fn new(config: &'a config::Config, client: &'a anchor_client::Client) -> Self {
        Self { config, client }
    }

    pub fn new_client(c: &'a config::Config) -> anyhow::Result<anchor_client::Client> {
        let mut buff = Cursor::new(c.keypair.clone());
        let kp = signature::read_keypair(&mut buff)
            .map_err(|e| CliError::KeypairError(e.to_string()))?;
        Ok(anchor_client::Client::new_with_options(
            c.cluster.clone(),
            Rc::new(kp),
            CommitmentConfig::processed(),
        ))
    }
}
pub fn f64_round(f: f64) -> f64 {
    (f * 100.0).round() / 100.0
}
