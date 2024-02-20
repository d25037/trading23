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
    code: String,
    name: String,
    atr: f64,
    unit: i32,
    required_amount: i32,
    latest_move: f64,
    standardized_diff: f64,
    current_price: f64,
    lower_bound: f64,
    upper_bound: f64,
    number_of_resistance_candles: usize,
    number_of_support_candles: usize,
    status: String,
    result_morning: Option<f64>,
    result_afternoon: Option<f64>,
    result_allday: Option<f64>,
    nextday_morning_close: Option<f64>,
    morning_move: Option<f64>,
    analyzed_at: String,
    result_at: Option<String>,
}

impl StocksWindow {
    pub fn from_vec(
        ohlc_vec: &[OhlcPremium],
        code: &str,
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

        let current_price = ohlc_vec[position].get_close();

        let ohlc_2 = &ohlc_vec[(position - 1)..=position];
        let ohlc_5 = &ohlc_vec[(position - 4)..=position];
        let ohlc_20 = &ohlc_vec[(position - 19)..=position];
        let ohlc_60 = &ohlc_vec[(position - 59)..=position];

        let (prev_19, last) = ohlc_20.split_at(19);
        let last_close = last[0].get_close();
        // let last2_close = prev_19[18].get_close();
        let prev_19_high = prev_19
            .iter()
            .map(|ohlc| ohlc.get_high())
            .fold(f64::NAN, f64::max);
        let prev_19_low = prev_19
            .iter()
            .map(|ohlc| ohlc.get_low())
            .fold(f64::NAN, f64::min);
        // let latest_move = (last_close - last2_close) / (prev_19_high - prev_19_low);
        // let latest_move = (latest_move * 100.0).round() / 100.0;

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

        let number_of_resistance_candles = ohlc_60
            .iter()
            .filter(|ohlc| {
                ohlc.get_high() > ohlc_vec[position].get_high() && current_price > ohlc.get_low()
            })
            .count();
        let number_of_support_candles = ohlc_60
            .iter()
            .filter(|ohlc| {
                ohlc.get_high() > current_price && ohlc_vec[position].get_low() > ohlc.get_low()
            })
            .count();

        let status = match ohlc_2[1].get_close() - ohlc_2[0].get_open() {
            x if x > 0.0 => {
                if ohlc_2[1].get_close() - ohlc_2[1].get_open() > 0.0 {
                    "Rise"
                } else {
                    "Rise bounded"
                }
            }
            x if x < 0.0 => {
                if ohlc_2[1].get_close() - ohlc_2[1].get_open() > 0.0 {
                    "Fall bounded"
                } else {
                    "Fall"
                }
            }
            _ => "Stable",
        };

        let latest_move = (ohlc_2[1].get_close() - ohlc_2[1].get_open())
            / (ohlc_2[0].get_high() - ohlc_2[0].get_low());
        let latest_move = (latest_move * 100.0).round() / 100.0;
        let latest_move = latest_move.abs();

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

        let analyzed_at = ohlc_vec[position].get_date().to_owned();

        let (
            nextday_morning_close,
            result_morning,
            result_afternoon,
            result_allday,
            morning_move,
            result_at,
        ) = match ohlc_vec.len() > position + 1 {
            true => {
                let nextday_morning_close = ohlc_vec[position + 1].get_morning_close();
                let result_morning = {
                    let price = (ohlc_vec[position + 1].get_morning_close()
                        - ohlc_vec[position + 1].get_open())
                        / atr;
                    (price * 100.0).round() / 100.0
                };
                let result_afternoon = {
                    let price = (ohlc_vec[position + 1].get_close()
                        - ohlc_vec[position + 1].get_afternoon_open())
                        / atr;
                    (price * 100.0).round() / 100.0
                };
                let result_allday = {
                    let price = (ohlc_vec[position + 1].get_close()
                        - ohlc_vec[position + 1].get_open())
                        / atr;
                    (price * 100.0).round() / 100.0
                };
                let morning_move = {
                    let price = (ohlc_vec[position + 1].get_morning_close()
                        - ohlc_vec[position].get_close())
                        / (prev_19_high - prev_19_low);
                    (price * 100.0).round() / 100.0
                };
                let result_at = ohlc_vec[position + 1].get_date().to_owned();
                (
                    Some(nextday_morning_close),
                    Some(result_morning),
                    Some(result_afternoon),
                    Some(result_allday),
                    Some(morning_move),
                    Some(result_at),
                )
            }
            false => (None, None, None, None, None, None),
        };

        Ok(Self {
            code: code.to_owned(),
            name: name.to_owned(),
            atr,
            unit,
            required_amount,
            latest_move,
            standardized_diff,
            current_price,
            lower_bound,
            upper_bound,
            number_of_resistance_candles,
            number_of_support_candles,
            status: status.to_owned(),
            result_morning,
            result_afternoon,
            result_allday,
            nextday_morning_close,
            morning_move,
            analyzed_at,
            result_at,
        })
    }

