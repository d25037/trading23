use std::{thread, time::Duration};

use crate::{database::stocks::Output, my_error::MyError};
use log::{error, info};
use reqwest::Client;

pub async fn send_message(client: &Client, message: &str) -> Result<(), MyError> {
    let url = "https://notify-api.line.me/api/notify";
    let config = crate::config::GdriveJson::new()?;
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
            info!("Line Notify, Status: {}", res.status());
        }
        Err(e) => {
            error!("Error: {}", e);
        }
    }
    Ok(())
}

pub async fn send_message_from_jquants_output(
    client: &Client,
    output: Output,
) -> Result<(), MyError> {
    send_message(client, &output.get_entry_long_or_short()).await?;
    thread::sleep(Duration::from_secs(1));
    send_message(client, output.get_long_stocks()).await?;
    thread::sleep(Duration::from_secs(1));
    send_message(client, output.get_short_stocks()).await?;
    Ok(())
}

// pub async fn send_message_from_jquants_daytrading(
//     output: crate::analysis::stocks_daytrading::Output,
// ) {
//     send_message(output.get_date()).await;
//     thread::sleep(Duration::from_secs(2));
//     send_message(output.get_breakout_resistance_stocks()).await;
//     thread::sleep(Duration::from_secs(2));
//     send_message(output.get_failed_breakout_resistance_stocks()).await;
//     thread::sleep(Duration::from_secs(2));
//     send_message(output.get_failed_breakout_support_stocks()).await;
//     thread::sleep(Duration::from_secs(2));
//     send_message(output.get_breakout_support_stocks()).await;
// }
