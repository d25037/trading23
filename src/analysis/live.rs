use crate::gmo_coin::fx_public::Symbol;
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
};

use cli_candlestick_chart::{Candle, Chart};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Ohlc {
    date: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

impl Ohlc {
    pub fn new(date: String, open: f64, high: f64, low: f64, close: f64) -> Self {
        Self {
            date,
            open,
            high,
            low,
            close,
        }
    }

    //getters
    pub fn get_date(&self) -> &str {
        self.date.as_str()
    }
    pub fn get_open(&self) -> f64 {
        self.open
    }
    pub fn get_high(&self) -> f64 {
        self.high
    }
    pub fn get_low(&self) -> f64 {
        self.low
    }
    pub fn get_close(&self) -> f64 {
        self.close
    }

    //setters
    pub fn set_open(&mut self, open: f64) {
        self.open = open;
    }
    pub fn set_high(&mut self, high: f64) {
        self.high = high;
    }
    pub fn set_low(&mut self, low: f64) {
        self.low = low;
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub enum BullBear {
    Bull,
    Bear,
    NoTrend,
}
impl Display for BullBear {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BullBear::Bull => write!(f, "Bull"),
            BullBear::Bear => write!(f, "Bear"),
            BullBear::NoTrend => write!(f, "NoTrend"),
        }
    }
}

pub enum OhlcSource {
    Jquants,
    GmoCoinFx(Symbol),
}

pub struct OhlcAnalyzer {
    source: OhlcSource,
    shorter_ohlc: Vec<Ohlc>,
    longer_ohlc: Vec<Ohlc>,
    position: Option<LongOrShort>,
}

impl OhlcAnalyzer {
    pub fn from_jquants(raw_ohlc: Vec<Ohlc>) -> Self {
        let shorter_ohlc = raw_ohlc.clone().into_iter().rev().take(60).rev().collect();
        let longer_ohlc = to_monthly_ohlc(raw_ohlc.clone());
        Self {
            source: OhlcSource::Jquants,
            shorter_ohlc,
            longer_ohlc,
            position: None,
        }
    }

    pub fn from_gmo_coin_fx(
        symbol: Symbol,
        raw_ohlc_shorter: Vec<Ohlc>,
        raw_ohlc_longer: Vec<Ohlc>,
        position: Option<LongOrShort>,
    ) -> Self {
        let shorter_ohlc = raw_ohlc_shorter.into_iter().rev().take(60).rev().collect();
        let longer_ohlc = raw_ohlc_longer.into_iter().rev().take(60).rev().collect();
        Self {
            source: OhlcSource::GmoCoinFx(symbol),
            shorter_ohlc,
            longer_ohlc,
            position,
        }
    }

    pub fn get_position(&self) -> &Option<LongOrShort> {
        &self.position
    }

    pub fn analyze_last20(&self, jquants_unit: Option<f64>) -> Last20Analysis {
        let last_20: Vec<Ohlc> = self
            .shorter_ohlc
            .clone()
            .into_iter()
            .rev()
            .take(20)
            .rev()
            .collect();

        let (prev_19, last) = last_20.split_at(19);
        let last_close = last[0].close;
        let high = prev_19
            .iter()
            .map(|ohlc| ohlc.high)
            .fold(f64::NAN, f64::max);
        let low = prev_19.iter().map(|ohlc| ohlc.low).fold(f64::NAN, f64::min);
        // info!("last_close: {}", last_close);

        match (last_close > high, last_close < low) {
            (true, false) => {
                let high = last_20
                    .iter()
                    .map(|ohlc| ohlc.high)
                    .fold(f64::NAN, f64::max);
                let low = last_20.iter().map(|ohlc| ohlc.low).fold(f64::NAN, f64::min);
                let stop_loss_order_naked = high - (high - low) * 0.38;
                let stop_loss_order = match &self.source {
                    OhlcSource::Jquants => stop_loss_order_naked,
                    OhlcSource::GmoCoinFx(symbol) => {
                        let coefficient = 1_f64 / symbol.pips();
                        (stop_loss_order_naked * coefficient).round() / coefficient
                    }
                };

                let units = match &self.source {
                    OhlcSource::Jquants => {
                        (jquants_unit.unwrap() / (last[0].close - stop_loss_order)) as i32
                    }
                    OhlcSource::GmoCoinFx(symbol) => {
                        let coefficient = match symbol {
                            Symbol::EurUsd | Symbol::GbpUsd | Symbol::AudUsd => 0.01,
                            _ => 1.0,
                        };
                        (3000.0 / (last[0].close - stop_loss_order) * coefficient).round() as i32
                    }
                };
                let is_too_strong_to_entry =
                    ((last[0].high - last[0].low) / (last[0].high - low)) > 0.75;
                let analyzed_at = last[0].date.to_string();

                Last20Analysis {
                    break_or_not: true,
                    long_or_short: Some(LongOrShort::Long),
                    stop_loss_order: Some(stop_loss_order),
                    units: Some(units),
                    is_too_strong_to_entry: Some(is_too_strong_to_entry),
                    analyzed_at,
                }
            }
            (false, true) => {
                let high = last_20
                    .iter()
                    .map(|ohlc| ohlc.high)
                    .fold(f64::NAN, f64::max);
                let low = last_20.iter().map(|ohlc| ohlc.low).fold(f64::NAN, f64::min);
                let stop_loss_order_naked = low + (high - low) * 0.38;

                let stop_loss_order = match &self.source {
                    OhlcSource::Jquants => stop_loss_order_naked,
                    OhlcSource::GmoCoinFx(symbol) => {
                        let coefficient = 1_f64 / symbol.pips();
                        (stop_loss_order_naked * coefficient).round() / coefficient
                    }
                };
                let units = match &self.source {
                    OhlcSource::Jquants => {
                        (jquants_unit.unwrap() / (stop_loss_order - last[0].close)) as i32
                    }
                    OhlcSource::GmoCoinFx(symbol) => {
                        let coefficient = match symbol {
                            Symbol::EurUsd | Symbol::GbpUsd | Symbol::AudUsd => 0.01,
                            _ => 1.0,
                        };
                        (3000.0 / (stop_loss_order - last[0].close) * coefficient).round() as i32
                    }
                };
                let is_too_strong_to_entry =
                    ((last[0].high - last[0].low) / (high - last[0].low)) > 0.75;
                let analyzed_at = last[0].date.to_string();

                Last20Analysis {
                    break_or_not: true,
                    long_or_short: Some(LongOrShort::Short),
                    stop_loss_order: Some(stop_loss_order),
                    units: Some(units),
                    is_too_strong_to_entry: Some(is_too_strong_to_entry),
                    analyzed_at,
                }
            }
            (false, false) => Last20Analysis {
                break_or_not: false,
                long_or_short: None,
                stop_loss_order: None,
                units: None,
                is_too_strong_to_entry: None,
                analyzed_at: last[0].date.to_string(),
            },
            (true, true) => {
                panic!("last_close is both higher than high and lower than low");
            }
        }
    }

    pub fn get_shorter_ohlc_standardized_diff(&self) -> f64 {
        let highest_high = self
            .shorter_ohlc
            .iter()
            .map(|ohlc| ohlc.high)
            .fold(f64::NAN, f64::max);
        let lowest_low = self
            .shorter_ohlc
            .iter()
            .map(|ohlc| ohlc.low)
            .fold(f64::NAN, f64::min);

        let diff_sum: f64 = self
            .shorter_ohlc
            .iter()
            .map(|ohlc| ohlc.high - ohlc.low)
            .sum();
        let average_diff = diff_sum / self.shorter_ohlc.len() as f64;

        (average_diff / (highest_high - lowest_low) * 1000.0).trunc() / 1000.0
    }

    pub fn get_longer_ohlc_standardized_diff_and_trend(&self) -> (f64, BullBear) {
        let highest_high = self
            .longer_ohlc
            .iter()
            .map(|ohlc| ohlc.high)
            .fold(f64::NAN, f64::max);
        let lowest_low = self
            .longer_ohlc
            .iter()
            .map(|ohlc| ohlc.low)
            .fold(f64::NAN, f64::min);

        let diff_sum: f64 = self
            .longer_ohlc
            .iter()
            .map(|ohlc| ohlc.high - ohlc.low)
            .sum();
        let average_diff = diff_sum / self.longer_ohlc.len() as f64;
        let standardized_diff =
            (average_diff / (highest_high - lowest_low) * 1000.0).trunc() / 1000.0;

        let last_close = self.longer_ohlc.last().unwrap().close;
        let last_close_position = (last_close - lowest_low) / (highest_high - lowest_low);

        let bull_bear = match (standardized_diff, last_close_position) {
            (x, _) if x > 0.14 => BullBear::NoTrend,
            (_, y) if (0.0..=0.2).contains(&y) => BullBear::Bear,
            (_, y) if (0.8..=1.0).contains(&y) => BullBear::Bull,
            (_, _) => BullBear::NoTrend,
        };

        (standardized_diff, bull_bear)
    }

    pub fn get_shorter_chart(&self) {
        let mut candles: Vec<Candle> = Vec::new();
        for ohlc in self.shorter_ohlc.clone() {
            let candle = Candle::new(ohlc.open, ohlc.high, ohlc.low, ohlc.close);
            candles.push(candle);
        }

        // Create and display the chart
        let mut chart = Chart::new(&candles);

        // Set the chart title
        chart.set_name(String::from("TEST"));

        // Set customs colors
        chart.set_bear_color(1, 205, 254);
        chart.set_bull_color(255, 107, 153);
        // chart.set_vol_bull_color(1, 205, 254);
        // chart.set_vol_bear_color(255, 107, 153);

        // chart.set_volume_pane_height(6);
        // chart.set_volume_pane_enabled(false);

        chart.draw();
    }

    pub fn position_follow(&self) -> f64 {
        let last_20: Vec<Ohlc> = self
            .shorter_ohlc
            .clone()
            .into_iter()
            .rev()
            .take(20)
            .rev()
            .collect();

        let high = last_20
            .iter()
            .map(|ohlc| ohlc.high)
            .fold(f64::NAN, f64::max);
        let low = last_20.iter().map(|ohlc| ohlc.low).fold(f64::NAN, f64::min);

        match self.position {
            Some(LongOrShort::Long) => {
                let stop_loss_order_naked = high - (high - low) * 0.38;
                match &self.source {
                    OhlcSource::Jquants => stop_loss_order_naked,
                    OhlcSource::GmoCoinFx(symbol) => {
                        let coefficient = 1_f64 / symbol.pips();
                        (stop_loss_order_naked * coefficient).round() / coefficient
                    }
                }
            }
            Some(LongOrShort::Short) => {
                let stop_loss_order_naked = low + (high - low) * 0.38;
                match &self.source {
                    OhlcSource::Jquants => stop_loss_order_naked,
                    OhlcSource::GmoCoinFx(symbol) => {
                        let coefficient = 1_f64 / symbol.pips();
                        (stop_loss_order_naked * coefficient).round() / coefficient
                    }
                }
            }
            None => panic!("No position"),
        }
    }
}

fn to_monthly_ohlc(ohlc_vec: Vec<Ohlc>) -> Vec<Ohlc> {
    let mut monthly_ohlc_map: HashMap<String, Vec<Ohlc>> = HashMap::new();

    for ohlc in ohlc_vec {
        let month = &ohlc.date[0..7]; // extract yyyy-mm
        monthly_ohlc_map
            .entry(month.to_string())
            .or_default()
            .push(ohlc);
    }

    let mut monthly_ohlc_vec: Vec<Ohlc> = Vec::new();

    for (month, ohlcs) in monthly_ohlc_map {
        let open = ohlcs.first().unwrap().open;
        let close = ohlcs.last().unwrap().close;
        let high = ohlcs.iter().map(|ohlc| ohlc.high).fold(f64::NAN, f64::max);
        let low = ohlcs.iter().map(|ohlc| ohlc.low).fold(f64::NAN, f64::min);
        monthly_ohlc_vec.push(Ohlc::new(month, open, high, low, close));
    }

    monthly_ohlc_vec.sort_by(|a, b| a.date.cmp(&b.date));

    monthly_ohlc_vec
}

#[derive(Debug)]
pub enum LongOrShort {
    Long,
    Short,
}
impl Display for LongOrShort {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LongOrShort::Long => write!(f, "Long"),
            LongOrShort::Short => write!(f, "Short"),
        }
    }
}