    // fn markdown_body_output_for_cloud(&self, afternoon: bool) -> Result<String, MyError> {
    //     let mut buffer = String::new();

    //     let (current_price, latest_move) = match afternoon {
    //         true => (
    //             self.nextday_morning_close.unwrap(),
    //             self.morning_move.unwrap(),
    //         ),
    //         false => (self.current_price, self.latest_move),
    //     };

    //     let name = match self.name.chars().count() > 5 {
    //         true => {
    //             let name: String = self.name.chars().take(4).collect();
    //             name
    //         }
    //         false => self.name.to_owned(),
    //     };

    //     let (status, difference) = match current_price {
    //         x if x < self.lower_bound => {
    //             let difference = (current_price - self.lower_bound) / self.atr;
    //             let difference = (difference * 100.0).round() / 100.0;

    //             ("Below", Some(difference))
    //         }
    //         x if x > self.upper_bound => {
    //             let difference = (current_price - self.upper_bound) / self.atr;

    //             let difference = (difference * 100.0).round() / 100.0;
    //             ("Above", Some(difference))
    //         }
    //         _ => ("Between", None),
    //     };

    //     let difference_str = match difference {
    //         Some(difference) => format!("{}%", difference),
    //         None => "".to_owned(),
    //     };

    //     writeln!(
    //         buffer,
    //         "{} {}, {}円 {}({} - {}) {}",
    //         self.code,
    //         name,
    //         current_price,
    //         status,
    //         self.lower_bound,
    //         self.upper_bound,
    //         difference_str,
    //     )?;

    //     writeln!(
    //         buffer,
    //         "ATR: {}, Unit: {}, Diff: {} Move: {}, 必要金額: {}円",
    //         self.atr, self.unit, self.standardized_diff, latest_move, self.required_amount
    //     )?;

    //     if self.result_allday.is_some() {
    //         writeln!(
    //             buffer,
    //             "MC: {}, AC: {}",
    //             self.result_morning.unwrap(),
    //             self.result_allday.unwrap()
    //         )?;
    //     }

    //     Ok(buffer)
    // }

    // fn markdown_body_output_for_cloud_default(&self) -> Result<String, MyError> {
    //     self.markdown_body_output_for_cloud(false)
    // }

