use crate::my_file_io::Nikkei225;
use crate::my_file_io::{get_fetched_ohlc_file_path, load_nikkei225_list, AssetType};
use crate::{analysis::live::Ohlc, my_error::MyError};
use anyhow::anyhow;
use chrono::{Duration, NaiveDate};
use futures::SinkExt;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use statrs::distribution::ContinuousCDF;
use statrs::distribution::{Continuous, StudentsT};
use statrs::statistics::Statistics;
use std::collections::HashSet;
use std::fmt::{write, Write};
use std::time::Instant;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StocksDaytrading {
    code: i32,
    name: String,
    status: Status,
    atr: f64,
    unit: i32,
    required_amount: i32,
    standardized_diff: f64,
    result: Option<f64>,
    analyzed_at: String,
}
impl StocksDaytrading {
    pub fn from_vec(
        ohlc_vec: &Vec<Ohlc>,
        code: i32,
        name: &str,
        unit: f64,
        date: &str,
    ) -> Result<Self, MyError> {
        let position = match ohlc_vec.iter().position(|ohlc| ohlc.get_date() == date) {
            Some(res) => res,
            None => return Err(MyError::OutOfRange),
        };

        if position < 59 {
            return Err(MyError::OutOfRange);
        }

        let ohlc_5 = &ohlc_vec[(position - 4)..=position];
        let ohlc_20 = &ohlc_vec[(position - 19)..=position];
        let ohlc_60 = &ohlc_vec[(position - 59)..=position];

        let (prev_19, last) = ohlc_20.split_at(19);
        let (last_high, last_low, last_close) =
            { (last[0].get_high(), last[0].get_low(), last[0].get_close()) };
        let prev_19_high = prev_19
            .iter()
            .map(|ohlc| ohlc.get_high())
            .fold(f64::NAN, f64::max);
        let prev_19_low = prev_19
            .iter()
            .map(|ohlc| ohlc.get_low())
            .fold(f64::NAN, f64::min);

        let status = match (last_close > prev_19_high) || (last_close < prev_19_low) {
            true => {
                if last_close > prev_19_high {
                    Status::BreakoutResistance
                } else {
                    Status::BreakoutSupport
                }
            }

            false => {
                if last_high > prev_19_high {
                    Status::FailedBreakoutResistance
                } else if last_low < prev_19_low {
                    Status::FailedBreakoutSupport
                } else {
                    Status::NoChange
                }
            }
        };

        let atr = ohlc_5
            .iter()
            .map(|ohlc| (ohlc.get_high() - ohlc.get_low()))
            .sum::<f64>()
            / ohlc_5.len() as f64;
        let atr = (atr * 10.0).round() / 10.0;

        let (unit, required_amount) = {
            let unit = unit / atr;
            let required_amount = (unit * last_close) as i32;
            (unit as i32, required_amount)
        };

        let highest_high = ohlc_60
            .iter()
            .map(|ohlc| ohlc.get_high())
            .fold(f64::NAN, f64::max);
        let lowest_low = ohlc_60
            .iter()
            .map(|ohlc| ohlc.get_low())
            .fold(f64::NAN, f64::min);

        let diff_sum: f64 = ohlc_60
            .iter()
            .map(|ohlc| ohlc.get_high() - ohlc.get_low())
            .sum();
        let average_diff = diff_sum / ohlc_60.len() as f64;

        let standardized_diff =
            (average_diff / (highest_high - lowest_low) * 1000.0).trunc() / 1000.0;

        let result = match ohlc_vec.len() > position + 1 {
            true => {
                let result =
                    (ohlc_vec[position + 1].get_close() - ohlc_vec[position + 1].get_open()) / atr;
                Some((result * 100.0).round() / 100.0)
            }
            false => None,
        };

        Ok(Self {
            code,
            name: name.to_owned(),
            status,
            atr,
            unit,
            required_amount,
            standardized_diff,
            result,
            analyzed_at: date.to_owned(),
        })
    }

