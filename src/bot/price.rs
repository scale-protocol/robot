use crate::com;
use anchor_client::solana_sdk::{account::Account, pubkey::Pubkey};
use bond::com as bcom;
use pyth_sdk_solana::{load_price_feed_from_account, Price, PriceFeed};

pub fn get_price(pubkey: &Pubkey, account: &mut Account) -> anyhow::Result<f64> {
    get_price_from_pyth(pubkey, account)
    // todo ,if error then get price from chainlink
}

pub fn get_price_from_pyth(pubkey: &Pubkey, account: &mut Account) -> anyhow::Result<f64> {
    let price_feed: PriceFeed = load_price_feed_from_account(pubkey, account)
        .map_err(|e| com::CliError::PriceError(e.to_string()))?;
    let current_price: Price = price_feed
        .get_current_price()
        .ok_or(com::CliError::PriceError("price none".to_string()))?;

    let price = bcom::f64_round(
        (current_price.price as f64 / 10u64.pow(current_price.expo.abs() as u32) as f64)
            * bcom::DECIMALS,
    );
    Ok(price)
}
