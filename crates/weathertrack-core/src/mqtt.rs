//! The MQTT side of a device, as pure functions and plain data: the topic names, what it
//! publishes, the commands it accepts, and the activity state that a status screen shows.
//! The firmware does the networking (a blocking client over a TCP socket, in the HAL).
//!
//! Topics, with `<id>` the device id (for example `c6-a1b2`):
//!
//! | topic | direction | payload |
//! |---|---|---|
//! | `weathertrack/<id>/status` | published, retained | `online`, or `offline` (the will) |
//! | `weathertrack/<id>/weather` | published, retained | JSON: place, temperature, humidity, ... |
//! | `weathertrack/<id>/command` | subscribed | `refresh` or `status` |

use crate::app::Model;

/// The default MQTT port.
pub const DEFAULT_PORT: u16 = 1883;
/// Payload of the retained status topic while the device is connected.
pub const ONLINE: &str = "online";
/// Payload of the retained status topic after the connection was lost (the will message).
pub const OFFLINE: &str = "offline";
/// Longest topic / payload kept for display, in characters.
pub const SEEN_MAX: usize = 48;

/// Where the broker is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl MqttConfig {
    /// Settings from the build-time environment (`config.toml`); `None` without a host.
    pub fn from_settings(
        host: Option<&str>,
        port: Option<&str>,
        user: Option<&str>,
        pass: Option<&str>,
    ) -> Option<Self> {
        let host = host?.trim();
        if host.is_empty() {
            return None;
        }
        let nonempty = |s: Option<&str>| {
            s.map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        Some(Self {
            host: host.to_string(),
            port: port
                .and_then(|p| p.trim().parse().ok())
                .filter(|p| *p != 0)
                .unwrap_or(DEFAULT_PORT),
            username: nonempty(user),
            password: nonempty(pass),
        })
    }

    /// `host:port`.
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// The topic names of one device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topics {
    base: String,
}

impl Topics {
    pub fn new(device_id: &str) -> Self {
        Self {
            base: format!("weathertrack/{device_id}"),
        }
    }

    pub fn status(&self) -> String {
        format!("{}/status", self.base)
    }

    pub fn weather(&self) -> String {
        format!("{}/weather", self.base)
    }

    pub fn command(&self) -> String {
        format!("{}/command", self.base)
    }
}

/// The current weather as the JSON payload of the weather topic; `None` before the first forecast.
pub fn weather_payload(model: &Model) -> Option<String> {
    let c = model.forecast.as_ref()?.current.as_ref()?;
    let round = |v: f64| (v * 10.0).round() / 10.0;
    Some(
        serde_json::json!({
            "place": model.location,
            "temperature": round(c.temperature),
            "feels_like": round(c.feels_like),
            "humidity": c.humidity.round(),
            "pressure": c.pressure.round(),
            "wind_kmh": round(c.wind_speed),
            "condition": c.description,
            "updated": c.timestamp,
        })
        .to_string(),
    )
}

/// What a message on the command topic asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Fetch the weather again now.
    Refresh,
    /// Publish the status and the weather again.
    Status,
}

/// Parse a command payload (case and surrounding spaces do not matter).
pub fn parse_command(payload: &[u8]) -> Option<Command> {
    match std::str::from_utf8(payload)
        .ok()?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "refresh" => Some(Command::Refresh),
        "status" => Some(Command::Status),
        _ => None,
    }
}

/// The state of the connection to the broker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Link {
    /// No broker configured.
    #[default]
    Off,
    Connecting,
    Connected,
    /// The last attempt or connection failed (why).
    Failed(String),
}

/// One message as shown on the screen (clipped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub topic: String,
    pub payload: String,
}

/// Clip to `max` characters, with an ellipsis when something was cut (never splits a character).
pub fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

impl Seen {
    pub fn new(topic: &str, payload: &[u8]) -> Self {
        Self {
            topic: clip(topic, SEEN_MAX),
            payload: clip(String::from_utf8_lossy(payload).trim(), SEEN_MAX),
        }
    }
}

