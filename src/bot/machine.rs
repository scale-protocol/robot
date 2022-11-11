use super::{price, storage};
use crate::{client, com, config};
use anchor_client::anchor_lang::AccountDeserialize;
use anchor_client::solana_sdk::{account::Account, pubkey::Pubkey};
use bond::com as bcom;
use bond::state::{market, position, user};
use chrono::{Datelike, NaiveDate, Utc};
use dashmap::DashMap;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
    time,
};
pub enum State {
    Market(market::Market),
    User(user::UserAccount),
    Position(position::Position),
    None,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match *self {
            Self::Market(_) => "market",
            Self::User(_) => "user",
            Self::Position(_) => "position",
            _ => "",
        };
        write!(f, "{}", t)
    }
}

impl<'a> From<&'a Account> for State {
    fn from(account: &'a Account) -> Self {
        let len = account.data.len() - 8;
        match len {
            market::Market::LEN => {
                let mut data: &[u8] = &account.data;
                let t = market::Market::try_deserialize(&mut data);
                match t {
                    Ok(r) => Self::Market(r),
                    Err(e) => {
                        error!("deserialize error:{}", e);
                        Self::None
                    }
                }
            }
            user::UserAccount::LEN => {
                let mut data: &[u8] = &account.data;
                let t = user::UserAccount::try_deserialize(&mut data);
                match t {
                    Ok(r) => Self::User(r),
                    Err(e) => {
                        error!("deserialize error:{}", e);
                        Self::None
                    }
                }
            }
            position::Position::LEN => {
                let mut data: &[u8] = &account.data;
                let t = position::Position::try_deserialize(&mut data);
                match t {
                    Ok(r) => Self::Position(r),
                    Err(e) => {
                        error!("deserialize error:{}", e);
                        Self::None
                    }
                }
            }
            _ => Self::None,
        }
    }
}
// key is market pubkey,value is market data
type DmMarket = DashMap<Pubkey, market::Market>;
// key is user account pubkey,value is user account data.
type DmUser = DashMap<Pubkey, user::UserAccount>;
// key is position account pubkey,value is position account data
type DmPosition = DashMap<Pubkey, position::Position>;
// key is price account key ,value is price
type DmPrice = DashMap<Pubkey, market::Price>;
// key is user account pubkey,value is position k-v map
type DmUserPosition = DashMap<Pubkey, DmPosition>;
// key is price account,value is market account
type DmIdxPriceMarket = DashMap<Pubkey, Pubkey>;
// key is user account pubkey
type DmUserDynamicData = DashMap<Pubkey, UserDynamicData>;

#[derive(Clone)]
pub struct StateMap {
    pub market: DmMarket,
    pub user: DmUser,
    pub position: DmUserPosition,
    pub price_account: DmPrice,
    pub price_idx_price_account: DmIdxPriceMarket,
    pub user_dynamic_idx: DmUserDynamicData,
    storage: storage::Storage,
}
pub type SharedStateMap = Arc<StateMap>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDynamicData {
    pub profit: f64,
    pub margin_percentage: f64,
    pub equity: f64,
}
impl Default for UserDynamicData {
    fn default() -> Self {
        UserDynamicData {
            profit: 0.0,
            margin_percentage: 0.0,
            equity: 0.0,
        }
    }
}

impl StateMap {
    pub fn new(config: config::Config) -> anyhow::Result<Self> {
        let storage = storage::Storage::new(config)?;
        let market: DmMarket = DashMap::new();
        let user: DmUser = DashMap::new();
        let position: DmUserPosition = DashMap::new();
        let price_account: DmPrice = DashMap::new();
        let price_idx_price_account: DmIdxPriceMarket = DashMap::new();
        let user_dynamic_idx: DmUserDynamicData = DashMap::new();
        Ok(Self {
            market,
            user,
            position,
            storage,
            price_account,
            price_idx_price_account,
            user_dynamic_idx,
        })
    }