    fn live_output(&self, mut buffer: String) -> String {
        writeln!(buffer).unwrap();

        writeln!(
            buffer,
            "{} {}, Atr: {}, Unit: {}, {}, {}円",
            self.code, self.name, self.atr, self.unit, self.standardized_diff, self.required_amount
        )
        .unwrap();

        buffer
    }
}

pub struct StocksDaytradingList {
    data: Vec<StocksDaytrading>,
}
impl StocksDaytradingList {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
    fn from_vec(vec: Vec<StocksDaytrading>) -> Self {
        Self { data: vec }
    }

    pub fn push(&mut self, stocks_daytrading: StocksDaytrading) {
        self.data.push(stocks_daytrading);
    }

    pub fn push_2(
        &mut self,
        ohlc_vec: Vec<Ohlc>,
        code: i32,
        name: &str,
        unit: f64,
        from: &str,
        to: &str,
    ) {
        let from = NaiveDate::parse_from_str(from, "%Y-%m-%d").unwrap();
        let to = NaiveDate::parse_from_str(to, "%Y-%m-%d").unwrap();
        let mut date = from;
        while date <= to {
            match StocksDaytrading::from_vec(
                &ohlc_vec,
                code,
                name,
                unit,
                &date.format("%Y-%m-%d").to_string(),
            ) {
                Ok(stocks_daytrading) => {
                    if stocks_daytrading.status != Status::NoChange {
                        self.data.push(stocks_daytrading)
                    }
                }
                Err(e) => match e {
                    MyError::OutOfRange => {}
                    _ => {
                        error!("{}", e);
                        return;
                    }
                },
            }
            date += Duration::days(1);
        }
    }

    pub fn append(&mut self, mut stocks_daytrading_list: StocksDaytradingList) {
        self.data.append(&mut stocks_daytrading_list.data);
    }

    pub fn sort_by_standardized_diff(&mut self) {
        self.data.sort_by(|a, b| {
            a.standardized_diff
                .partial_cmp(&b.standardized_diff)
                .unwrap()
        });
    }

    pub fn output(&self) {
        for stocks_daytrading in &self.data {
            debug!(
                "[{}] {} {}, Atr: {}, Unit: {}, {}, {:?}, result: {}",
                stocks_daytrading.analyzed_at,
                stocks_daytrading.code,
                stocks_daytrading.name,
                stocks_daytrading.atr,
                stocks_daytrading.unit,
                stocks_daytrading.standardized_diff,
                stocks_daytrading.status,
                stocks_daytrading.result.unwrap_or(0.0),
            );
        }
    }

    pub fn output_for_line_notify(&self) -> Output {
        let mut breakout_resistance_stocks = String::new();
        writeln!(breakout_resistance_stocks, "BreakoutResistance").unwrap();
        let mut failed_breakout_resistance_stocks = String::new();
        writeln!(
            failed_breakout_resistance_stocks,
            "FailedBreakoutResistance"
        )
        .unwrap();
        let mut failed_breakout_support_stocks = String::new();
        writeln!(failed_breakout_support_stocks, "FailedBreakoutSupport").unwrap();
        let mut breakout_support_stocks = String::new();
        writeln!(breakout_support_stocks, "BreakoutSupport").unwrap();

        for stocks_daytrading in &self.data {
            match stocks_daytrading.status {
                Status::BreakoutResistance => {
                    breakout_resistance_stocks =
                        stocks_daytrading.live_output(breakout_resistance_stocks);
                }
                Status::FailedBreakoutResistance => {
                    failed_breakout_resistance_stocks =
                        stocks_daytrading.live_output(failed_breakout_resistance_stocks);
                }
                Status::FailedBreakoutSupport => {
                    failed_breakout_support_stocks =
                        stocks_daytrading.live_output(failed_breakout_support_stocks);
                }
                Status::BreakoutSupport => {
                    breakout_support_stocks =
                        stocks_daytrading.live_output(breakout_support_stocks);
                }
                _ => {}
            }
        }

        Output {
            date: self.data[0].analyzed_at.clone(),
            breakout_resistance_stocks,
            failed_breakout_resistance_stocks,
            failed_breakout_support_stocks,
            breakout_support_stocks,
        }
    }

