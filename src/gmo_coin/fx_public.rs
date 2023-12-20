use crate::analysis::live::LongOrShort;
use crate::{
    analysis::live::{Ohlc, OhlcAnalyzer},
    my_error::MyError,
};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, Duration, Local, Utc};
use log::{debug, info};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration as StdDuration;
use std::{
    fmt::{Display, Formatter},
    thread,
};

#[derive(Deserialize, Serialize, Debug)]
struct KLinesResponse {
    status: i32,
    data: Vec<KLine>,
    #[serde(rename = "responsetime")]
    response_time: String,
}

impl KLinesResponse {
    fn to_ohlc_vec(&self) -> Vec<Ohlc> {
        let mut ohlc_vec = Vec::new();
        for kline in &self.data {
            ohlc_vec.push(kline.to_ohlc());
        }
        ohlc_vec
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct KLine {
    #[serde(rename = "openTime")]
    open_time: String,
    open: String,
    high: String,
    low: String,
    close: String,
}

impl KLine {
    //getters
    pub fn get_open_time(&self) -> String {
        let timestamp_secs: i64 = self.open_time.parse::<i64>().unwrap() / 1000;
        let datetime: DateTime<Utc> = DateTime::from_timestamp(timestamp_secs, 0).unwrap();
        let datetime_local: DateTime<Local> = datetime.with_timezone(&Local);

        datetime_local.format("%Y-%m-%d %H:%M:%S").to_string()
    }
    pub fn get_open(&self) -> f64 {
        self.open.parse().unwrap()
    }
    pub fn get_high(&self) -> f64 {
        self.high.parse().unwrap()
    }
    pub fn get_low(&self) -> f64 {
        self.low.parse().unwrap()
    }
    pub fn get_close(&self) -> f64 {
        self.close.parse().unwrap()
    }
    fn to_ohlc(&self) -> Ohlc {
        Ohlc::new(
            self.get_open_time(),
            self.get_open(),
            self.get_high(),
            self.get_low(),
            self.get_close(),
        )
    }
}

pub struct KLineQueryParams {
    symbol: Symbol,
    price_type: PriceType,
    interval: Interval,
    date: DateTime<Local>,
}
impl KLineQueryParams {
    pub fn new(
        symbol: Symbol,
        price_type: PriceType,
        interval: Interval,
        date: DateTime<Local>,
    ) -> Self {
        let date_6hours_ago = date - Duration::hours(6);
        Self {
            symbol,
            price_type,
            interval,
            date: date_6hours_ago,
        }
    }

    pub fn get_symbol(&self) -> &Symbol {
        &self.symbol
    }

    pub fn get_date_with_delta(&self, delta: i64) -> String {
        match self.interval {
            Interval::H1 | Interval::M30 => (self.date - Duration::days(delta))
                .format("%Y%m%d")
                .to_string(),
            Interval::D1 => (self.date - Duration::days(delta * 365))
                .format("%Y")
                .to_string(),
        }
    }

    fn date_with_delta_is_holiday(&self, delta: i64) -> bool {
        let date = self.date - Duration::days(delta);
        matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun)
    }

    pub async fn fetch_klines_with_delta(
        &self,
        client: &Client,
        delta: i64,
    ) -> Result<Vec<Ohlc>, MyError> {
        if self.date_with_delta_is_holiday(delta) && (self.interval != Interval::D1) {
            return Err(MyError::Holiday);
        }

        let url = "https://forex-api.coin.z.com/public/v1/klines";
        let date = self.get_date_with_delta(delta);

        let res = client
            .get(url)
            .query(&[
                ("symbol", self.symbol.to_string()),
                ("priceType", self.price_type.to_string()),
                ("interval", self.interval.to_string()),
                ("date", date),
            ])
            .send()
            .await
            .unwrap();

        match res.status() {
            StatusCode::OK => {
                info!("Status: {}", res.status());
                let json = res.json::<KLinesResponse>().await.unwrap();
                let ohlc_vec = json.to_ohlc_vec();
                debug!("{:?}", ohlc_vec);
                Ok(ohlc_vec)
            }
            _ => Err(MyError::Anyhow(anyhow!(
                "Status code: {}, {}",
                res.status(),
                res.text().await?
            ))),
        }
    }
}

#[derive(Clone)]
pub enum Symbol {
    UsdJpy,
    EurJpy,
    GbpJpy,
    AudJpy,
    EurUsd,
    GbpUsd,
    AudUsd,
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Symbol::UsdJpy => write!(f, "USD_JPY"),
            Symbol::EurJpy => write!(f, "EUR_JPY"),
            Symbol::GbpJpy => write!(f, "GBP_JPY"),
            Symbol::AudJpy => write!(f, "AUD_JPY"),
            Symbol::EurUsd => write!(f, "EUR_USD"),
            Symbol::GbpUsd => write!(f, "GBP_USD"),
            Symbol::AudUsd => write!(f, "AUD_USD"),
        }
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        match s {
            "USD_JPY" => Symbol::UsdJpy,
            "EUR_JPY" => Symbol::EurJpy,
            "GBP_JPY" => Symbol::GbpJpy,
            "AUD_JPY" => Symbol::AudJpy,
            "EUR_USD" => Symbol::EurUsd,
            "GBP_USD" => Symbol::GbpUsd,
            "AUD_USD" => Symbol::AudUsd,
            _ => panic!("Invalid symbol"),
        }
    }
}