    pub fn load_active_account_from_local(
        &mut self,
        pyth_price_account_sub: mpsc::UnboundedSender<Pubkey>,
    ) -> anyhow::Result<()> {
        info!("start load active account from local!");
        let p = storage::Prefix::Active;
        let r = self.storage.scan_prefix(&p);
        for i in r {
            match i {
                Ok((k, v)) => {
                    let key = String::from_utf8(k.to_vec())
                        .map_err(|e| com::CliError::JsonError(e.to_string()))?;
                    let keys = storage::Keys::from_str(key.as_str())?;
                    debug!("load account from db: {}", keys.get_storage_key());
                    let pk = keys.get_end();
                    debug!("load pubkey from db : {}", pk);
                    let pbk = Pubkey::try_from(pk.as_str())
                        .map_err(|e| com::CliError::Unknown(e.to_string()))?;
                    let values: Account = serde_json::from_slice(v.to_vec().as_slice())
                        .map_err(|e| com::CliError::JsonError(e.to_string()))?;
                    let s: State = (&values).into();
                    match s {
                        State::Market(m) => {
                            self.price_idx_price_account
                                .insert((&m).pyth_price_account, pbk);
                            // send price sub
                            match pyth_price_account_sub.send((&m).pyth_price_account) {
                                Ok(_) => {
                                    debug!("Send pyth price account to sub success!");
                                }
                                Err(e) => {
                                    info!("Send pyth price to sub error: {}", e);
                                }
                            }

                            self.price_idx_price_account
                                .insert((&m).chianlink_price_account, pbk);
                            self.market.insert(pbk, m);
                        }
                        State::User(m) => {
                            self.user.insert(pbk, m);
                        }
                        State::Position(m) => {
                            match self.position.get(&m.authority) {
                                Some(p) => {
                                    p.insert(pbk, m);
                                }
                                None => {
                                    let p: DmPosition = dashmap::DashMap::new();
                                    p.insert(pbk, m.clone());
                                    self.position.insert(m.authority, p);
                                }
                            };
                        }
                        State::None => {}
                    }
                }
                Err(e) => {
                    debug!("{}", e);
                }
            }
        }
        info!("complete load active account from local!");
        Ok(())
    }
}
pub struct Watch {
    account_shutdown_tx: oneshot::Sender<()>,
    price_shutdown_tx: oneshot::Sender<()>,
    pub account_watch_tx: UnboundedSender<(Pubkey, Account)>,
    pub price_watch_tx: UnboundedSender<(Pubkey, Account)>,
    aw: JoinHandle<anyhow::Result<()>>,
    pw: JoinHandle<anyhow::Result<()>>,
}

impl Watch {
    pub async fn new<'a>(
        mp: SharedStateMap,
        pyth_price_account_sub: mpsc::UnboundedSender<Pubkey>,
    ) -> Self {
        let (account_watch_tx, account_watch_rx) = mpsc::unbounded_channel::<(Pubkey, Account)>();
        let (account_shutdown_tx, account_shutdown_rx) = oneshot::channel::<()>();
        let (price_watch_tx, price_watch_rx) = mpsc::unbounded_channel::<(Pubkey, Account)>();
        let (price_shutdown_tx, price_shutdown_rx) = oneshot::channel::<()>();
        Self {
            account_shutdown_tx,
            price_shutdown_tx,
            account_watch_tx,
            price_watch_tx,
            aw: tokio::spawn(watch_account(
                mp.clone(),
                account_watch_rx,
                account_shutdown_rx,
                pyth_price_account_sub,
            )),
            pw: tokio::spawn(watch_price(mp.clone(), price_watch_rx, price_shutdown_rx)),
        }
    }

    pub async fn shutdown(self) {
        let _ = self.account_shutdown_tx.send(());
        let _ = self.aw.await;
        let _ = self.price_shutdown_tx.send(());
        let _ = self.pw.await;
    }
}

