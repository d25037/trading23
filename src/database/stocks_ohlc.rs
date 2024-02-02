use std::{env, path::Path};

use crate::{analysis::live::OhlcPremium, my_error::MyError};
use chrono::Local;
use log::debug;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct StocksOhlc {
    id: i32,
    created_at: String,
    #[serde(flatten)]
    inner: OhlcPremium,
}
impl StocksOhlc {
    pub fn get_inner(self) -> OhlcPremium {
        self.inner
    }
}

pub fn open_db() -> Result<Connection, MyError> {
    let gdrive_path = env::var("GDRIVE_PATH")?;
    let sqlite_path = Path::new(&gdrive_path)
        .join("trading23")
        .join("trading23.sqlite");
    let conn = Connection::open(sqlite_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS stocks_ohlc (
            id INTEGER PRIMARY KEY,
            code TEXT NOT NULL,
            date TEXT NOT NULL,
            open REAL NOT NULL,
            high REAL NOT NULL,
            low REAL NOT NULL,
            close REAL NOT NULL,
            morning_close REAL NOT NULL,
            afternoon_open REAL NOT NULL,
            created_at TEXT NOT NULL)",
        (),
    )?;
    Ok(conn)
}

pub fn select_by_code(conn: &Connection, code: &str) -> Result<Vec<StocksOhlc>, MyError> {
    let mut stmt = conn.prepare("SELECT * FROM stocks_ohlc WHERE code = ?1")?;
    let mut rows = stmt.query([&code])?;
    let mut ohlcs = Vec::new();
    while let Some(row) = rows.next()? {
        let inner = OhlcPremium::new(
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(7)?,
            row.get(8)?,
        );
        ohlcs.push(StocksOhlc {
            id: row.get(0)?,
            created_at: row.get(9)?,
            inner,
        });
    }
    // debug!("{:?}", ohlcs);
    Ok(ohlcs)
}

pub fn select_by_date(conn: &Connection, date: &str) -> Result<Vec<StocksOhlc>, MyError> {
    let mut stmt = conn.prepare("SELECT * FROM stocks_ohlc WHERE date = ?1")?;
    let mut rows = stmt.query([&date])?;
    let mut ohlcs = Vec::new();
    while let Some(row) = rows.next()? {
        let inner = OhlcPremium::new(
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(7)?,
            row.get(8)?,
        );
        ohlcs.push(StocksOhlc {
            id: row.get(0)?,
            created_at: row.get(9)?,
            inner,
        });
    }
    Ok(ohlcs)
}

// pub fn select_by_code_and_date(
//     conn: &Connection,
//     code: i32,
//     date: &str,
// ) -> Result<Vec<StocksOhlc>, MyError> {
//     let mut stmt = conn.prepare("SELECT * FROM stocks_ohlc WHERE code = ?1 AND date = ?2")?;
//     let mut rows = stmt.query([&code.to_string(), date])?;
//     let mut ohlcs = Vec::new();
//     while let Some(row) = rows.next()? {
//         let inner = OhlcPremium::new(
//             row.get(1)?,
//             row.get(2)?,
//             row.get(3)?,
//             row.get(4)?,
//             row.get(5)?,
//             row.get(6)?,
//             row.get(7)?,
//             row.get(8)?,
//         );
//         ohlcs.push(StocksOhlc {
//             id: row.get(0)?,
//             created_at: row.get(9)?,
//             inner,
//         });
//     }
//     Ok(ohlcs)
// }

pub fn insert(conn: &Connection, ohlc: &OhlcPremium) -> Result<(), MyError> {
    let created_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let code = ohlc.get_code();
    conn.execute(
        "INSERT INTO stocks_ohlc (code, date, open, high, low, close, morning_close, afternoon_open, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        [
            code.to_string(),
            ohlc.get_date().to_string(),
            ohlc.get_open().to_string(),
            ohlc.get_high().to_string(),
            ohlc.get_low().to_string(),
            ohlc.get_close().to_string(),
            ohlc.get_morning_close().to_string(),
            ohlc.get_afternoon_open().to_string(),
            created_at,
        ],
    )?;
    Ok(())
}

// pub fn delete_by_code(conn: &Connection, code: i32) -> Result<(), MyError> {
//     conn.execute("DELETE FROM stocks_ohlc WHERE code = ?1", [&code])?;
//     Ok(())
// }
