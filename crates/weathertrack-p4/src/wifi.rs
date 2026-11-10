//! WiFi via the board's ESP32-C6 coprocessor (esp_wifi_remote over SDIO).

use esp_idf_hal::modem::Modem;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};

pub type Wifi = BlockingWifi<EspWifi<'static>>;

/// Create the driver and start the radio (does not connect yet).
pub fn start(
    modem: Modem<'static>,
    sys_loop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
) -> anyhow::Result<Wifi> {
    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sys_loop.clone(), Some(nvs))?, sys_loop)?;
    wifi.set_configuration(&Configuration::Client(ClientConfiguration::default()))?;
    wifi.start()?;
    Ok(wifi)
}

/// Join a network and wait for an IP. Safe to call again after a failure.
pub fn join(wifi: &mut Wifi, ssid: &str, pass: &str) -> anyhow::Result<()> {
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid
            .try_into()
            .map_err(|_| anyhow::anyhow!("SSID too long"))?,
        password: pass
            .try_into()
            .map_err(|_| anyhow::anyhow!("password too long"))?,
        ..Default::default()
    }))?;
    let _ = wifi.disconnect();
    wifi.connect()?;
    wifi.wait_netif_up()?;
    log::info!("WiFi connected to {ssid}");
    Ok(())
}
