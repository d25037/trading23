use anyhow::anyhow;
use chrono::{Duration, NaiveDate};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{fmt::Write, time::Instant};

use crate::database::stocks;
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
    current_price: f64,
    lower_bound: f64,
    upper_bound: f64,
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

        let range = highest_high - lowest_low;
        let step = range / 10.0;

        let mut ranges = [0; 10];
        for ohlc in ohlc_60 {
            let values = vec![
                ohlc.get_open(),
                ohlc.get_high(),
                ohlc.get_low(),
                ohlc.get_close(),
            ];
            for value in values {
                if value >= lowest_low && value <= highest_high {
                    let index = ((value - lowest_low) / step).floor() as usize;
                    ranges[index.min(9)] += 1;
                }
            }
        }

        let (max_range_index, _) = ranges
            .iter()
            .enumerate()
            .max_by_key(|&(_, count)| count)
            .unwrap();
        let max_range = (lowest_low + step * max_range_index as f64)
            ..(lowest_low + step * (max_range_index as f64 + 1.0));

        let (lower_bound, upper_bound) = {
            let lower_bound = (max_range.start * 10.0).round() / 10.0;
            let upper_bound = (max_range.end * 10.0).round() / 10.0;
            (lower_bound, upper_bound)
        };

        let current_price = ohlc_vec[position].get_close();

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
            current_price,
            lower_bound,
            upper_bound,
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

        let (status, difference) = match self.current_price {
            x if x < self.lower_bound => {
                let difference = (self.current_price - self.lower_bound) / self.atr;
                let difference = (difference * 100.0).round() / 100.0;

                ("Below", Some(difference))
            }
            x if x > self.upper_bound => {
                let difference = (self.current_price - self.upper_bound) / self.atr;

                let difference = (difference * 100.0).round() / 100.0;
                ("Above", Some(difference))
            }
            _ => ("Between", None),
        };

        let difference_str = match difference {
            Some(difference) => format!("{}%", difference),
            None => "".to_owned(),
        };

        writeln!(
            buffer,
            "{} {}, {}円 {}({} - {}) {}",
            self.code,
            name,
            self.current_price,
            status,
            self.lower_bound,
            self.upper_bound,
            difference_str
        )
        .unwrap();

        writeln!(
            buffer,
            "ATR: {}, Unit: {}, Diff.: {}, Move: {}, 必要金額: {}円",
            self.atr, self.unit, self.standardized_diff, self.latest_move, self.required_amount
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

#[derive(Debug, Clone)]
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
            .sort_by(|a, b| b.latest_move.partial_cmp(&a.latest_move).unwrap());
    }

    pub fn output_for_markdown(&self, date: &str) -> Markdown {
        let mut markdown = Markdown::new();
        markdown.h1(date);

        let start_index = 0;
        let end_index = self.data.len().min(10);

        markdown.h2("Top 10");
        for stocks_window in &self.data[start_index..end_index] {
            markdown.body(&stocks_window.markdown_body_output());
        }

        if self.data.len() > 10 {
            let start_index = self.data.len() - 10;
            let end_index = self.data.len();

            markdown.h2("Bottom 10");
            for stocks_window in &self.data[start_index..end_index] {
                markdown.body(&stocks_window.markdown_body_output());
            }
        }

        let (long_morning_close, long_afternoon_close) = self.mean_long_5rows_someday();
        markdown.body(&format!(
            "<Long> MC_mean5: {}, AC_mean5: {}",
            long_morning_close, long_afternoon_close
        ));

        let (short_morning_close, short_afternoon_close) = self.mean_short_5rows_someday();
        markdown.body(&format!(
            "<Short> MC_mean5: {}, AC_mean5: {}",
            short_morning_close, short_afternoon_close
        ));

        info!("{}", markdown.buffer());

        markdown
    }

    fn mean_long_5rows_someday(&self) -> (f64, f64) {
        let start_index = 0;
        let end_index = self.data.len().min(5);

        let mut morning_close_sum = 0.0;
        let mut afternoon_close_sum = 0.0;
        for stocks_window in &self.data[start_index..end_index] {
            morning_close_sum += stocks_window.get_morning_close();
            afternoon_close_sum += stocks_window.get_afternoon_close();
        }

        (
            (morning_close_sum / 5.0 * 100.0).round() / 100.0,
            (afternoon_close_sum / 5.0 * 100.0).round() / 100.0,
        )
    }

    fn mean_short_5rows_someday(&self) -> (f64, f64) {
        let start_index = self.data.len() - 5;
        let end_index = self.data.len();

        let mut morning_close_sum = 0.0;
        let mut afternoon_close_sum = 0.0;
        for stocks_window in &self.data[start_index..end_index] {
            morning_close_sum += stocks_window.get_morning_close();
            afternoon_close_sum += stocks_window.get_afternoon_close();
        }

        (
            (morning_close_sum / 5.0 * 100.0).round() / 100.0,
            (afternoon_close_sum / 5.0 * 100.0).round() / 100.0,
        )
    }

    pub fn bbb(&self) -> Result<(), MyError> {
        let mut date_to_stocks: HashMap<_, Vec<_>> = HashMap::new();

        for stocks_window in &self.data {
            date_to_stocks
                .entry(stocks_window.analyzed_at.clone())
                .or_default()
                .push(stocks_window.clone());
        }

        for (date, stocks_window_list) in date_to_stocks {
            let mut stocks_window_list = StocksWindowList::from_vec(stocks_window_list);
            stocks_window_list.sort_by_latest_move();
            let markdown = stocks_window_list.output_for_markdown(&date);
            let path = crate::my_file_io::get_jquants_window_path(&date).unwrap();
            markdown.write_to_file(&path);
        }

        Ok(())
    }
}

