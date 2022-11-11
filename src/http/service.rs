use bond::state::{market, position, user};

use crate::bot::{self, machine::UserDynamicData};
use crate::com::CliError;
use anchor_client::solana_sdk::pubkey::Pubkey;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub balance: f64,
    pub profit: f64,
    pub margin_total: f64,
    pub margin_full_total: f64,
    pub margin_independent_total: f64,
    pub margin_full_buy_total: f64,
    pub margin_full_sell_total: f64,
    pub margin_independent_buy_total: f64,
    pub margin_independent_sell_total: f64,
    pub dynamic_data: Option<UserDynamicData>,
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
                balance: user.balance,
                profit: user.profit,
                margin_total: user.margin_total,
                margin_full_total: user.margin_full_total,
                margin_independent_total: user.margin_independent_total,
                margin_full_buy_total: user.margin_full_buy_total,
                margin_full_sell_total: user.margin_full_sell_total,
                margin_independent_buy_total: user.margin_independent_buy_total,
                margin_independent_sell_total: user.margin_independent_sell_total,
                dynamic_data: data,
            };
            Some(user_info)
        }
        None => None,
    };
    Ok(rs)
}
