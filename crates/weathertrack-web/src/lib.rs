//! WeatherTrack web (Topcoat): weather for any place, and one place for all your ESP32 devices,
//! kept in sync.
//!
//! | module | what |
//! |---|---|
//! | [`app`] | server, router, host guard, background loop |
//! | [`net`] | HTTP access behind a trait |
//! | [`weather`] | forecast, network location and city search (Open-Meteo) |
//! | [`device`] | reading a device's state and pushing it a place |
//! | [`state`] | the hub: master place, devices, polling, syncing, persistence |
//! | [`testing`] | fakes (simulated devices) for the tests |

pub mod api;
pub mod app;
pub(crate) mod components;
pub mod device;
pub mod net;
mod pages;
pub mod state;
pub mod testing;
pub mod weather;
