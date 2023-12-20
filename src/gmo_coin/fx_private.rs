use chrono::Local;
use hex::encode as hex_encode;
use log::info;
use reqwest::Client;
use ring::hmac::{sign, Key, HMAC_SHA256};
use serde_json::json;
use std::env;

pub async fn _get_assets() {
    let client = Client::new();

    let api_key = env::var("GMO_COIN_FX_API_KEY").unwrap();
    let secret_key = env::var("GMO_COIN_FX_API_SECRET").unwrap();
    let timestamp = Local::now().timestamp_millis();
    let method = "GET";
    let endpoint = "https://forex-api.coin.z.com/private";
    let path = "/v1/account/assets";

    let text = format!("{}{}{}", timestamp, method, path);
    let signed_key = Key::new(HMAC_SHA256, secret_key.as_bytes());
    let sign = hex_encode(sign(&signed_key, text.as_bytes()).as_ref());

    let res = client
        .get(&(endpoint.to_string() + path))
        .header("API-KEY", api_key)
        .header("API-TIMESTAMP", timestamp)
        .header("API-SIGN", sign)
        .send()
        .await
        .unwrap();

    info!("Status: {}", res.status());
    info!("body: {}", res.text().await.unwrap());
}

pub async fn _speed_order() {
    let client = Client::new();

    let api_key = env::var("GMO_COIN_FX_API_KEY").unwrap();
    let secret_key = env::var("GMO_COIN_FX_API_SECRET").unwrap();
    let timestamp = Local::now().timestamp_millis();
    let method = "POST";
    let endpoint = "https://forex-api.coin.z.com/private";
    let path = "/v1/speedOrder";
    let parameters = json!({
        "symbol": "USD_JPY",
        "side": "BUY",
        "size": "5000"

    });

    let text = format!("{}{}{}{}", timestamp, method, path, &parameters);
    let signed_key = Key::new(HMAC_SHA256, secret_key.as_bytes());
    let sign = hex_encode(sign(&signed_key, text.as_bytes()).as_ref());

    let res = client
        .post(&(endpoint.to_string() + path))
        .header("content-type", "application/json")
        .header("API-KEY", api_key)
        .header("API-TIMESTAMP", timestamp)
        .header("API-SIGN", sign)
        .json(&parameters)
        .send()
        .await
        .unwrap();

    info!("Status: {}", res.status());
    info!("body: {}", res.text().await.unwrap())
}
