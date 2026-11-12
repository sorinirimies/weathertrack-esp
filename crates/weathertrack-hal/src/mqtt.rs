//! The device's MQTT connection, driven by the main loop: call [`Mqtt::tick`] every iteration. It
//! connects (and reconnects with a growing pause), publishes what was queued, polls for commands,
//! and keeps the activity counters a status screen shows.
//!
//! It uses no thread on purpose: the C6 has no PSRAM, and a thread's stack plus the socket buffers
//! left too little heap for the HTTPS weather fetch (`pause` closes the connection around it).
//! What goes over the wire (topics, payloads, commands) is in `weathertrack_core::mqtt`, tested on
//! the host; this file is only the socket.

use std::collections::VecDeque;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use mqtt_client_blocking::{Client, Error, Options, QoS, WillMessage};
use weathertrack_core::mqtt::{parse_command, Activity, Command, Link, MqttConfig, Topics, OFFLINE, ONLINE};

/// First pause before reconnecting; it doubles up to [`MAX_BACKOFF`].
const MIN_BACKOFF_MS: u32 = 2_000;
const MAX_BACKOFF_MS: u32 = 30_000;
/// Free heap and largest block, for the log: the C6 runs close to its limit.
fn heap() -> String {
    // SAFETY: plain ESP-IDF heap statistics getters.
    let (free, largest) = unsafe {
        (
            esp_idf_svc::sys::esp_get_free_heap_size(),
            esp_idf_svc::sys::heap_caps_get_largest_free_block(esp_idf_svc::sys::MALLOC_CAP_8BIT),
        )
    };
    format!("free {free}, largest block {largest}")
}

/// How long a connection attempt may block the main loop.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(1_500);

pub struct Mqtt {
    config: MqttConfig,
    client_id: String,
    topics: Topics,
    client: Option<Client<TcpStream>>,
    activity: Activity,
    commands: VecDeque<Command>,
    /// Messages to publish: topic, payload, retain.
    outbox: VecDeque<(String, Vec<u8>, bool)>,
    paused: bool,
    next_attempt_ms: u32,
    backoff_ms: u32,
}

impl Mqtt {
    /// `device_id` names the topics and the MQTT client id. Nothing is connected until [`tick`](Self::tick).
    pub fn new(config: MqttConfig, device_id: &str) -> Self {
        Self {
            activity: Activity::new(&config.address()),
            client_id: format!("weathertrack-{device_id}"),
            topics: Topics::new(device_id),
            config,
            client: None,
            commands: VecDeque::new(),
            outbox: VecDeque::new(),
            paused: false,
            next_attempt_ms: 0,
            backoff_ms: MIN_BACKOFF_MS,
        }
    }

    pub fn activity(&self) -> &Activity {
        &self.activity
    }

    /// The next command that arrived on the command topic, if any.
    pub fn take_command(&mut self) -> Option<Command> {
        self.commands.pop_front()
    }

    /// Queue the weather (JSON) for the retained weather topic (the newest one replaces older ones).
    pub fn publish_weather(&mut self, json: String) {
        let topic = self.topics.weather();
        self.outbox.retain(|(t, _, _)| *t != topic);
        self.outbox.push_back((topic, json.into_bytes(), true));
    }

    /// Queue the retained status again.
    pub fn publish_status(&mut self) {
        self.outbox.push_back((self.topics.status(), ONLINE.as_bytes().to_vec(), true));
    }

    /// Close the connection and hold off reconnecting until [`resume`](Self::resume): the HTTPS
    /// weather fetch needs the memory the connection holds. The goodbye is clean, so no will is
    /// sent (the retained status stays `online`).
    pub fn pause(&mut self) {
        self.paused = true;
        log::info!("mqtt: pausing, heap {}", heap());
        if let Some(client) = self.client.take() {
            let _ = client.disconnect();
        }
        log::info!("mqtt: paused, heap {}", heap());
    }

