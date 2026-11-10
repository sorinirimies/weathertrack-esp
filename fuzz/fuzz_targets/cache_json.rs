#![no_main]
//! Cached snapshots live in flash/SD and can be corrupt: loading must fail cleanly,
//! and anything that loads must serialise again.

use libfuzzer_sys::fuzz_target;
use weathertrack_core::{WeatherSnapshot, deserialize_forecast, serialize_forecast};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    if let Ok(snapshot) = WeatherSnapshot::from_json(text) {
        snapshot.to_json().expect("a loaded snapshot serialises");
        let _ = snapshot.to_current_weather();
    }
    if let Ok(forecast) = deserialize_forecast(text) {
        serialize_forecast(&forecast).expect("a loaded forecast serialises");
    }
});
