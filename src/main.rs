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
mod notion;

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
        testrun: bool,
        #[arg(short, long)]
        date: String,
        #[arg(short, long)]
        notify: bool,
        #[arg(short, long)]
        sql: Option<String>,
    },
    Notion,
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
                    let output = my_db::select_stocks(&conn, None);
                    line_notify::send_message_from_jquants_output(output).await;
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
                    let last_date = jquants::live::fetch_ohlc_once(code).await.unwrap();

                    my_file_io::io_testrun(&last_date).unwrap();
                }
                _ => {}
            }
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
            }
        }
        Commands::Db {
            testrun,
            date,
            sql,
            notify,
        } => match testrun {
            false => {
                let year = date[0..4].parse().unwrap();
                let month = date[4..6].parse().unwrap();
                let day = date[6..8].parse().unwrap();

                let conn = my_db::open_db().unwrap();
                let output = my_db::select_stocks(&conn, Some(SelectDate::new(year, month, day)));
                if *notify {
                    line_notify::send_message_from_jquants_output(output).await;
                }
            }

            true => {
                // let conn = my_db::open_db().unwrap();
                match sql {
                    Some(sql) => {
                        info!("sql: {}", sql);
                        // my_db::select_stocks_manually(&conn, sql);
                    }
                    None => {
                        info!("sql statement is required")
                    }
                }
            }
        },
        Commands::Notion => {
            info!("notion");
            notion::get_notion_data().await.unwrap();
        }
    }
}
