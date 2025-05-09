pub mod validation {
    use {
        reqwest::Url,
        serde::{de, Deserialize, Deserializer},
    };

    pub fn deserialize_url_opt<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        if let Some(ref s) = opt {
            // Full URL validation
            Url::parse(s).map_err(de::Error::custom)?;
        }
        Ok(opt)
    }
}
