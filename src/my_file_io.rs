use crate::my_error::MyError;
use anyhow::{anyhow, Result};
use chrono::Datelike;
use log::debug;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize, Debug)]
pub struct Nikkei225 {
    code: String,
    name: String,
    category: String,
}

impl Nikkei225 {
    //getter
    pub fn get_code(&self) -> &str {
        &self.code
    }
    pub fn get_name(&self) -> &str {
        &self.name
    }
}

pub fn load_nikkei225_list() -> Result<Vec<Nikkei225>, MyError> {
    let gdrive_path = std::env::var("GDRIVE_PATH")?;
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
        let nikkei225 = result.map_err(|e| MyError::Anyhow(anyhow!(e.to_string())))?;

        nikkei225_vec.push(nikkei225);
    }
    debug!("{:?}", nikkei225_vec);
    Ok(nikkei225_vec)
}

pub enum AssetType {
    Stocks { code: Option<String> },
    Fx { symbol: Option<String> },
}

pub fn get_fetched_ohlc_file_path(asset_type: AssetType) -> Result<PathBuf, MyError> {
    let gdrive_path = std::env::var("GDRIVE_PATH")?;
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
    let gdrive_path = std::env::var("GDRIVE_PATH")?;
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

pub fn get_topix_ohlc_file_path() -> Result<PathBuf, MyError> {
    let gdrive_path = std::env::var("GDRIVE_PATH")?;
    let backtest_json_parent_dir_path = Path::new(&gdrive_path)
        .join("trading23")
        .join("fetched_ohlcs")
        .join("jquants");
    Ok(backtest_json_parent_dir_path.join("topix.json"))
}

pub enum JquantsStyle {
    // Break,
    // Window,
    // Cloud,
    Afternoon,
    Resistance,
}

pub fn get_jquants_path(jquants_style: JquantsStyle, file_name: &str) -> Result<PathBuf, MyError> {
    let dir_name = match jquants_style {
        // JquantsStyle::Break => "jquants_break",
        // JquantsStyle::Window => "jquants_window",
        // JquantsStyle::Cloud => "jquants_cloud",
        JquantsStyle::Afternoon => "jquants_afternoon",
        JquantsStyle::Resistance => "jquants_resistance",
    };

    let gdrive_path = std::env::var("GDRIVE_PATH")?;
    let backtest_json_parent_dir_path = Path::new(&gdrive_path).join("trading23").join(dir_name);

    // "YYYY-MM-DD" であれば NaiveDateに変換してpathを作成
    match chrono::NaiveDate::parse_from_str(file_name, "%Y-%m-%d") {
        Ok(datetime) => {
            let year = datetime.year();
            let month = datetime.month();
            let day = datetime.day();

            let path = backtest_json_parent_dir_path
                .join(format!("{}-{}", year, month))
                .join(format!("{}", day));
            Ok(path)
        }
        Err(_) => {
            debug!("{}", file_name);
            Ok(backtest_json_parent_dir_path.join(file_name))
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

    #[test]
    fn test_chrono_parse() {
        let file_name = "2021-01-01";
        let datetime = chrono::NaiveDate::parse_from_str(file_name, "%Y-%m-%d").unwrap();
        assert_eq!(datetime.year(), 2021);
    }
}
