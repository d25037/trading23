use super::fx_public::{Interval, KLineQueryParams, PriceType, Symbol};
use crate::analysis::backtesting::BacktestAnalyzer;
use crate::analysis::live::Ohlc;
use crate::my_error::MyError;
use crate::my_file_io::{get_backtest_json_file_path, get_fetched_ohlc_file_path, AssetType};
use anyhow::{anyhow, Result};
use chrono::Local;
use log::{error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::Duration as StdDuration;
use std::{fs::File, io::Write};

pub async fn _fetch_ohlc_for_backtesting(
    symbol: Symbol,
    interval: Interval,
) -> Result<(), MyError> {
    let client = Client::new();
    let params = KLineQueryParams::new(symbol, PriceType::Bid, interval, Local::now());

    let mut ohlc_vec: Vec<Ohlc> = Vec::new();
    for delta in 0..1000 {
        let day = params.get_date_with_delta(delta);
        if day == "20231027" {
            break;
        }

        thread::sleep(StdDuration::from_secs(2));

        match params.fetch_klines_with_delta(&client, delta).await {
            Ok(ohlc_vec_delta) => {
                let ohlc_vec_delta = ohlc_vec_delta.into_iter().rev().collect::<Vec<Ohlc>>();
                ohlc_vec.extend(ohlc_vec_delta)
            }
            Err(e) => match e {
                MyError::Holiday => {
                    info!("{} is Holiday", day);
                    continue;
                }
                _ => return Err(e),
            },
        }
    }

    ohlc_vec.reverse();

    match serde_json::to_string(&ohlc_vec) {
        Ok(res) => {
            let path = get_fetched_ohlc_file_path(AssetType::Fx {
                symbol: Some(params.get_symbol().to_string()),
            })
            .unwrap();
            std::fs::write(path, res).unwrap();
        }
        Err(e) => {
            error!("{}", e);
            return Err(MyError::Anyhow(anyhow!("{}", e)));
        }
    }
    info!("{} has been saved", params.get_symbol().to_string());

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct GmoCoinFxBacktest {
    symbol: String,
    #[serde(flatten)]
    backtest_analyzer: BacktestAnalyzer,
}

impl GmoCoinFxBacktest {
    fn new(symbol: String, day: usize) -> Result<Self, MyError> {
        let path = get_fetched_ohlc_file_path(AssetType::Fx {
            symbol: Some(symbol.clone()),
        })
        .unwrap();
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
            symbol,
            backtest_analyzer,
        })
    }
}

pub fn backtesting_to_json() -> Result<(), MyError> {
    let symbols = vec![
        Symbol::UsdJpy,
        Symbol::EurJpy,
        Symbol::GbpJpy,
        Symbol::AudJpy,
        Symbol::EurUsd,
        Symbol::GbpUsd,
        Symbol::AudUsd,
    ];

    let mut backtest_analyzer_vec: Vec<GmoCoinFxBacktest> = Vec::new();

    for symbol in symbols {
        info!("symbol: {} start", symbol);
        for step in (0..=1600).step_by(5) {
            match GmoCoinFxBacktest::new(symbol.to_string(), step) {
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

    let json_path = get_backtest_json_file_path(AssetType::Fx { symbol: None })?;

    //backtest_analyzer_vecをjsonに変換
    let json = serde_json::to_string(&backtest_analyzer_vec).unwrap();
    //jsonをファイルに書き込み
    let mut file = File::create(&json_path).unwrap();
    file.write_all(json.as_bytes()).unwrap();

    info!("saved to {}", json_path.to_str().unwrap());

    Ok(())
}