    fn markdown_body_output_for_resistance(&self, afternoon: bool) -> Result<String, MyError> {
        let mut buffer = String::new();

        let (current_price, latest_move) = match afternoon {
            true => (
                self.nextday_morning_close.unwrap(),
                self.morning_move.unwrap(),
            ),
            false => (self.current_price, self.latest_move),
        };

        let name = match self.name.chars().count() > 5 {
            true => {
                let name: String = self.name.chars().take(4).collect();
                name
            }
            false => self.name.to_owned(),
        };

        writeln!(
            buffer,
            "{} {}, {}円, {} [R: {}, S: {}] LM: {}",
            self.code,
            name,
            current_price,
            self.status,
            self.number_of_resistance_candles,
            self.number_of_support_candles,
            latest_move
        )?;

        writeln!(
            buffer,
            "ATR: {}, Unit: {}, 必要金額: {}円",
            self.atr, self.unit, self.required_amount
        )?;

        if self.result_allday.is_some() {
            writeln!(
                buffer,
                "Morning: {}, Afternoon: {}, Allday: {}",
                self.result_morning.unwrap(),
                self.result_afternoon.unwrap(),
                self.result_allday.unwrap()
            )?;
        }

        Ok(buffer)
    }
    fn markdown_body_output_for_resistance_default(&self) -> Result<String, MyError> {
        self.markdown_body_output_for_resistance(false)
    }
}

