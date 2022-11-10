use crate::com;
use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::solana_sdk::system_program;
use anchor_client::ClientError;
use bond::state::{market, position, user};
use bond::{accounts, com as bcom, instruction};
use log::debug;
use spl_associated_token_account;
use spl_token;
pub fn init_vault(ctx: com::Context) -> anyhow::Result<()> {
    let program = ctx.client.program(com::id());

    let (vault_account, bump) =
        Pubkey::find_program_address(&[bcom::VAULT_TOKEN_ACCOUNT_SEED], &com::id());
    let (vault_authority_account, abump) =
        Pubkey::find_program_address(&[bcom::VAULT_TOKEN_AUTHORITY_SEED], &com::id());
    let tx = program
        .request()
        .accounts(accounts::InitializeVault {
            initializer: program.payer(),
            vault_account,
            token_mint: ctx.config.accounts.spl_mint,
            system_program: system_program::id(),
            token_program: spl_token::id(),
            rent: solana_program::sysvar::rent::id(),
        })
        .args(instruction::InitializeVault { bump })
        .send()
        .map_err(|e| debug_rpc_error(e))?;
    println!("init vault success!\nvault account: {:?}\nbump: {:?}\nvault authority account: {:?}\nauthority bump: {:?}\ntx: {}",vault_account,bump,vault_authority_account,abump,tx);
    Ok(())
}

pub fn init_market(ctx: com::Context, args: &clap::ArgMatches) -> anyhow::Result<()> {
    let program = ctx.client.program(com::id());
    let pair = args.get_one::<String>("pair").expect("pair missing");
    let spread = args.get_one::<f64>("spread").expect("spread missing");
    let pyth_price_account = args
        .get_one::<String>("pyth_account")
        .expect("pyth_account missing");
    let chianlink_price_account = args
        .get_one::<String>("chain_link_account")
        .expect("chain_link_account missing");
    let (market_account, bump) =
        Pubkey::find_program_address(&[bcom::MARKET_ACCOUNT_SEED, pair.as_bytes()], &program.id());

    let tx = program
        .request()
        .accounts(accounts::InitializeMarket {
            initializer: program.payer(),
            market_account: market_account,
            system_program: system_program::id(),
        })
        .args(instruction::InitializeMarket {
            pair: pair.to_string(),
            spread: spread.to_degrees(),
            bump,
            pyth_price_account: pyth_price_account.to_string(),
            chianlink_price_account: chianlink_price_account.to_string(),
        })
        .send()
        .map_err(|e| debug_rpc_error(e))?;
    println!("init market account success!\nmarket account: {:?}\npair: {:?}\nspread: {:?}\nbump: {}\npyth_price_account: {}\nchianlink_price_account: {}\ntx:{}",
    market_account,
    pair,
    spread,
    bump,
    pyth_price_account,
    chianlink_price_account,
    tx
    );
    Ok(())
}

pub fn init_user(ctx: com::Context, _args: &clap::ArgMatches) -> anyhow::Result<()> {
    let program = ctx.client.program(com::id());
    let (user_account, bump) = Pubkey::find_program_address(
        &[bcom::USER_ACCOUNT_SEED, &program.payer().to_bytes()],
        &program.id(),
    );
    let tx = program
        .request()
        .accounts(accounts::InitUserAccount {
            initializer: program.payer(),
            user_account,
            system_program: system_program::id(),
        })
        .args(instruction::InitializeUserAccount { bump })
        .send()
        .map_err(|e| debug_rpc_error(e))?;
    println!(
        "init user account success!\nuser account: {}\ntx:{}",
        user_account, tx
    );
    Ok(())
}

