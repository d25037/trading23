use crate::analysis::live::OhlcAnalyzer;
use anyhow::Result;
use chrono::{Local, TimeZone};
use log::info;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{env, fmt::Write, path::Path};

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

    pub fn insert_record(self, conn: &Connection, unit: f64) {
        let last20_analysis = self.ohlc_analyzer.analyze_last20(Some(unit));
        if !last20_analysis.get_break_or_not() {
            return;
        }

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
    long_or_short: String,
    stop_loss_order: Option<f64>,
    units: Option<i32>,
    daily_diff: Option<f64>,
    monthly_diff: Option<f64>,
    monthly_trend: Option<String>,
    analyzed_at: String,
    created_at: String,
}

impl Stock {
    fn get_long_or_short(&self) -> &str {
        self.long_or_short.as_ref()
    }
    fn output_stock_data(&self, mut buffer: String) -> String {
        let required_amount = self.stop_loss_order.unwrap() * self.units.unwrap() as f64;
        let required_amount_rounded: i32 = (required_amount * 10.0).round() as i32 / 10;

        let stop_loss_order_rounded: i32 = self.stop_loss_order.unwrap().round() as i32;
        let stop_loss_order_str = stop_loss_order_rounded.to_string() + "円";

        writeln!(buffer).unwrap();
        writeln!(
            buffer,
            "{} {} {} {} {} {} {} {}円",
            self.code,
            self.name,
            stop_loss_order_str,
            self.units.unwrap(),
            self.daily_diff.unwrap(),
            self.monthly_diff.unwrap(),
            self.monthly_trend.as_ref().unwrap(),
            required_amount_rounded
        )
        .unwrap();

        buffer
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
            .filter(|stock| stock.long_or_short == "Long")
            .count()
    }

    fn count_short_stocks(&self) -> usize {
        self.stocks
            .iter()
            .filter(|stock| stock.long_or_short == "Short")
            .count()
    }

    fn determine_entry_long_or_short(&self, date: &str) -> EntryLongOrShort {
        let long = self.count_long_stocks();
        let short = self.count_short_stocks();

        EntryLongOrShort::new(date, long, short)
    }

    fn output_stocks_list(&self, date: &str) -> Output {
        let entry_long_or_short = self.determine_entry_long_or_short(date);

        let mut long_stocks = String::new();
        writeln!(long_stocks, "Long").unwrap();
        let mut short_stocks = String::new();
        writeln!(short_stocks, "Short").unwrap();

        for stock in self.stocks.iter() {
            match stock.get_long_or_short() {
                "Long" => long_stocks = stock.output_stock_data(long_stocks),
                "Short" => short_stocks = stock.output_stock_data(short_stocks),
                _ => (),
            }
        }

        Output {
            entry_long_or_short,
            long_stocks,
            short_stocks,
        }
    }
}

struct EntryLongOrShort {
    date: String,
    long_count: usize,
    short_count: usize,
}
impl EntryLongOrShort {
    fn new(date: &str, long_count: usize, short_count: usize) -> Self {
        Self {
            date: date.to_string(),
            long_count,
            short_count,
        }
    }
    fn output_entry_long_or_short(&self) -> String {
        let mut buffer = String::new();
        writeln!(buffer).unwrap();
        writeln!(buffer, "Date: {}", self.date).unwrap();
        write!(
            buffer,
            "Long: {}, Short: {}",
            self.long_count, self.short_count
        )
        .unwrap();
        buffer
    }
}

pub struct Output {
    entry_long_or_short: EntryLongOrShort,
    long_stocks: String,
    short_stocks: String,
}
impl Output {
    // getters
    pub fn get_entry_long_or_short(&self) -> String {
        self.entry_long_or_short.output_entry_long_or_short()
    }
    pub fn get_long_stocks(&self) -> &str {
        &self.long_stocks
    }
    pub fn get_short_stocks(&self) -> &str {
        &self.short_stocks
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

pub fn select_stocks(conn: &Connection, date_str: Option<SelectDate>) -> Output {
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
            "SELECT * FROM stocks WHERE analyzed_at=?1 AND break_or_not='true' ORDER BY long_or_short, daily_diff",
        )
        .unwrap();
    let stock_iter = stmt
        .query_map([date_str.clone()], |row| {
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
    let output = stock_list.output_stocks_list(&date_str);
    info!("{}", output.get_entry_long_or_short());
    info!("{}", output.get_long_stocks());
    info!("{}", output.get_short_stocks());

    output
}

// pub fn select_stocks_manually(conn: &Connection, sql: &str) -> Output {
//     let mut stmt = conn.prepare(sql).unwrap();
//     let stock_iter = stmt
//         .query_map([], |row| {
//             Ok(Stock {
//                 id: row.get(0)?,
//                 code: row.get(1)?,
//                 name: row.get(2)?,
//                 break_or_not: row.get(3)?,
//                 long_or_short: row.get(4)?,
//                 stop_loss_order: row.get(5)?,
//                 units: row.get(6)?,
//                 daily_diff: row.get(7)?,
//                 monthly_diff: row.get(8)?,
//                 monthly_trend: row.get(9)?,
//                 analyzed_at: row.get(10)?,
//                 created_at: row.get(11)?,
//             })
//         })
//         .unwrap();

//     let stock_list: Result<Vec<Stock>, rusqlite::Error> = stock_iter.collect();
//     let stock_list = StockList {
//         stocks: stock_list.unwrap(),
//     };
//     let output = stock_list.output_stocks_list(&date);
//     info!("{}", output.get_entry_long_or_short());
//     info!("{}", output.get_long_stocks());
//     info!("{}", output.get_short_stocks());

//     output
// }

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
