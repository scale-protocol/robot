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
        sync::{mpsc, oneshot},
        task::JoinHandle,
    },
    tokio_stream::StreamExt,
};

pub struct SubAccount {
    account_shutdown_tx: oneshot::Sender<()>,
    price_shutdown_tx: oneshot::Sender<()>,
    aw: JoinHandle<anyhow::Result<()>>,
    pw: JoinHandle<anyhow::Result<()>>,
}
impl SubAccount {
    pub async fn new(
        config: config::Config,
        account_watch_tx: mpsc::UnboundedSender<(Pubkey, Account)>,
        price_watch_tx: mpsc::UnboundedSender<(Pubkey, Account)>,
    ) -> Self {
        let (account_shutdown_tx, account_shutdown_rx) = oneshot::channel::<()>();
        let (price_shutdown_tx, price_shutdown_rx) = oneshot::channel::<()>();
        let pyth_price_program_pubkey = config.accounts.pyth_program_pubkey;
        Self {
            account_shutdown_tx,
            price_shutdown_tx,
            aw: tokio::spawn(subscribe_program_accounts(
                config.clone(),
                com::id(),
                account_shutdown_rx,
                account_watch_tx,
            )),
            pw: tokio::spawn(subscribe_program_accounts(
                config.clone(),
                pyth_price_program_pubkey,
                price_shutdown_rx,
                price_watch_tx,
            )),
        }
    }
    pub async fn shutdown(self) {
        let _ = self.account_shutdown_tx.send(());
        let _ = self.aw.await;
        let _ = self.price_shutdown_tx.send(());
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
            commitment: Some(CommitmentConfig::finalized()),
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
                info!("got shutdown signal,account sub exit.");
                break;
            },
        }
    }
    Ok(())
}