async fn watch_account<'a>(
    mp: SharedStateMap,
    mut watch_rx: UnboundedReceiver<(Pubkey, Account)>,
    mut shutdown_rx: oneshot::Receiver<()>,
    pyth_price_account_sub: mpsc::UnboundedSender<Pubkey>,
) -> anyhow::Result<()> {
    info!("start scale program account watch ...");
    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal,break watch account!");
                break;
            },
            r = watch_rx.recv()=>{
                match r {
                    Some(rs)=>{
                        let (pubkey,account) = rs;
                        debug!("account channel got data : {:?},{:?}",pubkey,account);
                        keep_account(mp.clone(), pubkey, account,pyth_price_account_sub.clone());
                    }
                    None=>{
                        debug!("account channel got none : {:?}",r);
                    }
                }
            }
        }
    }
    Ok(())
}

async fn watch_price(
    mp: SharedStateMap,
    mut watch_rx: UnboundedReceiver<(Pubkey, Account)>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    info!("start price account watch...");
    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal,break watch price!");
                break;
            },
            r = watch_rx.recv()=>{
                match r {
                    Some(rs)=>{
                        let (pubkey,account) = rs;
                        keep_price(mp.clone(), pubkey, account);
                    }
                    None=>{}
                }
            }
        }
    }
    Ok(())
}

fn keep_price(mp: SharedStateMap, pubkey: Pubkey, mut account: Account) {
    match mp.price_idx_price_account.get(&pubkey) {
        Some(k) => match mp.market.get(&k) {
            Some(m) => match price::get_price_from_pyth(&pubkey, &mut account) {
                Ok(p) => {
                    let spread = com::f64_round(p * m.spread);
                    let price = market::Price {
                        buy_price: com::f64_round(p + spread),
                        sell_price: com::f64_round(p - spread),
                        real_price: p,
                        spread,
                    };
                    mp.price_account.insert(pubkey, price);
                }
                Err(e) => {
                    error!("{}", e);
                }
            },
            None => {
                error!("keep price error,get index but cannot get market data");
            }
        },
        None => {
            debug!(
                "Can not found market of price account,ignore it: {}",
                pubkey
            );
        }
    }
}
fn keep_account(
    mp: SharedStateMap,
    pubkey: Pubkey,
    account: Account,
    pyth_price_account_sub: mpsc::UnboundedSender<Pubkey>,
) {
    let s: State = (&account).into();
    let tag = s.to_string();
    let keys = storage::Keys::new(storage::Prefix::Active);
    match s {
        State::Market(m) => {
            let pyth_account = m.pyth_price_account;
            let chainlink_account = m.chianlink_price_account;
            let mut keys = keys.add(tag).add(pubkey.to_string());
            if account.lamports <= 0 {
                mp.market.remove(&pubkey);
                mp.price_idx_price_account.remove(&pyth_account);
                mp.price_idx_price_account.remove(&chainlink_account);
                save_as_history(mp, &mut keys, &account);
            } else {
                mp.market.insert(pubkey, m);
                mp.price_idx_price_account.insert(pyth_account, pubkey);
                mp.price_idx_price_account.insert(chainlink_account, pubkey);
                save_to_active(mp, &mut keys, &account);
                // send price sub
                match pyth_price_account_sub.send(pyth_account) {
                    Ok(_) => {
                        debug!("Send pyth price account to sub success!");
                    }
                    Err(e) => {
                        info!("Send pyth price to sub error: {}", e);
                    }
                }
            }
        }
        State::User(m) => {
            let mut keys = keys.add(tag).add(pubkey.to_string());
            if account.lamports <= 0 {
                mp.user.remove(&pubkey);
                save_as_history(mp, &mut keys, &account);
            } else {
                let x = mp.user.insert(pubkey, m);
                save_to_active(mp, &mut keys, &account);
            }
        }
        State::Position(m) => {
            let mut keys = keys
                .add(tag)
                .add(m.authority.to_string())
                .add(pubkey.to_string());
            if account.lamports <= 0
                || m.position_status == position::PositionStatus::NormalClosing
                || m.position_status == position::PositionStatus::ForceClosing
            {
                mp.position.remove(&pubkey);
                match mp.position.get(&m.authority) {
                    Some(p) => {
                        p.remove(&pubkey);
                    }
                    None => {
                        // nothing to do
                    }
                };
                save_as_history(mp, &mut keys, &account);
            } else {
                match mp.position.get(&m.authority) {
                    Some(p) => {
                        p.insert(pubkey, m);
                    }
                    None => {
                        let p: DmPosition = dashmap::DashMap::new();
                        p.insert(pubkey, m.clone());
                        mp.position.insert(m.authority, p);
                    }
                };
                save_to_active(mp, &mut keys, &account);
            }
        }
        State::None => {
            warn!(
                "Unrecognized structure of account: {:?},{:?}",
                pubkey, account,
            );
        }
    }
}