/// What a status screen shows about the MQTT connection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Activity {
    pub link: Link,
    /// `host:port`, empty when MQTT is off.
    pub broker: String,
    /// Application messages sent and received.
    pub tx: u64,
    pub rx: u64,
    pub bytes_tx: u64,
    pub bytes_rx: u64,
    pub last_tx: Option<Seen>,
    pub last_rx: Option<Seen>,
    /// How many connections were made (a reconnect counts).
    pub connects: u32,
    /// Grows with every message in either direction: the screen compares it with the value it saw
    /// last to flash its activity dot.
    pub pulse: u64,
}

impl Activity {
    pub fn new(broker: &str) -> Self {
        Self {
            broker: broker.to_string(),
            link: Link::Connecting,
            ..Self::default()
        }
    }

    pub fn sent(&mut self, topic: &str, payload: &[u8]) {
        self.tx += 1;
        self.pulse += 1;
        self.last_tx = Some(Seen::new(topic, payload));
    }

    pub fn received(&mut self, topic: &str, payload: &[u8]) {
        self.rx += 1;
        self.pulse += 1;
        self.last_rx = Some(Seen::new(topic, payload));
    }

    /// The connection came up.
    pub fn connected(&mut self) {
        self.link = Link::Connected;
        self.connects += 1;
    }

    pub fn failed(&mut self, why: &str) {
        self.link = Link::Failed(clip(why, SEEN_MAX));
    }

