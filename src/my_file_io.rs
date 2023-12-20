use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use crate::my_error::MyError;
use log::debug;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct Nikkei225 {
    code: i32,
    name: String,
    category: String,
}

impl Nikkei225 {
    //getter
    pub fn get_code(&self) -> i32 {
        self.code
    }
    pub fn get_name(&self) -> &str {
        &self.name
    }
}

pub fn load_nikkei225_list() -> Result<Vec<Nikkei225>, MyError> {
    let gdrive_path = std::env::var("GDRIVE_PATH").unwrap();
    let nikkei225_path = Path::new(&gdrive_path)
        .join("trading23")
        .join("nikkei225_lists")
        .join("20231002.csv");
    let mut rdr = match csv::Reader::from_path(nikkei225_path) {
        Ok(rdr) => rdr,
        Err(e) => return Err(MyError::Anyhow(anyhow!(e.to_string()))),
    };
    let mut nikkei225_vec = Vec::new();
    for result in rdr.deserialize() {
        let nikkei225: Nikkei225 = match result {
            Ok(nikkei225) => nikkei225,
            Err(e) => return Err(MyError::Anyhow(anyhow!(e.to_string()))),
        };
        nikkei225_vec.push(nikkei225);
    }
    debug!("{:?}", nikkei225_vec);
    Ok(nikkei225_vec)
}

pub enum AssetType {
    Stocks { code: Option<i32> },
    Fx { symbol: Option<String> },
}

pub fn get_fetched_ohlc_file_path(asset_type: AssetType) -> Result<PathBuf, MyError> {
    let gdrive_path = std::env::var("GDRIVE_PATH").unwrap();
    let fetched_ohlcs_dir_path = Path::new(&gdrive_path)
        .join("trading23")
        .join("fetched_ohlcs");

    match asset_type {
        AssetType::Stocks { code: Some(code) } => Ok(fetched_ohlcs_dir_path
            .join("jquants")
            .join(format!("{}.json", code))),
        AssetType::Stocks { code: None } => {
            Err(MyError::Anyhow(anyhow!("code is None. Please set code")))
        }
        AssetType::Fx {
            symbol: Some(symbol),
        } => Ok(fetched_ohlcs_dir_path
            .join("gmo_coin_fx")
            .join(format!("{}.json", symbol))),
        AssetType::Fx { symbol: None } => Err(MyError::Anyhow(anyhow!(
            "symbol is None. Please set symbol"
        ))),
    }
}

pub fn get_backtest_json_file_path(ohlc_type: AssetType) -> Result<PathBuf, MyError> {
    let gdrive_path = std::env::var("GDRIVE_PATH").unwrap();
    let backtest_json_parent_dir_path = Path::new(&gdrive_path)
        .join("trading23")
        .join("backtest_json");
    match ohlc_type {
        AssetType::Stocks { code: _ } => {
            Ok(backtest_json_parent_dir_path.join("jquants_backtest.json"))
        }
        AssetType::Fx { symbol: _ } => {
            Ok(backtest_json_parent_dir_path.join("gmo_coin_backtest.json"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_reader() {
        dotenvy::from_filename(".env_local").unwrap();
        let nikkei225_vec = load_nikkei225_list().unwrap();
        assert_eq!(nikkei225_vec.len(), 225);
    }
}
