use std::collections::HashSet;

use {
    crate::{com, config},
    anchor_client::solana_sdk::commitment_config::CommitmentConfig,
    anchor_client::solana_sdk::{account::Account, pubkey::Pubkey},
    log::{debug, error, info},
    solana_account_decoder::UiAccountEncoding,
    solana_client::nonblocking::{pubsub_client, rpc_client},
    solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    std::convert::TryFrom,
    tokio::{
        self,
        sync::{mpsc, oneshot, watch},
        task::JoinHandle,
    },
    tokio_stream::StreamExt,
};

pub struct SubAccount {
    program_shutdown_tx: oneshot::Sender<()>,
    price_shutdown_tx: watch::Sender<bool>,
    pw: JoinHandle<anyhow::Result<()>>,
    aw: JoinHandle<anyhow::Result<()>>,
}
impl SubAccount {
    pub async fn new(
        config: config::Config,
        account_watch_tx: mpsc::UnboundedSender<(Pubkey, Account)>,
        price_watch_tx: mpsc::UnboundedSender<(Pubkey, Account)>,
        subscribe_rx: mpsc::UnboundedReceiver<Pubkey>,
    ) -> Self {
        let (program_shutdown_tx, program_shutdown_rx) = oneshot::channel::<()>();
        let (price_shutdown_tx, price_shutdown_rx) = watch::channel(false);
        // let pyth_price_program_pubkey = config.accounts.pyth_program_pubkey;
        Self {
            program_shutdown_tx,
            price_shutdown_tx,
            aw: tokio::spawn(subscribe_program_accounts(
                config.clone(),
                com::id(),
                program_shutdown_rx,
                account_watch_tx,
            )),
            pw: tokio::spawn(subscribe_price_accounts(
                config.clone(),
                subscribe_rx,
                price_shutdown_rx.clone(),
                price_watch_tx.clone(),
            )),
        }
    }
    pub async fn shutdown(self) {
        let _ = self.program_shutdown_tx.send(());
        let _ = self.aw.await;
        let _ = self.price_shutdown_tx.send(true);
        let _ = self.pw.await;
    }

    pub async fn get_all_program_accounts(
        &self,
        config: config::Config,
        watch_tx: mpsc::UnboundedSender<(Pubkey, Account)>,
    ) -> anyhow::Result<()> {
        let client = rpc_client::RpcClient::new(config.cluster.url().to_string());
        let id = com::id();
        let accounts = client.get_program_accounts(&id).await?;
        for a in accounts {
            debug!("get all program accounts for rpc node:{}", a.0);
            match watch_tx.send(a) {
                Ok(()) => {}
                Err(e) => {
                    error!("message channel error:{},sub program exit.", e);
                    break;
                }
            }
        }
        Ok(())
    }
}

async fn subscribe_program_accounts(
    config: config::Config,
    program_pubkey: Pubkey,
    mut shutdown_rx: oneshot::Receiver<()>,
    watch_tx: mpsc::UnboundedSender<(Pubkey, Account)>,
) -> anyhow::Result<()> {
    let sol_sub_client = pubsub_client::PubsubClient::new(config.cluster.ws_url())
        .await
        .map_err(|e| {
            debug!("{:#?}", e);
            com::CliError::SubscriptionAccountFailed(e.to_string())
        })?;
    info!("start program account subscription ...");
    let rpc_config = RpcProgramAccountsConfig {
        filters: None,
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64Zstd),
            commitment: Some(CommitmentConfig::processed()),
            data_slice: None,
            min_context_slot: None,
        },
        with_context: None,
    };
    let (mut s, _r) = sol_sub_client
        .program_subscribe(&program_pubkey, Some(rpc_config))
        .await
        .map_err(|e| com::CliError::SubscriptionAccountFailed(e.to_string()))?;
    let mut s = s.as_mut();

    loop {
        tokio::select! {
            response = s.next() => {
                match response {
                    Some(i_account)=>{
                        let pda_pubkey = Pubkey::try_from(i_account.value.pubkey.as_str());
                        let pda_account:Option<Account> = i_account.value.account.decode();
                        match pda_account {
                            Some(account)=>{
                                debug!("got account: {:?} data: {:#?},len:{}",pda_pubkey,account,account.data.len());
                                match pda_pubkey {
                                    Ok(pubkey)=>{
                                        match watch_tx.send((pubkey,account)) {
                                            Ok(())=>{
                                                debug!("send {:?} to account watch success!",pda_pubkey);
                                            }
                                            Err(e)=>{
                                                error!("message channel error:{},sub program exit.",e);
                                                break;
                                            }
                                        }
                                    }
                                    Err(e)=>{
                                        error!("Parse pubkey error:{}",e);
                                    }
                                }
                            }
                            None=>{
                                error!("Can not decode account,got None");
                            }
                        }
                    }
                    None=>{
                        info!("message channel close");
                        break;
                    }
                }
            }
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal, account sub exit.");
                break;
            },
        }
    }
    Ok(())
}