    /// Short text for the state: what the status screen prints.
    pub fn state_text(&self) -> String {
        match &self.link {
            Link::Off => "MQTT off".to_string(),
            Link::Connecting => "Connecting".to_string(),
            Link::Connected => "Connected".to_string(),
            Link::Failed(why) => format!("Offline: {why}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Msg;

    #[test]
    fn the_config_needs_a_host_and_defaults_the_port() {
        assert_eq!(MqttConfig::from_settings(None, None, None, None), None);
        assert_eq!(
            MqttConfig::from_settings(Some("  "), None, None, None),
            None
        );
        let c = MqttConfig::from_settings(Some("192.168.1.23"), None, None, None).expect("config");
        assert_eq!(
            (c.port, c.username.clone(), c.address()),
            (1883, None, "192.168.1.23:1883".to_string())
        );
        let c =
            MqttConfig::from_settings(Some("broker.lan"), Some("8883"), Some("alice"), Some(""))
                .expect("config");
        assert_eq!(
            (c.port, c.username.as_deref(), c.password),
            (8883, Some("alice"), None)
        );
        let bad = MqttConfig::from_settings(Some("h"), Some("nope"), None, None).expect("config");
        assert_eq!(bad.port, 1883, "a bad port falls back to the default");
        assert_eq!(
            MqttConfig::from_settings(Some("h"), Some("0"), None, None)
                .expect("config")
                .port,
            1883
        );
    }

    #[test]
    fn topics_are_namespaced_by_device() {
        let t = Topics::new("c6-a1b2");
        assert_eq!(t.status(), "weathertrack/c6-a1b2/status");
        assert_eq!(t.weather(), "weathertrack/c6-a1b2/weather");
        assert_eq!(t.command(), "weathertrack/c6-a1b2/command");
        assert_ne!(Topics::new("c6-0000").status(), t.status());
    }

    #[test]
    fn the_weather_payload_is_json_with_the_current_values() {
        let mut model = Model::new("Stuttgart", 48.78, 9.18);
        assert_eq!(weather_payload(&model), None, "no forecast yet");
        let sample = a_forecast();
        crate::app::update(&mut model, Msg::WeatherLoaded(Box::new(sample)));
        let json: serde_json::Value =
            serde_json::from_str(&weather_payload(&model).expect("payload")).expect("json");
        assert_eq!(json["place"], "Stuttgart");
        assert!(json["temperature"].is_number() && json["humidity"].is_number());
        assert!(json["condition"].as_str().is_some_and(|c| !c.is_empty()));
        let t = json["temperature"].as_f64().expect("number");
        assert_eq!((t * 10.0).round(), t * 10.0, "rounded to one decimal");
    }

    /// A forecast with only a current reading.
    fn a_forecast() -> crate::Forecast {
        let current = crate::CurrentWeather {
            timestamp: "2026-11-11T20:00".into(),
            temperature: 8.46,
            feels_like: 6.0,
            humidity: 91.4,
            pressure: 1009.2,
            wind_speed: 3.04,
            wind_direction: 180.0,
            wind_gusts: 5.0,
            condition: crate::WeatherCondition::Clouds,
            weather_code: 3,
            description: "Cloudy".into(),
            cloud_cover: 100.0,
            is_day: false,
            precipitation: 0.0,
            rain: 0.0,
            snowfall: 0.0,
            visibility: 10.0,
            uv_index: 0.0,
            sunrise: "07:25".into(),
            sunset: "16:48".into(),
        };
        crate::Forecast {
            current: Some(current),
            hourly: Vec::new(),
            daily: Vec::new(),
            timezone: "Europe/Berlin".into(),
            location: None,
        }
    }

    #[test]
    fn commands_ignore_case_and_spaces_and_unknown_ones_are_none() {
        assert_eq!(parse_command(b"refresh"), Some(Command::Refresh));
        assert_eq!(parse_command(b"  REFRESH\n"), Some(Command::Refresh));
        assert_eq!(parse_command(b"Status"), Some(Command::Status));
        assert_eq!(parse_command(b"reboot"), None);
        assert_eq!(parse_command(b""), None);
        assert_eq!(parse_command(&[0xFF, 0xFE]), None, "not UTF-8");
    }

    #[test]
    fn clipping_never_splits_a_character() {
        assert_eq!(clip("short", 10), "short");
        assert_eq!(clip("abcdefghij", 5), "abcd…");
        assert_eq!(clip("Zürich Fluntern", 8), "Zürich …");
        assert_eq!(clip("日本語のトピック", 4).chars().count(), 4);
        assert_eq!(clip("abc", 0), "…");
    }

    #[test]
    fn what_was_seen_is_clipped_and_lossy_for_binary() {
        let s = Seen::new("weathertrack/c6-a1b2/command", b"  refresh \n");
        assert_eq!(s.payload, "refresh");
        let long = Seen::new(&"t".repeat(100), &[b'x'; 100]);
        assert!(long.topic.chars().count() <= SEEN_MAX && long.payload.chars().count() <= SEEN_MAX);
        assert_eq!(Seen::new("t", &[0xFF]).payload, "\u{fffd}");
    }

    #[test]
    fn activity_counts_both_directions_and_pulses() {
        let mut a = Activity::new("192.168.1.23:1883");
        assert_eq!(
            (a.link.clone(), a.state_text()),
            (Link::Connecting, "Connecting".to_string())
        );
        a.connected();
        a.sent("weathertrack/x/status", b"online");
        a.received("weathertrack/x/command", b"refresh");
        a.sent("weathertrack/x/weather", b"{}");
        assert_eq!((a.tx, a.rx, a.pulse, a.connects), (2, 1, 3, 1));
        assert_eq!(a.last_tx.as_ref().map(|s| s.payload.as_str()), Some("{}"));
        assert_eq!(
            a.last_rx.as_ref().map(|s| s.topic.as_str()),
            Some("weathertrack/x/command")
        );
        assert_eq!(a.state_text(), "Connected");
        a.failed("connection refused");
        assert_eq!(a.state_text(), "Offline: connection refused");
        a.connected();
        assert_eq!(a.connects, 2, "a reconnect counts");
    }

    #[test]
    fn without_a_broker_the_state_says_off() {
        let a = Activity::default();
        assert_eq!(a.link, Link::Off);
        assert_eq!(a.state_text(), "MQTT off");
    }
}