    /// Reconnect (at the next tick) after a [`pause`](Self::pause).
    pub fn resume(&mut self) {
        log::info!("mqtt: resuming, heap {}", heap());
        self.paused = false;
        self.next_attempt_ms = 0;
        self.activity.link = Link::Connecting;
    }

    /// One step: connect if due, send the queue, look for incoming messages. `now_ms` is any
    /// millisecond clock that only moves forward (wrapping is fine). Blocks for at most a
    /// connection attempt (1.5 s, and only every 2 to 30 s while the broker is unreachable);
    /// otherwise it returns within a few milliseconds.
    pub fn tick(&mut self, now_ms: u32) {
        if self.paused {
            return;
        }
        if self.client.is_none() {
            if now_ms.wrapping_sub(self.next_attempt_ms) as i32 >= 0 {
                self.connect(now_ms);
            }
            return;
        }
        if let Err(e) = self.run() {
            log::warn!("mqtt: {e}");
            self.activity.failed(&e.to_string());
            self.client = None; // drops the socket
            self.schedule_retry(now_ms);
        }
    }

    fn schedule_retry(&mut self, now_ms: u32) {
        self.next_attempt_ms = now_ms.wrapping_add(self.backoff_ms);
        self.backoff_ms = (self.backoff_ms * 2).min(MAX_BACKOFF_MS);
    }

    fn connect(&mut self, now_ms: u32) {
        self.activity.link = Link::Connecting;
        match self.open() {
            Ok(client) => {
                self.client = Some(client);
                self.activity.connected();
                self.backoff_ms = MIN_BACKOFF_MS;
                self.outbox.push_front((self.topics.status(), ONLINE.as_bytes().to_vec(), true));
                log::info!("mqtt: connected to {}, heap {}", self.config.address(), heap());
            }
            Err(e) => {
                log::warn!("mqtt: {e}");
                self.activity.failed(&e.to_string());
                self.schedule_retry(now_ms);
            }
        }
    }

    fn open(&self) -> Result<Client<TcpStream>, Error> {
        let addr = self
            .config
            .address()
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "the broker address does not resolve"))?;
        log::info!("mqtt: step 1 resolved, heap {}", heap());
        let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)?;
        log::info!("mqtt: step 2 tcp connected, heap {}", heap());
        stream.set_nodelay(true)?;
        let mut opts = Options::new(self.client_id.clone()).will(WillMessage {
            topic: self.topics.status(),
            payload: OFFLINE.as_bytes().to_vec(),
            qos: QoS::AtLeastOnce,
            retain: true,
        });
        if let (Some(user), Some(pass)) = (&self.config.username, &self.config.password) {
            opts = opts.credentials(user.clone(), pass.as_bytes());
        }
        let mut client = Client::connect(stream, opts)?;
        log::info!("mqtt: step 3 mqtt connected, heap {}", heap());
        client.subscribe(&self.topics.command(), QoS::AtLeastOnce)?;
        log::info!("mqtt: step 4 subscribed, heap {}", heap());
        Ok(client)
    }

    /// Publish the queue and read what arrived, on a live connection.
    fn run(&mut self) -> Result<(), Error> {
        let Some(client) = self.client.as_mut() else { return Ok(()) };
        while let Some((topic, payload, retain)) = self.outbox.pop_front() {
            client.publish(&topic, &payload, QoS::AtLeastOnce, retain)?;
            self.activity.sent(&topic, &payload);
        }
        if let Some(msg) = client.poll(Duration::from_millis(1))? {
            self.activity.received(&msg.topic, &msg.payload);
            if msg.topic == self.topics.command() {
                match parse_command(&msg.payload) {
                    Some(cmd) => self.commands.push_back(cmd),
                    None => log::info!("mqtt: unknown command {:?}", String::from_utf8_lossy(&msg.payload)),
                }
            }
        }
        let stats = client.stats();
        self.activity.bytes_tx = stats.bytes_tx;
        self.activity.bytes_rx = stats.bytes_rx;
        Ok(())
    }
}
