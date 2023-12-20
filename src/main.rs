use clap::{Args, Parser, Subcommand};
use log::{error, info};
use my_db::SelectDate;
use std::env;

mod analysis;
mod config;
mod gmo_coin;
mod jquants;
mod line_notify;
mod my_db;
mod my_error;
mod my_file_io;

#[derive(Parser)]
pub struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Stocks(MyArgs),
    Fx(MyArgs),
    /// date: YYYYMMDD
    Db {
        #[arg(short, long)]
        date: String,
    },
}

#[derive(Args)]
struct MyArgs {
    #[arg(short, long)]
    backtest: bool,
    #[arg(short, long)]
    fetch: bool,
    #[arg(short, long)]
    testrun: bool,
    #[arg(short, long, default_value = "7203")]
    code: Option<i32>,
}

#[tokio::main]
async fn main() {
    // 環境変数の読み込み
    env::set_var("RUST_LOG", "info");
    env_logger::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Stocks(args) => {
            match (args.backtest, args.testrun) {
                // live
                (false, false) => {
                    match jquants::live::fetch_nikkei225().await {
                        Ok(_) => info!("fetch_nikkei225 success"),
                        Err(e) => return error!("fetch_nikkei225 failed: {}", e),
                    };

                    let conn = my_db::open_db().unwrap();
                    let body = my_db::select_stocks(&conn, None);
                    line_notify::send_message(&body).await;
                }

                // backtesting
                (true, false) => {
                    if let true = args.fetch {
                        match jquants::backtesting::fetch_ohlcs_and_save().await {
                            Ok(_) => info!("fetch_nikkei225 success"),
                            Err(e) => return error!("fetch_nikkei225 failed: {}", e),
                        };
                    }
                    jquants::backtesting::backtesting_to_json().unwrap();
                }

                // testrun
                (false, true) => {
                    let code = args.code.unwrap_or(7203);
                    jquants::live::fetch_ohlc_once(code).await.unwrap();
                }
                _ => {}
            };
        }
        Commands::Fx(args) => {
            match (args.backtest, args.testrun) {
                // live
                (false, false) => {
                    let _ohlc_vec = gmo_coin::fx_public::fetch_gmo_coin_fx().await;
                }

                // backtesting
                (true, false) => {
                    gmo_coin::backtesting::backtesting_to_json().unwrap();
                }
                _ => {}
            };
        }
        Commands::Db { date } => {
            let year = date[0..4].parse().unwrap();
            let month = date[4..6].parse().unwrap();
            let day = date[6..8].parse().unwrap();

            let conn = my_db::open_db().unwrap();
            my_db::select_stocks(&conn, Some(SelectDate::new(year, month, day)));
        }
    }
}