fn save_as_history(mp: SharedStateMap, ks: &mut storage::Keys, account: &Account) {
    match mp.storage.save_as_history(ks, account) {
        Ok(()) => {
            debug!(
                "save a account as history success!account:{}",
                ks.get_storage_key()
            );
        }
        Err(e) => {
            error!(
                "save a account as history error:{},account:{}",
                e,
                ks.get_storage_key()
            );
        }
    }
}

fn save_to_active(mp: SharedStateMap, ks: &mut storage::Keys, account: &Account) {
    match mp.storage.save_to_active(ks, account) {
        Ok(()) => {
            debug!(
                "save a account as active success!account:{}",
                ks.get_storage_key()
            );
        }
        Err(e) => {
            error!(
                "save a account as active error:{},account:{}",
                e,
                ks.get_storage_key()
            );
        }
    }
}

pub struct Liquidation {
    shutdown_tx: watch::Sender<bool>,
    tp: Vec<JoinHandle<anyhow::Result<()>>>,
}

impl Liquidation {
    pub async fn new(config: config::Config, mp: SharedStateMap, tasks: usize) -> Self {
        let mut ts = tasks;
        if ts <= 0 {
            ts = 2;
        }
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (task_ch_tx, task_ch_rx) = flume::bounded::<Pubkey>(ts);
        let (timer_ch_tx, timer_ch_rx) = flume::bounded::<Pubkey>(ts);
        let mut send_shutdown_rx = shutdown_rx.clone();

        // The position capital fee is charged every eight hours (fixed at 0:00, 8:00 and 16:00 GMT+0)
        let tmp = mp.clone();
        tokio::spawn(async move {
            let next_run_time = time_to_next_run();
            let start = time::Instant::now() + time::Duration::from_secs(next_run_time as u64);
            let mut interval = time::interval_at(start, time::Duration::from_secs(8 * 3600));
            loop {
                let i = interval.tick().await;
                info!(
                    "The timer is awakened. The current time is: {} ,Run every: {:?}",
                    Utc::now().format("%Y-%m-%d %H:%M:%S"),
                    i.elapsed()
                );
                let now = time::Instant::now();
                debug!("Start a new round of funding complete...");

                for v in &tmp.user {
                    let pubkey = *v.key();
                    match timer_ch_tx.send(pubkey) {
                        Ok(()) => {}
                        Err(e) => {
                            debug!("timer task msg send error:{},exit send loop !", e);
                            break;
                        }
                    }
                }
                let t = now.elapsed();
                debug!("Complete a new round of funding... use time:{:?}", t);
            }
        });
        // Keep checking position
        let lmp = mp.clone();

        tokio::spawn(async move {
            let mut count = 1u64;
            loop {
                tokio::select! {
                    _ = send_shutdown_rx.changed() => {
                        info!("got shutdown signal, user loop program exit.");
                        break;
                    }
                    _=async{}=>{
                        let now = time::Instant::now();

                        debug!("Start a new round of liquidation... count: {}",count);

                        for v in &lmp.user {
                            let pubkey = *v.key();
                           match task_ch_tx.send(pubkey){
                                Ok(())=>{}
                                Err(e)=>{
                                    debug!("task msg send error:{},exit send loop !",e);
                                    break;
                                }
                           }
                        }
                        let t = now.elapsed();
                        count+=1;
                        debug!("Complete a new round of liquidation... use time: {:?},count: {}", t,count);
                    }
                }
            }
        });
        let mut workers: Vec<JoinHandle<anyhow::Result<()>>> = Vec::with_capacity(ts);
        for _ in 0..ts {
            let cfg = config.clone();
            let smp = mp.clone();
            workers.push(tokio::spawn(loop_position_by_user(
                cfg,
                smp,
                task_ch_rx.clone(),
                timer_ch_rx.clone(),
                shutdown_rx.clone(),
            )));
        }
        Self {
            shutdown_tx,
            tp: workers,
        }
    }
    pub async fn shutdown(self) {
        _ = self.shutdown_tx.send(true);
        // wait
        for v in self.tp {
            let _ = v.await;
        }
    }
}
// Return seconds
fn time_to_next_run() -> i64 {
    let now = Utc::now().naive_utc();
    let tv = vec![
        NaiveDate::from_ymd(now.year(), now.month(), now.day()).and_hms(0, 0, 0),
        NaiveDate::from_ymd(now.year(), now.month(), now.day()).and_hms(8, 0, 0),
        NaiveDate::from_ymd(now.year(), now.month(), now.day()).and_hms(16, 0, 0),
    ];
    for v in &tv {
        let ds = v.signed_duration_since(now).num_seconds();
        if ds > 0 {
            return ds;
        }
    }
    &tv[0].timestamp() + (3600 * 24) - now.timestamp()
}

