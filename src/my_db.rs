use crate::analysis::live::{LongOrShort, OhlcAnalyzer};
use anyhow::Result;
use chrono::{Local, TimeZone};
use log::info;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{
    env,
    fmt::{Display, Formatter, Write},
    path::Path,
};

pub fn open_db() -> Result<Connection> {
    let gdrive_path = env::var("GDRIVE_PATH").unwrap();
    let sqlite_path = Path::new(&gdrive_path)
        .join("trading23")
        .join("trading23.sqlite");
    let conn = Connection::open(sqlite_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS stocks (
            id INTEGER PRIMARY KEY,
            code INTEGER NOT NULL,
            name TEXT NOT NULL,
            break_or_not TEXT NOT NULL,
            long_or_short TEXT,
            stop_loss_order REAL,
            units INTEGER,
            daily_diff REAL,
            monthly_diff REAL,
            monthly_trend TEXT,
            analyzed_at TEXT NOT NULL,
            created_at TEXT NOT NULL)",
        (),
    )?;
    Ok(conn)
}

pub struct NewStock {
    code: i32,
    name: String,
    ohlc_analyzer: OhlcAnalyzer,
}

impl NewStock {
    pub fn new(code: i32, name: &str, ohlc_analyzer: OhlcAnalyzer) -> Self {
        let name = name.to_string();

        Self {
            code,
            name,
            ohlc_analyzer,
        }
    }

    pub fn insert_record(self, conn: &Connection) {
        let last20_analysis = self.ohlc_analyzer.analyze_last20();
        // if !last20_analysis.get_break_or_not() {
        //     return;
        // }

        let created_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        match last20_analysis.get_break_or_not() {
            true => {
                let daily_ohlc_diff = self.ohlc_analyzer.get_shorter_ohlc_standardized_diff();

                let monthly_ohlc_diff = self
                    .ohlc_analyzer
                    .get_longer_ohlc_standardized_diff_and_trend();

                conn.execute(
                    "INSERT INTO stocks (code, name, break_or_not, long_or_short, stop_loss_order, units, daily_diff, monthly_diff, monthly_trend, analyzed_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    (&self.code, &self.name, "true", &last20_analysis.get_long_or_short(), &last20_analysis.get_stop_loss_order(), &last20_analysis.get_units(), &daily_ohlc_diff, &monthly_ohlc_diff.0, &monthly_ohlc_diff.1.to_string(), last20_analysis.get_analyzed_at(), &created_at)
                ).unwrap();

                info!("Insert record: {} {} {}", self.code, self.name, "true");
            }
            false => {
                conn.execute(
                    "INSERT INTO stocks (code, name, break_or_not, analyzed_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    (&self.code, &self.name, "false", last20_analysis.get_analyzed_at(), &created_at),
                ).unwrap();

                info!("Insert record: {} {} {}", self.code, self.name, "false")
            }
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Stock {
    id: i32,
    code: i32,
    name: String,
    break_or_not: String,
    long_or_short: Option<String>,
    stop_loss_order: Option<f64>,
    units: Option<i32>,
    daily_diff: Option<f64>,
    monthly_diff: Option<f64>,
    monthly_trend: Option<String>,
    analyzed_at: String,
    created_at: String,
}

impl Stock {
    fn get_long_or_short(&self) -> String {
        match &self.long_or_short {
            Some(long_or_short) => long_or_short.to_string(),
            None => "None".to_string(),
        }
    }
    fn to_line_notify(&self, mut buffer: String) -> String {
        // match self.long_or_short.as_ref().unwrap().as_str() {
        //     "Long" => {
        //         if self.daily_diff.unwrap() > 0.09 {
        //             // if self.daily_diff.unwrap() < 0.085 || self.daily_diff.unwrap() > 0.12 {
        //             return buffer;
        //         }
        //     }
        //     "Short" => {
        //         if self.daily_diff.unwrap() > 0.09 {
        //             return buffer;
        //         }
        //     }
        //     _ => return buffer,
        // }

        let required_amount = self.stop_loss_order.unwrap() * self.units.unwrap() as f64;
        let required_amount_rounded: i32 = (required_amount * 10.0).round() as i32 / 10;

        writeln!(buffer).unwrap();
        writeln!(
            buffer,
            "{} {} {} {} {} {} {} {} {} {}円",
            self.code,
            self.name,
            self.long_or_short.as_ref().unwrap(),
            self.stop_loss_order.as_ref().unwrap(),
            self.units.unwrap(),
            self.daily_diff.unwrap(),
            self.monthly_diff.unwrap(),
            self.monthly_trend.as_ref().unwrap(),
            self.analyzed_at,
            // self.created_at
            required_amount_rounded
        )
        .unwrap();

        buffer
    }
}

impl Display for Stock {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{} {} {} {} {} {} {} {} {} {}",
            self.code,
            self.name,
            self.long_or_short.as_ref().unwrap(),
            self.stop_loss_order.as_ref().unwrap(),
            self.units.unwrap(),
            self.daily_diff.unwrap(),
            self.monthly_diff.unwrap(),
            self.monthly_trend.as_ref().unwrap(),
            self.analyzed_at,
            self.created_at
        )
    }
}

#[derive(Debug)]
pub struct StockList {
    stocks: Vec<Stock>,
}

impl StockList {
    fn count_long_stocks(&self) -> usize {
        self.stocks
            .iter()
            .filter(|stock| stock.long_or_short.as_ref().unwrap() == "Long")
            .count()
    }

