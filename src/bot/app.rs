use crate::com;
use log::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::{runtime::Builder, signal};

use super::{
    machine::{self, Liquidation},
    sub,
};

pub fn run(ctx: com::Context) -> anyhow::Result<()> {
    let threads: usize = 4;
    let runtime = Builder::new_multi_thread()
        .worker_threads(threads)
        .thread_name_fn(|| {
            static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
            let id = ATOMIC_ID.fetch_add(1, Ordering::Relaxed);
            format!("scale-robot-{}", id)
        })
        .enable_all()
        .build()
        .map_err(|e| com::CliError::TokioRuntimeCreateField(e.to_string()))?;
    let mut sate_map = machine::StateMap::new(ctx.config.clone())?;

    sate_map.load_active_account_from_local()?;

    let config = ctx.config.clone();
    let mp = Arc::new(sate_map);
    let task = runtime.spawn(async move {
        let watch = machine::Watch::new(mp.clone()).await;
        let sub = sub::SubAccount::new(
            config.clone(),
            watch.account_watch_tx.clone(),
            watch.price_watch_tx.clone(),
        )
        .await;
        let liquidation = Liquidation::new(config.clone(), mp, 2).await;
        (watch, sub, liquidation)
    });

    let s = runtime.block_on(async { signal::ctrl_c().await });
    match s {
        Ok(()) => {
            info!("got exit signal...Start execution exit.")
        }
        Err(err) => {
            error!("Unable to listen for shutdown signal: {}", err);
        }
    }
    runtime.block_on(async {
        let (wt, sb, lb) = task.await.unwrap();
        wt.shutdown().await;
        sb.shutdown().await;
        lb.shutdown().await;
        info!("robot server shutdown!");
    });
    Ok(())
}
