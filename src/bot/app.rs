use crate::{
    com,
    http::web_server::{self, HttpServer},
};
use log::*;
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::{runtime::Builder, signal};

use super::{
    machine::{self, Liquidation},
    sub,
};
use std::net::ToSocketAddrs;

pub fn run(ctx: com::Context, args: &clap::ArgMatches) -> anyhow::Result<()> {
    let tasks = match args.get_one::<usize>("tasks") {
        Some(t) => *t,
        None => 20,
    };
    let port = match args.get_one::<u64>("port") {
        Some(p) => *p,
        None => 3000,
    };
    let ip = match args.get_one::<String>("ip") {
        Some(i) => i.to_string(),
        None => "127.0.0.1".to_string(),
    };
    let address = format!("{}:{}", ip, port);
    let mut socket_addr: Option<SocketAddr> = None;
    if port > 0 {
        let addr = address
            .to_socket_addrs()
            .map_err(|e| com::CliError::HttpServerError(e.to_string()))?
            .next()
            .ok_or(com::CliError::HttpServerError("parsing none".to_string()))?;
        socket_addr = Some(addr);
    }
    let mut builder = Builder::new_multi_thread();
    match args.get_one::<usize>("threads") {
        Some(t) => {
            builder.worker_threads(*t);
        }
        None => {}
    }
    let runtime = builder
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
        let liquidation = Liquidation::new(config.clone(), mp, tasks).await;
        // start http server
        let web_server: Option<HttpServer> = match socket_addr {
            Some(addr) => Some(web_server::HttpServer::new(&addr).await),
            None => None,
        };
        (watch, sub, liquidation, web_server)
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
        let (wt, sb, lb, wb) = task.await.unwrap();
        wt.shutdown().await;
        sb.shutdown().await;
        lb.shutdown().await;
        match wb {
            Some(s) => {
                s.shutdown().await;
            }
            None => {}
        }
        info!("robot server shutdown!");
    });
    Ok(())
}