pub async fn create_stocks_window_list(from: &str, to: &str) -> Result<StocksWindowList, MyError> {
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

pub fn mean_analysis(stock_window_list: StocksWindowList, from: &str, to: &str) {
    let topix_list = crate::analysis::backtesting_topix::TopixDailyWindowList::new(
        &crate::analysis::backtesting_topix::BacktestingTopixList::from_json_file().unwrap(),
    );

    let from = NaiveDate::parse_from_str(from, "%Y-%m-%d").unwrap();
    let to = NaiveDate::parse_from_str(to, "%Y-%m-%d").unwrap();
    let mut date = from;
    let mut i_sp = 0.0;
    let mut i_mp = 0.0;
    let mut i_sn = 0.0;
    let mut i_mn = 0.0;
    // let mut long_morning_sum = 0.0;
    let mut long_afternoon_sum_sp = 0.0;
    let mut long_afternoon_sum_mp = 0.0;
    let mut long_afternoon_sum_mn = 0.0;
    let mut long_afternoon_sum_sn = 0.0;
    // let mut short_morning_sum = 0.0;
    let mut short_afternoon_sum_sp = 0.0;
    let mut short_afternoon_sum_mp = 0.0;
    let mut short_afternoon_sum_mn = 0.0;
    let mut short_afternoon_sum_sn = 0.0;
    while date <= to {
        let someday = &date.format("%Y-%m-%d").to_string();
        let someday_list = stock_window_list
            .clone()
            .data
            .into_iter()
            .filter(|x| x.analyzed_at == *someday)
            .collect::<Vec<_>>();
        if !someday_list.is_empty() {
            let someday_list = StocksWindowList::from_vec(someday_list);
            println!("{}", someday);

            let (long_morning_close, long_afternoon_close) = someday_list.mean_long_5rows_someday();
            let (short_morning_close, short_afternoon_close) =
                someday_list.mean_short_5rows_someday();
            // long_morning_sum += long_morning_close;
            // long_afternoon_sum_mp += long_afternoon_close;
            // // short_morning_sum += short_morning_close;
            // short_afternoon_sum_mp += short_afternoon_close;

            if topix_list.get_mild_positive().contains(someday) {
                println!("mild_positive");
                long_afternoon_sum_mp += long_afternoon_close;
                short_afternoon_sum_mp += short_afternoon_close;
                i_mp += 1.0;
            } else if topix_list.get_mild_negative().contains(someday) {
                println!("mild_negative");
                long_afternoon_sum_mn += long_afternoon_close;
                short_afternoon_sum_mn += short_afternoon_close;
                i_mn += 1.0;
            } else if topix_list.get_strong_positive().contains(someday) {
                println!("strong_positive");
                long_afternoon_sum_sp += long_afternoon_close;
                short_afternoon_sum_sp += short_afternoon_close;
                i_sp += 1.0;
            } else if topix_list.get_strong_negative().contains(someday) {
                println!("strong_negative");
                long_afternoon_sum_sn += long_afternoon_close;
                short_afternoon_sum_sn += short_afternoon_close;
                i_sn += 1.0;
            } else {
                println!("no_change");
            }

            // println!(
            //     "long_morning_close: {}, long_afternoon_close: {}",
            //     long_morning_close, long_afternoon_close
            // );
            // println!(
            //     "short_morning_close: {}, short_afternoon_close: {}",
            //     short_morning_close, short_afternoon_close
            // );
            // i += 1.0;
        }
        date += Duration::days(1);
    }

    println!(
        "long_afternoon_mean_sp: {}",
        // (long_morning_sum / i * 100.0).round() / 100.0,
        (long_afternoon_sum_sp / i_sp * 100.0).round() / 100.0
    );
    println!(
        "short_afternoon_mean_sp: {}",
        // (short_morning_sum / i * 100.0).round() / 100.0,
        (short_afternoon_sum_sp / i_sp * 100.0).round() / 100.0
    );
    println!(
        "long_afternoon_mean_mp: {}",
        // (long_morning_sum / i * 100.0).round() / 100.0,
        (long_afternoon_sum_mp / i_mp * 100.0).round() / 100.0
    );
    println!(
        "short_afternoon_mean_mp: {}",
        // (short_morning_sum / i * 100.0).round() / 100.0,
        (short_afternoon_sum_mp / i_mp * 100.0).round() / 100.0
    );
    println!(
        "long_afternoon_mean_mn: {}",
        // (long_morning_sum / i * 100.0).round() / 100.0,
        (long_afternoon_sum_mn / i_mn * 100.0).round() / 100.0
    );
    println!(
        "short_afternoon_mean_mn: {}",
        // (short_morning_sum / i * 100.0).round() / 100.0,
        (short_afternoon_sum_mn / i_mn * 100.0).round() / 100.0
    );
    println!(
        "long_afternoon_mean_sn: {}",
        // (long_morning_sum / i * 100.0).round() / 100.0,
        (long_afternoon_sum_sn / i_sn * 100.0).round() / 100.0
    );
    println!(
        "short_afternoon_mean_sn: {}",
        // (short_morning_sum / i * 100.0).round() / 100.0,
        (short_afternoon_sum_sn / i_sn * 100.0).round() / 100.0
    );
}