async fn loop_position_by_user(
    config: config::Config,
    mp: SharedStateMap,
    task_rx: flume::Receiver<Pubkey>,
    timer_task_rx: flume::Receiver<Pubkey>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    info!("start position loop program...");

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                info!("got shutdown signal, user position loop task exit.");
                break;
            }
            r = task_rx.recv_async() => {
                // time::sleep(time::Duration::from_secs(10)).await;
                match r {
                    Ok(user_pubkey)=>{
                        match mp.user.get(&user_pubkey){
                            Some(v)=>{
                                match mp.position.get(&user_pubkey) {
                                    Some(ps) => {
                                        match compute_position(&config,&user_pubkey,&v,&ps.value(),&mp.market,&mp.price_account,&mp.user_dynamic_idx){
                                            Ok(())=>{
                                                debug!("loop user {} success!",user_pubkey);
                                            }
                                            Err(e)=>{
                                                debug!("loop user {} error: {}",user_pubkey,e);
                                            }
                                        }
                                    }
                                    None => {
                                        debug!("loop user {} positions none,continue!",user_pubkey);
                                    },
                                }
                            },
                            None=>{
                                debug!("Recv the user pubkey in task ,but get user data none!");
                            }
                        }
                    }
                    Err(e)=>{
                        info!("position loop task recv error:{},exit!",e);
                        break;
                    }
                }
            }
            r = timer_task_rx.recv_async() => {
                match r {
                    Ok(user_pubkey)=>{
                        match mp.user.get(&user_pubkey){
                            Some(v)=>{
                                match mp.position.get(&user_pubkey) {
                                    Some(ps) => {
                                        match funding_rate_settlement(&config,&user_pubkey,&v,ps.value()){
                                            Ok(())=>{
                                                debug!("timer loop user {} success!",user_pubkey);
                                            }
                                            Err(e)=>{
                                                debug!("timer loop user {} error: {}",user_pubkey,e);
                                            }
                                        }
                                    }
                                    None => {
                                        debug!("timer loop user {} positions none,continue!",user_pubkey);
                                    },
                                };

                            },
                            None=>{
                                debug!("Recv the user pubkey in timer task ,but get user data none");
                                continue;
                            }
                        }

                    }
                    Err(e)=>{
                        info!("timer position loop task recv error:{},exit!",e);
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

fn funding_rate_settlement(
    _config: &config::Config,
    user_pubkey: &Pubkey,
    _user_account: &user::UserAccount,
    _position: &DmPosition,
) -> anyhow::Result<()> {
    debug!("Funding rate settlement: {}", user_pubkey);
    // todo
    // let client = com::Context::new_client(config);
    Ok(())
}

fn compute_position(
    config: &config::Config,
    user_pubkey: &Pubkey,
    user_account: &user::UserAccount,
    position: &DmPosition,
    market_mp: &DmMarket,
    price_map: &DmPrice,
    user_dynamic_idx_mp: &DmUserDynamicData,
) -> anyhow::Result<()> {
    debug!("compute user's position: {}", user_pubkey);
    let client = com::Context::new_client(config)?;
    let data_full = compute_pl_all_full_position(
        config,
        &client,
        user_pubkey,
        user_account,
        market_mp,
        price_map,
    )?;
    let data_independent =
        compute_pl_all_independent_position(&client, user_pubkey, position, market_mp, price_map)?;
    let equity = data_full.equity + data_independent.equity + user_account.balance;
    let data = UserDynamicData {
        profit: data_independent.profit + data_full.profit,
        margin_percentage: bcom::f64_round(equity / user_account.margin_total),
        equity,
    };
    user_dynamic_idx_mp.insert(*user_pubkey, data);
    Ok(())
}

pub fn compute_pl_all_independent_position(
    anchor_client: &anchor_client::Client,
    user_pubkey: &Pubkey,
    positions: &DmPosition,
    market_mp: &DmMarket,
    price_map: &DmPrice,
) -> anyhow::Result<UserDynamicData> {
    let mut data = UserDynamicData::default();

    for v in positions {
        if v.position_type == position::PositionType::Full {
            continue;
        }
        match market_mp.get(&v.market_account) {
            Some(market) => match price_map.get(&market.pyth_price_account) {
                Some(price) => {
                    let pl = v.get_pl_price(price.value());
                    data.profit += pl;
                    let total_pl =
                        pl + market.get_position_fund(v.direction.clone(), v.get_fund_size());
                    let equity = v.margin + total_pl;
                    data.equity = equity;
                    if equity / v.margin < bcom::BURST_RATE {
                        match client::burst_position(
                            anchor_client,
                            *user_pubkey,
                            *market.key(),
                            *v.key(),
                            market.pyth_price_account,
                            market.chianlink_price_account,
                        ) {
                            Ok(()) => {
                                info!("burst position success! pubkey: {}", v.key());
                            }
                            Err(e) => {
                                error!("burst position success error:{}", e);
                                continue;
                            }
                        }
                    }
                }
                None => {
                    error!(
                            "Cannot get price data, continue! position pubkey: {},market_pubkey: {},pyth_price_pubkey: {}",
                            v.key(),
                            v.market_account,
                            market.pyth_price_account
                        );
                }
            },
            None => {
                error!(
                    "Cannot get market data , continue! position pubkey: {},market_pubkey: {}",
                    v.key(),
                    v.market_account
                );
            }
        }
    }
    Ok(data)
}
#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PositionSort {
    pub offset: u32,
    pub profit: i64,
    pub direction: position::Direction,
    pub margin: u64,
    pub market_account: Option<Pubkey>,
}

// Floating P/L
pub fn compute_pl_all_full_position(
    config: &config::Config,
    anchor_client: &anchor_client::Client,
    user_pubkey: &Pubkey,
    user_account_data: &user::UserAccount,
    market_mp: &DmMarket,
    price_map: &DmPrice,
) -> anyhow::Result<UserDynamicData> {
    let btc_price = price_map
        .get(config.get_pyth_btc_pubkey())
        .ok_or(com::CliError::PriceError("get none btc price".to_string()))?;
    let eth_price = price_map
        .get(config.get_pyth_eth_pubkey())
        .ok_or(com::CliError::PriceError("get none eth price".to_string()))?;
    let sol_price = price_map
        .get(config.get_pyth_sol_pubkey())
        .ok_or(com::CliError::PriceError("get none sol price".to_string()))?;

    let headers = &user_account_data.open_full_position_headers;
    let mut total_pl: f64 = 0.0;

    let mut data = UserDynamicData::default();

    let mut position_sort: Vec<PositionSort> = Vec::with_capacity(headers.len());

    for header in headers.iter() {
        let (profit_and_fund_rate, market_pubkey) = match header.market {
            bcom::FullPositionMarket::BtcUsd => {
                let mut pl = header.get_pl_price(btc_price.value());
                data.profit += pl;
                let market_account = bcom::FullPositionMarket::BtcUsd.to_pubkey().0;
                match market_mp.get(&market_account) {
                    Some(v) => {
                        pl += v.get_position_fund(header.direction.clone(), header.get_fund_size());
                    }
                    None => {
                        debug!("missing BTC/USD account data. full position compute continue");
                        continue;
                    }
                }
                (pl, Some(market_account))
            }

            bcom::FullPositionMarket::EthUsd => {
                let mut pl = header.get_pl_price(eth_price.value());
                data.profit += pl;
                let market_account = bcom::FullPositionMarket::EthUsd.to_pubkey().0;
                match market_mp.get(&market_account) {
                    Some(v) => {
                        pl += v.get_position_fund(header.direction.clone(), header.get_fund_size());
                    }
                    None => {
                        debug!("missing ETH/USD account data. full position compute continue");
                        continue;
                    }
                }
                (pl, Some(market_account))
            }

            bcom::FullPositionMarket::SolUsd => {
                let mut pl = header.get_pl_price(sol_price.value());
                data.profit += pl;
                let market_account = bcom::FullPositionMarket::SolUsd.to_pubkey().0;
                match market_mp.get(&market_account) {
                    Some(v) => {
                        pl += v.get_position_fund(header.direction.clone(), header.get_fund_size());
                    }
                    None => {
                        debug!("missing SOL/USD account data. full position compute continue");
                        continue;
                    }
                }
                (pl, Some(market_account))
            }
            _ => (0.0, None),
        };

        position_sort.push(PositionSort {
            profit: (profit_and_fund_rate * 100.0) as i64,
            offset: header.position_seed_offset,
            direction: header.direction,
            margin: (header.margin * 100.0) as u64,
            market_account: market_pubkey,
        });
        total_pl += profit_and_fund_rate
    }
    data.equity = total_pl;
    let equity = user_account_data.balance + total_pl;

    let mut margin_full_buy_total = user_account_data.margin_full_buy_total;
    let mut margin_full_sell_total = user_account_data.margin_full_sell_total;

    let margin_full_total = bcom::f64_round(margin_full_buy_total.max(margin_full_sell_total));
    // Forced close
    if (equity / margin_full_total) < bcom::BURST_RATE {
        // sort
        position_sort.sort_by(|a, b| b.profit.cmp(&a.profit).reverse());
        for p in position_sort {
            // start burst
            match p.market_account {
                Some(market_pubkey) => match market_mp.get(&market_pubkey) {
                    Some(v) => {
                        let (position_pubkey, _pbump) = Pubkey::find_program_address(
                            &[
                                bcom::POSITION_ACCOUNT_SEED,
                                &user_account_data.authority.to_bytes(),
                                &user_pubkey.to_bytes(),
                                &p.offset.to_string().as_bytes(),
                            ],
                            &com::id(),
                        );

                        match client::burst_position(
                            anchor_client,
                            *user_pubkey,
                            market_pubkey,
                            position_pubkey,
                            v.pyth_price_account,
                            v.chianlink_price_account,
                        ) {
                            Ok(()) => {
                                info!("burst position success! pubkey: {}", position_pubkey);
                            }
                            Err(e) => {
                                error!("burst position success error:{}", e);
                                continue;
                            }
                        }
                    }
                    None => {
                        continue;
                    }
                },
                None => {
                    continue;
                }
            }

            match p.direction {
                position::Direction::Buy => {
                    margin_full_buy_total -= (p.margin / 100) as f64;
                }
                position::Direction::Sell => {
                    margin_full_sell_total -= (p.margin / 100) as f64;
                }
            }
            if ((equity - (p.profit / 100) as f64)
                / margin_full_buy_total.max(margin_full_sell_total))
                > bcom::BURST_RATE
            {
                break;
            }
        }
    }
    Ok(data)
}
