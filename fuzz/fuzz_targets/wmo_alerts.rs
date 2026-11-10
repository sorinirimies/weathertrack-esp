#![no_main]
//! Every WMO code maps to a condition and a description, and the alert generator
//! accepts any finite or non-finite reading without panicking.

use libfuzzer_sys::fuzz_target;
use weathertrack_core::{wmo_code_to_condition, wmo_code_to_description};
use weathertrack_core::wmo::generate_alerts_from_conditions;

fuzz_target!(|input: (u8, f64, f64, f64, bool)| {
    let (code, wind, temp, uv, is_day) = input;
    let _ = wmo_code_to_condition(code);
    let desc = wmo_code_to_description(code, is_day);
    assert!(!desc.description.is_empty());
    let _ = generate_alerts_from_conditions(code, wind, temp, uv);
});