    pub fn mean_result(&self) {
        let breakout_resistance = self
            .data
            .iter()
            .filter(|stocks_daytrading| stocks_daytrading.status == Status::BreakoutResistance)
            .collect::<Vec<_>>();
        let breakout_resistance_t_test = TTestResult::new(
            breakout_resistance
                .iter()
                .map(|stocks_daytrading| stocks_daytrading.result.unwrap_or(0.0))
                .collect::<Vec<_>>(),
        );
        info!(
            "BreakoutResistance: {}, p_value: {}",
            breakout_resistance_t_test.get_mean(),
            breakout_resistance_t_test.get_p_value()
        );

        let failed_breakout_resistance = self
            .data
            .iter()
            .filter(|stocks_daytrading| {
                stocks_daytrading.status == Status::FailedBreakoutResistance
            })
            .collect::<Vec<_>>();
        let failed_breakout_resistance_t_test = TTestResult::new(
            failed_breakout_resistance
                .iter()
                .map(|stocks_daytrading| stocks_daytrading.result.unwrap_or(0.0))
                .collect::<Vec<_>>(),
        );
        info!(
            "FailedBreakoutResistance: {}, p_value: {}",
            failed_breakout_resistance_t_test.get_mean(),
            failed_breakout_resistance_t_test.get_p_value()
        );

        let failed_breakout_support = self
            .data
            .iter()
            .filter(|stocks_daytrading| stocks_daytrading.status == Status::FailedBreakoutSupport)
            .collect::<Vec<_>>();
        let failed_breakout_support_t_test = TTestResult::new(
            failed_breakout_support
                .iter()
                .map(|stocks_daytrading| stocks_daytrading.result.unwrap_or(0.0))
                .collect::<Vec<_>>(),
        );
        info!(
            "FailedBreakoutSupport: {}, p_value: {}",
            failed_breakout_support_t_test.get_mean(),
            failed_breakout_support_t_test.get_p_value()
        );

        let breakout_support = self
            .data
            .iter()
            .filter(|stocks_daytrading| stocks_daytrading.status == Status::BreakoutSupport)
            .collect::<Vec<_>>();
        let breakout_support_t_test = TTestResult::new(
            breakout_support
                .iter()
                .map(|stocks_daytrading| stocks_daytrading.result.unwrap_or(0.0))
                .collect::<Vec<_>>(),
        );
        info!(
            "BreakoutSupport: {}, p_value: {}",
            breakout_support_t_test.get_mean(),
            breakout_support_t_test.get_p_value()
        );
    }

    pub fn get_window_related_result(
        &self,
        some_list: Vec<String>,
        lower_limit: f64,
        upper_limit: f64,
    ) {
        let data = self.data.clone();
        let filtered = data
            .into_iter()
            .filter(|stocks_daytrading| {
                some_list.contains(&stocks_daytrading.analyzed_at)
                    && (lower_limit..upper_limit).contains(&stocks_daytrading.standardized_diff)
            })
            .collect::<Vec<_>>();
        let new_list = StocksDaytradingList::from_vec(filtered);

        let unique_dates: HashSet<String> = new_list
            .data
            .clone()
            .into_iter()
            .map(|item| item.analyzed_at)
            .collect();

        info!("{:?}", unique_dates);
        info!("{} stocks", new_list.data.len());
        new_list.mean_result()
    }

    // t_test
}

