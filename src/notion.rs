use crate::my_error::MyError;
use log::info;
use reqwest::Client;
use serde_json::Value;

pub async fn get_notion_data() -> Result<(), MyError> {
    let client = Client::new();
    let db_id = "xxxxxxx";
    let url = format! {"https://api.notion.com/v1/databases/{}/query", db_id};
    let token = "my_secret_token";

    info!("fetch notion data");
    let res = client
        .post(url)
        .header("Notion-Version", "2022-06-28")
        .bearer_auth(token)
        .send()
        .await
        .unwrap();

    info!("status: {}", res.status());

    let text = res.text().await.unwrap();
    // textをdeserializeする
    let notion_data: Value = serde_json::from_str(&text).unwrap();
    info!("notion data: {:#?}", notion_data);

    Ok(())
}
