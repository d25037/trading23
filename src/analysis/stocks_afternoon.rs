use log::{debug, error, info};
use serde::{Deserialize, Serialize};

use crate::jquants::fetcher::{PricesAm, PricesAmInner};
use crate::markdown::Markdown;
use crate::my_error::MyError;
use crate::my_file_io::{get_fetched_ohlc_file_path, load_nikkei225_list, AssetType, JquantsStyle};

use super::live::OhlcPremium;
use anyhow::anyhow;
use std::fmt::Write;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StocksAfternoon {
    code: String,
    name: String,
    atr: f64,
    unit: i32,
    required_amount: i32,
    latest_move: f64,
    standardized_diff: f64,
    number_of_resistance_candles: usize,
    number_of_support_candles: usize,
    status: String,
    yesterday_close: f64,
    morning_open: f64,
    morning_close: f64,
    analyzed_at: String,
}

impl StocksAfternoon {
    pub fn from_vec(
        ohlc_vec: &[OhlcPremium],
        prices_am: PricesAmInner,
        code: &str,
        name: &str,
        unit: f64,
        date: &str,
    ) -> Result<Self, MyError> {
        let position = match ohlc_vec[ohlc_vec.len() - 1].get_date() {
            x if x == date => ohlc_vec.len() - 2,
            _ => ohlc_vec.len() - 1,
        };

        if position < 60 {
            return Err(MyError::OutOfRange);
        }

        let ohlc_5 = &ohlc_vec[(position - 4)..=position];
        let ohlc_20 = &ohlc_vec[(position - 19)..=position];
        let ohlc_60 = &ohlc_vec[(position - 59)..=position];

        let (morning_open, morning_close) = (prices_am.get_open(), prices_am.get_close());

        let (prev_19, last) = ohlc_20.split_at(19);
        // let prev_19_high = prev_19
        //     .iter()
        //     .map(|ohlc| ohlc.get_high())
        //     .fold(f64::NAN, f64::max);
        // let prev_19_low = prev_19
        //     .iter()
        //     .map(|ohlc| ohlc.get_low())
        //     .fold(f64::NAN, f64::min);

        let atr = ohlc_5
            .iter()
            .map(|ohlc| (ohlc.get_high() - ohlc.get_low()))
            .sum::<f64>()
            / ohlc_5.len() as f64;
        let atr = (atr * 10.0).round() / 10.0;

        let (unit, required_amount) = {
            let unit = unit / atr;
            let required_amount = (unit * last[0].get_close()) as i32;
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
            .filter(|ohlc| ohlc.get_high() > prices_am.get_high() && morning_close > ohlc.get_low())
            .count();
        let number_of_support_candles = ohlc_60
            .iter()
            .filter(|ohlc| ohlc.get_high() > morning_close && prices_am.get_low() > ohlc.get_low())
            .count();

        let status = match prices_am.get_close() - ohlc_5[4].get_open() {
            x if x > 0.0 => {
                if prices_am.get_close() - ohlc_5[4].get_open() > 0.0 {
                    "Rise"
                } else {
                    "Rise bounded"
                }
            }
            x if x < 0.0 => {
                if prices_am.get_close() - ohlc_5[4].get_open() > 0.0 {
                    "Fall bounded"
                } else {
                    "Fall"
                }
            }
            _ => "Stable",
        };

        let yesterday_close = ohlc_vec[position - 1].get_close();

        let latest_move = (morning_close - morning_open) / (last[0].get_high() - last[0].get_low());
        let latest_move = (latest_move * 100.0).round() / 100.0;
        let latest_move = latest_move.abs();

        Ok(Self {
            code: code.to_owned(),
            name: name.to_owned(),
            atr,
            unit,
            required_amount,
            latest_move,
            standardized_diff,
            number_of_resistance_candles,
            number_of_support_candles,
            status: status.to_owned(),
            yesterday_close,
            morning_open,
            morning_close,
            analyzed_at: date.to_owned(),
        })
    }

    fn markdown_body_output(&self) -> Result<String, MyError> {
        let mut buffer = String::new();
        let name = match self.name.chars().count() > 5 {
            true => {
                let name: String = self.name.chars().take(4).collect();
                name
            }
            false => self.name.to_owned(),
        };

        let morning_result = (self.morning_close - self.morning_open) / self.atr;
        let morning_result = (morning_result * 100.0).round() / 100.0;

        writeln!(
            buffer,
            "{} {}, {}円, {} [R: {}, S: {}] LM: {}",
            self.code,
            name,
            self.morning_close,
            self.status,
            self.number_of_resistance_candles,
            self.number_of_support_candles,
            self.latest_move,
        )?;

        writeln!(
            buffer,
            "ATR: {}, Unit: {}, 必要金額: {}円",
            self.atr, self.unit, self.required_amount
        )?;

        writeln!(buffer, "Morning Result: {}", morning_result)?;

        Ok(buffer)
    }
}

#[derive(Debug, Clone)]
pub struct StocksAfternoonList {
    data: Vec<StocksAfternoon>,
}
impl From<Vec<StocksAfternoon>> for StocksAfternoonList {
    fn from(data: Vec<StocksAfternoon>) -> Self {
        StocksAfternoonList { data }
    }
}
impl StocksAfternoonList {
    // pub fn new() -> Self {
    //     Self { data: Vec::new() }
    // }
    fn from_vec(vec: Vec<StocksAfternoon>) -> Self {
        Self { data: vec }
    }

