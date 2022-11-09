use crate::com;
use anchor_client::{self, Cluster};
use anyhow::Ok;
use home;
use log::debug;
use std::{fs, path::PathBuf, str::FromStr};
extern crate serde;
extern crate serde_yaml;
use anchor_client::solana_sdk::pubkey::Pubkey;
use bond::com as bcom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const PYTH_PROGRAM_DEVNET: &str = "gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s";
const PYTH_PROGRAM_TESTNET: &str = "8tfDNiaEyrV6Q1U4DEXrEigs9DoDtkugzFbybENEbCDz";
const PYTH_PROGRAM_MAINNET: &str = "FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH";

const SPL_MINT_DEVNET: &str = "5Uzq44UgPkPNxG4E4m4m7F8fsnrHKc4jFvFuPapV4jN2";
const SPL_MINT_TESTNET: &str = "6jTrKM4mobEdWyW3VC2XcDadXTVZ7x9qMfCmF4ZVbSgq";
const SPL_MINT_MAINNET: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
#[derive(Debug, Clone)]
pub struct Config {
    pub config_file: PathBuf,
    pub cluster: anchor_client::Cluster,
    pub wallet: PathBuf,
    pub store_path: PathBuf,
    pub accounts: Accounts,
    pub keypair: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigBody {
    pub rpc_url: String,
    pub ws_url: String,
    pub keypair_path: String,
    pub cluster: String,
    pub store_path: String,
    pub accounts: Accounts,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Accounts {
    pub pyth: HashMap<String, Pubkey>,
    pub chainlink: HashMap<String, Pubkey>,
    pub spl_mint: Pubkey,
    pub pyth_program_pubkey: Pubkey,
}
impl From<&Config> for ConfigBody {
    fn from(c: &Config) -> Self {
        Self {
            rpc_url: c.cluster.url().to_string(),
            ws_url: c.cluster.ws_url().to_string(),
            keypair_path: c.wallet.to_str().unwrap().to_string(),
            cluster: c.cluster.to_string(),
            store_path: c.store_path.to_str().unwrap().to_string(),
            accounts: c.accounts.clone(),
        }
    }
}

impl From<&ConfigBody> for Config {
    fn from(c: &ConfigBody) -> Self {
        let config = Config::default();
        let wallet = PathBuf::from(c.keypair_path.clone());
        let keypair = fs::read(&wallet).expect("Cannot read keypar from local.");
        Self {
            config_file: config.config_file,
            cluster: match c.cluster.as_str() {
                "debug" => Cluster::Debug,
                "devnet" => Cluster::Devnet,
                "localnet" => Cluster::Localnet,
                "testnet" => Cluster::Testnet,
                "mainnet" => Cluster::Mainnet,
                _ => Cluster::Custom(c.rpc_url.clone(), c.ws_url.clone()),
            },
            wallet,
            store_path: PathBuf::from(c.store_path.clone()),
            accounts: c.accounts.clone(),
            keypair,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let home_dir = match home::home_dir() {
            Some(p) => p,
            None => PathBuf::from("/tmp/"),
        };
        let home_dir = home_dir.join(".scale");
        if !home_dir.is_dir() {
            fs::create_dir(&home_dir).unwrap();
        }
        let mut pyth = HashMap::new();
        pyth.insert(
            bcom::FullPositionMarket::BtcUsd.to_string(),
            Pubkey::try_from("HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J").unwrap(),
        );
        pyth.insert(
            bcom::FullPositionMarket::EthUsd.to_string(),
            Pubkey::try_from("EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw").unwrap(),
        );
        pyth.insert(
            bcom::FullPositionMarket::SolUsd.to_string(),
            Pubkey::try_from("J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix").unwrap(),
        );
        let mut chainlink = HashMap::new();
        chainlink.insert(
            bcom::FullPositionMarket::BtcUsd.to_string(),
            Pubkey::try_from("CzZQBrJCLqjXRfMjRN3fhbxur2QYHUzkpaRwkWsiPqbz").unwrap(),
        );
        chainlink.insert(
            bcom::FullPositionMarket::EthUsd.to_string(),
            Pubkey::try_from("2ypeVyYnZaW2TNYXXTaZq9YhYvnqcjCiifW1C6n8b7Go").unwrap(),
        );
        chainlink.insert(
            bcom::FullPositionMarket::SolUsd.to_string(),
            Pubkey::try_from("HgTtcbcmp5BeThax5AU8vg4VwK79qAvAKKFMs8txMLW6").unwrap(),
        );
        Config {
            config_file: home_dir.join("config.yml"),
            cluster: Cluster::Localnet,
            wallet: home_dir.join("id.json"),
            store_path: home_dir.join("data"),

            accounts: Accounts {
                pyth,
                chainlink,
                spl_mint: Pubkey::try_from(SPL_MINT_DEVNET).unwrap(),
                pyth_program_pubkey: Pubkey::try_from(PYTH_PROGRAM_DEVNET).unwrap(),
            },
            keypair: vec![],
        }
    }
}
impl Config {
    pub fn init(&self) {
        let config_body: ConfigBody = self.into();
        // save
        debug!("init config file:{:?}", self.config_file);
        fs::write(
            &self.config_file.clone(),
            serde_yaml::to_string(&config_body).unwrap().as_bytes(),
        )
        .unwrap()
    }
    pub fn get(&self) {
        println!(
            r#"Config File : {:?}
Cluster : {}
Wallet keypair file : {:?}
Local store path : {:?}
Rpc url : {}
Ws url : {}
pyth  program account: {} "#,
            self.config_file,
            self.cluster,
            self.wallet,
            self.store_path,
            self.cluster.url(),
            self.cluster.ws_url(),
            self.accounts.pyth_program_pubkey,
        );
    }
    pub fn get_pyth_btc_pubkey(&self) -> &Pubkey {
        let p = self
            .accounts
            .pyth
            .get(&bcom::FullPositionMarket::BtcUsd.to_string())
            .unwrap();
        p
    }
    pub fn get_pyth_eth_pubkey(&self) -> &Pubkey {
        let p = self
            .accounts
            .pyth
            .get(&bcom::FullPositionMarket::EthUsd.to_string())
            .unwrap();
        p
    }
    pub fn get_pyth_sol_pubkey(&self) -> &Pubkey {
        let p = self
            .accounts
            .pyth
            .get(&bcom::FullPositionMarket::SolUsd.to_string())
            .unwrap();
        p
    }
    pub fn set(
        &mut self,
        store_path: Option<&PathBuf>,
        keypair_file: Option<&PathBuf>,
        rpc_url: Option<&String>,
        ws_url: Option<&String>,
        cluster: Option<&String>,
    ) {
        match store_path {
            Some(s) => self.store_path = s.to_path_buf(),
            None => {}
        }
        match keypair_file {
            Some(k) => self.wallet = k.to_path_buf(),
            None => {}
        }
        match cluster {
            Some(c) => {
                self.cluster = Cluster::from_str(c.as_str()).unwrap();
                match self.cluster {
                    Cluster::Testnet => {
                        let mut pyth = HashMap::new();
                        pyth.insert(
                            bcom::FullPositionMarket::BtcUsd.to_string(),
                            Pubkey::try_from("DJW6f4ZVqCnpYNN9rNuzqUcCvkVtBgixo8mq9FKSsCbJ")
                                .unwrap(),
                        );
                        pyth.insert(
                            bcom::FullPositionMarket::EthUsd.to_string(),
                            Pubkey::try_from("7A98y76fcETLHnkCxjmnUrsuNrbUae7asy4TiVeGqLSs")
                                .unwrap(),
                        );
                        pyth.insert(
                            bcom::FullPositionMarket::SolUsd.to_string(),
                            Pubkey::try_from("7VJsBtJzgTftYzEeooSDYyjKXvYRWJHdwvbwfBvTg9K")
                                .unwrap(),
                        );
                        let chainlink = HashMap::new();
                        self.accounts = Accounts {
                            pyth,
                            chainlink,
                            spl_mint: Pubkey::try_from(SPL_MINT_TESTNET).unwrap(),
                            pyth_program_pubkey: Pubkey::try_from(PYTH_PROGRAM_TESTNET).unwrap(),
                        }
                    }
                    Cluster::Mainnet => {
                        let mut pyth = HashMap::new();
                        pyth.insert(
                            bcom::FullPositionMarket::BtcUsd.to_string(),
                            Pubkey::try_from("GVXRSBjFk6e6J3NbVPXohDJetcTjaeeuykUpbQF8UoMU")
                                .unwrap(),
                        );
                        pyth.insert(
                            bcom::FullPositionMarket::EthUsd.to_string(),
                            Pubkey::try_from("JBu1AL4obBcCMqKBBxhpWCNUt136ijcuMZLFvTP7iWdB")
                                .unwrap(),
                        );
                        pyth.insert(
                            bcom::FullPositionMarket::SolUsd.to_string(),
                            Pubkey::try_from("H6ARHf6YXhGYeQfUzQNGk6rDNnLBQKrenN712K4AQJEG")
                                .unwrap(),
                        );
                        let mut chainlink = HashMap::new();
                        chainlink.insert(
                            bcom::FullPositionMarket::BtcUsd.to_string(),
                            Pubkey::try_from("CGmWwBNsTRDENT5gmVZzRu38GnNnMm1K5C3sFiUUyYQX")
                                .unwrap(),
                        );
                        chainlink.insert(
                            bcom::FullPositionMarket::EthUsd.to_string(),
                            Pubkey::try_from("5WyTBrEgvkAXjTdYTLY9PVrztjmz4edP5W9wks9KPFg5")
                                .unwrap(),
                        );
                        chainlink.insert(
                            bcom::FullPositionMarket::SolUsd.to_string(),
                            Pubkey::try_from("CcPVS9bqyXbD9cLnTbhhHazLsrua8QMFUHTutPtjyDzq")
                                .unwrap(),
                        );
                        self.accounts = Accounts {
                            pyth,
                            chainlink,
                            spl_mint: Pubkey::try_from(SPL_MINT_MAINNET).unwrap(),
                            pyth_program_pubkey: Pubkey::try_from(PYTH_PROGRAM_MAINNET).unwrap(),
                        }
                    }
                    _ => {
                        let c = Self::default();
                        self.accounts = c.accounts.clone();
                    }
                }
            }
            None => {}
        }
        match rpc_url {
            Some(r) => {
                self.cluster = Cluster::Custom(r.to_string(), self.cluster.ws_url().to_string());
            }
            None => {}
        }
        match ws_url {
            Some(r) => {
                self.cluster = Cluster::Custom(self.cluster.url().to_string(), r.to_string());
            }
            None => {}
        }
        let config = &(*self);
        let config_body: ConfigBody = config.into();
        // save
        debug!("init config file:{:?}", self.config_file);
        fs::write(
            &self.config_file.clone(),
            serde_yaml::to_string(&config_body).unwrap().as_bytes(),
        )
        .unwrap();
        self.get();
    }
    // load config from local file
    pub fn load(&mut self) -> anyhow::Result<()> {
        let config_body: ConfigBody = serde_yaml::from_str(
            fs::read_to_string(&self.config_file)
                .map_err(|e| com::CliError::LoadConfigFileError(e.to_string()))?
                .as_str(),
        )
        .map_err(|e| com::CliError::LoadConfigFileError(e.to_string()))?;
        let s: Config = (&config_body).into();
        self.config_file = s.config_file;
        self.cluster = s.cluster;
        self.store_path = s.store_path;
        self.wallet = s.wallet;
        self.keypair = s.keypair;
        Ok(())
    }
}
