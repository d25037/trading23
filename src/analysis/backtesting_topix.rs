use crate::jquants::fetcher::Topix;
use crate::my_error::MyError;
use chrono::{Datelike, NaiveDate};
use log::info;
// use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fs::File;

#[derive(Serialize, Deserialize, Debug)]
pub struct BacktestingTopix {
    date: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    next_open: f64,
    window: f64,
    window_diff: f64,
    weekday: String,
}

pub struct BacktestingTopixList {
    data: Vec<BacktestingTopix>,
}
impl BacktestingTopixList {
    // pub async fn from_fetch_topix(client: &Client) -> Result<Self, MyError> {
    //     let topix = Topix::new(client).await?;

    //     Ok(Self {
    //         data: Self::into_backtesting_topix_list(topix),
    //     })
    // }

    pub fn from_json_file() -> Result<Self, MyError> {
        let path = crate::my_file_io::get_topix_ohlc_file_path().unwrap();
        let file = File::open(path).unwrap();
        let topix: Topix = serde_json::from_reader(file).unwrap();

        Ok(Self {
            data: Self::into_backtesting_topix_list(topix),
        })
    }

    fn into_backtesting_topix_list(topix: Topix) -> Vec<BacktestingTopix> {
        let mut backtesting_topix = Vec::new();
        for i in 0..topix.get_len_of_topix() - 1 {
            let ohlc = topix.get_ohlc(i);
            let date = NaiveDate::parse_from_str(ohlc.get_date(), "%Y-%m-%d").unwrap();
            let weekday = date.weekday().to_string();
            let next_open = topix.get_ohlc(i + 1).get_open();
            let window = ((next_open - ohlc.get_close()) * 100.0).round() / 100.0;
            let window_diff = (next_open / ohlc.get_close() * 1000.0).round() / 1000.0;
            let backtesting_inner = BacktestingTopix {
                date: ohlc.get_date().to_string(),
                open: ohlc.get_open(),
                high: ohlc.get_high(),
                low: ohlc.get_low(),
                close: ohlc.get_close(),
                next_open,
                window,
                window_diff,
                weekday,
            };
            backtesting_topix.push(backtesting_inner);
        }
        backtesting_topix
    }

    pub fn get_positive_window_list(&self) -> (Vec<String>, Vec<String>, Vec<String>) {
        let (lower_tertile, upper_tertile) = self.get_positive_window_tertile();

        let mut strong_positive_window_list = Vec::new();
        let mut moderate_positive_window_list = Vec::new();
        let mut mild_positive_window_list = Vec::new();

        for x in &self.data {
            if x.window_diff > upper_tertile {
                strong_positive_window_list.push(x.date.to_string());
            } else if x.window_diff > lower_tertile {
                moderate_positive_window_list.push(x.date.to_string());
            } else if x.window_diff > 1.0 {
                mild_positive_window_list.push(x.date.to_string());
            } else {
                // do nothing
            };
        }

        (
            strong_positive_window_list,
            moderate_positive_window_list,
            mild_positive_window_list,
        )

        // positive_window_list
    }

    // fn get_positive_window_mean(&self) -> f64 {
    //     let positive_window_diffs: Vec<f64> = self
    //         .data
    //         .iter()
    //         .filter(|x| x.window_diff > 1.0)
    //         .map(|x| x.window_diff)
    //         .collect();

    //     let sum: f64 = positive_window_diffs.iter().sum();
    //     let mean = sum / positive_window_diffs.len() as f64;
    //     mean
    // }
    fn get_positive_window_median(&self) -> f64 {
        let mut positive_window_diffs: Vec<f64> = self
            .data
            .iter()
            .filter(|x| x.window_diff > 1.0)
            .map(|x| x.window_diff)
            .collect();

        positive_window_diffs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        let median_index = positive_window_diffs.len() / 2;
        positive_window_diffs[median_index]
    }
    fn get_positive_window_tertile(&self) -> (f64, f64) {
        let mut positive_window_diffs: Vec<f64> = self
            .data
            .iter()
            .filter(|x| x.window_diff > 1.0)
            .map(|x| x.window_diff)
            .collect();

        positive_window_diffs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        let lower_tertile_index = positive_window_diffs.len() / 3;
        let upper_tertile_index = positive_window_diffs.len() * 2 / 3;
        info!(
            "lower_tertile: {}, upper_tertile: {}",
            positive_window_diffs[lower_tertile_index], positive_window_diffs[upper_tertile_index]
        );
        (
            positive_window_diffs[lower_tertile_index],
            positive_window_diffs[upper_tertile_index],
        )
    }

    pub fn get_strong_positive_window_list(&self) -> Vec<String> {
        let median = self.get_positive_window_median();

        let mut strong_positive_window_list = Vec::new();
        for x in &self.data {
            if x.window_diff > median {
                strong_positive_window_list.push(x.date.to_string());
            }
        }
        strong_positive_window_list
    }
    pub fn get_mild_positive_window_list(&self) -> Vec<String> {
        let median = self.get_positive_window_median();

        let mut mild_positive_window_list = Vec::new();
        for x in &self.data {
            if x.window_diff > 1.0 && x.window_diff < median {
                mild_positive_window_list.push(x.date.to_string());
            }
        }
        mild_positive_window_list
    }

    // pub fn get_negative_window_list(&self) -> Vec<String> {
    //     let mut negative_window_list = Vec::new();
    //     for x in &self.data {
    //         if x.window < 0.0 {
    //             negative_window_list.push(x.date.to_string());
    //         }
    //     }
    //     negative_window_list
    // }
    // fn get_negative_window_mean(&self) -> f64 {
    //     let negative_window_diffs: Vec<f64> = self
    //         .data
    //         .iter()
    //         .filter(|x| x.window_diff < 1.0)
    //         .map(|x| x.window_diff)
    //         .collect();

