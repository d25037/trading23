use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
pub struct GdriveJson {
    #[serde(rename = "jquantsMail")]
    jquants_mail: String,
    #[serde(rename = "jquantsPw")]
    jquants_pw: String,
    #[serde(rename = "jquantsRefreshToken")]
    jquants_refresh_token: String,
    #[serde(rename = "jquantsIdToken")]
    jquants_id_token: String,
    #[serde(rename = "jquantsUnit")]
    jquants_unit: String,
    #[serde(rename = "lineToken")]
    line_token: String,
    #[serde(rename = "gmoCoinFxApiKey")]
    gmo_coin_fx_api_key: String,
    #[serde(rename = "gmoCoinFxApiSecret")]
    gmo_coin_fx_api_secret: String,
}

impl GdriveJson {
    pub fn new() -> Self {
        let file_path = {
            let gdrive_path = std::env::var("GDRIVE_PATH").unwrap();
            Path::new(&gdrive_path)
                .join("trading23")
                .join("config.json")
        };
        let file = File::open(file_path).unwrap();

        serde_json::from_reader(file).unwrap()
    }

    pub fn write_to_file(&self) {
        let file_path = {
            let gdrive_path = std::env::var("GDRIVE_PATH").unwrap();
            Path::new(&gdrive_path)
                .join("trading23")
                .join("config.json")
        };
        let file = File::create(file_path).unwrap();

        serde_json::to_writer_pretty(file, self).unwrap();
    }

    pub fn jquants_mail(&self) -> &str {
        &self.jquants_mail
    }
    pub fn jquants_id_token(&self) -> &str {
        &self.jquants_id_token
    }
    pub fn jquants_refresh_token(&self) -> &str {
        &self.jquants_refresh_token
    }
    pub fn jquants_pw(&self) -> &str {
        &self.jquants_pw
    }
    pub fn jquants_unit(&self) -> f64 {
        self.jquants_unit.parse::<f64>().unwrap()
    }
    pub fn line_token(&self) -> &str {
        &self.line_token
    }
    pub fn _gmo_coin_fx_api_key(&self) -> &str {
        &self.gmo_coin_fx_api_key
    }
    pub fn _gmo_coin_fx_api_secret(&self) -> &str {
        &self.gmo_coin_fx_api_secret
    }
    pub fn set_jquants_refresh_token(&mut self, token: String) {
        self.jquants_refresh_token = token;
    }
    pub fn set_jquants_id_token(&mut self, token: String) {
        self.jquants_id_token = token;
    }
}