async fn subscribe_price_accounts(
    config: config::Config,
    mut subscribe_rx: mpsc::UnboundedReceiver<Pubkey>,
    mut shutdown_rx: watch::Receiver<bool>,
    watch_tx: mpsc::UnboundedSender<(Pubkey, Account)>,
) -> anyhow::Result<()> {
    info!("start price account subscription ...");
    let mut price_account: HashSet<Pubkey> = HashSet::new();
    let mut sub_tasks: Vec<JoinHandle<anyhow::Result<()>>> = Vec::new();
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                info!("got shutdown signal, price accounts sub exit.");
                for t in sub_tasks{
                    match t.await {
                        Ok(_)=>{
                            info!("Close price account sub task success!");
                        }
                        Err(e)=>{
                            error!("Close price account sub task error: {}", e);
                        }
                    }
                }
                break;
            },
            price_pubkey = subscribe_rx.recv() => {
                match price_pubkey {
                    Some(pubkey)=>{
                        if !price_account.insert(pubkey) {
                            sub_tasks.push(tokio::spawn(subscribe_one_price_account(
                                config.clone(),
                                pubkey,
                                shutdown_rx.clone(),
                                watch_tx.clone(),
                            )));
                        }
                    }
                    None=>{
                        debug!("Sub accounts continue,got none pubkey");
                    }
                }
            }
        }
    }
    Ok(())
}

async fn subscribe_one_price_account(
    config: config::Config,
    pubkey: Pubkey,
    mut shutdown_rx: watch::Receiver<bool>,
    watch_tx: mpsc::UnboundedSender<(Pubkey, Account)>,
) -> anyhow::Result<()> {
    let sol_sub_client = pubsub_client::PubsubClient::new(config.cluster.ws_url())
        .await
        .map_err(|e| {
            debug!("{:#?}", e);
            com::CliError::SubscriptionAccountFailed(e.to_string())
        })?;
    info!("start pyth price account {} subscription ...", pubkey);
    let rpc_config = RpcAccountInfoConfig {
        encoding: Some(UiAccountEncoding::Base64Zstd),
        commitment: Some(CommitmentConfig::processed()),
        data_slice: None,
        min_context_slot: None,
    };
    let (mut s, _r) = sol_sub_client
        .account_subscribe(&pubkey, Some(rpc_config))
        .await
        .map_err(|e| com::CliError::SubscriptionAccountFailed(e.to_string()))?;
    let mut s = s.as_mut();

    loop {
        tokio::select! {
            response = s.next() => {
                match response {
                    Some(i_account)=>{
                        let pda_account:Option<Account> = i_account.value.decode();
                        match pda_account {
                            Some(account)=>{
                                debug!("got price account: {:?} data: {:#?},len:{}",pubkey,account,account.data.len());
                                match watch_tx.send((pubkey,account)) {
                                    Ok(())=>{
                                        debug!("send {:?} to price account watch success!",pubkey);
                                    }
                                    Err(e)=>{
                                        error!("message channel error:{},price account sub program exit.",e);
                                        break;
                                    }
                                }
                            }
                            None=>{
                                error!("Can not decode price account,got None");
                            }
                        }
                    }
                    None=>{
                        info!("message channel close");
                        break;
                    }
                }
            }
            _ = shutdown_rx.changed() => {
                info!("got shutdown signal,price account {} sub exit.",pubkey);
                break;
            },
        }
    }
    Ok(())
}
