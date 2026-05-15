use std::path::Path;

fn main() {
    // Read config.toml from workspace root (two levels up from this crate)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let config_path = Path::new(&manifest_dir).join("../../config.toml");

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).expect("Failed to read config.toml");
        let config: toml::Value = content.parse().expect("Failed to parse config.toml");

        if let Some(wifi) = config.get("wifi") {
            if let Some(ssid) = wifi.get("ssid").and_then(|v| v.as_str()) {
                println!("cargo:rustc-env=WIFI_SSID={ssid}");
            }
            if let Some(pass) = wifi.get("password").and_then(|v| v.as_str()) {
                println!("cargo:rustc-env=WIFI_PASS={pass}");
            }
        }
        if let Some(loc) = config.get("location") {
            if let Some(lat) = loc.get("latitude").and_then(|v| v.as_float()) {
                println!("cargo:rustc-env=DEFAULT_LAT={lat}");
            }
            if let Some(lon) = loc.get("longitude").and_then(|v| v.as_float()) {
                println!("cargo:rustc-env=DEFAULT_LON={lon}");
            }
            if let Some(name) = loc.get("name").and_then(|v| v.as_str()) {
                println!("cargo:rustc-env=DEFAULT_LOCATION={name}");
            }
        }
        println!("cargo:rerun-if-changed={}", config_path.display());
    } else {
        println!(
            "cargo:warning=config.toml not found at {}, using defaults. \
             Copy config.toml.example to config.toml",
            config_path.display()
        );
        println!("cargo:rustc-env=WIFI_SSID=YOUR_WIFI_SSID");
        println!("cargo:rustc-env=WIFI_PASS=YOUR_WIFI_PASSWORD");
        println!("cargo:rustc-env=DEFAULT_LAT=46.77");
        println!("cargo:rustc-env=DEFAULT_LON=23.60");
        println!("cargo:rustc-env=DEFAULT_LOCATION=Cluj-Napoca");
    }

    embuild::espidf::sysenv::output();
}
