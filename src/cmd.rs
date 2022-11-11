use crate::bot::app;
use crate::client;
use crate::com;
use crate::config;
use anyhow;
use clap::{arg, Command};
use log::debug;
use std::ffi::OsString;
use std::path::PathBuf;
fn cli() -> Command {
    Command::new("Scale contract command line operator.")
        .about("Scale contract command line operator. More https://www.scale.exchange .")
        .version("0.1.0")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .author("scale development team.")
        .arg(arg!(-f --file <CONFIG_FILE> "The custom config file."))
        .subcommand(
            Command::new("config").about("cli program config.")
            .args_conflicts_with_subcommands(true)
            .subcommand_required(true)
            .subcommand(Command::new("get").about("get cli program config."))
            .subcommand(
                Command::new("set").about("set cli program config.")
                .arg(arg!(-p --path <PATH> "Parameter file storage directory.").value_parser(clap::value_parser!(PathBuf)))
                .arg(arg!(-k --keypair <PATH> "Wallet key pair address.").value_parser(clap::value_parser!(PathBuf)))
                .arg(arg!(-r --rpc_url <PATH> "Custom rpc url."))
                .arg(arg!(-w --ws_url <PATH> "Custom websocket url."))
                .arg(arg!(-c --cluster <PATH> "set the cluster.Optional values: Testnet,Mainnet,Devnet,Localnet,Debug."))
        )
        )
        .subcommand(Command::new("init_vault").about("init the system vault account."))
        .subcommand(
            Command::new("init_market")
                .about("init a market.")
                .arg(arg!(-p --pair<PAIR> "Marketplace Trading Pair Mark. .e.g BTC/USD."))
                .arg(arg!(-s --spread<SPREAD> "Difference value of market quotation (proportion).").value_parser(clap::value_parser!(f64)))
                .arg(arg!(-y --pyth_account<PYTH_ACCOUNT> "The pyth.network price account."))
                .arg(arg!(-t --chain_link_account<CHAIN_LINK_ACCOUNT> "The chainlink price account.")),
        )
        .subcommand(Command::new("init_user").about("init a user account."))
        .subcommand(
            Command::new("deposit")
                .about("init a user account")
                .arg(arg!(-a --amount <AMOUNT> "Amount of deposit.").value_parser(clap::value_parser!(u64))),
        )
        .subcommand(
            Command::new("open_position")
                .about("open a position.")
                .arg(arg!(-p --pair <PAIR> "The market account pair. .e.g BTC/USD."))
                .arg(arg!(-s --size <SIZE> "The position size").value_parser(clap::value_parser!(f64)))
                .arg(arg!(-l --leverage <LEVERAGE> "The leverage size.").value_parser(clap::value_parser!(u16)))
                .arg(arg!(-t --position_type <POSITION_TYPE> "1 full position mode, 2 independent position modes.").value_parser(clap::value_parser!(u8)))
                .arg(arg!(-d --direction <DIRECTION> "1 buy long, 2 sell short.").value_parser(clap::value_parser!(u8))),
        )
        .subcommand(
            Command::new("close_position")
                .about("close a position")                
                .arg(arg!(-a --account <PAIR> "The position account."))
                .arg(arg!(-o --offset <SIZE> "Or give the position seed offset.").value_parser(clap::value_parser!(u32))),
        )
        .subcommand(
            Command::new("investment")
                .about("This is only used for testing and should actually be called by NFT program.")
                .arg(arg!(-p --pair <PAIR> "The market account pair. .e.g BTC/USD."))
                .arg(arg!(-a --amount <AMOUNT> "Amount of investment.").value_parser(clap::value_parser!(u64))),
        )
        .subcommand(
            Command::new("divestment")
                .about("This is only used for testing and should actually be called by NFT coin program.")
                .arg(arg!(-p --pair <PAIR> "The market account pair. .e.g BTC/USD."))
                .arg(arg!(-a --amount <AMOUNT> "Amount of divestment.").value_parser(clap::value_parser!(u64))),
        )
        .subcommand(
            Command::new("bot")
                .about("Start a settlement robot. Monitor the trading market and close risk positions in a timely manner.")
                .arg(arg!(-T --threads <THREADS> "The number of threads that can be started by the robot, which defaults to the number of system cores.").value_parser(clap::value_parser!(usize)))
                .arg(arg!(-t --tasks <TASKS> "The number of settlement tasks that the robot can open, corresponding to the number of tasks in the tokio, 1 by default.").value_parser(clap::value_parser!(usize)))
                .arg(arg!(-p --port <PORT> "The web server port provides http query service and websocket push service. The default value is 3000. If it is set to 0, the web service is disabled.").value_parser(clap::value_parser!(u64)))
                .arg(arg!(-i --ip <IP> "The IP address bound to the web server. The default is 127.0.0.1."))
        )
}

