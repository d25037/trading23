use anyhow::anyhow;
use chrono::{Duration, NaiveDate};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{fmt::Write, time::Instant};

use crate::{
    markdown::Markdown,
    my_error::MyError,
    my_file_io::{
        get_fetched_ohlc_file_path, load_nikkei225_list, AssetType, JquantsStyle, Nikkei225,
    },
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
    nextday_morning_close: Option<f64>,
    morning_move: Option<f64>,
    analyzed_at: String,
    result_at: Option<String>,
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
        let step = range / 5.0;

        let mut ranges = [0; 5];
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
                    ranges[index.min(4)] += 1;
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

        let nextday_morning_close = match ohlc_vec.len() > position + 1 {
            true => Some(ohlc_vec[position + 1].get_morning_close()),
            false => None,
        };

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

        let morning_move = match ohlc_vec.len() > position + 1 {
            true => {
                let morning_move = (ohlc_vec[position + 1].get_morning_close()
                    - ohlc_vec[position].get_close())
                    / (prev_19_high - prev_19_low);
                Some((morning_move * 100.0).round() / 100.0)
            }
            false => None,
        };

        let analyzed_at = ohlc_vec[position].get_date().to_owned();
        let result_at = match ohlc_vec.len() > position + 1 {
            true => Some(ohlc_vec[position + 1].get_date().to_owned()),
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
            nextday_morning_close,
            morning_move,
            analyzed_at,
            result_at,
        })
    }

    fn markdown_body_output(&self, afternoon: bool) -> String {
        let (current_price, latest_move) = match afternoon {
            true => (
                self.nextday_morning_close.unwrap(),
                self.morning_move.unwrap(),
            ),
            false => (self.current_price, self.latest_move),
        };

        let mut buffer = String::new();
        let name = match self.name.chars().count() > 5 {
            true => {
                let name: String = self.name.chars().take(4).collect();
                name
            }
            false => self.name.to_owned(),
        };

        let (status, difference) = match current_price {
            x if x < self.lower_bound => {
                let difference = (current_price - self.lower_bound) / self.atr;
                let difference = (difference * 100.0).round() / 100.0;

                ("Below", Some(difference))
            }
            x if x > self.upper_bound => {
                let difference = (current_price - self.upper_bound) / self.atr;

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
            current_price,
            status,
            self.lower_bound,
            self.upper_bound,
            difference_str
        )
        .unwrap();

        writeln!(
            buffer,
            "ATR: {}, Unit: {}, Diff.: {}, Move: {}, 必要金額: {}円",
            self.atr, self.unit, self.standardized_diff, latest_move, self.required_amount
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

    fn markdown_body_output_default(&self) -> String {
        self.markdown_body_output(false)
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
    pub fn push(
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

    fn append(&mut self, mut stocks_daytrading_list: StocksWindowList) {
        self.data.append(&mut stocks_daytrading_list.data);
    }

    fn sort_by_latest_move(&mut self, absolute: bool) {
        match absolute {
            true => self.data.sort_by(|a, b| {
                b.latest_move
                    .abs()
                    .partial_cmp(&a.latest_move.abs())
                    .unwrap()
            }),
            false => self
                .data
                .sort_by(|a, b| b.latest_move.partial_cmp(&a.latest_move).unwrap()),
        }
    }
    fn sort_by_latest_move_default(&mut self) {
        self.sort_by_latest_move(true)
    }

    fn sort_by_morning_move(&mut self, absolute: bool) {
        match absolute {
            true => self.data.sort_by(|a, b| {
                b.morning_move
                    .unwrap()
                    .abs()
                    .partial_cmp(&a.morning_move.unwrap().abs())
                    .unwrap()
            }),
            false => self
                .data
                .sort_by(|a, b| b.morning_move.partial_cmp(&a.morning_move).unwrap()),
        }
    }
    fn sort_by_morning_move_default(&mut self) {
        self.sort_by_morning_move(true)
    }

    fn sort_by_difference(&mut self, absolute: bool) {
        match absolute {
            true => self.data.sort_by(|a, b| {
                let a_difference = (a.current_price - a.lower_bound) / a.atr;
                let b_difference = (b.current_price - b.lower_bound) / b.atr;
                b_difference.abs().partial_cmp(&a_difference.abs()).unwrap()
            }),
            false => self.data.sort_by(|a, b| {
                let a_difference = (a.current_price - a.lower_bound) / a.atr;
                let b_difference = (b.current_price - b.lower_bound) / b.atr;
                b_difference.partial_cmp(&a_difference).unwrap()
            }),
        }
    }

    fn sort_by_difference_default(&mut self) {
        self.sort_by_difference(true)
    }

    fn get_morning_mover(&self) -> Option<StocksWindowList> {
        self.data[0].result_morning_close?;

        let mut morning_mover = self.data.clone();
        morning_mover.sort_by(|a, b| {
            b.morning_move
                .unwrap()
                .abs()
                .partial_cmp(&a.morning_move.unwrap().abs())
                .unwrap()
        });

        Some(StocksWindowList::from_vec(
            morning_mover.into_iter().take(20).collect(),
        ))
    }

    fn get_on_the_cloud(&self, afternoon: bool) -> StocksWindowList {
        let on_the_cloud = self
            .data
            .iter()
            .filter(|x| {
                let current_price = match afternoon {
                    true => x.nextday_morning_close.unwrap(),
                    false => x.current_price,
                };
                let difference = (current_price - x.upper_bound) / x.atr;
                difference > 0.0 && difference < 0.2 && x.standardized_diff < 0.12
            })
            .collect::<Vec<_>>();

        StocksWindowList::from_vec(on_the_cloud.into_iter().cloned().collect())
    }
    fn get_on_the_cloud_default(&self) -> StocksWindowList {
        self.get_on_the_cloud(false)
    }

    fn get_between_the_cloud(&self, afternoon: bool) -> StocksWindowList {
        let between_the_cloud = self
            .data
            .iter()
            .filter(|x| {
                let current_price = match afternoon {
                    true => x.nextday_morning_close.unwrap(),
                    false => x.current_price,
                };
                current_price > x.lower_bound
                    && current_price < x.upper_bound
                    && x.standardized_diff < 0.12
            })
            .collect::<Vec<_>>();

        StocksWindowList::from_vec(between_the_cloud.into_iter().cloned().collect())
    }
    // fn get_between_the_cloud_default(&self) -> StocksWindowList {
    //     self.get_between_the_cloud(false)
    // }

    fn get_under_the_cloud(&self, afternoon: bool) -> StocksWindowList {
        let under_the_cloud = self
            .data
            .iter()
            .filter(|x| {
                let current_price = match afternoon {
                    true => x.nextday_morning_close.unwrap(),
                    false => x.current_price,
                };
                let difference = (current_price - x.lower_bound) / x.atr;
                difference < 0.0 && difference > -0.2 && x.standardized_diff < 0.12
            })
            .collect::<Vec<_>>();

        StocksWindowList::from_vec(under_the_cloud.into_iter().cloned().collect())
    }
    fn get_under_the_cloud_default(&self) -> StocksWindowList {
        self.get_under_the_cloud(false)
    }

    fn get_around_the_cloud(&self, afternoon: bool) -> StocksWindowList {
        let mut around_the_cloud = StocksWindowList::new();
        around_the_cloud.append(self.get_on_the_cloud(afternoon));
        around_the_cloud.append(self.get_between_the_cloud(afternoon));
        around_the_cloud.append(self.get_under_the_cloud(afternoon));

        around_the_cloud
    }
    fn get_around_the_cloud_default(&self) -> StocksWindowList {
        self.get_around_the_cloud(false)
    }

    fn output_for_markdown_window(&self, date: &str) -> Result<Markdown, MyError> {
        let mut markdown = Markdown::new();
        markdown.h1(date)?;

        let start_index = 0;
        let end_index = self.data.len().min(10);

        markdown.h2("Top 10")?;
        for stocks_window in &self.data[start_index..end_index] {
            markdown.body(&stocks_window.markdown_body_output_default())?;
        }

        if self.data.len() > 10 {
            let start_index = self.data.len() - 10;
            let end_index = self.data.len();

            markdown.h2("Bottom 10")?;
            for stocks_window in &self.data[start_index..end_index] {
                markdown.body(&stocks_window.markdown_body_output_default())?;
            }
        }

        let row = 3;
        let (long_morning_close, long_afternoon_close) = self.mean_long_rows_someday(row);
        markdown.body(&format!(
            "<Long> MC_mean{}: {}, AC_mean{}: {}",
            row, long_morning_close, row, long_afternoon_close
        ))?;

        let (short_morning_close, short_afternoon_close) = self.mean_short_rows_someday(row);
        markdown.body(&format!(
            "<Short> MC_mean{}: {}, AC_mean{}: {}",
            row, short_morning_close, row, short_afternoon_close
        ))?;

        debug!("{}", markdown.buffer());

        Ok(markdown)
    }

    pub fn output_for_markdown_cloud(
        &self,
        afternoon: bool,
    ) -> Result<(Markdown, String), MyError> {
        let date = match afternoon {
            true => self.data[0].result_at.clone().unwrap(),
            false => self.data[0].analyzed_at.clone(),
        };

        let mut markdown = Markdown::new();
        markdown.h1(&date)?;

        for stocks_window in &self.data {
            match afternoon {
                true => markdown.body(&stocks_window.markdown_body_output(true))?,
                false => markdown.body(&stocks_window.markdown_body_output_default())?,
            }
        }

        let (long_morning_close, long_afternoon_close) = self.mean_on_the_cloud();
        markdown.body(&format!(
            "<Above> MC_mean5: {}, AC_mean5: {}",
            long_morning_close, long_afternoon_close
        ))?;

        let (short_morning_close, short_afternoon_close) = self.mean_under_the_cloud();
        markdown.body(&format!(
            "<Below> MC_mean5: {}, AC_mean5: {}",
            short_morning_close, short_afternoon_close
        ))?;

        debug!("{}", markdown.buffer());

        Ok((markdown, date))
    }

    pub fn output_for_markdown_cloud_default(&self) -> Result<(Markdown, String), MyError> {
        self.output_for_markdown_cloud(false)
    }

    fn mean_long_rows_someday(&self, row: usize) -> (f64, f64) {
        let start_index = 0;
        let end_index = self.data.len().min(row);

        let mut morning_close_sum = 0.0;
        let mut afternoon_close_sum = 0.0;
        for stocks_window in &self.data[start_index..end_index] {
            morning_close_sum += stocks_window.get_morning_close();
            afternoon_close_sum += stocks_window.get_afternoon_close();
        }

        (
            (morning_close_sum / row as f64 * 100.0).round() / 100.0,
            (afternoon_close_sum / row as f64 * 100.0).round() / 100.0,
        )
    }

    fn mean_short_rows_someday(&self, row: usize) -> (f64, f64) {
        let start_index = self.data.len() - row;
        let end_index = self.data.len();

        let mut morning_close_sum = 0.0;
        let mut afternoon_close_sum = 0.0;
        for stocks_window in &self.data[start_index..end_index] {
            morning_close_sum += stocks_window.get_morning_close();
            afternoon_close_sum += stocks_window.get_afternoon_close();
        }

        (
            (morning_close_sum / row as f64 * 100.0).round() / 100.0,
            (afternoon_close_sum / row as f64 * 100.0).round() / 100.0,
        )
    }

    fn mean_on_the_cloud(&self) -> (f64, f64) {
        let on_the_cloud = self.get_on_the_cloud_default();

        let mut morning_close_sum = 0.0;
        let mut afternoon_close_sum = 0.0;
        for stocks_cloud in &on_the_cloud.data {
            morning_close_sum += stocks_cloud.get_morning_close();
            afternoon_close_sum += stocks_cloud.get_afternoon_close();
        }

        (
            (morning_close_sum / on_the_cloud.data.len() as f64 * 100.0).round() / 100.0,
            (afternoon_close_sum / on_the_cloud.data.len() as f64 * 100.0).round() / 100.0,
        )
    }
    fn mean_under_the_cloud(&self) -> (f64, f64) {
        let under_the_cloud = self.get_under_the_cloud_default();

        let mut morning_close_sum = 0.0;
        let mut afternoon_close_sum = 0.0;
        for stocks_cloud in &under_the_cloud.data {
            morning_close_sum += stocks_cloud.get_morning_close();
            afternoon_close_sum += stocks_cloud.get_afternoon_close();
        }

        (
            (morning_close_sum / under_the_cloud.data.len() as f64 * 100.0).round() / 100.0,
            (afternoon_close_sum / under_the_cloud.data.len() as f64 * 100.0).round() / 100.0,
        )
    }

    pub fn for_window_strategy(&self) -> Result<(), MyError> {
        let mut date_to_stocks: HashMap<_, Vec<_>> = HashMap::new();

        for stocks_window in &self.data {
            date_to_stocks
                .entry(stocks_window.analyzed_at.clone())
                .or_default()
                .push(stocks_window.clone());
        }

        for (date, stocks_window_list) in date_to_stocks {
            let mut stocks_window_list = StocksWindowList::from_vec(stocks_window_list);
            // stocks_window_list.sort_by_latest_move();
            stocks_window_list.sort_by_difference(false);

            let markdown = stocks_window_list.output_for_markdown_window(&date)?;
            let path = crate::my_file_io::get_jquants_path(JquantsStyle::Window, &date)?;
            markdown.write_to_file(&path)?;
        }

        Ok(())
    }

    pub fn for_cloud_strategy(&self) -> Result<(), MyError> {
        let mut date_to_stocks: HashMap<_, Vec<_>> = HashMap::new();

        for stocks_window in &self.data {
            date_to_stocks
                .entry(stocks_window.analyzed_at.clone())
                .or_default()
                .push(stocks_window.clone());
        }

        for (date, stocks_window_list) in date_to_stocks {
            let stocks_window_list = StocksWindowList::from_vec(stocks_window_list);
            let mut around_the_cloud = stocks_window_list.get_around_the_cloud_default();
            around_the_cloud.sort_by_latest_move(true);

            let (markdown, analyzed_at) = around_the_cloud.output_for_markdown_cloud_default()?;
            let path = crate::my_file_io::get_jquants_path(JquantsStyle::Cloud, &analyzed_at)?;
            markdown.write_to_file(&path)?;

            if stocks_window_list.data[0].nextday_morning_close.is_some() {
                let mut afternoon_list = stocks_window_list.get_around_the_cloud(true);
                afternoon_list.sort_by_morning_move(true);
                let (markdown, result_at) = afternoon_list.output_for_markdown_cloud(true)?;
                let path =
                    crate::my_file_io::get_jquants_path(JquantsStyle::Afternoon, &result_at)?;
                markdown.write_to_file(&path)?;
            }
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
        let mut stocks_window_list = StocksWindowList::new();
        stocks_window_list.push(ohlc_vec, code, name, unit, &from, &to);

        Ok(stocks_window_list)
    }

    let nikkei225 = match load_nikkei225_list() {
        Ok(res) => res,
        Err(e) => {
            error!("{}", e);
            return Err(e);
        }
    };
    info!("Nikkei225 has been loaded");

    let config = crate::config::GdriveJson::new()?;
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

            let row = 3;
            let (long_morning_close, long_afternoon_close) =
                someday_list.mean_long_rows_someday(row);
            let (short_morning_close, short_afternoon_close) =
                someday_list.mean_short_rows_someday(row);
            // long_morning_sum += long_morning_close;
            // long_afternoon_sum_mp += long_afternoon_close;
            // // short_morning_sum += short_morning_close;
            // short_afternoon_sum_mp += short_afternoon_close;

            if topix_list.get_mild_positive().contains(someday) {
                println!("mild_positive");
                long_afternoon_sum_mp += long_morning_close;
                short_afternoon_sum_mp += short_morning_close;
                i_mp += 1.0;
            } else if topix_list.get_mild_negative().contains(someday) {
                println!("mild_negative");
                long_afternoon_sum_mn += long_morning_close;
                short_afternoon_sum_mn += short_morning_close;
                i_mn += 1.0;
            } else if topix_list.get_strong_positive().contains(someday) {
                println!("strong_positive");
                long_afternoon_sum_sp += long_morning_close;
                short_afternoon_sum_sp += short_morning_close;
                i_sp += 1.0;
            } else if topix_list.get_strong_negative().contains(someday) {
                println!("strong_negative");
                long_afternoon_sum_sn += long_morning_close;
                short_afternoon_sum_sn += short_morning_close;
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
