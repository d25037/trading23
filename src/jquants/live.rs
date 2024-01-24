use crate::analysis::live::{Ohlc, OhlcPremium};
use crate::config::GdriveJson;
use crate::my_error::MyError;
use crate::my_file_io::{get_fetched_ohlc_file_path, AssetType};
use anyhow::{anyhow, Result};
use log::error;
use log::{debug, info};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs::File;
use std::time::Duration;
use std::{env, thread};

#[derive(Deserialize, Serialize, Debug)]
struct RefreshToken {
    #[serde(rename = "refreshToken")]
    refresh_token: String,
}
impl RefreshToken {
    async fn fetch_and_save_to_file(client: &Client) -> Result<(), MyError> {
        info!("Fetch Refresh Token");
        let mut gdrive_json = GdriveJson::new()?;

        let mut map = HashMap::new();
        map.insert("mailaddress", gdrive_json.jquants_mail());
        map.insert("password", gdrive_json.jquants_pw());

        let res = client
            .post("https://api.jquants.com/v1/token/auth_user")
            .json(&map)
            .send()
            .await?;

        let (status, text) = {
            let status = res.status();
            let text = res.text().await?;
            (status, text)
        };

        match status {
            StatusCode::OK => {
                info!("Status code: {}", status);
                debug!("{}", text);
                let refresh_token: RefreshToken = serde_json::from_str(&text)?;
                gdrive_json.set_jquants_refresh_token(refresh_token.refresh_token);
                gdrive_json.write_to_file()?;
                Ok(())
            }
            _ => Err(MyError::Anyhow(anyhow!(
                "Status code: {}, {}",
                status,
                text
            ))),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct IdToken {
    #[serde(rename = "idToken")]
    id_token: String,
}

impl IdToken {
    async fn fetch_and_save_to_file(client: &Client) -> Result<(), MyError> {
        info!("Fetch ID Token");
        let mut gdrive_json = GdriveJson::new()?;
        let url = "https://api.jquants.com/v1/token/auth_refresh";
        let query = json!({"refreshtoken": gdrive_json.jquants_refresh_token()});

        let res = client.post(url).query(&query).send().await?;

        let (status, text) = {
            let status = res.status();
            let text = res.text().await?;
            (status, text)
        };

        match status {
            StatusCode::OK => {
                info!("Status code: {}", status);
                debug!("{}", text);
                let id_token: IdToken = serde_json::from_str(&text)?;
                gdrive_json.set_jquants_id_token(id_token.id_token);
                gdrive_json.write_to_file()?;
                Ok(())
            }
            StatusCode::BAD_REQUEST => {
                info!("Status code 401 {}", text);
                Err(MyError::RefreshTokenExpired)
            }
            _ => Err(MyError::Anyhow(anyhow!(
                "Status code: {}, {}",
                status,
                text
            ))),
        }
    }
}

// async fn fetch_refresh_token(client: &Client) -> Result<(), MyError> {
//     info!("Fetch Refresh Token");
//     let mut config = crate::config::GdriveJson::new()?;

//     let mut map = HashMap::new();
//     map.insert("mailaddress", config.jquants_mail());
//     map.insert("password", config.jquants_pw());

//     let res = client
//         .post("https://api.jquants.com/v1/token/auth_user")
//         .json(&map)
//         .send()
//         .await?;

//     match res.status() {
//         StatusCode::OK => {
//             info!("Status code: {}", res.status());
//             let body = res.text().await?;
//             debug!("{}", body);
//             let refresh_token: RefreshToken = serde_json::from_str(&body)?;
//             config.set_jquants_refresh_token(refresh_token.refresh_token);
//             config.write_to_file();
//             info!("Overwrite the jquantsRefreshToken in the config.json file");
//             Ok(())
//         }
//         _ => Err(MyError::Anyhow(anyhow!(
//             "Status code: {}, {}",
//             res.status(),
//             res.text().await?
//         ))),
//     }
// }

// async fn fetch_id_token(client: &Client) -> Result<(), MyError> {
//     info!("Fetch ID Token");
//     let mut config = crate::config::GdriveJson::new()?;
//     // debug!("{}", refresh_token);

//     let url = "https://api.jquants.com/v1/token/auth_refresh";
//     let query = json!({"refreshtoken": config.jquants_refresh_token()});

//     let res = client.post(url).query(&query).send().await?;

//     match res.status() {
//         StatusCode::OK => {
//             info!("Status code: {}", res.status());
//             let body = res.text().await?;
//             debug!("{}", body);
//             let id_token: IdToken = serde_json::from_str(&body)?;
//             config.set_jquants_id_token(id_token.id_token);
//             config.write_to_file();
//             info!("Overwrite the jquantsIdToken in the config.json file");
//             Ok(())
//         }
//         StatusCode::BAD_REQUEST => {
//             let body = res.text().await?;
//             info!("Status code 401 {}", body);
//             Err(MyError::RefreshTokenExpired)
//         }
//         _ => Err(MyError::Anyhow(anyhow!(
//             "Status code: {}, {}",
//             res.status(),
//             res.text().await?
//         ))),
//     }
// }

#[allow(dead_code)]
async fn fetch_listed_info(client: &Client, code: i32) -> Result<(), MyError> {
    let id_token = env::var("JQUANTS_ID_TOKEN")?;
    let base_url = "https://api.jquants.com/v1/listed/info";
    let date = {
        let now = chrono::Local::now();
        now.format("%Y-%m-%d").to_string()
    };

    // let url = base_url.to_string() + "?code=" + &code.to_string() + "&date=" + &date;
    let query = json!({"code": code, "date": date});

    info!("Fetch Listed Info. code: {}", code);
    let res = client
        .get(base_url)
        .query(&query)
        .bearer_auth(id_token)
        .send()
        .await?;

    match res.status() {
        StatusCode::OK => {
            info!("Status code: {}", res.status());
            let body = res.text().await?;
            info!("{}", body);
            Ok(())
        }
        StatusCode::UNAUTHORIZED => {
            let body = res.text().await?;
            info!("Status code 401 {}", body);
            Err(MyError::IdTokenExpired(body))
        }
        _ => Err(MyError::Anyhow(anyhow!(
            "Status code: {}, {}",
            res.status(),
            res.text().await?
        ))),
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TradingCalender {
    trading_calendar: Vec<TradingCalenderInner>,
}

impl TradingCalender {
    pub async fn new(client: &Client, from: Option<&str>) -> Result<Self, MyError> {
        let config = crate::config::GdriveJson::new()?;
        let url = "https://api.jquants.com/v1/markets/trading_calendar";
        let today = {
            let now = chrono::Local::now();
            now.format("%Y-%m-%d").to_string()
        };
        let json = match from {
            Some(from) => json!({"from": from, "to": today}),
            None => json!({"to": today}),
        };
        info!("Fetch Calender");
        let res = client
            .get(url)
            .query(&json)
            .bearer_auth(config.jquants_id_token())
            .send()
            .await?;

        let (status, text) = {
            let status = res.status();
            let text = res.text().await?;
            (status, text)
        };

        match status {
            StatusCode::OK => {
                info!("Status code: {}", status);
                let json = serde_json::from_str::<TradingCalender>(&text)?;
                info!("{:?}", json);
                Ok(json)
            }
            StatusCode::UNAUTHORIZED => {
                info!("Status code 401 {}", text);
                Err(MyError::IdTokenExpired(text))
            }
            _ => Err(MyError::Anyhow(anyhow!(
                "Status code: {}, {}",
                status,
                text
            ))),
        }
    }

    pub fn is_today_trading_day(&self) -> bool {
        let today = {
            let now = chrono::Local::now();
            now.format("%Y-%m-%d").to_string()
        };
        for row in &self.trading_calendar {
            if row.date == today && row.holiday_division == "1" {
                return true;
            }
        }
        false
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct TradingCalenderInner {
    #[serde(rename = "Date")]
    date: String,
    #[serde(rename = "HolidayDivision")]
    holiday_division: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Topix {
    topix: Vec<TopixInner>,
}
impl Topix {
    pub async fn new(client: &Client) -> Result<Self, MyError> {
        let config = crate::config::GdriveJson::new()?;
        let id_token = config.jquants_id_token();
        let url = "https://api.jquants.com/v1/indices/topix";

        info!("Fetch Topix");
        let res = client.get(url).bearer_auth(id_token).send().await?;

        let (status, text) = {
            let status = res.status();
            let text = res.text().await?;
            (status, text)
        };

        match status {
            StatusCode::OK => {
                info!("Status code: {}", status);
                debug!("{}", text);
                let json = serde_json::from_str::<Topix>(&text)?;
                Ok(json)
            }
            StatusCode::UNAUTHORIZED => {
                info!("Status code 401 {}", text);
                Err(MyError::IdTokenExpired(text))
            }
            _ => Err(MyError::Anyhow(anyhow!(
                "Status code: {}, {}",
                status,
                text
            ))),
        }
    }

    pub fn get_len_of_topix(&self) -> usize {
        self.topix.len()
    }

    pub fn get_ohlc(&self, i: usize) -> Ohlc {
        Ohlc::new(
            self.topix[i].get_date().to_owned(),
            self.topix[i].get_open(),
            self.topix[i].get_high(),
            self.topix[i].get_low(),
            self.topix[i].get_close(),
        )
    }

    pub fn save_to_json_file(&self) -> Result<(), MyError> {
        let path = crate::my_file_io::get_topix_ohlc_file_path()?;
        let file = File::create(&path)?;
        serde_json::to_writer(file, &self)?;
        info!("Topix has been saved, path: {:?}", path);
        Ok(())
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct TopixInner {
    #[serde(rename = "Date")]
    date: String,
    #[serde(rename = "Open")]
    open: f64,
    #[serde(rename = "High")]
    high: f64,
    #[serde(rename = "Low")]
    low: f64,
    #[serde(rename = "Close")]
    close: f64,
}
impl TopixInner {
    pub fn get_date(&self) -> &str {
        &self.date
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
}

#[derive(Deserialize, Serialize, Debug)]
pub struct DailyQuotes {
    daily_quotes: Vec<DailyQuotesInner>,
}

impl DailyQuotes {
    pub async fn new(client: &Client, code: i32) -> Result<Self, MyError> {
        let config = crate::config::GdriveJson::new()?;
        let id_token = config.jquants_id_token();
        let url = "https://api.jquants.com/v1/prices/daily_quotes";

        let query = json!({"code": code});

        info!("Fetch Daily OHLC, code: {}", code);
        let res = client
            .get(url)
            .query(&query)
            .bearer_auth(id_token)
            .send()
            .await?;

        let (status, text) = {
            let status = res.status();
            let text = res.text().await?;
            (status, text)
        };

        match status {
            StatusCode::OK => {
                info!("Status code: {}", status);
                debug!("{}", text);
                let json = serde_json::from_str::<DailyQuotes>(&text)?;
                Ok(json)
            }
            StatusCode::UNAUTHORIZED => {
                info!("Status code 401 {}", text);
                Err(MyError::IdTokenExpired(text))
            }
            _ => Err(MyError::Anyhow(anyhow!(
                "Status code: {}, {}",
                status,
                text
            ))),
        }
    }

    pub fn get_ohlc(self) -> Vec<Ohlc> {
        let mut ohlc_vec = Vec::new();
        for jquants_ohlc in self.daily_quotes {
            if jquants_ohlc.open.is_none()
                || jquants_ohlc.high.is_none()
                || jquants_ohlc.low.is_none()
                || jquants_ohlc.close.is_none()
            {
                continue;
            }
            let jquants_ohlc = Ohlc::new(
                jquants_ohlc.date,
                jquants_ohlc.open.expect("Expected open to be Some"),
                jquants_ohlc.high.expect("Expected high to be Some"),
                jquants_ohlc.low.expect("Expected low to be Some"),
                jquants_ohlc.close.expect("Expected close to be Some"),
            );
            ohlc_vec.push(jquants_ohlc);
        }
        ohlc_vec
    }

    pub fn get_ohlc_premium(self) -> Vec<OhlcPremium> {
        let mut ohlc_vec = Vec::new();
        for jquants_ohlc in self.daily_quotes {
            if jquants_ohlc.open.is_none()
                || jquants_ohlc.high.is_none()
                || jquants_ohlc.low.is_none()
                || jquants_ohlc.close.is_none()
                || jquants_ohlc.morning_close.is_none()
                || jquants_ohlc.afternoon_open.is_none()
            {
                continue;
            }
            let jquants_ohlc = OhlcPremium::new(
                jquants_ohlc.date,
                jquants_ohlc.open.expect("Expected open to be Some"),
                jquants_ohlc.high.expect("Expected high to be Some"),
                jquants_ohlc.low.expect("Expected low to be Some"),
                jquants_ohlc.close.expect("Expected close to be Some"),
                jquants_ohlc
                    .morning_close
                    .expect("Expected morning_close to be Some"),
                jquants_ohlc
                    .afternoon_open
                    .expect("Expected afternoon_open to be Some"),
            );
            ohlc_vec.push(jquants_ohlc);
        }
        ohlc_vec
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct DailyQuotesInner {
    #[serde(rename = "Date")]
    date: String,
    // #[serde(rename = "Code")]
    // code: String,
    // #[serde(rename = "Open")]
    // open: Option<f64>,
    // #[serde(rename = "High")]
    // high: Option<f64>,
    // #[serde(rename = "Low")]
    // low: Option<f64>,
    // #[serde(rename = "Close")]
    // close: Option<f64>,
    // #[serde(rename = "UpperLimit")]
    // upper_limit: String,
    // #[serde(rename = "LowerLimit")]
    // lower_limit: String,
    // #[serde(rename = "Volume")]
    // volume: Option<f64>,
    // #[serde(rename = "TurnoverValue")]
    // turnover_value: Option<f64>,
    // #[serde(rename = "AdjustmentFactor")]
    // adjustment_factor: f64,
    #[serde(rename = "AdjustmentOpen")]
    open: Option<f64>,
    #[serde(rename = "AdjustmentHigh")]
    high: Option<f64>,
    #[serde(rename = "AdjustmentLow")]
    low: Option<f64>,
    #[serde(rename = "AdjustmentClose")]
    close: Option<f64>,
    // #[serde(rename = "AdjustmentVolume")]
    // adjustment_volume: Option<f64>,
    #[serde(rename = "MorningAdjustmentClose")]
    morning_close: Option<f64>,
    #[serde(rename = "AfternoonAdjustmentOpen")]
    afternoon_open: Option<f64>,
}

pub async fn first_fetch(client: &Client, from: Option<&str>) -> Result<TradingCalender, MyError> {
    match TradingCalender::new(client, from).await {
        Ok(res) => return Ok(res),
        Err(MyError::IdTokenExpired(_)) => {
            info!("ID token expired, attempting to fetch a new one...")
        }
        Err(e) => return Err(e),
    };

    match IdToken::fetch_and_save_to_file(client).await {
        Ok(_) => return TradingCalender::new(client, from).await,
        Err(MyError::RefreshTokenExpired) => {
            info!("Refresh token expired, attempting to fetch a new one...")
        }
        Err(e) => return Err(e),
    }

    match RefreshToken::fetch_and_save_to_file(client).await {
        Ok(_) => {
            info!("Refresh token has been updated. Attempting to fetch a new ID token...")
        }
        Err(e) => return Err(e),
    }

    match IdToken::fetch_and_save_to_file(client).await {
        Ok(_) => {
            info!("ID token has been updated. Attempting to fetch a new Trading Calender...");
        }
        Err(e) => return Err(e),
    }

    TradingCalender::new(client, from).await
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PricesAm {
    prices_am: Vec<PricesAmInner>,
}

impl PricesAm {
    pub async fn new(client: &Client) -> Result<Self, MyError> {
        let config = crate::config::GdriveJson::new()?;
        let id_token = config.jquants_id_token();
        let url = "https://api.jquants.com/v1/prices/prices_am";

        info!("Fetch morning market OHLC");
        let res = client.get(url).bearer_auth(id_token).send().await?;

        match res.status() {
            StatusCode::OK => {
                info!("Status code: {}", res.status());
                let body = res.text().await?;
                let json = serde_json::from_str::<PricesAm>(&body)?;
                debug!("{:?}", json);

                Ok(json)
            }
            StatusCode::UNAUTHORIZED => {
                let body = res.text().await?;
                info!("Status code 401 {}", body);
                Err(MyError::IdTokenExpired(body))
            }
            _ => Err(MyError::Anyhow(anyhow!(
                "Status code: {}, {}",
                res.status(),
                res.text().await?
            ))),
        }
    }

    pub fn get_stock_ohlc(&self, code: i32) -> Option<(f64, f64)> {
        let code = {
            let str = code.to_string();
            str + "0"
        };
        self.prices_am
            .iter()
            .filter(|x| x.code == code)
            .map(|x| {
                (
                    x.morning_open.expect("Expected morning_open to be Some"),
                    x.morning_close.expect("Expected morning_close to be Some"),
                )
            })
            .next()
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct PricesAmInner {
    #[serde(rename = "Date")]
    date: String,
    #[serde(rename = "Code")]
    code: String,
    #[serde(rename = "MorningOpen")]
    morning_open: Option<f64>,
    #[serde(rename = "MorningHigh")]
    morning_high: Option<f64>,
    #[serde(rename = "MorningLow")]
    morning_low: Option<f64>,
    #[serde(rename = "MorningClose")]
    morning_close: Option<f64>,
    #[serde(rename = "MorningVolume")]
    morning_volume: Option<f64>,
    #[serde(rename = "MorningTurnoverValue")]
    morning_turnover_value: Option<f64>,
}

pub async fn fetch_nikkei225(force: bool) -> Result<(), MyError> {
    let client = Client::new();

    info!("Starting First Fetch");
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let first_fetched = first_fetch(&client, Some(&today)).await?;
    match (first_fetched.is_today_trading_day(), force) {
        (true, _) => info!("Today is Trading Day"),
        (false, true) => info!("Today is Holiday, but force is true"),
        (false, false) => {
            error!("Today is Holiday");
            return Err(MyError::Holiday);
        }
    };

    let topix = Topix::new(&client).await?;
    topix.save_to_json_file()?;

    let nikkei225 = crate::my_file_io::load_nikkei225_list()?;
    info!("Nikkei225 list has been loaded");

    let config = crate::config::GdriveJson::new()?;
    let unit = config.jquants_unit();
    info!("unit: {}", unit);

    info!("Starting Fetch Nikkei225");

    for row in nikkei225 {
        thread::sleep(Duration::from_secs(1));

        let code = row.get_code();

        let daily_quotes: DailyQuotes = DailyQuotes::new(&client, code).await?;

        let raw_ohlc: Vec<OhlcPremium> = daily_quotes.get_ohlc_premium();
        let now = chrono::Local::now().format("%Y-%m-%d").to_string();
        let last_date = raw_ohlc
            .last()
            .expect("Expected raw_ohlc to be Some")
            .get_date()
            .to_string();
        if now != last_date && !force {
            error!("Not Latest Data");
            return Err(MyError::NotLatestData);
        }

        let raw_ohlc_serialized = serde_json::to_string(&raw_ohlc)?;
        let path = get_fetched_ohlc_file_path(AssetType::Stocks { code: Some(code) })?;
        std::fs::write(path, raw_ohlc_serialized)?;
    }
    Ok(())
}

// pub async fn fetch_nikkei225_daytrading(force: bool) -> Result<crate::markdown::Markdown, MyError> {
//     let client = Client::new();

//     info!("Starting First Fetch");
//     let today = chrono::Local::now().format("%Y-%m-%d").to_string();
//     match first_fetch(&client, Some(&today)).await {
//         Ok(res) => match res.is_today_trading_day() {
//             true => info!("Today is Trading Day"),
//             false => match force {
//                 true => info!("Today is Holiday, but force is true"),
//                 false => {
//                     error!("Today is Holiday");
//                     return Err(MyError::Holiday);
//                 }
//             },
//         },
//         Err(e) => {
//             error!("{}", e);
//             return Err(e);
//         }
//     };

//     let topix = match Topix::new(&client).await {
//         Ok(res) => res,
//         Err(e) => {
//             error!("{}", e);
//             return Err(e);
//         }
//     };
//     topix.save_to_json_file()?;

//     let nikkei225 = match crate::my_file_io::load_nikkei225_list() {
//         Ok(res) => res,
//         Err(e) => {
//             error!("{}", e);
//             return Err(e);
//         }
//     };
//     info!("Nikkei225 list has been loaded");

//     let config = crate::config::GdriveJson::new();
//     let unit = config.jquants_unit();
//     info!("unit: {}", unit);

//     info!("Starting Fetch Nikkei225");

//     for row in nikkei225 {
//         thread::sleep(Duration::from_secs(1));

//         let code = row.get_code();

//         let daily_quotes: DailyQuotes = match DailyQuotes::new(&client, code).await {
//             Ok(res) => res,
//             Err(e) => {
//                 error!("{}", e);
//                 return Err(e);
//             }
//         };

//         let raw_ohlc: Vec<OhlcPremium> = daily_quotes.get_ohlc_premium();
//         let now = chrono::Local::now().format("%Y-%m-%d").to_string();
//         let last_date = raw_ohlc
//             .last()
//             .expect("Expected raw_ohlc to be Some")
//             .get_date()
//             .to_string();
//         if now != last_date && !force {
//             error!("Not Latest Data");
//             return Err(MyError::NotLatestData);
//         }
//         match serde_json::to_string(&raw_ohlc) {
//             Ok(res) => {
//                 let path = get_fetched_ohlc_file_path(AssetType::Stocks { code: Some(code) })?;
//                 std::fs::write(path, res)?;
//             }
//             Err(e) => {
//                 error!("{}", e);
//                 return Err(MyError::Anyhow(anyhow!("{}", e)));
//             }
//         }
//     }

//     let mut stocks_daytrading_list = match stocks_daytrading::async_exec(&today, &today).await {
//         Ok(res) => res,
//         Err(e) => {
//             error!("{}", e);
//             return Err(e);
//         }
//     };
//     stocks_daytrading_list.sort_by_standardized_diff();

//     let markdown = stocks_daytrading_list.output_for_markdown(&today);
//     let markdown_path = crate::my_file_io::get_jquants_break_path(&today)?;
//     markdown.write_to_file(&markdown_path);

//     Ok(stocks_daytrading_list.output_for_markdown(&today))
// }

// pub async fn _fetch_nikkei225_old(force: bool) -> Result<(), MyError> {
//     let client = Client::new();

//     info!("Starting First Fetch");
//     let today = chrono::Local::now().format("%Y-%m-%d").to_string();
//     match first_fetch(&client, Some(&today)).await {
//         Ok(res) => match res.is_today_trading_day() {
//             true => info!("Today is Trading Day"),
//             false => {
//                 error!("Today is Holiday");
//                 return Err(MyError::Holiday);
//             }
//         },
//         Err(e) => {
//             error!("{}", e);
//             return Err(e);
//         }
//     };

//     let nikkei225 = match crate::my_file_io::load_nikkei225_list() {
//         Ok(res) => res,
//         Err(e) => {
//             error!("{}", e);
//             return Err(e);
//         }
//     };
//     info!("Nikkei225 has been loaded");

//     let config = crate::config::GdriveJson::new();
//     let unit = config.jquants_unit();
//     info!("unit: {}", unit);

//     info!("Starting Fetch Nikkei225");

//     for row in nikkei225 {
//         thread::sleep(Duration::from_secs(1));

//         let code = row.get_code();
//         let name = row.get_name();

//         let daily_quotes: DailyQuotes = match DailyQuotes::new(&client, code).await {
//             Ok(res) => res,
//             Err(e) => {
//                 error!("{}", e);
//                 return Err(e);
//             }
//         };

//         let raw_ohlc: Vec<Ohlc> = daily_quotes.get_ohlc();
//         let now = chrono::Local::now().format("%Y-%m-%d").to_string();
//         let last_date = raw_ohlc
//             .last()
//             .expect("Expected raw_ohlc to be Some")
//             .get_date()
//             .to_string();
//         if now != last_date && !force {
//             error!("Not Latest Data");
//             return Err(MyError::NotLatestData);
//         }
//         match serde_json::to_string(&raw_ohlc) {
//             Ok(res) => {
//                 let path = get_fetched_ohlc_file_path(AssetType::Stocks { code: Some(code) })?;
//                 std::fs::write(path, res)?;
//             }
//             Err(e) => {
//                 error!("{}", e);
//                 return Err(MyError::Anyhow(anyhow!("{}", e)));
//             }
//         }

//         let ohlc_analyzer = OhlcAnalyzer::from_jquants(raw_ohlc);

//         let conn = crate::database::stocks::open_db()?;
//         let new_stock = crate::database::stocks::NewStock::new(code, name, ohlc_analyzer);
//         new_stock.insert_record(&conn, unit);
//     }
//     Ok(())
// }

// pub async fn fetch_daily_quotes_once(client: &Client, code: i32) -> Result<String, MyError> {
//     info!("Starting Ohlc Fetch once");
//     let today = chrono::Local::now().format("%Y-%m-%d").to_string();
//     if let Err(e) = first_fetch(client, Some(&today)).await {
//         error!("{}", e);
//         return Err(e);
//     }

//     info!("Fetch Daily Quotes");
//     let daily_quotes: DailyQuotes = match DailyQuotes::new(client, code).await {
//         Ok(res) => res,
//         Err(e) => {
//             error!("{}", e);
//             return Err(e);
//         }
//     };

//     let raw_ohlc: Vec<OhlcPremium> = daily_quotes.get_ohlc_premium();
//     let last_data = raw_ohlc.last().expect("Expected raw_ohlc to be Some");
//     let last_date = last_data.get_date().to_string();
//     info!("last_data: {:?}", last_data);
//     // let ohlc_analyzer = OhlcAnalyzer::from_jquants(raw_ohlc);

//     // ohlc_analyzer.get_shorter_chart();
//     // info!(
//     //     "daily standardized diff: {}",
//     //     ohlc_analyzer.get_shorter_ohlc_standardized_diff()
//     // );

//     Ok(last_date)
// }

// #[cfg(test)]
// mod tests {
//     #[test]
//     fn chrono_test() {
//         let now = chrono::Local::now();
//         let now_string = now.format("%Y-%m-%d").to_string();
//         assert_eq!(now_string, "2022-12-31")
//     }
// }
