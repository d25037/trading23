use anyhow::anyhow;
use chrono::{Duration, NaiveDate};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::{fmt::Write, time::Instant};

use crate::{
    markdown::Markdown,
    my_error::MyError,
    my_file_io::{get_fetched_ohlc_file_path, load_nikkei225_list, AssetType, Nikkei225},
};

use super::live::OhlcPremium;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StocksWindow {
    code: i32,
    name: String,
    atr: f64,
    unit: i32,
    required_amount: i32,
    latest_move: f64,
    standardized_diff: f64,
    result_morning_close: Option<f64>,
    result_afternoon_open: Option<f64>,
    result_close: Option<f64>,
    analyzed_at: String,
}

impl StocksWindow {
    pub fn from_vec(
        ohlc_vec: &Vec<OhlcPremium>,
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
        let last_close = last[0].get_close();
        let last2_close = prev_19[18].get_close();
        let prev_19_high = prev_19
            .iter()
            .map(|ohlc| ohlc.get_high())
            .fold(f64::NAN, f64::max);
        let prev_19_low = prev_19
            .iter()
            .map(|ohlc| ohlc.get_low())
            .fold(f64::NAN, f64::min);
        let latest_move = (last_close - last2_close) / (prev_19_high - prev_19_low);
        let latest_move = (latest_move * 100.0).round() / 100.0;

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

        let result_morning_close = match ohlc_vec.len() > position + 1 {
            true => {
                let result_morning_close = (ohlc_vec[position + 1].get_morning_close()
                    - ohlc_vec[position + 1].get_open())
                    / atr;
                Some((result_morning_close * 100.0).round() / 100.0)
            }
            false => None,
        };
        let result_afternoon_open = match ohlc_vec.len() > position + 1 {
            true => {
                let result_afternoon_open = (ohlc_vec[position + 1].get_afternoon_open()
                    - ohlc_vec[position + 1].get_open())
                    / atr;
                Some((result_afternoon_open * 100.0).round() / 100.0)
            }
            false => None,
        };
        let result_close = match ohlc_vec.len() > position + 1 {
            true => {
                let result_close =
                    (ohlc_vec[position + 1].get_close() - ohlc_vec[position + 1].get_open()) / atr;
                Some((result_close * 100.0).round() / 100.0)
            }
            false => None,
        };

        Ok(Self {
            code,
            name: name.to_owned(),
            atr,
            unit,
            required_amount,
            latest_move,
            standardized_diff,
            result_morning_close,
            result_afternoon_open,
            result_close,
            analyzed_at: date.to_owned(),
        })
    }

    fn markdown_body_output(&self) -> String {
        let mut buffer = String::new();
        let name = match self.name.chars().count() > 5 {
            true => {
                let name: String = self.name.chars().take(4).collect();
                name
            }
            false => self.name.to_owned(),
        };

        writeln!(
            buffer,
            "{} {}, ({}, {}, {}, {}), {}円",
            self.code,
            name,
            self.atr,
            self.unit,
            self.standardized_diff,
            self.latest_move,
            self.required_amount,
        )
        .unwrap();

        if self.result_close.is_some() {
            writeln!(
                buffer,
                "MC: {}, AO: {}, AC: {}",
                self.result_morning_close.unwrap(),
                self.result_afternoon_open.unwrap(),
                self.result_close.unwrap()
            )
            .unwrap();
        }

        buffer
    }

    pub fn get_afternoon_close(&self) -> f64 {
        self.result_close.unwrap_or(0.0)
    }

    pub fn get_morning_close(&self) -> f64 {
        self.result_morning_close.unwrap_or(0.0)
    }
}

