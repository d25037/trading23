use analysis::backtesting;
use clap::{Args, Parser, Subcommand};
use core::arch;
use database::stocks::SelectDate;
use log::{error, info};
use std::{ascii::AsciiExt, env};

mod analysis;
mod config;
mod database;
mod gmo_coin;
mod jquants;
mod line_notify;
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
        #[arg(long)]
        testrun: bool,
        #[arg(long)]
        date: Option<String>,
        #[arg(long)]
        notify: bool,
    },
    Notion,
}

#[derive(Args)]
struct MyArgs {
    #[arg(long)]
    backtest: bool,
    #[arg(long)]
    fetch: bool,
    #[arg(long)]
    testrun: bool,
    #[arg(long, default_value = "7203")]
    code: Option<i32>,
    #[arg(long)]
    force: bool,
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
                    // match jquants::live::fetch_nikkei225(args.force).await {
                    //     Ok(_) => info!("fetch_nikkei225 success"),
                    //     Err(e) => match e {
                    //         my_error::MyError::NotLatestData => return error!("{}", e),
                    //         _ => return error!("fetch_nikkei225 failed: {}", e),
                    //     },
                    // };

                    // let conn = database::stocks::open_db().unwrap();
                    // let output = database::stocks::select_stocks(&conn, None);
                    // line_notify::send_message_from_jquants_output(output).await;
                    let output = match jquants::live::fetch_nikkei225_daytrading(args.force).await {
                        Ok(output) => {
                            info!("fetch_nikkei225 success");
                            output
                        }
                        Err(e) => match e {
                            my_error::MyError::NotLatestData => return error!("{}", e),
                            _ => return error!("fetch_nikkei225 failed: {}", e),
                        },
                    };

                    line_notify::send_message_from_jquants_daytrading(output).await;
                }

                // backtesting
                (true, false) => {
                    // if let true = args.fetch {
                    //     match jquants::backtesting::fetch_ohlcs_and_save().await {
                    //         Ok(_) => info!("fetch_nikkei225 success"),
                    //         Err(e) => return error!("fetch_nikkei225 failed: {}", e),
                    //     };
                    // }
                    // jquants::backtesting::backtesting_to_json().unwrap();
                    let mut stocks_daytrading_list =
                        analysis::stocks_daytrading::async_exec("2019-04-01", "2023-12-28")
                            .await
                            .unwrap();
                    let topix_list =
                        analysis::backtesting_topix::BacktestingTopixList::from_json_file()
                            .unwrap();

                    let limit = [(0.06, 0.09), (0.09, 0.12), (0.12, 0.15)];
                    info!("strong positive window");
                    for (lower_limit, upper_limit) in limit.iter() {
                        info!("lower_limit: {}, upper_limit: {}", lower_limit, upper_limit);
                        stocks_daytrading_list.get_window_related_result(
                            topix_list.get_strong_positive_window_list(),
                            *lower_limit,
                            *upper_limit,
                        );
                    }
                    info!("mild positive window");
                    for (lower_limit, upper_limit) in limit.iter() {
                        info!("lower_limit: {}, upper_limit: {}", lower_limit, upper_limit);

                        stocks_daytrading_list.get_window_related_result(
                            topix_list.get_mild_positive_window_list(),
                            *lower_limit,
                            *upper_limit,
                        );
                    }
                    info!("mild negative window");
                    for (lower_limit, upper_limit) in limit.iter() {
                        info!("lower_limit: {}, upper_limit: {}", lower_limit, upper_limit);

                        stocks_daytrading_list.get_window_related_result(
                            topix_list.get_mild_negative_window_list(),
                            *lower_limit,
                            *upper_limit,
                        );
                    }
                    info!("strong negative window");
                    for (lower_limit, upper_limit) in limit.iter() {
                        info!("lower_limit: {}, upper_limit: {}", lower_limit, upper_limit);

                        stocks_daytrading_list.get_window_related_result(
                            topix_list.get_strong_negative_window_list(),
                            *lower_limit,
                            *upper_limit,
                        );
                    }
                }

                // testrun
                (false, true) => {
                    let code = args.code.unwrap_or(7203);
                    let client = reqwest::Client::new();
                    jquants::live::fetch_daily_quotes_once(&client, code)
                        .await
                        .unwrap();
                    let topix = jquants::live::Topix::new(&client).await.unwrap();
                    topix.save_to_json_file().unwrap();
                    let backtesting_topix_list =
                        analysis::backtesting_topix::BacktestingTopixList::from_json_file()
                            .unwrap();
                    info!("{:?}", backtesting_topix_list.get_positive_window_list());
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
            notify,
        } => match testrun {
            // live
            false => {
                let date = match date {
                    Some(date) => date,
                    None => {
                        return error!("date is required");
                    }
                };
                let year = date[0..4].parse().unwrap();
                let month = date[4..6].parse().unwrap();
                let day = date[6..8].parse().unwrap();

                let conn = database::stocks::open_db().unwrap();
                let output =
                    database::stocks::select_stocks(&conn, Some(SelectDate::new(year, month, day)));
                if *notify {
                    line_notify::send_message_from_jquants_output(output).await;
                }
            }

            // testrun
            true => {
                let conn = database::stocks::open_db().unwrap();
                let all_stocks = database::stocks::select_all_stocks(&conn);
                info!("all_stocks: {}", all_stocks.len());
                info!("all_stocks: {:?}", all_stocks);
            }
        },
        Commands::Notion => {
            info!("notion");
            notion::get_notion_data().await.unwrap();
        }
    }
}