    //     let sum: f64 = negative_window_diffs.iter().sum();
    //     let mean = sum / negative_window_diffs.len() as f64;
    //     mean
    // }
    fn get_negative_window_median(&self) -> f64 {
        let mut negative_window_diffs: Vec<f64> = self
            .data
            .iter()
            .filter(|x| x.window_diff < 1.0)
            .map(|x| x.window_diff)
            .collect();

        negative_window_diffs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        let median_index = negative_window_diffs.len() / 2;
        negative_window_diffs[median_index]
    }

    fn get_negative_window_tertile(&self) -> (f64, f64) {
        let mut negative_window_diffs: Vec<f64> = self
            .data
            .iter()
            .filter(|x| x.window_diff < 1.0)
            .map(|x| x.window_diff)
            .collect();

        negative_window_diffs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        let lower_tertile_index = negative_window_diffs.len() / 3;
        let upper_tertile_index = negative_window_diffs.len() * 2 / 3;
        info!(
            "lower_tertile: {}, upper_tertile: {}",
            negative_window_diffs[lower_tertile_index], negative_window_diffs[upper_tertile_index]
        );
        (
            negative_window_diffs[lower_tertile_index],
            negative_window_diffs[upper_tertile_index],
        )
    }
    pub fn get_negative_window_list(&self) -> (Vec<String>, Vec<String>, Vec<String>) {
        let (lower_tertile, upper_tertile) = self.get_negative_window_tertile();

        let mut strong_negative_window_list = Vec::new();
        let mut moderate_negative_window_list = Vec::new();
        let mut mild_negative_window_list = Vec::new();

        for x in &self.data {
            if x.window_diff < lower_tertile {
                strong_negative_window_list.push(x.date.to_string());
            } else if x.window_diff < upper_tertile {
                moderate_negative_window_list.push(x.date.to_string());
            } else if x.window_diff < 1.0 {
                mild_negative_window_list.push(x.date.to_string());
            } else {
                // do nothing
            };
        }

        (
            strong_negative_window_list,
            moderate_negative_window_list,
            mild_negative_window_list,
        )
    }

    pub fn get_mild_negative_window_list(&self) -> Vec<String> {
        let median = self.get_negative_window_median();

        let mut mild_negative_window_list = Vec::new();
        for x in &self.data {
            if x.window_diff < 1.0 && x.window_diff > median {
                mild_negative_window_list.push(x.date.to_string());
            }
        }
        mild_negative_window_list
    }
    pub fn get_strong_negative_window_list(&self) -> Vec<String> {
        let median = self.get_negative_window_median();

        let mut strong_negative_window_list = Vec::new();
        for x in &self.data {
            if x.window_diff < median {
                strong_negative_window_list.push(x.date.to_string());
            }
        }
        strong_negative_window_list
    }
}

pub struct TopixDailyWindowList {
    strong_positive: Vec<String>,
    mild_positive: Vec<String>,
    mild_negative: Vec<String>,
    strong_negative: Vec<String>,
}
impl TopixDailyWindowList {
    pub fn new(backtesting_topix_list: &BacktestingTopixList) -> Self {
        let strong_positive = backtesting_topix_list.get_strong_positive_window_list();
        let mild_positive = backtesting_topix_list.get_mild_positive_window_list();
        let mild_negative = backtesting_topix_list.get_mild_negative_window_list();
        let strong_negative = backtesting_topix_list.get_strong_negative_window_list();

        Self {
            strong_positive,
            mild_positive,
            mild_negative,
            strong_negative,
        }
    }
    //getters
    pub fn get_strong_positive(&self) -> &Vec<String> {
        &self.strong_positive
    }
    pub fn get_mild_positive(&self) -> &Vec<String> {
        &self.mild_positive
    }
    pub fn get_mild_negative(&self) -> &Vec<String> {
        &self.mild_negative
    }
    pub fn get_strong_negative(&self) -> &Vec<String> {
        &self.strong_negative
    }
}

pub struct TopixDailyWindowList2 {
    strong_positive: Vec<String>,
    moderate_positive: Vec<String>,
    mild_positive: Vec<String>,
    mild_negative: Vec<String>,
    moderate_negative: Vec<String>,
    strong_negative: Vec<String>,
}
impl TopixDailyWindowList2 {
    pub fn new(backtesting_topix_list: &BacktestingTopixList) -> Self {
        let (strong_positive, moderate_positive, mild_positive) =
            backtesting_topix_list.get_positive_window_list();
        let (strong_negative, moderate_negative, mild_negative) =
            backtesting_topix_list.get_negative_window_list();

        Self {
            strong_positive,
            moderate_positive,
            mild_positive,
            mild_negative,
            moderate_negative,
            strong_negative,
        }
    }

    //getters
    pub fn get_strong_positive(&self) -> &Vec<String> {
        &self.strong_positive
    }
    pub fn get_moderate_positive(&self) -> &Vec<String> {
        &self.moderate_positive
    }
    pub fn get_mild_positive(&self) -> &Vec<String> {
        &self.mild_positive
    }
    pub fn get_mild_negative(&self) -> &Vec<String> {
        &self.mild_negative
    }
    pub fn get_moderate_negative(&self) -> &Vec<String> {
        &self.moderate_negative
    }
    pub fn get_strong_negative(&self) -> &Vec<String> {
        &self.strong_negative
    }
}
