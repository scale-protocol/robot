use crate::bot::{
    self,
    machine::{PositionDynamicData, UserDynamicData},
};
use crate::bot::{machine, storage};
use crate::com::CliError;
use anchor_client::solana_sdk::{account::Account, pubkey::Pubkey};
use bond::com as bcom;
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
    pub dynamic_data: Option<PositionDynamicData>,
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
                Some(d) => {
                    let mut dynamic_data = machine::UserDynamicData::default();
                    dynamic_data.equity = bcom::f64_round(d.value().equity / bcom::DECIMALS);
                    dynamic_data.margin_percentage = bcom::f64_round(d.value().margin_percentage);
                    dynamic_data.profit = bcom::f64_round(d.value().profit / bcom::DECIMALS);
                    dynamic_data.profit_rate = bcom::f64_round(d.value().profit_rate);
                    Some(dynamic_data)
                }
                None => None,
            };
            let mut user_account = (*user.value()).clone();
            user_account.margin_total = bcom::f64_round(user_account.margin_total / bcom::DECIMALS);
            user_account.balance = bcom::f64_round(user_account.balance / bcom::DECIMALS);
            user_account.margin_full_buy_total =
                bcom::f64_round(user_account.margin_full_buy_total / bcom::DECIMALS);
            user_account.margin_full_sell_total =
                bcom::f64_round(user_account.margin_full_sell_total / bcom::DECIMALS);
            user_account.margin_full_total =
                bcom::f64_round(user_account.margin_full_total / bcom::DECIMALS);
            user_account.margin_independent_buy_total = bcom::f64_round(
                f64::from(user_account.margin_independent_buy_total) / bcom::DECIMALS,
            );
            user_account.margin_independent_sell_total = bcom::f64_round(
                f64::from(user_account.margin_independent_sell_total) / bcom::DECIMALS,
            );
            user_account.margin_independent_total =
                bcom::f64_round(f64::from(user_account.margin_independent_total) / bcom::DECIMALS);
            let user_info = UserInfo {
                account: user_account,
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
                        let mut p = (*v.value()).clone();
                        p.open_price = bcom::f64_round(p.open_price / bcom::DECIMALS);
                        p.open_real_price = bcom::f64_round(p.open_real_price / bcom::DECIMALS);
                        if p.position_status == position::PositionStatus::ForceClosing
                            || p.position_status == position::PositionStatus::NormalClosing
                        {
                            p.close_price = bcom::f64_round(p.close_price / bcom::DECIMALS);
                            p.close_real_price =
                                bcom::f64_round(p.close_real_price / bcom::DECIMALS);
                        }
                        p.profit = bcom::f64_round(p.profit / bcom::DECIMALS);
                        p.margin = bcom::f64_round(p.margin / bcom::DECIMALS);
                        let data = mp.position_dynamic_idx.get(v.key()).map(|d| {
                            let mut dynamic_data = machine::PositionDynamicData::default();
                            dynamic_data.profit_rate = bcom::f64_round(d.value().profit_rate);
                            dynamic_data
                        });
                        rs.push(PositionInfo {
                            account: p,
                            pubkey: *v.key(),
                            dynamic_data: data,
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
                        let data = mp.position_dynamic_idx.get(&pbk).map(|d| {
                            let mut dynamic_data = machine::PositionDynamicData::default();
                            dynamic_data.profit_rate = bcom::f64_round(d.value().profit_rate);
                            dynamic_data
                        });
                        match s {
                            machine::State::Position(m) => {
                                rs.push(PositionInfo {
                                    account: m,
                                    pubkey: pbk,
                                    dynamic_data: data,
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
