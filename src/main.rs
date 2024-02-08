use clap::{Args, Parser, Subcommand};
use database::stocks::SelectDate;
use log::{error, info};
use reqwest::Client;
use std::env;

mod analysis;
mod config;
mod database;
mod gmo_coin;
mod jquants;
mod line_notify;
mod markdown;
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
    afternoon: bool,
    #[arg(long)]
    nextday: bool,
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

    let client = Client::new();

    match &cli.command {
        Commands::Stocks(args) => {
            if args.nextday {
                line_notify::send_message(&client, "Starting Next day process")
                    .await
                    .unwrap();

                match jquants::fetcher::fetch_nikkei225_db(&client, args.force).await {
                    Ok(_) => {
                        info!("fetch_nikkei225 success");
                    }
                    Err(e) => return error!("fetch_nikkei225 failed: {}", e),
                };

                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                let day_before_5 = chrono::Local::now()
                    .checked_sub_signed(chrono::Duration::days(5))
                    .unwrap()
                    .format("%Y-%m-%d")
                    .to_string();

                let stocks_window_list =
                    match analysis::stocks_window::create_stocks_window_list_db(
                        "2023-12-01",
                        &today,
                    )
                    .await
                    {
                        Ok(output) => output,
                        Err(e) => {
                            error!("create_stocks_window_list_db failed: {}", e);
                            line_notify::send_message(
                                &client,
                                "create_stocks_window_list_db failed",
                            )
                            .await
                            .unwrap();
                            return;
                        }
                    };

                if let Err(e) = stocks_window_list.for_resistance_strategy() {
                    error!("for_resistance_strategy failed: {}", e);
                    line_notify::send_message(&client, "for_resistance_strategy failed")
                        .await
                        .unwrap();
                    return;
                };

                line_notify::send_message(&client, "Next day process, success")
                    .await
                    .unwrap();
            }

            if args.afternoon {
                line_notify::send_message(&client, "Starting Afternoon process")
                    .await
                    .unwrap();

                let prices_am = match jquants::fetcher::PricesAm::new(&client, true).await {
                    Ok(prices_am) => prices_am,
                    Err(e) => {
                        error!("fetch morning market failed: {}", e);

                        line_notify::send_message(&client, "fetch morning market failed")
                            .await
                            .unwrap();
                        return;
                    }
                };

                let stocks_afternoon_list =
                    match analysis::stocks_afternoon::StocksAfternoonList::from_nikkei225_db(
                        &prices_am,
                    ) {
                        Ok(output) => output,
                        Err(e) => {
                            error!("StocksAfternoonList::from_nikkei225_db failed: {}", e);
                            line_notify::send_message(
                                &client,
                                "StocksAfternoonList::from_nikkei225_db failed",
                            )
                            .await
                            .unwrap();
                            return;
                        }
                    };

                if let Err(e) = stocks_afternoon_list.for_resistance_strategy() {
                    error!("for_afternoon_strategy failed: {}", e);
                    line_notify::send_message(&client, "for_afternoon_strategy failed")
                        .await
                        .unwrap();
                    return;
                };

                line_notify::send_message(&client, "Success").await.unwrap();
            }

            if args.backtest {
                // if let true = args.fetch {
                //     match jquants::backtesting::fetch_ohlcs_and_save().await {
                //         Ok(_) => info!("fetch_nikkei225 success"),
                //         Err(e) => return error!("fetch_nikkei225 failed: {}", e),
                //     };
                // }
                // jquants::backtesting::backtesting_to_json().unwrap();
                let stocks_daytrading_list =
                    analysis::stocks_daytrading::async_exec("2023-07-01", "2024-01-01")
                        .await
                        .unwrap();
                // let topix_list =
                //     analysis::backtesting_topix::BacktestingTopixList::from_json_file()
                //         .unwrap();

                let topix_daily_window_list =
                    analysis::backtesting_topix::TopixDailyWindowList::new(
                        &analysis::backtesting_topix::BacktestingTopixList::from_json_file()
                            .unwrap(),
                    );

                let status = [
                    analysis::stocks_daytrading::Status::BreakoutResistance,
                    analysis::stocks_daytrading::Status::FailedBreakoutResistance,
                    analysis::stocks_daytrading::Status::FailedBreakoutSupport,
                    analysis::stocks_daytrading::Status::BreakoutSupport,
                ];
                for x in status.into_iter() {
                    let result = stocks_daytrading_list
                        .get_windows_related_result_2(x, &topix_daily_window_list);
                    info!("result: {}", result);
                }
            }

            if args.testrun {
                // let code = args.code.unwrap_or(7203);
                // let client = reqwest::Client::new();
                // jquants::live::fetch_daily_quotes_once(&client, code)
                //     .await
                //     .unwrap();

                // match jquants::fetcher::fetch_nikkei225_db(true).await {
                //     Ok(_) => info!("fetch_nikkei225 success"),
                //     Err(e) => return error!("fetch_nikkei225 failed: {}", e),
                // };

                let force = args.force;
                match jquants::fetcher::fetch_nikkei225_db(&client, force).await {
                    Ok(_) => info!("fetch_nikkei225 success"),
                    Err(e) => return error!("fetch_nikkei225 failed: {}", e),
                }

                // let from = "2023-12-01";
                // let today = chrono::Local::now().format("%Y-%m-%d").to_string();

                // let stocks_window_list =
                //     analysis::stocks_window::create_stocks_window_list(from, &today)
                //         .await
                //         .unwrap();

                // stocks_window_list.for_resistance_strategy().unwrap();

                // let prices_am = match jquants::fetcher::PricesAm::new(&client, true).await {
                //     Ok(prices_am) => prices_am,
                //     Err(e) => {
                //         error!("fetch morning market failed: {}", e);

                //         line_notify::send_message(&client, "fetch morning market failed")
                //             .await
                //             .unwrap();
                //         return;
                //     }
                // };
                // let aaa =
                //     analysis::stocks_afternoon::StocksAfternoonList::from_nikkei225(&prices_am)
                //         .unwrap();

                // aaa.for_resistance_strategy().unwrap();
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
                    line_notify::send_message_from_jquants_output(&client, output)
                        .await
                        .unwrap();
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
