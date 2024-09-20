use crate::api::schema::*;
use mirror_error::MirrorError;
use mirror_utils::fs_handler;

// read the 'image set config' file
pub async fn load_config(config_file: String) -> Result<String, MirrorError> {
    // Create a path to the desired file
    let data = fs_handler(config_file.clone(), "read", None).await?;
    Ok(data.clone())
}

// parse the 'image set config' file
pub fn parse_yaml_config(data: String) -> Result<MicroserviceConfig, MirrorError> {
    // Parse the string of data into serde_json::ImageSetConfig.
    let res = serde_yaml::from_str(&data);
    if res.is_err() {
        return Err(MirrorError::new(&format!(
            "[parse_yaml_config] {}",
            res.err().unwrap().to_string().to_lowercase()
        )));
    }
    let root: MicroserviceConfig = res.unwrap();
    Ok(root)
}