    fn count_short_stocks(&self) -> usize {
        self.stocks
            .iter()
            .filter(|stock| stock.long_or_short.as_ref().unwrap() == "Short")
            .count()
    }

    fn determine_entry_long_or_short(&self) -> EntryLongOrShort {
        let long = self.count_long_stocks();
        let short = self.count_short_stocks();

        let diff = long as i32 - short as i32;
        match diff {
            diff if diff > 0 => EntryLongOrShort {
                long_or_short: Some(LongOrShort::Long),
                long_count: long,
                short_count: short,
                diff,
            },
            diff if diff < 0 => EntryLongOrShort {
                long_or_short: Some(LongOrShort::Short),
                long_count: long,
                short_count: short,
                diff,
            },
            _ => EntryLongOrShort {
                long_or_short: None,
                long_count: long,
                short_count: short,
                diff,
            },
        }
    }

    fn write_stocks_list(&self, mut buffer: String) -> String {
        let entry_long_of_short = self.determine_entry_long_or_short();

        match entry_long_of_short.get_long_or_short().as_str() {
            "None" => (),
            x => {
                for stock in self.stocks.iter() {
                    if stock.get_long_or_short() == x {
                        buffer = stock.to_line_notify(buffer);
                    }
                }
            }
        }

        writeln!(
            buffer,
            "long: {}, short: {}, diff: {}",
            entry_long_of_short.get_long_count(),
            entry_long_of_short.get_short_count(),
            entry_long_of_short.get_diff()
        )
        .unwrap();
        buffer
    }
}

struct EntryLongOrShort {
    long_or_short: Option<LongOrShort>,
    long_count: usize,
    short_count: usize,
    diff: i32,
}
impl EntryLongOrShort {
    //getters
    fn get_long_or_short(&self) -> String {
        match &self.long_or_short {
            Some(long_or_short) => long_or_short.to_string(),
            None => "None".to_string(),
        }
    }
    fn get_long_count(&self) -> usize {
        self.long_count
    }
    fn get_short_count(&self) -> usize {
        self.short_count
    }
    fn get_diff(&self) -> i32 {
        self.diff
    }
}

pub struct SelectDate {
    year: i32,
    month: u32,
    day: u32,
}

impl SelectDate {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }
    //getters
    pub fn get_year(&self) -> i32 {
        self.year
    }
    pub fn get_month(&self) -> u32 {
        self.month
    }
    pub fn get_day(&self) -> u32 {
        self.day
    }
}

pub fn select_stocks(conn: &Connection, date_str: Option<SelectDate>) -> String {
    let date_str = match date_str {
        Some(date_str) => {
            let dt = Local
                .with_ymd_and_hms(
                    date_str.get_year(),
                    date_str.get_month(),
                    date_str.get_day(),
                    0,
                    0,
                    0,
                )
                .unwrap();
            dt.format("%Y-%m-%d").to_string()
        }
        None => Local::now().format("%Y-%m-%d").to_string(),
    };
    let mut stmt = conn
        .prepare(
            "SELECT * FROM stocks WHERE analyzed_at=?1 AND break_or_not='true' ORDER BY daily_diff",
        )
        .unwrap();
    let stock_iter = stmt
        .query_map([date_str], |row| {
            Ok(Stock {
                id: row.get(0)?,
                code: row.get(1)?,
                name: row.get(2)?,
                break_or_not: row.get(3)?,
                long_or_short: row.get(4)?,
                stop_loss_order: row.get(5)?,
                units: row.get(6)?,
                daily_diff: row.get(7)?,
                monthly_diff: row.get(8)?,
                monthly_trend: row.get(9)?,
                analyzed_at: row.get(10)?,
                created_at: row.get(11)?,
            })
        })
        .unwrap();

    let stock_list: Result<Vec<Stock>, rusqlite::Error> = stock_iter.collect();
    let stock_list = StockList {
        stocks: stock_list.unwrap(),
    };
    let mut buffer = String::new();

    buffer = stock_list.write_stocks_list(buffer);
    info!("{}", buffer);

    buffer
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_boolean() {
        let a = true;

        assert_eq!("true", a.to_string())
    }

    #[test]
    fn test_open_db() {
        dotenvy::from_filename(".env_local").unwrap();
        open_db().unwrap();
    }
}
