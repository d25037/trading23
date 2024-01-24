use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::jquants::live::PricesAm;
use crate::markdown::Markdown;
use crate::my_error::MyError;
use crate::my_file_io::{get_fetched_ohlc_file_path, load_nikkei225_list, AssetType};

use super::live::OhlcPremium;
use anyhow::anyhow;
use std::fmt::Write;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StocksAfternoon {
    code: i32,
    name: String,
    atr: f64,
    unit: i32,
    required_amount: i32,
    latest_move: f64,
    standardized_diff: f64,
    yesterday_close: f64,
    lower_bound: f64,
    upper_bound: f64,
    morning_open: f64,
    morning_close: f64,
    analyzed_at: String,
}

impl StocksAfternoon {
    pub fn from_vec(
        ohlc_vec: &Vec<OhlcPremium>,
        prices_am: &PricesAm,
        code: i32,
        name: &str,
        unit: f64,
        date: &str,
    ) -> Result<Self, MyError> {
        let len = ohlc_vec.len();
        if len < 59 {
            return Err(MyError::OutOfRange);
        }
        // info!("{}: {} has been loaded", code, name);
        // info!("len: {}", len);

        let ohlc_5 = &ohlc_vec[(len - 5)..len];
        let ohlc_20 = &ohlc_vec[(len - 20)..len];
        let ohlc_60 = &ohlc_vec[(len - 60)..len];

        let (prev_19, last) = ohlc_20.split_at(19);
        let last_close = last[0].get_close();
        let prev_19_high = prev_19
            .iter()
            .map(|ohlc| ohlc.get_high())
            .fold(f64::NAN, f64::max);
        let prev_19_low = prev_19
            .iter()
            .map(|ohlc| ohlc.get_low())
            .fold(f64::NAN, f64::min);

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

        let yesterday_close = ohlc_vec[len - 1].get_close();

        let (morning_open, morning_close) = prices_am.get_stock_ohlc(code).unwrap_or((0.0, 0.0));

        let latest_move = (morning_close - last_close) / (prev_19_high - prev_19_low);
        let latest_move = (latest_move * 100.0).round() / 100.0;

        Ok(Self {
            code,
            name: name.to_owned(),
            atr,
            unit,
            required_amount,
            latest_move,
            standardized_diff,
            yesterday_close,
            lower_bound,
            upper_bound,
            morning_open,
            morning_close,
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

        let (status, difference) = match self.morning_close {
            x if x < self.lower_bound => {
                let difference = (self.morning_close - self.lower_bound) / self.atr;
                let difference = (difference * 100.0).round() / 100.0;

                ("Below", Some(difference))
            }
            x if x > self.upper_bound => {
                let difference = (self.morning_close - self.upper_bound) / self.atr;

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
            self.morning_close,
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

        buffer
    }
}

#[derive(Debug, Clone)]
pub struct StocksAfternoonList {
    data: Vec<StocksAfternoon>,
}
impl StocksAfternoonList {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
    fn from_vec(vec: Vec<StocksAfternoon>) -> Self {
        Self { data: vec }
    }

    fn append(&mut self, mut stocks_daytrading_list: StocksAfternoonList) {
        self.data.append(&mut stocks_daytrading_list.data);
    }

    pub fn from_nikkei225(prices_am: &PricesAm) -> Result<Self, MyError> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

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

        let result = nikkei225
            .into_iter()
            .map(|row| {
                let code = row.get_code();
                let name = row.get_name();
                let ohlc_vec_path =
                    match get_fetched_ohlc_file_path(AssetType::Stocks { code: Some(code) }) {
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
                let stocks_afternoon =
                    match StocksAfternoon::from_vec(&ohlc_vec, prices_am, code, name, unit, &today)
                    {
                        Ok(res) => res,
                        Err(e) => {
                            error!("{}", e);
                            return Err(e);
                        }
                    };
                Ok(stocks_afternoon)
            })
            .collect::<Result<Vec<StocksAfternoon>, MyError>>()
            .map(Self::from_vec);

        result
    }

    pub fn sort_by_abs_latest_move(&mut self) {
        self.data.sort_by(|a, b| {
            b.latest_move
                .abs()
                .partial_cmp(&a.latest_move.abs())
                .unwrap()
        });
    }

    fn get_on_the_cloud(&self) -> StocksAfternoonList {
        let on_the_cloud = self
            .data
            .iter()
            .filter(|x| {
                let difference = (x.morning_close - x.upper_bound) / x.atr;
                difference > 0.0 && difference < 0.2 && x.standardized_diff < 0.12
            })
            .collect::<Vec<_>>();

        StocksAfternoonList::from_vec(on_the_cloud.into_iter().cloned().collect())
    }

    fn get_between_the_cloud(&self) -> StocksAfternoonList {
        let between_the_cloud = self
            .data
            .iter()
            .filter(|x| {
                x.morning_close > x.lower_bound
                    && x.morning_close < x.upper_bound
                    && x.standardized_diff < 0.12
            })
            .collect::<Vec<_>>();

        StocksAfternoonList::from_vec(between_the_cloud.into_iter().cloned().collect())
    }

    fn get_under_the_cloud(&self) -> StocksAfternoonList {
        let under_the_cloud = self
            .data
            .iter()
            .filter(|x| {
                let difference = (x.morning_close - x.lower_bound) / x.atr;
                difference < 0.0 && difference > -0.2 && x.standardized_diff < 0.12
            })
            .collect::<Vec<_>>();

        StocksAfternoonList::from_vec(under_the_cloud.into_iter().cloned().collect())
    }

    pub fn output_for_markdown_afternoon(&self, date: &str) -> Markdown {
        let mut markdown = Markdown::new();
        markdown.h1(date);

        for stocks_afternoon in self.data.iter().take(10) {
            markdown.body(&stocks_afternoon.markdown_body_output());
        }

        // let (long_morning_close, long_afternoon_close) = self.mean_on_the_cloud();
        // markdown.body(&format!(
        //     "<Above> MC_mean5: {}, AC_mean5: {}",
        //     long_morning_close, long_afternoon_close
        // ));

        // let (short_morning_close, short_afternoon_close) = self.mean_under_the_cloud();
        // markdown.body(&format!(
        //     "<Below> MC_mean5: {}, AC_mean5: {}",
        //     short_morning_close, short_afternoon_close
        // ));

        info!("{}", markdown.buffer());

        markdown
    }

    pub fn for_afternoon_strategy(&self) -> Result<(), MyError> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut on_the_cloud = self.get_on_the_cloud();
        on_the_cloud.append(self.get_between_the_cloud());
        on_the_cloud.append(self.get_under_the_cloud());

        on_the_cloud.sort_by_abs_latest_move();
        let markdown = on_the_cloud.output_for_markdown_afternoon(&today);
        let path = crate::my_file_io::get_jquants_afternoon_path(&today).unwrap();
        markdown.write_to_file(&path);

        Ok(())
    }
}