#[derive(Debug)]
pub struct StocksWindowList {
    data: Vec<StocksWindow>,
}
impl StocksWindowList {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
    fn from_vec(vec: Vec<StocksWindow>) -> Self {
        Self { data: vec }
    }
    pub fn push_2(
        &mut self,
        ohlc_vec: Vec<OhlcPremium>,
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
            match StocksWindow::from_vec(
                &ohlc_vec,
                code,
                name,
                unit,
                &date.format("%Y-%m-%d").to_string(),
            ) {
                Ok(stocks_daytrading) => self.data.push(stocks_daytrading),
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

    pub fn append(&mut self, mut stocks_daytrading_list: StocksWindowList) {
        self.data.append(&mut stocks_daytrading_list.data);
    }

    pub fn sort_by_latest_move(&mut self) {
        self.data
            .sort_by(|a, b| a.latest_move.partial_cmp(&b.latest_move).unwrap());
    }

    pub fn output_for_markdown(&self, date: &str) -> Markdown {
        let mut markdown = Markdown::new();
        markdown.h1(date);

        let start_index = 0;
        let end_index = self.data.len().min(5);

        for stocks_window in &self.data[start_index..end_index] {
            markdown.body(&stocks_window.markdown_body_output());
        }

        if self.data.len() > 5 {
            let start_index = self.data.len() - 5;
            let end_index = self.data.len();

            for stocks_window in &self.data[start_index..end_index] {
                markdown.body(&stocks_window.markdown_body_output());
            }
        }

        info!("{}", markdown.buffer());

        markdown
    }

    fn mean_negative_window5(&self) {
        let start_index = 0;
        let end_index = self.data.len().min(5);

        let mut morning_close_sum = 0.0;
        let mut afternoon_close_sum = 0.0;
        for stocks_window in &self.data[start_index..end_index] {
            morning_close_sum += stocks_window.get_morning_close();
            afternoon_close_sum += stocks_window.get_afternoon_close();
        }
        let morning_close_sum = (morning_close_sum * 100.0).round() / 100.0;
        let afternoon_close_sum = (afternoon_close_sum * 100.0).round() / 100.0;

        println!(
            "<NW> MC_sum5: {}, AC_sum5: {}",
            morning_close_sum, afternoon_close_sum
        );
    }

    fn mean_positive_window5(&self) {
        let start_index = self.data.len() - 5;
        let end_index = self.data.len();

        let mut morning_close_sum = 0.0;
        let mut afternoon_close_sum = 0.0;
        for stocks_window in &self.data[start_index..end_index] {
            morning_close_sum += stocks_window.get_morning_close();
            afternoon_close_sum += stocks_window.get_afternoon_close();
        }

        let morning_close_sum = (morning_close_sum * 100.0).round() / 100.0;
        let afternoon_close_sum = (afternoon_close_sum * 100.0).round() / 100.0;

        println!(
            "<PW> MC_sum5: {}, AC_sum5: {}",
            morning_close_sum, afternoon_close_sum
        )
    }
}

pub async fn async_exec(from: &str, to: &str) -> Result<StocksWindowList, MyError> {
    async fn inner(
        row: Nikkei225,
        unit: f64,
        from: String,
        to: String,
    ) -> Result<StocksWindowList, MyError> {
        let code = row.get_code();
        let name = row.get_name();
        let ohlc_vec_path = match get_fetched_ohlc_file_path(AssetType::Stocks { code: Some(code) })
        {
            Ok(res) => res,
            Err(e) => {
                error!("{}", e);
                return Err(e);
            }
        };
        let ohlc_vec: Vec<OhlcPremium> =
            match serde_json::from_str(&std::fs::read_to_string(ohlc_vec_path).unwrap()) {
                Ok(res) => res,
                Err(e) => {
                    error!("{}", e);
                    return Err(MyError::Anyhow(anyhow!("{}", e)));
                }
            };
        // let stocks_daytrading = StocksWindow::from_vec(&ohlc_vec, code, name, unit, &date)?;
        let mut stocks_daytrading_list = StocksWindowList::new();
        stocks_daytrading_list.push_2(ohlc_vec, code, name, unit, &from, &to);

        // Ok(stocks_daytrading)
        Ok(stocks_daytrading_list)
    }

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

    let mut stocks_daytrading_list = StocksWindowList::new();
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
