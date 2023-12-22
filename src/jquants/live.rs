use crate::analysis::live::{Ohlc, OhlcAnalyzer};
use crate::my_error::MyError;
use anyhow::{anyhow, Result};
use log::error;
use log::{debug, info};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use std::{env, thread};

#[derive(Deserialize, Serialize, Debug)]
struct RefreshToken {
    #[serde(rename = "refreshToken")]
    refresh_token: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct IdToken {
    #[serde(rename = "idToken")]
    id_token: String,
}

async fn fetch_refresh_token(client: &Client) -> Result<(), MyError> {
    info!("Fetch Refresh Token");
    let mut config = crate::config::GdriveJson::new();

    let mut map = HashMap::new();
    map.insert("mailaddress", config.jquants_mail());
    map.insert("password", config.jquants_pw());

    let res = client
        .post("https://api.jquants.com/v1/token/auth_user")
        .json(&map)
        .send()
        .await?;

    match res.status() {
        StatusCode::OK => {
            info!("Status code: {}", res.status());
            let body = res.text().await?;
            debug!("{}", body);
            let refresh_token: RefreshToken = serde_json::from_str(&body)?;
            config.set_jquants_refresh_token(refresh_token.refresh_token);
            config.write_to_file();
            info!("Overwrite the jquantsRefreshToken in the .env file");
            Ok(())
        }
        _ => Err(MyError::Anyhow(anyhow!(
            "Status code: {}, {}",
            res.status(),
            res.text().await?
        ))),
    }
}

async fn fetch_id_token(client: &Client) -> Result<(), MyError> {
    info!("Fetch ID Token");
    let mut config = crate::config::GdriveJson::new();
    // debug!("{}", refresh_token);

    let url = "https://api.jquants.com/v1/token/auth_refresh";
    let query = json!({"refreshtoken": config.jquants_refresh_token()});

    let res = client.post(url).query(&query).send().await?;

    match res.status() {
        StatusCode::OK => {
            info!("Status code: {}", res.status());
            let body = res.text().await?;
            debug!("{}", body);
            let id_token: IdToken = serde_json::from_str(&body)?;
            config.set_jquants_id_token(id_token.id_token);
            config.write_to_file();
            info!("Overwrite the jquantsIdToken in the config.json file");
            Ok(())
        }
        StatusCode::BAD_REQUEST => {
            let body = res.text().await?;
            info!("Status code 401 {}", body);
            Err(MyError::RefreshTokenExpired)
        }
        _ => Err(MyError::Anyhow(anyhow!(
            "Status code: {}, {}",
            res.status(),
            res.text().await?
        ))),
    }
}

#[allow(dead_code)]
async fn fetch_listed_info(client: &Client, code: i32) -> Result<(), MyError> {
    let id_token = env::var("JQUANTS_ID_TOKEN").unwrap();
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

async fn fetch_topix(client: &Client) -> Result<(), MyError> {
    let config = crate::config::GdriveJson::new();
    let id_token = config.jquants_id_token();
    let base_url = "https://api.jquants.com/v1/indices/topix";
    let now = chrono::Local::now();
    let now_string = now.format("%Y-%m-%d").to_string();
    let yesterday = now - chrono::Duration::days(7);
    let yesterday_string = yesterday.format("%Y-%m-%d").to_string();

    let query = json!({"from": yesterday_string, "to": now_string});

    info!("Fetch Topix");
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
            debug!("{}", body);
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

pub async fn fetch_ohlc(client: &Client, code: i32) -> Result<String, MyError> {
    let config = crate::config::GdriveJson::new();
    let id_token = config.jquants_id_token();
    let url = "https://api.jquants.com/v1/prices/daily_quotes";

    let query = json!({"code": code});

    // info!("Fetch Daily OHLC");
    let res = client
        .get(url)
        .query(&query)
        .bearer_auth(id_token)
        .send()
        .await?;

    match res.status() {
        StatusCode::OK => {
            info!("Status code: {}, code: {}", res.status(), code);
            let body = res.text().await?;
            // info!("{}", body);
            Ok(body)
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
pub struct DailyQuotes {
    daily_quotes: Vec<JquantsOhlc>,
}

impl DailyQuotes {
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
                jquants_ohlc.open.unwrap(),
                jquants_ohlc.high.unwrap(),
                jquants_ohlc.low.unwrap(),
                jquants_ohlc.close.unwrap(),
            );
            ohlc_vec.push(jquants_ohlc);
        }
        ohlc_vec
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct JquantsOhlc {
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
}

pub async fn first_fetch(client: &Client) -> Result<(), MyError> {
    match fetch_topix(client).await {
        Ok(_) => Ok(()),
        Err(MyError::IdTokenExpired(_)) => match fetch_id_token(client).await {
            Ok(_) => fetch_topix(client).await,
            Err(MyError::RefreshTokenExpired) => match fetch_refresh_token(client).await {
                Ok(_) => match fetch_id_token(client).await {
                    Ok(_) => fetch_topix(client).await,
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    }
}

pub async fn fetch_nikkei225(force: bool) -> Result<(), MyError> {
    let client = Client::new();

    info!("Starting First Fetch");
    if let Err(e) = first_fetch(&client).await {
        error!("{}", e);
        return Err(e);
    }

    let nikkei225 = match crate::my_file_io::load_nikkei225_list() {
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

    info!("Starting Fetch Nikkei225");

    for row in nikkei225 {
        thread::sleep(Duration::from_secs(2));

        let code = row.get_code();
        let name = row.get_name();

        let daily_quotes: DailyQuotes = match fetch_ohlc(&client, code).await {
            Ok(res) => {
                // debug!("{:?}", res);
                serde_json::from_str(&res).unwrap()
            }
            Err(e) => {
                error!("{}", e);
                return Err(e);
            }
        };

        let raw_ohlc: Vec<Ohlc> = daily_quotes.get_ohlc();
        let now = chrono::Local::now().format("%Y-%m-%d").to_string();
        let last_date = raw_ohlc.last().unwrap().get_date().to_string();
        if now != last_date && !force {
            error!("Not Latest Data");
            return Err(MyError::NotLatestData);
        }

        let ohlc_analyzer = OhlcAnalyzer::from_jquants(raw_ohlc);

        let conn = crate::my_db::open_db().unwrap();
        let new_stock = crate::my_db::NewStock::new(code, name, ohlc_analyzer);
        new_stock.insert_record(&conn, unit);
    }
    Ok(())
}

pub async fn fetch_ohlc_once(code: i32) -> Result<String, MyError> {
    info!("Starting Ohlc Fetch once");
    let client = Client::new();
    if let Err(e) = first_fetch(&client).await {
        error!("{}", e);
        return Err(e);
    }

    let daily_quotes: DailyQuotes = match fetch_ohlc(&client, code).await {
        Ok(res) => {
            // debug!("{:?}", res);
            serde_json::from_str(&res).unwrap()
        }
        Err(e) => {
            error!("{}", e);
            return Err(e);
        }
    };

    let raw_ohlc: Vec<Ohlc> = daily_quotes.get_ohlc();
    let last_data = raw_ohlc.last().unwrap();
    let last_date = last_data.get_date().to_string();
    info!("last_data: {:?}", last_data);
    let ohlc_analyzer = OhlcAnalyzer::from_jquants(raw_ohlc);

    ohlc_analyzer.get_shorter_chart();
    info!(
        "daily standardized diff: {}",
        ohlc_analyzer.get_shorter_ohlc_standardized_diff()
    );

    Ok(last_date)
}

// #[cfg(test)]
// mod tests {
//     #[test]
//     fn chrono_test() {
//         let now = chrono::Local::now();
//         let now_string = now.format("%Y-%m-%d").to_string();
//         assert_eq!(now_string, "2022-12-31")
//     }
// }
