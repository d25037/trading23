use log::{error, info};

pub async fn send_message(message: &str) {
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
