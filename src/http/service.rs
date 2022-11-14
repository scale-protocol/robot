use crate::bot::{self, machine::UserDynamicData};
use crate::bot::{machine, storage};
use crate::com::CliError;
use anchor_client::solana_sdk::{account::Account, pubkey::Pubkey};
use log::*;

use bond::state::{position, user};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::str::FromStr;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub account: user::UserAccount,
    pub pubkey: Pubkey,
    pub dynamic_data: Option<UserDynamicData>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub account: position::Position,
    pub pubkey: Pubkey,
}

pub fn get_user_info(
    pubkey: String,
    mp: bot::machine::SharedStateMap,
) -> anyhow::Result<Option<UserInfo>> {
    let pubkey =
        Pubkey::try_from(pubkey.as_str()).map_err(|e| CliError::HttpServerError(e.to_string()))?;
    let rs = match mp.user.get(&pubkey) {
        Some(user) => {
            let data = match mp.user_dynamic_idx.get(&pubkey) {
                Some(d) => Some((*d.value()).clone()),
                None => None,
            };
            let user_info = UserInfo {
                account: (*user.value()).clone(),
                dynamic_data: data,
                pubkey,
            };
            Some(user_info)
        }
        None => None,
    };
    Ok(rs)
}

pub fn get_position_list(
    mp: machine::SharedStateMap,
    prefix: String,
    pubkey: String,
) -> anyhow::Result<Vec<PositionInfo>> {
    let pubkey =
        Pubkey::try_from(pubkey.as_str()).map_err(|e| CliError::HttpServerError(e.to_string()))?;
    let prefix = storage::Prefix::from_str(prefix.as_str())?;
    let mut rs: Vec<PositionInfo> = Vec::new();
    match prefix {
        storage::Prefix::Active => {
            let r = mp.position.get(&pubkey);
            match r {
                Some(p) => {
                    for v in p.value() {
                        rs.push(PositionInfo {
                            account: (*v.value()).clone(),
                            pubkey: *v.key(),
                        });
                    }
                }
                None => {}
            }
        }
        storage::Prefix::History => {
            let items = mp.storage.get_position_history_list(&pubkey);
            for i in items {
                match i {
                    Ok((k, v)) => {
                        let key = String::from_utf8(k.to_vec())
                            .map_err(|e| CliError::JsonError(e.to_string()))?;
                        let keys = storage::Keys::from_str(key.as_str())?;
                        let pk = keys.get_end();
                        let pbk = Pubkey::try_from(pk.as_str())
                            .map_err(|e| CliError::Unknown(e.to_string()))?;
                        let values: Account = serde_json::from_slice(v.to_vec().as_slice())
                            .map_err(|e| CliError::JsonError(e.to_string()))?;
                        let s: machine::State = (&values).into();
                        match s {
                            machine::State::Position(m) => {
                                rs.push(PositionInfo {
                                    account: m,
                                    pubkey: pbk,
                                });
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        error!("{}", e);
                    }
                }
            }
        }
        storage::Prefix::None => {}
    }
    Ok(rs)
}