    // fn append(&mut self, mut stocks_daytrading_list: StocksAfternoonList) {
    //     self.data.append(&mut stocks_daytrading_list.data);
    // }

    pub fn from_nikkei225(prices_am: &PricesAm) -> Result<Self, MyError> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        let nikkei225 = load_nikkei225_list()?;
        info!("Nikkei225 has been loaded");

        let config = crate::config::GdriveJson::new()?;
        let unit = config.jquants_unit();
        info!("unit: {}", unit);

        let result = nikkei225
            .into_iter()
            .filter(|row| {
                let code = row.get_code();
                prices_am.get_stock_am(code).is_ok()
            })
            .map(|row| {
                let (code, name) = (row.get_code(), row.get_name());
                let conn = crate::database::stocks_ohlc::open_db()?;

                let ohlc_vec = crate::database::stocks_ohlc::select_by_code(&conn, code)?;
                let mut ohlc_vec = ohlc_vec
                    .into_iter()
                    .map(|ohlc| ohlc.get_inner())
                    .collect::<Vec<_>>();
                ohlc_vec.reverse();
                // debug!("{:?}", ohlc_vec);

                let stock_am = prices_am.get_stock_am(code)?;
                let stocks_afternoon =
                    StocksAfternoon::from_vec(&ohlc_vec, stock_am, code, name, unit, &today)?;
                Ok(stocks_afternoon)
            })
            .collect::<Result<Vec<StocksAfternoon>, MyError>>()
            .map(Self::from_vec);

        result
    }

    fn filter_by_standardized_diff(&mut self, diff: f64) {
        self.data.retain(|x| x.standardized_diff < diff);
    }

    fn filter_by_latest_move(&mut self, latest_move: f64) {
        self.data.retain(|x| x.latest_move < latest_move);
    }

    fn get_resistance_candles_top10(&self) -> StocksAfternoonList {
        let mut resistance_candles_top10 = StocksAfternoonList::from(self.data.to_vec());
        resistance_candles_top10.data.sort_by(|a, b| {
            b.number_of_resistance_candles
                .partial_cmp(&a.number_of_resistance_candles)
                .unwrap()
        });
        StocksAfternoonList::from(
            resistance_candles_top10
                .data
                .into_iter()
                .take(10)
                .collect::<Vec<_>>(),
        )
    }
    fn get_support_candles_top10(&self) -> StocksAfternoonList {
        let mut support_candles_top10 = StocksAfternoonList::from(self.data.to_vec());
        support_candles_top10.data.sort_by(|a, b| {
            b.number_of_support_candles
                .partial_cmp(&a.number_of_support_candles)
                .unwrap()
        });
        StocksAfternoonList::from(
            support_candles_top10
                .data
                .into_iter()
                .take(10)
                .collect::<Vec<_>>(),
        )
    }

    fn output_for_markdown_afternoon(&self, date: &str) -> Result<Markdown, MyError> {
        let mut markdown = Markdown::new();
        markdown.h1(date)?;
        markdown.h2("Afternoon Strategy")?;

        let resistance_candles_top10 = self.get_resistance_candles_top10();
        markdown.h3("Resistance Candles Top 10")?;
        for stocks_afternoon in &resistance_candles_top10.data {
            markdown.body(&stocks_afternoon.markdown_body_output()?)?;
        }

        let support_candles_top10 = self.get_support_candles_top10();
        markdown.h3("Support Candles Top 10")?;
        for stocks_afternoon in &support_candles_top10.data {
            markdown.body(&stocks_afternoon.markdown_body_output()?)?;
        }

        info!("{}", markdown.buffer());

        Ok(markdown)
    }

    pub fn for_resistance_strategy(&mut self, consolidating: bool) -> Result<(), MyError> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.filter_by_standardized_diff(0.12);
        if consolidating {
            self.filter_by_latest_move(0.25);
        }

        let markdown = self.output_for_markdown_afternoon(&today)?;
        let path = match consolidating {
            true => {
                crate::my_file_io::get_jquants_path(JquantsStyle::ConsolidatingAfternoon, &today)?
            }
            false => crate::my_file_io::get_jquants_path(JquantsStyle::Afternoon, &today)?,
        };
        markdown.write_to_html(&path)?;

        Ok(())
    }
    pub fn for_resistance_strategy_default(&mut self) -> Result<(), MyError> {
        self.for_resistance_strategy(false)
    }
}
