use super::live::{first_fetch, DailyQuotes};
use crate::analysis::backtesting::BacktestAnalyzer;
use crate::analysis::live::Ohlc;
use crate::my_error::MyError;
use crate::my_file_io::{get_backtest_json_file_path, get_fetched_ohlc_file_path, AssetType};
use anyhow::anyhow;
use log::{error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::Duration;
use std::{fs::File, io::Write};

pub async fn fetch_ohlcs_and_save() -> Result<(), MyError> {
    let client = Client::new();

    info!("Starting First Fetch");
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    if let Err(e) = first_fetch(&client, Some(&today)).await {
        error!("{}", e);
        return Err(e);
    }

    let nikkei225 = match crate::my_file_io::load_nikkei225_list() {
        Ok(res) => res,
        Err(e) => {
            error!("{}", e);
            return Err(e);
        }
    };
    info!("Nikkei225 has been loaded");

    info!("Starting Fetch Nikkei225");

    for row in nikkei225 {
        thread::sleep(Duration::from_secs(2));

        let code = row.get_code();

        let daily_quotes: DailyQuotes = match DailyQuotes::new(&client, code).await {
            Ok(res) => res,
            Err(e) => {
                error!("{}", e);
                return Err(e);
            }
        };

        let raw_ohlc: Vec<Ohlc> = daily_quotes.get_ohlc();
        // code.jsonを保存
        match serde_json::to_string(&raw_ohlc) {
            Ok(res) => {
                let path =
                    get_fetched_ohlc_file_path(AssetType::Stocks { code: Some(code) }).unwrap();
                std::fs::write(path, res).unwrap();
            }
            Err(e) => {
                error!("{}", e);
                return Err(MyError::Anyhow(anyhow!("{}", e)));
            }
        }
        info!("{} has been saved", code)
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct StocksBacktest {
    code: i32,
    #[serde(flatten)]
    backtest_analyzer: BacktestAnalyzer,
}

impl StocksBacktest {
    fn new(code: i32, day: usize) -> Result<Self, MyError> {
        let path = get_fetched_ohlc_file_path(AssetType::Stocks { code: Some(code) }).unwrap();
        let raw_ohlc: Vec<Ohlc> =
            match serde_json::from_str(&std::fs::read_to_string(path).unwrap()) {
                Ok(res) => res,
                Err(e) => {
                    error!("{}", e);
                    return Err(MyError::Anyhow(anyhow!("{}", e)));
                }
            };
        if raw_ohlc.len() - day < 82 {
            return Err(MyError::OutOfRange);
        }

        let backtest_analyzer = BacktestAnalyzer::new(raw_ohlc, day)?;

        Ok(Self {
            code,
            backtest_analyzer,
        })
    }
}

pub fn backtesting_to_json() -> Result<(), MyError> {
    let nikkei225 = match crate::my_file_io::load_nikkei225_list() {
        Ok(res) => res,
        Err(e) => {
            error!("{}", e);
            return Err(e);
        }
    };
    info!("Nikkei225 has been loaded");

    let mut backtest_analyzer_vec: Vec<StocksBacktest> = Vec::new();

    for (i, row) in nikkei225.iter().enumerate() {
        if i % 20 == 0 {
            info!("{} / {}", i, nikkei225.len());
        }

        let code = row.get_code();
        for step in (0..=1200).step_by(9) {
            match StocksBacktest::new(code, step) {
                Ok(backtest_analyzer) => backtest_analyzer_vec.push(backtest_analyzer),
                Err(e) => match e {
                    MyError::OutOfRange => break,
                    _ => {
                        error!("{}", e);
                        return Err(e);
                    }
                },
            }
        }
    }

    let json_path = get_backtest_json_file_path(AssetType::Stocks { code: None })?;

    //backtest_analyzer_vecをserialize
    let serialized = serde_json::to_string(&backtest_analyzer_vec).unwrap();
    //jsonをファイルに書き込み
    let mut file = File::create(&json_path).unwrap();
    file.write_all(serialized.as_bytes()).unwrap();

    info!("saved to {}", json_path.to_str().unwrap());
    Ok(())
}

pub fn _backtesting_len() -> Result<(), MyError> {
    let nikkei225 = match crate::my_file_io::load_nikkei225_list() {
        Ok(res) => res,
        Err(e) => {
            error!("{}", e);
            return Err(e);
        }
    };
    info!("Nikkei225 has been loaded");

    let mut max_len = 0;
    for row in nikkei225 {
        let code = row.get_code();
        let path = format!("./jquants_ohlcs/{}.json", code);
        let raw_ohlc: Vec<Ohlc> =
            match serde_json::from_str(&std::fs::read_to_string(path).unwrap()) {
                Ok(res) => res,
                Err(e) => {
                    error!("{}", e);
                    return Err(MyError::Anyhow(anyhow!("{}", e)));
                }
            };
        if raw_ohlc.len() > max_len {
            max_len = raw_ohlc.len();
        }
    }
    info!("max_len:{}", max_len);
    Ok(())
}