impl Symbol {
    pub fn pips(&self) -> f64 {
        match self {
            Symbol::UsdJpy => 0.01,
            Symbol::EurJpy => 0.01,
            Symbol::GbpJpy => 0.01,
            Symbol::AudJpy => 0.01,
            Symbol::EurUsd => 0.0001,
            Symbol::GbpUsd => 0.0001,
            Symbol::AudUsd => 0.0001,
        }
    }
}

pub enum PriceType {
    Bid,
    #[allow(dead_code)]
    Ask,
}
impl Display for PriceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PriceType::Bid => write!(f, "BID"),
            PriceType::Ask => write!(f, "ASK"),
        }
    }
}

#[derive(PartialEq)]
pub enum Interval {
    M30,
    #[allow(dead_code)]
    H1,
    D1,
}
impl Display for Interval {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Interval::M30 => write!(f, "30min"),
            Interval::H1 => write!(f, "1hour"),
            Interval::D1 => write!(f, "1day"),
        }
    }
}

pub async fn fetch_ohlc(
    client: &Client,
    symbol: Symbol,
    interval: Interval,
) -> Result<Vec<Ohlc>, MyError> {
    let params = KLineQueryParams::new(symbol, PriceType::Bid, interval, Local::now());

    let mut ohlc_vec: Vec<Ohlc> = Vec::new();

    for delta in 0..10 {
        if ohlc_vec.len() >= 60 {
            break;
        }

        thread::sleep(StdDuration::from_secs(2));

        match params.fetch_klines_with_delta(client, delta).await {
            Ok(ohlc_vec_delta) => {
                let ohlc_vec_delta = ohlc_vec_delta.into_iter().rev().collect::<Vec<Ohlc>>();
                ohlc_vec.extend(ohlc_vec_delta)
            }
            Err(e) => match e {
                MyError::Holiday => {
                    info!("Holiday");
                    continue;
                }
                _ => return Err(e),
            },
        }
    }

    ohlc_vec.remove(0);
    ohlc_vec.reverse();
    Ok(ohlc_vec)
}

pub async fn fetch_gmo_coin_fx() {
    let client = Client::new();
    let symbols = vec![
        Symbol::UsdJpy,
        Symbol::EurJpy,
        Symbol::GbpJpy,
        Symbol::AudJpy,
        Symbol::EurUsd,
        Symbol::GbpUsd,
        Symbol::AudUsd,
    ];
    for symbol in symbols {
        info!("symbol: {}", symbol);
        let position: Option<LongOrShort> = match symbol {
            Symbol::UsdJpy => None,
            Symbol::EurJpy => None,
            Symbol::GbpJpy => None,
            Symbol::AudJpy => None,
            Symbol::EurUsd => None,
            Symbol::GbpUsd => None,
            Symbol::AudUsd => None,
        };

        let ohlc_vec_m30 = fetch_ohlc(&client, symbol.clone(), Interval::M30)
            .await
            .unwrap();
        let ohlc_vec_d1 = fetch_ohlc(&client, symbol.clone(), Interval::D1)
            .await
            .unwrap();

        let ohlc_analyzer =
            OhlcAnalyzer::from_gmo_coin_fx(symbol, ohlc_vec_m30, ohlc_vec_d1, position);

        info!(
            "M30 standardized diff: {}",
            ohlc_analyzer.get_shorter_ohlc_standardized_diff()
        );
        info!(
            "D1 trend: {:?}",
            ohlc_analyzer.get_longer_ohlc_standardized_diff_and_trend()
        );

        match ohlc_analyzer.get_position() {
            Some(_) => info!("stop loss order: {:?}", ohlc_analyzer.position_follow()),
            None => info!("{:?}", ohlc_analyzer.analyze_last20()),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_symbol() {
        let str = "USD_JPY";
        let symbol = Symbol::from(str);
        assert_eq!(symbol.to_string(), str);
    }

    #[test]
    fn test_weekday() {
        use chrono::TimeZone;
        let today = Local.with_ymd_and_hms(2023, 12, 6, 0, 0, 0).unwrap();
        let weekday = today.weekday();
        assert_eq!(weekday, chrono::Weekday::Wed)
    }
}
