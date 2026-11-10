#![no_main]
//! Open-Meteo responses come from the network: arbitrary JSON must never panic the
//! parsers, and whatever parses must survive the cache round trip unchanged.

use libfuzzer_sys::fuzz_target;
use weathertrack_core::{deserialize_forecast, parse_current, parse_forecast, serialize_forecast};

fuzz_target!(|data: &[u8]| {
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(data) else { return };
    let _ = parse_current(&json);
    if let Ok(forecast) = parse_forecast(&json) {
        let text = serialize_forecast(&forecast).expect("a parsed forecast serialises");
        let back = deserialize_forecast(&text).expect("a serialised forecast parses");
        assert_eq!(back, forecast);
    }
});
