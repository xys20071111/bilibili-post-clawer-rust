use std::{error::Error, path::PathBuf, str::FromStr, time::Duration};

use headless_chrome::{Browser, LaunchOptionsBuilder, Tab};
use std::sync::Arc;

pub fn open_browser(
    headless: bool,
    debug: bool,
    user_data_dir_path: &str,
) -> Result<Browser, Box<dyn Error>> {
    let user_data_dir = Some(PathBuf::from_str(user_data_dir_path)?);
    let options = LaunchOptionsBuilder::default()
        .headless(headless)
        .devtools(debug)
        .user_data_dir(user_data_dir)
        .idle_browser_timeout(Duration::from_hours(1))
        .build()?;
    let browser = Browser::new(options)?;
    Ok(browser)
}

pub fn inject_functions(tab: &Tab) {
    tab.expose_function(
        "denoAlert",
        Arc::new(|msg: serde_json::Value| {
            eprintln!("[WebAlert] {}", msg);
        }),
    )
    .unwrap();
    tab.expose_function(
        "denoLog",
        Arc::new(|msg: serde_json::Value| {
            println!("[WebLog] {}", msg);
        }),
    )
    .unwrap();
}
