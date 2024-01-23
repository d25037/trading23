use clap::{Args, Parser, Subcommand};
use database::stocks::SelectDate;
use log::{error, info};
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
    backtest: bool,
    #[arg(long)]
    fetch: bool,
    #[arg(long)]
    testrun: bool,
    #[arg(long, default_value = "7203")]
    code: Option<i32>,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    afternoon: bool,
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
                (false, false) => match args.afternoon {
                    true => {
                        line_notify::send_message("Afternoon process, start").await;

                        let client = reqwest::Client::new();
                        let prices_am = jquants::live::PricesAm::new(&client).await.unwrap();
                        let aaa = analysis::stocks_afternoon::StocksAfternoonList::from_nikkei225(
                            &prices_am,
                        )
                        .unwrap();

                        aaa.for_afternoon_strategy().unwrap();
                        line_notify::send_message("Afternoon process, success").await;
                    }
                    false => {
                        line_notify::send_message("Next day process, start").await;

                        match jquants::live::fetch_nikkei225(args.force).await {
                            Ok(output) => {
                                info!("fetch_nikkei225 success");
                                output
                            }
                            Err(e) => match e {
                                my_error::MyError::NotLatestData => return error!("{}", e),
                                _ => return error!("fetch_nikkei225 failed: {}", e),
                            },
                        };

                        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                        let day_before_5 = chrono::Local::now()
                            .checked_sub_signed(chrono::Duration::days(5))
                            .unwrap()
                            .format("%Y-%m-%d")
                            .to_string();

                        let stocks_window_list =
                            analysis::stocks_window::create_stocks_window_list(
                                &day_before_5,
                                &today,
                            )
                            .await
                            .unwrap();
                        stocks_window_list.for_cloud_strategy().unwrap();

                        line_notify::send_message("Next day process, success").await;
                    }
                },

                // backtesting
                (true, false) => {
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

                // testrun
                (false, true) => {
                    // let code = args.code.unwrap_or(7203);
                    // let client = reqwest::Client::new();
                    // jquants::live::fetch_daily_quotes_once(&client, code)
                    //     .await
                    //     .unwrap();

                    let from = "2023-12-01";
                    let to = "2024-01-22";
                    // let mut break_output =
                    //     analysis::stocks_daytrading::async_exec(someday, someday)
                    //         .await
                    //         .unwrap();

                    // break_output.sort_by_standardized_diff();
                    // let break_markdown = break_output.output_for_markdown(someday);
                    // let break_path = my_file_io::get_jquants_break_path(someday).unwrap();
                    // break_markdown.write_to_file(&break_path);

                    let stocks_window_list =
                        analysis::stocks_window::create_stocks_window_list(from, to)
                            .await
                            .unwrap();

                    // stocks_window_list.ccc().unwrap();

                    let client = reqwest::Client::new();
                    let prices_am = jquants::live::PricesAm::new(&client).await.unwrap();
                    let aaa =
                        analysis::stocks_afternoon::StocksAfternoonList::from_nikkei225(&prices_am)
                            .unwrap();

                    aaa.for_afternoon_strategy().unwrap();

                    // analysis::stocks_window::mean_analysis(stocks_window_list, from, to)
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
