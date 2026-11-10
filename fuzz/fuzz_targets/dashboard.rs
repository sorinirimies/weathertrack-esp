#![no_main]
//! The dashboard is built from text that arrives over the network (place names, condition text,
//! alert descriptions). Whatever it contains: never panic, escape everything, and stay small
//! enough for a microcontroller to build.

use libfuzzer_sys::fuzz_target;
use weathertrack_view::{ViewModel, html, sample};

fuzz_target!(|input: (String, String, String, String, u8, u8)| {
    let (name, event, description, condition, hours, days) = input;
    let mut vm = ViewModel::from_model(&sample::model(true), 24);
    vm.place.name = name;
    vm.place.source = condition.clone();
    if let Some(c) = vm.current.as_mut() {
        c.condition = condition;
    }
    if let Some(a) = vm.alerts.first_mut() {
        a.event = event;
        a.description = description;
    }
    vm.hours.truncate(usize::from(hours) % 25);
    vm.days.truncate(usize::from(days) % 8);
    let page = html::dashboard(&vm);
    assert!(!page.to_ascii_lowercase().contains("<script"), "raw script tag in the page");
    assert!(page.starts_with("<!doctype html>") && page.ends_with("</html>"));
});