pub fn deposit(ctx: com::Context, args: &clap::ArgMatches) -> anyhow::Result<()> {
    let program = ctx.client.program(com::id());
    let amount = args.get_one::<u64>("amount").expect("missing amount");
    if *amount <= 0u64 {
        panic!("Please fill in the valid deposit amount");
    }
    let (user_account, _bump) = Pubkey::find_program_address(
        &[bcom::USER_ACCOUNT_SEED, &program.payer().to_bytes()],
        &program.id(),
    );

    let (vault_account, _bump) =
        Pubkey::find_program_address(&[bcom::VAULT_TOKEN_ACCOUNT_SEED], &com::id());
    let mint = ctx.config.accounts.spl_mint;
    let user_token_account =
        spl_associated_token_account::get_associated_token_address(&program.payer(), &mint);
    let tx = program
        .request()
        .accounts(accounts::Deposit {
            authority: program.payer(),
            token_mint: mint,
            user_token_account,
            user_account,
            vault_token_account: vault_account,
            token_program: spl_token::id(),
        })
        .args(instruction::Deposit { amount: *amount })
        .send()
        .map_err(|e| debug_rpc_error(e))?;

    let u: user::UserAccount = program
        .account(user_account)
        .map_err(|e| debug_rpc_error(e))?;
    println!(
        "deposit success!\nuser account: {}\nuser account balance: {:#?}\ntx:{}",
        user_account, u.balance, tx
    );
    Ok(())
}
pub fn open_position(ctx: com::Context, args: &clap::ArgMatches) -> anyhow::Result<()> {
    let program = ctx.client.program(com::id());
    let pair = args.get_one::<String>("pair").expect("missing pair");
    let size = args.get_one::<f64>("size").expect("missing size");
    let leverage = args.get_one::<u16>("leverage").expect("missing leverage");
    let position_type = args
        .get_one::<u8>("position_type")
        .expect("missing position_type");
    let direction = args.get_one::<u8>("direction").expect("missing direction");
    if *size < 0.0 {
        panic!("invalid size");
    }
    if *position_type != 1 && *position_type != 2 {
        panic!("invalid position type");
    }
    if *direction != 1 && *direction != 2 {
        panic!("invalid direction");
    }
    let (market_account, __bump) =
        Pubkey::find_program_address(&[bcom::MARKET_ACCOUNT_SEED, pair.as_bytes()], &program.id());
    let m: market::Market = program
        .account(market_account)
        .map_err(|e| debug_rpc_error(e))?;
    let (user_account, _bump) = Pubkey::find_program_address(
        &[bcom::USER_ACCOUNT_SEED, &program.payer().to_bytes()],
        &program.id(),
    );
    let u: user::UserAccount = program
        .account(user_account)
        .map_err(|e| debug_rpc_error(e))?;
    let (position_account, _pbump) = Pubkey::find_program_address(
        &[
            bcom::POSITION_ACCOUNT_SEED,
            &program.payer().to_bytes(),
            &user_account.to_bytes(),
            &u.position_seed_offset.to_string().as_bytes(),
        ],
        &program.id(),
    );
    let market_account_btc = bcom::FullPositionMarket::BtcUsd.to_pubkey().0;
    let market_account_eth = bcom::FullPositionMarket::EthUsd.to_pubkey().0;
    let market_account_sol = bcom::FullPositionMarket::SolUsd.to_pubkey().0;
    let tx = program
        .request()
        .accounts(accounts::OpenPosition {
            authority: program.payer(),
            market_account,
            pyth_price_account: m.pyth_price_account,
            chianlink_price_account: m.chianlink_price_account,
            user_account,
            position_account,
            market_account_btc,
            market_account_eth,
            market_account_sol,
            pyth_price_account_btc: *ctx
                .config
                .accounts
                .pyth
                .get(&bcom::FullPositionMarket::BtcUsd.to_string())
                .unwrap(),
            pyth_price_account_eth: *ctx
                .config
                .accounts
                .pyth
                .get(&bcom::FullPositionMarket::EthUsd.to_string())
                .unwrap(),
            pyth_price_account_sol: *ctx
                .config
                .accounts
                .pyth
                .get(&bcom::FullPositionMarket::SolUsd.to_string())
                .unwrap(),
            chainlink_price_account_btc: *ctx
                .config
                .accounts
                .chainlink
                .get(&bcom::FullPositionMarket::BtcUsd.to_string())
                .unwrap(),
            chainlink_price_account_eth: *ctx
                .config
                .accounts
                .chainlink
                .get(&bcom::FullPositionMarket::EthUsd.to_string())
                .unwrap(),
            chainlink_price_account_sol: *ctx
                .config
                .accounts
                .chainlink
                .get(&bcom::FullPositionMarket::SolUsd.to_string())
                .unwrap(),
            system_program: system_program::id(),
        })
        .args(instruction::OpenPosition {
            pair: pair.to_string(),
            size: *size,
            leverage: *leverage,
            position_type: *position_type,
            direction: *direction,
        })
        .send()
        .map_err(|e| debug_rpc_error(e))?;
    let p: position::Position = program
        .account(position_account)
        .map_err(|e| debug_rpc_error(e))?;
    println!(
        r#"open position success!
market pair: {:?}
position account: {}
open_price: {:#?}
size: {:#?}
leverage: {:#?}
margin: {:#?}
position_type: {:#?}
direction: {:#?}
position_seed_offset: {:#?}
tx:{}"#,
        pair,
        position_account,
        p.open_price,
        p.size,
        p.leverage,
        p.margin,
        p.position_type,
        p.direction,
        p.position_seed_offset,
        tx
    );
    Ok(())
}
pub fn close_position(ctx: com::Context, args: &clap::ArgMatches) -> anyhow::Result<()> {
    let program = ctx.client.program(com::id());
    let (user_account, _bump) = Pubkey::find_program_address(
        &[bcom::USER_ACCOUNT_SEED, &program.payer().to_bytes()],
        &program.id(),
    );
    let position_account: Pubkey = match args.get_one::<String>("account") {
        Some(a) => Pubkey::try_from(a.as_str()).expect("invalid position account"),
        None => match args.get_one::<u32>("offset") {
            Some(o) => {
                let (position_account, _pbump) = Pubkey::find_program_address(
                    &[
                        bcom::POSITION_ACCOUNT_SEED,
                        &program.payer().to_bytes(),
                        &user_account.to_bytes(),
                        &o.to_string().as_bytes(),
                    ],
                    &program.id(),
                );
                position_account
            }
            None => {
                panic!("invalid params")
            }
        },
    };

    let p: position::Position = program
        .account(position_account)
        .map_err(|e| debug_rpc_error(e))?;

    let m: market::Market = program
        .account(p.market_account)
        .map_err(|e| debug_rpc_error(e))?;

    let tx = program
        .request()
        .accounts(accounts::ClosePosition {
            authority: program.payer(),
            market_account: p.market_account,
            pyth_price_account: m.pyth_price_account,
            chianlink_price_account: m.chianlink_price_account,
            user_account,
            position_account,
        })
        .send()
        .map_err(|e| debug_rpc_error(e))?;

    let p: position::Position = program
        .account(position_account)
        .map_err(|e| debug_rpc_error(e))?;
    println!(
        r#"close position success!
market pair: {:?}
position account: {}
open_price: {:#?}
close_price: {:#?}
size: {:#?}
leverage: {:#?}
margin: {:#?}
position_type: {:#?}
direction: {:#?}
position_seed_offset: {:#?}
profit: {:?}
tx:{}"#,
        m.pair,
        position_account,
        p.open_price,
        p.close_price,
        p.size,
        p.leverage,
        p.margin,
        p.position_type,
        p.direction,
        p.position_seed_offset,
        p.profit,
        tx
    );
    Ok(())
}
pub fn burst_position(
    client: &anchor_client::Client,
    user_account: Pubkey,
    market_account: Pubkey,
    position_account: Pubkey,
    pyth_price_account: Pubkey,
    chianlink_price_account: Pubkey,
) -> anyhow::Result<()> {
    let program = client.program(com::id());
    let tx = program
        .request()
        .accounts(accounts::ClosePosition {
            authority: program.payer(),
            market_account,
            pyth_price_account,
            chianlink_price_account,
            user_account,
            position_account,
        })
        .send()
        .map_err(|e| debug_rpc_error(e))?;
    debug!("burst position success! tx: {}", tx);
    Ok(())
}
pub fn investment(ctx: com::Context, args: &clap::ArgMatches) -> anyhow::Result<()> {
    let program = ctx.client.program(com::id());
    let pair = args.get_one::<String>("pair").expect("missing pair");
    let amount = args.get_one::<u64>("amount").expect("missing amount");
    if *amount <= 0u64 {
        panic!("Please fill in the valid investment amount");
    }
    let (market_account, __bump) =
        Pubkey::find_program_address(&[bcom::MARKET_ACCOUNT_SEED, pair.as_bytes()], &program.id());

    let (vault_token_account, _bump) =
        Pubkey::find_program_address(&[bcom::VAULT_TOKEN_ACCOUNT_SEED], &com::id());
    let mint = ctx.config.accounts.spl_mint;
    let user_token_account =
        spl_associated_token_account::get_associated_token_address(&program.payer(), &mint);
    let tx = program
        .request()
        .accounts(accounts::Investment {
            user: program.payer(),
            user_token_account,
            market_account,
            token_mint: mint,
            vault_token_account,
            token_program: spl_token::id(),
        })
        .args(instruction::Investment {
            pair: pair.to_string(),
            amount: *amount,
        })
        .send()
        .map_err(|e| debug_rpc_error(e))?;

    let m: market::Market = program
        .account(market_account)
        .map_err(|e| debug_rpc_error(e))?;
    println!(
        r#"investment market success!
market pair:{:?}
market account: {}
vault_full: {:#?}
vault base balance: {:#?}
tx:{}"#,
        pair, market_account, m.vault_full, m.vault_base_balance, tx
    );
    Ok(())
}
pub fn divestment(ctx: com::Context, args: &clap::ArgMatches) -> anyhow::Result<()> {
    let program = ctx.client.program(com::id());
    let pair = args.get_one::<String>("pair").expect("missing pair");
    let amount = args.get_one::<u64>("amount").expect("missing amount");
    if *amount <= 0u64 {
        panic!("Please fill in the valid investment amount");
    }
    let (market_account, __bump) =
        Pubkey::find_program_address(&[bcom::MARKET_ACCOUNT_SEED, pair.as_bytes()], &program.id());

    let (vault_token_account, _bump) =
        Pubkey::find_program_address(&[bcom::VAULT_TOKEN_ACCOUNT_SEED], &com::id());

    let (pda_authority_account, _vbump) =
        Pubkey::find_program_address(&[bcom::VAULT_TOKEN_AUTHORITY_SEED], &com::id());
    let mint = ctx.config.accounts.spl_mint;
    let user_token_account =
        spl_associated_token_account::get_associated_token_address(&program.payer(), &mint);
    let tx = program
        .request()
        .accounts(accounts::Divestment {
            user: program.payer(),
            user_token_account,
            market_account,
            token_mint: mint,
            vault_token_account,
            token_program: spl_token::id(),
            pda_authority_account,
        })
        .args(instruction::Divestment {
            pair: pair.to_string(),
            amount: *amount,
        })
        .send()
        .map_err(|e| debug_rpc_error(e))?;

    let m: market::Market = program
        .account(market_account)
        .map_err(|e| debug_rpc_error(e))?;
    println!(
        r#"divestment market success!
market pair:{:?}
market account: {}
vault_full: {:#?}
vault base balance: {:#?}
tx:{}"#,
        pair, market_account, m.vault_full, m.vault_base_balance, tx
    );
    Ok(())
}
fn debug_rpc_error(e: ClientError) -> com::CliError {
    let err = com::CliError::Unknown(e.to_string());
    match e {
        ClientError::AccountNotFound => {
            debug!("rpc error: Account not found");
        }
        ClientError::AnchorError(err) => {
            debug!("{:#?}", err);
        }
        ClientError::ProgramError(err) => {
            debug!("{:#?}", err)
        }
        ClientError::SolanaClientError(err) => {
            debug!("{:#?}", err);
        }
        ClientError::SolanaClientPubsubError(err) => {
            debug!("{:#?}", err)
        }
        ClientError::LogParseError(err) => {
            debug!("{:#?}", err)
        }
    }
    err
}