struct TTestResult {
    mean: f64,
    p_value: f64,
}
impl TTestResult {
    fn new(data: Vec<f64>) -> Self {
        let mean = data.clone().mean();
        let variance = data.clone().variance();
        let len = data.len() as f64;
        let t = mean / (variance / len).sqrt();
        let df = len - 1.0;
        let t_distribution = StudentsT::new(0.0, 1.0, df).unwrap();

        if mean >= 0.0 {
            Self {
                mean: (mean * 1000.0).round() / 1000.0,
                p_value: 1.0 - (t_distribution.cdf(t) * 1000.0).round() / 1000.0,
            }
        } else {
            Self {
                mean: (mean * 1000.0).round() / 1000.0,
                p_value: (t_distribution.cdf(t) * 1000.0).round() / 1000.0,
            }
        }
    }
    //getters
    fn get_mean(&self) -> f64 {
        self.mean
    }
    fn get_p_value(&self) -> f64 {
        self.p_value
    }
}

pub struct Output {
    date: String,
    breakout_resistance_stocks: String,
    failed_breakout_resistance_stocks: String,
    failed_breakout_support_stocks: String,
    breakout_support_stocks: String,
}
impl Output {
    pub fn get_date(&self) -> &str {
        &self.date
    }
    pub fn get_breakout_resistance_stocks(&self) -> &str {
        &self.breakout_resistance_stocks
    }
    pub fn get_failed_breakout_resistance_stocks(&self) -> &str {
        &self.failed_breakout_resistance_stocks
    }
    pub fn get_failed_breakout_support_stocks(&self) -> &str {
        &self.failed_breakout_support_stocks
    }
    pub fn get_breakout_support_stocks(&self) -> &str {
        &self.breakout_support_stocks
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub enum Status {
    BreakoutResistance,
    FailedBreakoutResistance,
    NoChange,
    FailedBreakoutSupport,
    BreakoutSupport,
}

pub async fn async_exec(from: &str, to: &str) -> Result<StocksDaytradingList, MyError> {
    let nikkei225 = match load_nikkei225_list() {
        Ok(res) => res,
        Err(e) => {
            error!("{}", e);
            return Err(e);
        }
    };
    info!("Nikkei225 has been loaded");

    let config = crate::config::GdriveJson::new();
    let unit = config.jquants_unit();
    info!("unit: {}", unit);

    let start_time = Instant::now();

    let handles = nikkei225
        .into_iter()
        .map(|row| tokio::spawn(inner(row, unit, from.to_owned(), to.to_owned())))
        .collect::<Vec<_>>();

    let results = futures::future::join_all(handles).await;

    let mut stocks_daytrading_list = StocksDaytradingList::new();
    for result in results {
        match result {
            Ok(res) => {
                let stock = res.unwrap();
                // if stock.status == Status::NoChange {
                //     continue;
                // }
                stocks_daytrading_list.append(stock);
            }
            Err(e) => {
                error!("{}", e);
                return Err(MyError::Anyhow(anyhow!("{}", e)));
            }
        }
    }

    let end_time = Instant::now();

    info!("Elapsed time: {:?}", end_time - start_time);
    Ok(stocks_daytrading_list)
}

async fn inner(
    row: Nikkei225,
    unit: f64,
    from: String,
    to: String,
) -> Result<StocksDaytradingList, MyError> {
    let code = row.get_code();
    let name = row.get_name();
    let ohlc_vec_path = match get_fetched_ohlc_file_path(AssetType::Stocks { code: Some(code) }) {
        Ok(res) => res,
        Err(e) => {
            error!("{}", e);
            return Err(e);
        }
    };
    let ohlc_vec: Vec<Ohlc> =
        match serde_json::from_str(&std::fs::read_to_string(ohlc_vec_path).unwrap()) {
            Ok(res) => res,
            Err(e) => {
                error!("{}", e);
                return Err(MyError::Anyhow(anyhow!("{}", e)));
            }
        };
    // let stocks_daytrading = StocksDaytrading::from_vec(&ohlc_vec, code, name, unit, &date)?;
    let mut stocks_daytrading_list = StocksDaytradingList::new();
    stocks_daytrading_list.push_2(ohlc_vec, code, name, unit, &from, &to);

    // Ok(stocks_daytrading)
    Ok(stocks_daytrading_list)
}
