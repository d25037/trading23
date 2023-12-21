use std::{thread, time::Duration};

use crate::my_db::Output;
use log::{error, info};

async fn send_message(message: &str) {
    let client = reqwest::Client::new();
    let url = "https://notify-api.line.me/api/notify";
    let config = crate::config::GdriveJson::new();
    let token = config.line_token();

    let res = client
        .post(url)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .bearer_auth(token)
        .body(format!("message={}", message))
        .send()
        .await;

    match res {
        Ok(res) => {
            info!("Status: {}", res.status());
        }
        Err(e) => {
            error!("Error: {}", e);
        }
    }
}

pub async fn send_message_from_jquants_output(output: Output) {
    send_message(&output.get_entry_long_or_short()).await;
    thread::sleep(Duration::from_secs(2));
    send_message(&output.get_long_stocks()).await;
    thread::sleep(Duration::from_secs(2));
    send_message(&output.get_short_stocks()).await;
}
