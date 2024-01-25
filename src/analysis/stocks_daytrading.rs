use crate::markdown::Markdown;
use crate::my_file_io::Nikkei225;
use crate::my_file_io::{get_fetched_ohlc_file_path, load_nikkei225_list, AssetType};
use crate::{analysis::live::OhlcPremium, my_error::MyError};
use anyhow::anyhow;
use chrono::{Duration, NaiveDate};
use log::{error, info};
use serde::{Deserialize, Serialize};
use statrs::distribution::ContinuousCDF;
use statrs::distribution::StudentsT;
use statrs::statistics::Statistics;
use std::fmt::Write;
use std::time::Instant;

use super::backtesting_topix::{TopixDailyWindowList, TopixDailyWindowList2};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StocksDaytrading {
    code: i32,
    name: String,
    status: Status,
    atr: f64,
    unit: i32,
    required_amount: i32,
    standardized_diff: f64,
    result_push_close: Option<f64>,
    result_morning_close: Option<f64>,
    result_afternoon_open: Option<f64>,
    result_close: Option<f64>,
    analyzed_at: String,
}
impl StocksDaytrading {
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

        let result_push_close = match ohlc_vec.len() > position + 1 {
            true => {
                let mean_price = (ohlc_vec[position + 1].get_morning_close()
                    + ohlc_vec[position + 1].get_open())
                    / 2.0;
                let result_push_price = (mean_price - ohlc_vec[position + 1].get_open()) / atr;
                Some((result_push_price * 100.0).round() / 100.0)
            }
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

        Ok(Self {
            code,
            name: name.to_owned(),
            status,
            atr,
            unit,
            required_amount,
            standardized_diff,
            result_push_close,
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
            "{} {}, ({}, {}, {}), {}円",
            self.code, name, self.atr, self.unit, self.standardized_diff, self.required_amount
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
}

#[derive(Debug)]
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

    // pub fn push(&mut self, stocks_daytrading: StocksDaytrading) {
    //     self.data.push(stocks_daytrading);
    // }

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

    pub fn output_for_markdown(&self, date: &str) -> Result<Markdown, MyError> {
        let mut markdown = Markdown::new();
        markdown.h1(date)?;

        let mut markdown_br = Markdown::new();
        let mut markdown_fbr = Markdown::new();
        let mut markdown_fbs = Markdown::new();
        let mut markdown_bs = Markdown::new();

        markdown_br.h2("Breakout Resistance")?;
        markdown_fbr.h2("Failed Breakout Resistance")?;
        markdown_fbs.h2("Failed Breakout Support")?;
        markdown_bs.h2("Breakout Support")?;

        for stocks_daytrading in &self.data {
            match stocks_daytrading.status {
                Status::BreakoutResistance => {
                    markdown_br.body(&stocks_daytrading.markdown_body_output())?;
                }
                Status::FailedBreakoutResistance => {
                    markdown_fbr.body(&stocks_daytrading.markdown_body_output())?;
                }
                Status::FailedBreakoutSupport => {
                    markdown_fbs.body(&stocks_daytrading.markdown_body_output())?;
                }
                Status::BreakoutSupport => {
                    markdown_bs.body(&stocks_daytrading.markdown_body_output())?;
                }
                _ => {}
            }
        }

        markdown.append(markdown_br);
        markdown.append(markdown_fbr);
        markdown.append(markdown_fbs);
        markdown.append(markdown_bs);

        info!("{}", markdown.buffer());

        Ok(markdown)
    }

    fn t_test(&self) -> String {
        let morning_close = TTestResult::new(
            self.data
                .iter()
                .map(|stocks_daytrading| stocks_daytrading.result_morning_close.unwrap_or(0.0))
                .collect::<Vec<_>>(),
        );

        let afternoon_open = TTestResult::new(
            self.data
                .iter()
                .map(|stocks_daytrading| stocks_daytrading.result_afternoon_open.unwrap_or(0.0))
                .collect::<Vec<_>>(),
        );

        let close = TTestResult::new(
            self.data
                .iter()
                .map(|stocks_daytrading| stocks_daytrading.result_close.unwrap_or(0.0))
                .collect::<Vec<_>>(),
        );

        let threshold = 0.7;

        let close_with_mc_mc_loss_cut = if morning_close.get_mean() > 0.0 {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) < -threshold {
                            stocks_daytrading.result_morning_close.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) > threshold {
                            stocks_daytrading.result_morning_close.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        };

        let close_with_mc_ao_loss_cut = if morning_close.get_mean() > 0.0 {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) < -threshold {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) > threshold {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        };

        let close_with_ao_ao_loss_cut = if morning_close.get_mean() > 0.0 {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_afternoon_open.unwrap_or(0.0) < -threshold {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_afternoon_open.unwrap_or(0.0) > threshold {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        };

        let close_with_mc_mc_rikaku = if morning_close.get_mean() > 0.0 {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) > threshold {
                            stocks_daytrading.result_morning_close.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) < -threshold {
                            stocks_daytrading.result_morning_close.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let close_with_mc_ao_rikaku = if morning_close.get_mean() > 0.0 {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) > threshold {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) < -threshold {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        };

        let close_with_ao_ao_rikaku = if morning_close.get_mean() > 0.0 {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_afternoon_open.unwrap_or(0.0) > threshold {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_afternoon_open.unwrap_or(0.0) < -threshold {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        };

        let close_with_loss_cut_and_rikaku_1 = if morning_close.get_mean() > 0.0 {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) > threshold {
                            stocks_daytrading.result_morning_close.unwrap_or(0.0)
                        } else if stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                            < -threshold
                        {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) < -threshold {
                            stocks_daytrading.result_morning_close.unwrap_or(0.0)
                        } else if stocks_daytrading.result_afternoon_open.unwrap_or(0.0) > threshold
                        {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        };

        let close_with_loss_cut_and_rikaku_2 = if morning_close.get_mean() > 0.0 {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) < -threshold {
                            stocks_daytrading.result_morning_close.unwrap_or(0.0)
                        } else if stocks_daytrading.result_afternoon_open.unwrap_or(0.0) > threshold
                        {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            TTestResult::new(
                self.data
                    .iter()
                    .map(|stocks_daytrading| {
                        if stocks_daytrading.result_morning_close.unwrap_or(0.0) > threshold {
                            stocks_daytrading.result_morning_close.unwrap_or(0.0)
                        } else if stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                            < -threshold
                        {
                            stocks_daytrading.result_afternoon_open.unwrap_or(0.0)
                        } else {
                            stocks_daytrading.result_close.unwrap_or(0.0)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        };

        let mut buffer = String::new();
        writeln!(buffer, "morning_close: {}", morning_close).unwrap();
        // writeln!(buffer, "afternoon_open: {}", afternoon_open).unwrap();
        writeln!(buffer, "close: {}", close).unwrap();
        // writeln!(
        //     buffer,
        //     "close_with_mc_mc_loss_cut: {}",
        //     close_with_mc_mc_loss_cut
        // )
        // .unwrap();
        // writeln!(
        //     buffer,
        //     "close_with_mc_ao_loss_cut: {}",
        //     close_with_mc_ao_loss_cut
        // )
        // .unwrap();
        // writeln!(
        //     buffer,
        //     "close_with_ao_ao_loss_cut: {}",
        //     close_with_ao_ao_loss_cut
        // )
        // .unwrap();
        // writeln!(
        //     buffer,
        //     "close_with_mc_mc_rikaku: {}",
        //     close_with_mc_mc_rikaku
        // )
        // .unwrap();
        // writeln!(
        //     buffer,
        //     "close_with_mc_ao_rikaku: {}",
        //     close_with_mc_ao_rikaku
        // )
        // .unwrap();
        // writeln!(
        //     buffer,
        //     "close_with_ao_ao_rikaku: {}",
        //     close_with_ao_ao_rikaku
        // )
        // .unwrap();
        // writeln!(buffer, "close_with_rikaku: {}", close_with_rikaku).unwrap();
        // writeln!(
        //     buffer,
        //     "close_with_afternoon_rikaku: {}",
        //     close_with_afternoon_rikaku
        // )
        // .unwrap();
        // writeln!(
        //     buffer,
        //     "close_with_loss_cut_and_rikaku_1: {}",
        //     close_with_loss_cut_and_rikaku_1
        // )
        // .unwrap();
        // writeln!(
        //     buffer,
        //     "close_with_loss_cut_and_rikaku_2: {}",
        //     close_with_loss_cut_and_rikaku_2
        // )
        // .unwrap();
        // writeln!(buffer, "close_with_push: {}", close_with_push).unwrap();
        // writeln!(
        //     buffer,
        //     "close_with_loss_cut_and_push: {}",
        //     close_with_loss_cut_and_push
        // )
        // .unwrap();

        buffer
    }

    pub fn get_windows_related_result_2(
        &self,
        status: Status,
        topix_daily_window_list: &TopixDailyWindowList,
    ) -> String {
        let mut buffer = String::new();
        writeln!(buffer).unwrap();
        writeln!(buffer, "<{:?}>", status).unwrap();

        let limit = [(0.0, 0.09), (0.09, 0.12), (0.12, 0.40)];

        writeln!(buffer, "Strong Positive").unwrap();
        for (lower_limit, upper_limit) in limit.iter() {
            let data = self.data.clone();
            let strong_positive = data
                .into_iter()
                .filter(|stocks_daytrading| {
                    stocks_daytrading.status == status
                        && topix_daily_window_list
                            .get_strong_positive()
                            .contains(&stocks_daytrading.analyzed_at)
                        && (*lower_limit..*upper_limit)
                            .contains(&stocks_daytrading.standardized_diff)
                })
                .collect::<Vec<_>>();
            let strong_positive_list = StocksDaytradingList::from_vec(strong_positive);
            writeln!(
                buffer,
                "{}-{}: N={}",
                lower_limit,
                upper_limit,
                strong_positive_list.data.len(),
                // strong_positive_list.t_test()
            )
            .unwrap();
            writeln!(buffer, "{}", strong_positive_list.t_test()).unwrap();
        }

        writeln!(buffer).unwrap();
        writeln!(buffer, "Mild Positive").unwrap();
        for (lower_limit, upper_limit) in limit.iter() {
            let data = self.data.clone();
            let mild_positive = data
                .into_iter()
                .filter(|stocks_daytrading| {
                    stocks_daytrading.status == status
                        && topix_daily_window_list
                            .get_mild_positive()
                            .contains(&stocks_daytrading.analyzed_at)
                        && (*lower_limit..*upper_limit)
                            .contains(&stocks_daytrading.standardized_diff)
                })
                .collect::<Vec<_>>();
            let mild_positive_list = StocksDaytradingList::from_vec(mild_positive);
            writeln!(
                buffer,
                "{}-{}: N={}",
                lower_limit,
                upper_limit,
                mild_positive_list.data.len(),
                // mild_positive_list.t_test()
            )
            .unwrap();
            writeln!(buffer, "{}", mild_positive_list.t_test()).unwrap();
        }

        writeln!(buffer).unwrap();
        writeln!(buffer, "Mild Negative").unwrap();
        for (lower_limit, upper_limit) in limit.iter() {
            let data = self.data.clone();
            let mild_negative = data
                .into_iter()
                .filter(|stocks_daytrading| {
                    stocks_daytrading.status == status
                        && topix_daily_window_list
                            .get_mild_negative()
                            .contains(&stocks_daytrading.analyzed_at)
                        && (*lower_limit..*upper_limit)
                            .contains(&stocks_daytrading.standardized_diff)
                })
                .collect::<Vec<_>>();
            let mild_negative_list = StocksDaytradingList::from_vec(mild_negative);
            writeln!(
                buffer,
                "{}-{}: N={}",
                lower_limit,
                upper_limit,
                mild_negative_list.data.len(),
                // mild_negative_list.t_test()
            )
            .unwrap();
            writeln!(buffer, "{}", mild_negative_list.t_test()).unwrap();
        }

        writeln!(buffer).unwrap();
        writeln!(buffer, "Strong Negative").unwrap();
        for (lower_limit, upper_limit) in limit.iter() {
            let data = self.data.clone();
            let strong_negative = data
                .into_iter()
                .filter(|stocks_daytrading| {
                    stocks_daytrading.status == status
                        && topix_daily_window_list
                            .get_strong_negative()
                            .contains(&stocks_daytrading.analyzed_at)
                        && (*lower_limit..*upper_limit)
                            .contains(&stocks_daytrading.standardized_diff)
                })
                .collect::<Vec<_>>();
            let strong_negative_list = StocksDaytradingList::from_vec(strong_negative);
            writeln!(
                buffer,
                "{}-{}: N={}",
                lower_limit,
                upper_limit,
                strong_negative_list.data.len(),
                // strong_negative_list.t_test()
            )
            .unwrap();
            writeln!(buffer, "{}", strong_negative_list.t_test()).unwrap();
        }

        buffer
    }

    pub fn get_windows_related_result_3(
        &self,
        status: Status,
        topix_daily_window_list: &TopixDailyWindowList2,
    ) -> String {
        let mut buffer = String::new();
        writeln!(buffer).unwrap();
        writeln!(buffer, "<{:?}>", status).unwrap();

        let limit = [(0.0, 0.09), (0.09, 0.115), (0.115, 0.4)];

        writeln!(buffer, "Strong Positive").unwrap();
        for (lower_limit, upper_limit) in limit.iter() {
            let data = self.data.clone();
            let strong_positive = data
                .into_iter()
                .filter(|stocks_daytrading| {
                    stocks_daytrading.status == status
                        && topix_daily_window_list
                            .get_strong_positive()
                            .contains(&stocks_daytrading.analyzed_at)
                        && (*lower_limit..*upper_limit)
                            .contains(&stocks_daytrading.standardized_diff)
                })
                .collect::<Vec<_>>();
            let strong_positive_list = StocksDaytradingList::from_vec(strong_positive);
            writeln!(
                buffer,
                "{}-{}: N={}",
                lower_limit,
                upper_limit,
                strong_positive_list.data.len(),
                // strong_positive_list.t_test()
            )
            .unwrap();
            writeln!(buffer, "{}", strong_positive_list.t_test()).unwrap();
        }

        writeln!(buffer).unwrap();
        writeln!(buffer, "Moderate Positive").unwrap();
        for (lower_limit, upper_limit) in limit.iter() {
            let data = self.data.clone();
            let strong_positive = data
                .into_iter()
                .filter(|stocks_daytrading| {
                    stocks_daytrading.status == status
                        && topix_daily_window_list
                            .get_moderate_positive()
                            .contains(&stocks_daytrading.analyzed_at)
                        && (*lower_limit..*upper_limit)
                            .contains(&stocks_daytrading.standardized_diff)
                })
                .collect::<Vec<_>>();
            let strong_positive_list = StocksDaytradingList::from_vec(strong_positive);
            writeln!(
                buffer,
                "{}-{}: N={}",
                lower_limit,
                upper_limit,
                strong_positive_list.data.len(),
                // strong_positive_list.t_test()
            )
            .unwrap();
            writeln!(buffer, "{}", strong_positive_list.t_test()).unwrap();
        }

        writeln!(buffer).unwrap();
        writeln!(buffer, "Mild Positive").unwrap();
        for (lower_limit, upper_limit) in limit.iter() {
            let data = self.data.clone();
            let mild_positive = data
                .into_iter()
                .filter(|stocks_daytrading| {
                    stocks_daytrading.status == status
                        && topix_daily_window_list
                            .get_mild_positive()
                            .contains(&stocks_daytrading.analyzed_at)
                        && (*lower_limit..*upper_limit)
                            .contains(&stocks_daytrading.standardized_diff)
                })
                .collect::<Vec<_>>();
            let mild_positive_list = StocksDaytradingList::from_vec(mild_positive);
            writeln!(
                buffer,
                "{}-{}: N={}",
                lower_limit,
                upper_limit,
                mild_positive_list.data.len(),
                // mild_positive_list.t_test()
            )
            .unwrap();
            writeln!(buffer, "{}", mild_positive_list.t_test()).unwrap();
        }

        writeln!(buffer).unwrap();
        writeln!(buffer, "Mild Negative").unwrap();
        for (lower_limit, upper_limit) in limit.iter() {
            let data = self.data.clone();
            let mild_negative = data
                .into_iter()
                .filter(|stocks_daytrading| {
                    stocks_daytrading.status == status
                        && topix_daily_window_list
                            .get_mild_negative()
                            .contains(&stocks_daytrading.analyzed_at)
                        && (*lower_limit..*upper_limit)
                            .contains(&stocks_daytrading.standardized_diff)
                })
                .collect::<Vec<_>>();
            let mild_negative_list = StocksDaytradingList::from_vec(mild_negative);
            writeln!(
                buffer,
                "{}-{}: N={}",
                lower_limit,
                upper_limit,
                mild_negative_list.data.len(),
                // mild_negative_list.t_test()
            )
            .unwrap();
            writeln!(buffer, "{}", mild_negative_list.t_test()).unwrap();
        }

        writeln!(buffer).unwrap();
        writeln!(buffer, "Moderate Negative").unwrap();
        for (lower_limit, upper_limit) in limit.iter() {
            let data = self.data.clone();
            let mild_negative = data
                .into_iter()
                .filter(|stocks_daytrading| {
                    stocks_daytrading.status == status
                        && topix_daily_window_list
                            .get_moderate_negative()
                            .contains(&stocks_daytrading.analyzed_at)
                        && (*lower_limit..*upper_limit)
                            .contains(&stocks_daytrading.standardized_diff)
                })
                .collect::<Vec<_>>();
            let mild_negative_list = StocksDaytradingList::from_vec(mild_negative);
            writeln!(
                buffer,
                "{}-{}: N={}",
                lower_limit,
                upper_limit,
                mild_negative_list.data.len(),
                // mild_negative_list.t_test()
            )
            .unwrap();
            writeln!(buffer, "{}", mild_negative_list.t_test()).unwrap();
        }

        writeln!(buffer).unwrap();
        writeln!(buffer, "Strong Negative").unwrap();
        for (lower_limit, upper_limit) in limit.iter() {
            let data = self.data.clone();
            let strong_negative = data
                .into_iter()
                .filter(|stocks_daytrading| {
                    stocks_daytrading.status == status
                        && topix_daily_window_list
                            .get_strong_negative()
                            .contains(&stocks_daytrading.analyzed_at)
                        && (*lower_limit..*upper_limit)
                            .contains(&stocks_daytrading.standardized_diff)
                })
                .collect::<Vec<_>>();
            let strong_negative_list = StocksDaytradingList::from_vec(strong_negative);
            writeln!(
                buffer,
                "{}-{}: N={}",
                lower_limit,
                upper_limit,
                strong_negative_list.data.len(),
                // strong_negative_list.t_test()
            )
            .unwrap();
            writeln!(buffer, "{}", strong_negative_list.t_test()).unwrap();
        }

        buffer
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

        let (mean, p_value) = match mean >= 0.0 {
            true => (
                (mean * 1000.0).round() / 1000.0,
                1.0 - (t_distribution.cdf(t) * 1000.0).round() / 1000.0,
            ),
            false => (
                (mean * 1000.0).round() / 1000.0,
                (t_distribution.cdf(t) * 1000.0).round() / 1000.0,
            ),
        };

        Self { mean, p_value }
    }
    //getters
    fn get_mean(&self) -> f64 {
        self.mean
    }
    fn get_p_value(&self) -> f64 {
        self.p_value
    }
}
impl std::fmt::Display for TTestResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let p_value = (self.get_p_value() * 100.0).round() / 100.0;
        match p_value < 0.05 {
            true => write!(
                f,
                "mean: {}, p: {} ... sig. diff. (95%)",
                self.get_mean(),
                p_value
            ),
            false => write!(f, "mean: {}, p: {}", self.get_mean(), p_value),
        }
    }
}

// pub struct Output {
//     date: String,
//     breakout_resistance_stocks: String,
//     failed_breakout_resistance_stocks: String,
//     failed_breakout_support_stocks: String,
//     breakout_support_stocks: String,
// }
// impl Output {
//     pub fn get_date(&self) -> &str {
//         &self.date
//     }
//     pub fn get_breakout_resistance_stocks(&self) -> &str {
//         &self.breakout_resistance_stocks
//     }
//     pub fn get_failed_breakout_resistance_stocks(&self) -> &str {
//         &self.failed_breakout_resistance_stocks
//     }
//     pub fn get_failed_breakout_support_stocks(&self) -> &str {
//         &self.failed_breakout_support_stocks
//     }
//     pub fn get_breakout_support_stocks(&self) -> &str {
//         &self.breakout_support_stocks
//     }
// }

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub enum Status {
    BreakoutResistance,
    FailedBreakoutResistance,
    NoChange,
    FailedBreakoutSupport,
    BreakoutSupport,
}

pub async fn async_exec(from: &str, to: &str) -> Result<StocksDaytradingList, MyError> {
    async fn inner(
        row: Nikkei225,
        unit: f64,
        from: String,
        to: String,
    ) -> Result<StocksDaytradingList, MyError> {
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
        // let stocks_daytrading = StocksDaytrading::from_vec(&ohlc_vec, code, name, unit, &date)?;
        let mut stocks_daytrading_list = StocksDaytradingList::new();
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

    let config = crate::config::GdriveJson::new()?;
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