pub fn run() -> anyhow::Result<()> {
    env_logger::init();
    let matches = cli().get_matches();
    let config_file = matches.get_one::<PathBuf>("file");
    let mut config = config::Config::default();
    match config_file {
        Some(c) => config.config_file = c.to_path_buf(),
        None => {}
    }
    match config.load() {
        Ok(_) => {}
        Err(e) => {
            debug!("{}", e);
            config.init();
        }
    }
    match matches.subcommand() {
        Some(("config", sub_matches)) => {
            let config_command = sub_matches.subcommand().unwrap_or(("get", sub_matches));
            match config_command {
                ("get", _sub_matches) => config.get(),
                ("set", sub_matches) => {
                    let path = sub_matches.get_one::<PathBuf>("path");
                    let keypair = sub_matches.get_one::<PathBuf>("keypair");
                    let rpc_url = sub_matches.get_one::<String>("rpc_url");
                    let ws_url = sub_matches.get_one::<String>("ws_url");
                    let cluster = sub_matches.get_one::<String>("cluster");
                    config.set(path, keypair, rpc_url, ws_url, cluster);
                }
                (name, _) => {
                    unreachable!("Unsupported subcommand `{}`", name)
                }
            }
        }
        Some(("init_vault", _sub_matches)) => {
            let client = com::Context::new_client(&config)?;
            let ctx = com::Context::new(&config, &client);
            client::init_vault(ctx)?
        }
        Some(("init_market", sub_matches)) => {
            let client = com::Context::new_client(&config)?;
            let ctx = com::Context::new(&config, &client);
            client::init_market(ctx, sub_matches)?;
        }
        Some(("init_user", sub_matches)) => {
            let client = com::Context::new_client(&config)?;
            let ctx = com::Context::new(&config, &client);
            client::init_user(ctx, sub_matches)?;
        }
        Some(("deposit", sub_matches)) => {
            let client = com::Context::new_client(&config)?;
            let ctx = com::Context::new(&config, &client);
            client::deposit(ctx, sub_matches)?;
        }
        Some(("open_position", sub_matches)) => {
            let client = com::Context::new_client(&config)?;
            let ctx = com::Context::new(&config, &client);
            client::open_position(ctx, sub_matches)?;
        }
        Some(("close_position", sub_matches)) => {
            let client = com::Context::new_client(&config)?;
            let ctx = com::Context::new(&config, &client);
            client::close_position(ctx, sub_matches)?;
        }
        Some(("investment", sub_matches)) => {
            let client = com::Context::new_client(&config)?;
            let ctx = com::Context::new(&config, &client);
            client::investment(ctx, sub_matches)?;
        }
        Some(("divestment", sub_matches)) => {
            let client = com::Context::new_client(&config)?;
            let ctx = com::Context::new(&config, &client);
            client::divestment(ctx, sub_matches)?;
        }
        Some(("bot", sub_matches)) => {
            let client = com::Context::new_client(&config)?;
            let ctx = com::Context::new(&config, &client);
            app::run(ctx, sub_matches)?
        }
        Some((ext, sub_matches)) => {
            let args = sub_matches
                .get_many::<OsString>("")
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            println!("Calling out to {:?} with {:?}", ext, args);
        }
        _ => unreachable!(),
    }
    Ok(())
}