#[derive(Debug, Clone)]
pub struct StocksWindowList {
    data: Vec<StocksWindow>,
}
impl From<Vec<StocksWindow>> for StocksWindowList {
    fn from(data: Vec<StocksWindow>) -> Self {
        StocksWindowList { data }
    }
}
impl StocksWindowList {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
    // fn from_vec(vec: Vec<StocksWindow>) -> Self {
    //     Self { data: vec }
    // }
    pub fn push(
        &mut self,
        ohlc_vec: Vec<OhlcPremium>,
        code: &str,
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
                Ok(stocks_window) => self.data.push(stocks_window),
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

    fn filter_by_standardized_diff(&mut self, diff: f64) {
        self.data.retain(|x| x.standardized_diff < diff);
    }

    fn filter_by_latest_move(&mut self, latest_move: f64) {
        self.data.retain(|x| x.latest_move < latest_move);
    }

    fn get_resistance_candles_top10(&self) -> StocksWindowList {
        let mut resistance_candles_top10 = StocksWindowList::from(self.data.to_vec());
        resistance_candles_top10.data.sort_by(|a, b| {
            b.number_of_resistance_candles
                .partial_cmp(&a.number_of_resistance_candles)
                .unwrap()
        });
        StocksWindowList::from(
            resistance_candles_top10
                .data
                .into_iter()
                .take(10)
                .collect::<Vec<_>>(),
        )
    }
    fn get_support_candles_top10(&self) -> StocksWindowList {
        let mut support_candles_top10 = StocksWindowList::from(self.data.to_vec());
        support_candles_top10.data.sort_by(|a, b| {
            b.number_of_support_candles
                .partial_cmp(&a.number_of_support_candles)
                .unwrap()
        });
        StocksWindowList::from(
            support_candles_top10
                .data
                .into_iter()
                .take(10)
                .collect::<Vec<_>>(),
        )
    }

    fn number_of_morning_gainers(&self) -> f64 {
        self.data
            .iter()
            .filter(|x| x.result_morning.unwrap_or(0.0) > 0.0)
            .count() as f64
    }
    fn number_of_afternoon_gainers(&self) -> f64 {
        self.data
            .iter()
            .filter(|x| x.result_afternoon.unwrap_or(0.0) > 0.0)
            .count() as f64
    }
    fn number_of_allday_gainers(&self) -> f64 {
        self.data
            .iter()
            .filter(|x| x.result_allday.unwrap_or(0.0) > 0.0)
            .count() as f64
    }

    fn output_for_markdown_resistance_support(
        &self,
        afternoon: bool,
    ) -> Result<(Markdown, String), MyError> {
        let (date, title) = match afternoon {
            true => (self.data[0].result_at.clone().unwrap(), "This afternoon"),
            false => (self.data[0].analyzed_at.clone(), "Nextday"),
        };

        let len = self.data.len() as f64;

        let resistance = self.get_resistance_candles_top10();
        let support = self.get_support_candles_top10();

        let mut markdown = Markdown::new();
        markdown.h1(&date)?;
        markdown.h2(title)?;

        markdown.h3("Summary")?;
        markdown.body(&format!("Number of Stocks: {}", len))?;
        markdown.body(&format!(
            "Morning Gainers: {}%",
            (self.number_of_morning_gainers() / len * 100.0).round()
        ))?;
        markdown.body(&format!(
            "Afternoon Gainers: {}%",
            (self.number_of_afternoon_gainers() / len * 100.0).round()
        ))?;
        markdown.body(&format!(
            "Allday Gainers: {}%",
            (self.number_of_allday_gainers() / len * 100.0).round()
        ))?;

        markdown.h3("Resistance Candles Top 10")?;

        for resistance_row in resistance.data {
            match afternoon {
                true => {
                    markdown.body(&resistance_row.markdown_body_output_for_resistance(true)?)?
                }
                false => {
                    markdown.body(&resistance_row.markdown_body_output_for_resistance_default()?)?
                }
            }
        }
        markdown.h3("Support Candles Top 10")?;
        for support_row in support.data {
            match afternoon {
                true => markdown.body(&support_row.markdown_body_output_for_resistance(true)?)?,
                false => {
                    markdown.body(&support_row.markdown_body_output_for_resistance_default()?)?
                }
            }
        }

        debug!("{}", markdown.buffer());

        Ok((markdown, date))
    }

    pub fn for_resistance_strategy(&self, consolidating: bool) -> Result<(), MyError> {
        let mut date_to_stocks: HashMap<_, Vec<_>> = HashMap::new();

        for stocks_window in &self.data {
            date_to_stocks
                .entry(stocks_window.analyzed_at.clone())
                .or_default()
                .push(stocks_window.clone());
        }

        for (_, stocks_window_list) in date_to_stocks {
            let mut stocks_window_list = StocksWindowList::from(stocks_window_list);
            stocks_window_list.filter_by_standardized_diff(0.12);
            if consolidating {
                stocks_window_list.filter_by_latest_move(0.25);
            }

            let (markdown, analyzed_at) =
                stocks_window_list.output_for_markdown_resistance_support(false)?;
            let path = match consolidating {
                true => {
                    crate::my_file_io::get_jquants_path(JquantsStyle::Consolidating, &analyzed_at)?
                }
                false => {
                    crate::my_file_io::get_jquants_path(JquantsStyle::Resistance, &analyzed_at)?
                }
            };
            info!("{}", path.display());
            markdown.write_to_html(&path)?;
        }

        Ok(())
    }
    pub fn for_resistance_strategy_default(&self) -> Result<(), MyError> {
        self.for_resistance_strategy(false)
    }
}

pub async fn create_stocks_window_list_db(
    from: &str,
    to: &str,
) -> Result<StocksWindowList, MyError> {
    async fn inner(
        row: Nikkei225,
        unit: f64,
        from: String,
        to: String,
    ) -> Result<StocksWindowList, MyError> {
        let code = row.get_code();
        let name = row.get_name();
        let conn = crate::database::stocks_ohlc::open_db()?;

        let records = crate::database::stocks_ohlc::select_by_code(&conn, code)?;
        let mut ohlc_vec: Vec<OhlcPremium> = records
            .into_iter()
            .map(|x| x.get_inner())
            .collect::<Vec<_>>();
        ohlc_vec.sort_by(|a, b| {
            let date_a = NaiveDate::parse_from_str(a.get_date(), "%Y-%m-%d").unwrap();
            let date_b = NaiveDate::parse_from_str(b.get_date(), "%Y-%m-%d").unwrap();
            date_a.partial_cmp(&date_b).unwrap()
        });
        // debug!("{:?}", ohlc_vec);
        let mut stocks_window_list = StocksWindowList::new();
        stocks_window_list.push(ohlc_vec, code, name, unit, &from, &to);
        // debug!("{:?}", ohlc_vec);

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
    info!("Elapsed time: {:?}", start_time.elapsed());
    debug!("{:?}", stocks_daytrading_list);
    Ok(stocks_daytrading_list)
}