#[derive(Debug)]
pub struct Last20Analysis {
    break_or_not: bool,
    long_or_short: Option<LongOrShort>,
    stop_loss_order: Option<f64>,
    units: Option<i32>,
    #[allow(dead_code)]
    is_too_strong_to_entry: Option<bool>,
    analyzed_at: String,
}

impl Last20Analysis {
    //getter
    pub fn get_break_or_not(&self) -> bool {
        self.break_or_not
    }
    pub fn get_long_or_short(&self) -> &str {
        match self.long_or_short {
            Some(LongOrShort::Long) => "Long",
            Some(LongOrShort::Short) => "Short",
            None => "None",
        }
    }
    pub fn get_stop_loss_order(&self) -> f64 {
        self.stop_loss_order.unwrap()
    }
    pub fn get_units(&self) -> i32 {
        self.units.unwrap()
    }
    pub fn get_analyzed_at(&self) -> &str {
        self.analyzed_at.as_str()
    }
}

#[allow(dead_code)]
pub fn get_candlestick_chart(ohlc_vec: Vec<Ohlc>) {
    let mut candles: Vec<Candle> = Vec::new();
    for ohlc in ohlc_vec {
        let candle = Candle::new(ohlc.open, ohlc.high, ohlc.low, ohlc.close);
        candles.push(candle);
    }

    // Create and display the chart
    let mut chart = Chart::new(&candles);

    // Set the chart title
    chart.set_name(String::from("TEST"));

    // Set customs colors
    chart.set_bear_color(1, 205, 254);
    chart.set_bull_color(255, 107, 153);
    // chart.set_vol_bull_color(1, 205, 254);
    // chart.set_vol_bear_color(255, 107, 153);

    // chart.set_volume_pane_height(6);
    // chart.set_volume_pane_enabled(false);

    chart.draw();
}

// fn parse_time2(t: &str) -> DateTime<Local> {
//     let date = NaiveDate::parse_from_str(t, "%Y-%m-%d").expect("Failed to parse date");
//     DateTime::from_naive_utc_and_offset(date.and_hms_opt(0, 0, 0), Local)
// }

#[cfg(test)]
mod test {
    #[test]
    fn test_float() {
        let a = 142.3466;
        let pips = 1_f64 / 0.01;
        let b = (a * pips).round() / pips;
        assert_eq!(b, 142.35);
        let c = 1.252244;
        let pips2 = 1_f64 / 0.0001;
        let d = (c * pips2).round() / pips2;
        assert_eq!(d, 1.2522);
    }
}
