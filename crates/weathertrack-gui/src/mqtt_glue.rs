//! The MQTT connection as the main loop uses it. With the `mqtt` feature it starts the HAL's MQTT
//! thread (when `config.toml` has a `[mqtt]` host), publishes the weather, hands over commands and
//! keeps the MQTT page up to date. Without the feature (or without a broker) every call does nothing
//! and the page does not exist.

use weathertrack_core::app::Model;
use weathertrack_core::mqtt::Command;

use crate::WeatherApp;

#[cfg(feature = "mqtt")]
mod real {
    use super::*;
    use crate::MqttData;
    use slint::SharedString;
    use weathertrack_core::mqtt::{weather_payload, Activity, Link, MqttConfig};
    use weathertrack_hal::mqtt::Mqtt;

    /// How long the activity dot stays lit after a message.
    const PULSE_MS: u32 = 350;

    pub struct Glue {
        mqtt: Option<Mqtt>,
        shown: Option<MqttData>,
        last_pulse: u64,
        pulse_since: Option<u32>,
    }

    fn data(a: &Activity, pulse: bool) -> MqttData {
        let seen = |s: &Option<weathertrack_core::mqtt::Seen>| {
            s.as_ref().map(|s| format!("{}: {}", short_topic(&s.topic), s.payload)).unwrap_or_default()
        };
        MqttData {
            state: match a.link {
                Link::Off => 0,
                Link::Connecting => 1,
                Link::Connected => 2,
                Link::Failed(_) => 3,
            },
            state_text: SharedString::from(a.state_text()),
            broker: SharedString::from(a.broker.as_str()),
            sent: i32::try_from(a.tx).unwrap_or(i32::MAX),
            received: i32::try_from(a.rx).unwrap_or(i32::MAX),
            last_sent: SharedString::from(seen(&a.last_tx)),
            last_received: SharedString::from(seen(&a.last_rx)),
            connects: i32::try_from(a.connects).unwrap_or(i32::MAX),
            pulse,
        }
    }

    /// `weathertrack/c6-a1b2/weather` -> `weather` (the device part is the same on every line).
    fn short_topic(topic: &str) -> &str {
        topic.rsplit('/').next().unwrap_or(topic)
    }

    impl Glue {
        pub fn start(mac: Option<[u8; 6]>) -> Self {
            let config = MqttConfig::from_settings(
                option_env!("MQTT_HOST"),
                option_env!("MQTT_PORT"),
                option_env!("MQTT_USER"),
                option_env!("MQTT_PASS"),
            );
            let mqtt = config.map(|c| {
                let id = mac.map_or_else(|| "c6".to_string(), |m| format!("c6-{:02x}{:02x}", m[4], m[5]));
                Mqtt::new(c, &id)
            });
            Self { mqtt, shown: None, last_pulse: 0, pulse_since: None }
        }

        pub fn enabled(&self) -> bool {
            self.mqtt.is_some()
        }

        /// Free the connection's memory for the weather fetch (HTTPS) ...
        pub fn pause(&mut self) {
            if let Some(m) = self.mqtt.as_mut() {
                m.pause();
            }
        }

        /// ... and connect again after it.
        pub fn resume(&mut self) {
            if let Some(m) = self.mqtt.as_mut() {
                m.resume();
            }
        }

        pub fn publish_weather(&mut self, model: &Model) {
            if let (Some(m), Some(json)) = (self.mqtt.as_mut(), weather_payload(model)) {
                m.publish_weather(json);
            }
        }

        /// Refresh the page, and return a command that arrived.
        pub fn update(&mut self, app: &WeatherApp, now_ms: u32) -> Option<Command> {
            let m = self.mqtt.as_mut()?;
            m.tick(now_ms);
            let activity = m.activity().clone();
            if activity.pulse != self.last_pulse {
                self.last_pulse = activity.pulse;
                self.pulse_since = Some(now_ms);
            }
            let pulse = self.pulse_since.is_some_and(|t| now_ms.wrapping_sub(t) < PULSE_MS);
            let now = data(&activity, pulse);
            if self.shown.as_ref() != Some(&now) {
                app.set_mqtt(now.clone());
                self.shown = Some(now);
            }
            let command = m.take_command();
            if command == Some(Command::Status) {
                m.publish_status();
            }
            command
        }
    }
}

#[cfg(feature = "mqtt")]
pub use real::Glue;

#[cfg(not(feature = "mqtt"))]
pub struct Glue;

#[cfg(not(feature = "mqtt"))]
impl Glue {
    pub fn start(_mac: Option<[u8; 6]>) -> Self {
        Self
    }

    pub fn enabled(&self) -> bool {
        false
    }

    pub fn pause(&mut self) {}

    pub fn resume(&mut self) {}

    pub fn publish_weather(&mut self, _model: &Model) {}

    pub fn update(&mut self, _app: &WeatherApp, _now_ms: u32) -> Option<Command> {
        None
    }
}
