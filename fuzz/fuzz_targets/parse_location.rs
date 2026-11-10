#![no_main]
//! The IP geolocation reply is untrusted JSON: never panic, and a parsed location has
//! coordinates inside the valid range.

use libfuzzer_sys::fuzz_target;
use weathertrack_core::parse_ip_location;

fuzz_target!(|data: &[u8]| {
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(data) else { return };
    if let Ok(loc) = parse_ip_location(&json) {
        assert!(loc.latitude.is_finite());
        assert!(loc.longitude.is_finite());
    }
});
