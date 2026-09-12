//! MOIFEU-WEBSOCKET — Real-time WebSocket Transport for Moifeu Protocol
//!
//! A WebSocket-based transport layer for the Moifeu protocol,
//! enabling real-time beam streaming between Zed agents.
//!
//! ## Features
//!
//! - WebSocket message format for Moifeu beam transport
//! - Message serialization/deserialization
//! - WebSocket server for receiving beams
//! - WebSocket client for sending beams
//! - Handler trait for custom beam processing
//! - Broadcast and targeted messaging
//! - Channel subscription support

use crate::moifeu::{MoifeuBeam, ZedMoifeuBeam};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// Conditional imports for network layer
#[cfg(feature = "websocket-network")]
use {
    futures_util::{SinkExt, StreamExt},
    std::collections::HashMap,
    std::net::SocketAddr,
    std::sync::Arc,
    tokio::net::{TcpListener, TcpStream},
    tokio::sync::{broadcast, mpsc, Mutex as AsyncMutex},
    tokio_tungstenite::{accept_async, connect_async, MaybeTlsStream, WebSocketStream},
    url::Url,
};

/// Generate a unique message ID
fn generate_message_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    format!("msg_{}", timestamp)
}

/// Get current timestamp as RFC3339 string
fn timestamp_rfc3339() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let micros = now.subsec_micros();
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z", 1970, 1, 1, hours as u32, minutes as u32, seconds as u32, micros)
}

// ============================================================================
// MESSAGE TYPES
// ============================================================================

/// WebSocket message type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum MoifeuWebSocketMessageType {
    #[default]
    Beam,
    Batch,
    Ping,
    Pong,
    Ack,
    Error,
    Subscribe,
    Unsubscribe,
}

/// WebSocket message wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoifeuWebSocketMessage {
    #[serde(default)]
    pub message_type: MoifeuWebSocketMessageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beam: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beams: Option<Vec<String>>,
    pub seat_id: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_seat_id: Option<u8>,
    #[serde(default = "generate_message_id")]
    pub message_id: String,
    #[serde(default = "timestamp_rfc3339")]
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
}

impl MoifeuWebSocketMessage {
    pub fn new_beam(beam: String, seat_id: u8, target_seat_id: Option<u8>) -> Self {
        Self {
            message_type: MoifeuWebSocketMessageType::Beam,
            beam: Some(beam),
            beams: None,
            seat_id,
            target_seat_id,
            message_id: generate_message_id(),
            timestamp: timestamp_rfc3339(),
            priority: None,
            error: None,
            channel: None,
        }
    }

    pub fn new_batch(beams: Vec<String>, seat_id: u8, target_seat_id: Option<u8>) -> Self {
        Self {
            message_type: MoifeuWebSocketMessageType::Batch,
            beam: None,
            beams: Some(beams),
            seat_id,
            target_seat_id,
            message_id: generate_message_id(),
            timestamp: timestamp_rfc3339(),
            priority: None,
            error: None,
            channel: None,
        }
    }

    pub fn new_zed_beam(zed_beam: &ZedMoifeuBeam, seat_id: u8, target_seat_id: Option<u8>) -> Self {
        Self::new_beam(zed_beam.beam.raw.clone(), seat_id, target_seat_id)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn get_all_beams(&self) -> Vec<String> {
        match &self.message_type {
            MoifeuWebSocketMessageType::Beam => self.beam.clone().map(|b| vec![b]).unwrap_or_default(),
            MoifeuWebSocketMessageType::Batch => self.beams.clone().unwrap_or_default(),
            _ => vec![],
        }
    }
}

// ============================================================================
// CONFIGURATION
// ============================================================================

#[derive(Debug, Clone)]
pub struct MoifeuWebSocketConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub message_buffer_size: usize,
}

impl Default for MoifeuWebSocketConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            max_connections: 100,
            message_buffer_size: 1000,
        }
    }
}

impl MoifeuWebSocketConfig {
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
    pub fn ws_url(&self) -> String {
        format!("ws://{}", self.server_addr())
    }
}

// ============================================================================
// HANDLER TRAIT
// ============================================================================

pub trait MoifeuWebSocketHandler: Send + Sync {
    fn handle_beam(&self, beam: MoifeuBeam, source_seat: u8, target_seat: Option<u8>);
    fn handle_connect(&self, seat_id: u8);
    fn handle_disconnect(&self, seat_id: u8);
}

#[derive(Debug, Clone)]
pub struct DefaultMoifeuWebSocketHandler;

impl MoifeuWebSocketHandler for DefaultMoifeuWebSocketHandler {
    fn handle_beam(&self, beam: MoifeuBeam, source_seat: u8, _target_seat: Option<u8>) {
        eprintln!("[MoifeuWebSocket] Beam from seat {}: {}", source_seat, beam.raw);
    }
    fn handle_connect(&self, seat_id: u8) {
        eprintln!("[MoifeuWebSocket] Seat {} connected", seat_id);
    }
    fn handle_disconnect(&self, seat_id: u8) {
        eprintln!("[MoifeuWebSocket] Seat {} disconnected", seat_id);
    }
}

// ============================================================================
// INTEGRATION HELPERS
// ============================================================================

pub mod spacetime_integration {
    use super::*;

    pub fn beam_to_websocket_message(
        zed_beam: &ZedMoifeuBeam,
        seat_id: u8,
        target_seat_id: Option<u8>,
    ) -> MoifeuWebSocketMessage {
        MoifeuWebSocketMessage::new_zed_beam(zed_beam, seat_id, target_seat_id)
    }

    pub fn raw_beam_to_websocket_message(
        beam: &str,
        seat_id: u8,
        target_seat_id: Option<u8>,
    ) -> MoifeuWebSocketMessage {
        MoifeuWebSocketMessage::new_beam(beam.to_string(), seat_id, target_seat_id)
    }

    pub fn to_moifeu_beam(msg: &MoifeuWebSocketMessage) -> Option<MoifeuBeam> {
        use crate::moifeu::parse_beam;
        msg.get_all_beams().into_iter().filter_map(|s| parse_beam(&s).ok()).next()
    }
}

pub use spacetime_integration::*;

// ============================================================================
// NETWORK LAYER (with websocket-network feature)
// ============================================================================

/// Client information
#[cfg(feature = "websocket-network")]
#[derive(Debug)]
pub struct WebSocketClientInfo {
    pub seat_id: u8,
    pub subscribed_channels: Vec<String>,
}

/// Server state
#[cfg(feature = "websocket-network")]
#[derive(Debug)]
pub struct WebSocketServerState {
    pub clients: HashMap<u64, WebSocketClientInfo>,
}

#[cfg(feature = "websocket-network")]
impl WebSocketServerState {
    pub fn new() -> Self {
        Self { clients: HashMap::new() }
    }
}

/// WebSocket server
#[cfg(feature = "websocket-network")]
pub struct MoifeuWebSocketServer {
    config: MoifeuWebSocketConfig,
    handler: Arc<dyn MoifeuWebSocketHandler + Send + Sync>,
    state: Arc<AsyncMutex<WebSocketServerState>>,
    broadcast_tx: broadcast::Sender<MoifeuWebSocketMessage>,
}

#[cfg(feature = "websocket-network")]
impl MoifeuWebSocketServer {
    pub fn new(config: MoifeuWebSocketConfig, handler: impl MoifeuWebSocketHandler + Send + Sync + 'static) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        Self {
            config,
            handler: Arc::new(handler),
            state: Arc::new(AsyncMutex::new(WebSocketServerState::new())),
            broadcast_tx,
        }
    }

    pub fn with_default(config: MoifeuWebSocketConfig) -> Self {
        Self::new(config, DefaultMoifeuWebSocketHandler)
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(self.config.server_addr()).await?;
        eprintln!("[MoifeuWebSocket] Server on {}", self.config.server_addr());

        while let Ok((stream, _)) = listener.accept().await {
            let handler = self.handler.clone();
            let state = self.state.clone();
            let broadcast_tx = self.broadcast_tx.clone();
            let broadcast_rx = broadcast_tx.subscribe();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(
                    stream, state, broadcast_tx, broadcast_rx, handler,
                ).await {
                    eprintln!("[MoifeuWebSocket] Connection error: {}", e);
                }
            });
        }
        Ok(())
    }

    async fn handle_connection(
        stream: TcpStream,
        state: Arc<AsyncMutex<WebSocketServerState>>,
        broadcast_tx: broadcast::Sender<MoifeuWebSocketMessage>,
        mut broadcast_rx: broadcast::Receiver<MoifeuWebSocketMessage>,
        handler: Arc<dyn MoifeuWebSocketHandler + Send + Sync>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ws_stream = accept_async(stream).await?;
        let (mut write, mut read) = ws_stream.split();
        let client_id = rand::random::<u64>();

        let mut seat_id: Option<u8> = None;
        let mut subscribed_channels: Vec<String> = Vec::new();

        // Outgoing broadcast forwarder
        let write_task = tokio::spawn(async move {
            while let Ok(msg) = broadcast_rx.recv().await {
                let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(msg.to_json().unwrap_or_default())).await;
            }
        });

        // Incoming message handler
        while let Some(Ok(msg)) = read.next().await {
            if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                if let Ok(mut moifeu_msg) = MoifeuWebSocketMessage::from_json(&text) {
                    // First message sets seat_id
                    if seat_id.is_none() {
                        seat_id = Some(moifeu_msg.seat_id);
                        state.lock().await.clients.insert(client_id, WebSocketClientInfo {
                            seat_id: moifeu_msg.seat_id,
                            subscribed_channels: vec![],
                        });
                        handler.handle_connect(moifeu_msg.seat_id);
                    }

                    match moifeu_msg.message_type {
                        MoifeuWebSocketMessageType::Ping => {
                            let pong = MoifeuWebSocketMessage {
                                message_type: MoifeuWebSocketMessageType::Pong,
                                message_id: moifeu_msg.message_id,
                                seat_id: moifeu_msg.seat_id,
                                ..Default::default()
                            };
                            let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(pong.to_json()?)).await;
                        }
                        MoifeuWebSocketMessageType::Beam | MoifeuWebSocketMessageType::Batch => {
                            if let Some(seat) = seat_id {
                                for beam in moifeu_msg.get_all_beams() {
                                    if let Ok(parsed) = crate::moifeu::parse_beam(&beam) {
                                        handler.handle_beam(parsed, seat, moifeu_msg.target_seat_id);
                                    }
                                }
                                let _ = broadcast_tx.send(moifeu_msg);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Cleanup
        {
            let mut state = state.lock().await;
            if let Some(sid) = seat_id {
                state.clients.remove(&client_id);
                handler.handle_disconnect(sid);
            }
        }
        write_task.abort();
        Ok(())
    }

    pub async fn broadcast(&self, message: MoifeuWebSocketMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.broadcast_tx.send(message)?;
        Ok(())
    }

    pub async fn client_count(&self) -> usize {
        self.state.lock().await.clients.len()
    }
}

/// WebSocket client
#[cfg(feature = "websocket-network")]
pub struct MoifeuWebSocketClient {
    seat_id: u8,
    config: MoifeuWebSocketConfig,
    sender: Option<mpsc::Sender<MoifeuWebSocketMessage>>,
    receiver: Option<mpsc::Receiver<MoifeuWebSocketMessage>>,
    handler: Arc<dyn MoifeuWebSocketHandler + Send + Sync>,
}

#[cfg(feature = "websocket-network")]
impl MoifeuWebSocketClient {
    pub fn new(config: MoifeuWebSocketConfig, seat_id: u8, handler: impl MoifeuWebSocketHandler + Send + Sync + 'static) -> Self {
        Self {
            seat_id,
            config,
            sender: None,
            receiver: None,
            handler: Arc::new(handler),
        }
    }

    pub fn with_default(config: MoifeuWebSocketConfig, seat_id: u8) -> Self {
        Self::new(config, seat_id, DefaultMoifeuWebSocketHandler)
    }

    pub async fn connect(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = Url::parse(&self.config.ws_url())?;
        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        let (tx, rx) = mpsc::channel(self.config.message_buffer_size);
        self.sender = Some(tx.clone());
        self.receiver = Some(rx);
        let handler = self.handler.clone();
        let seat_id = self.seat_id;

        tokio::spawn(async move {
            while let Some(Ok(msg)) = read.next().await {
                if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                    if let Ok(moifeu_msg) = MoifeuWebSocketMessage::from_json(&text) {
                        match moifeu_msg.message_type {
                            MoifeuWebSocketMessageType::Ping => {
                                let pong = MoifeuWebSocketMessage {
                                    message_type: MoifeuWebSocketMessageType::Pong,
                                    message_id: moifeu_msg.message_id,
                                    seat_id,
                                    ..Default::default()
                                };
                                let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(pong.to_json().unwrap_or_default())).await;
                            }
                            MoifeuWebSocketMessageType::Beam | MoifeuWebSocketMessageType::Batch => {
                                for beam in moifeu_msg.get_all_beams() {
                                    if let Ok(parsed) = crate::moifeu::parse_beam(&beam) {
                                        handler.handle_beam(parsed, moifeu_msg.seat_id, moifeu_msg.target_seat_id);
                                    }
                                }
                                let _ = tx.send(moifeu_msg).await;
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        // Send connection message
        let connect_msg = MoifeuWebSocketMessage::new_beam(
            format!("[0000000]|connected_seat_{}", self.seat_id),
            self.seat_id,
            None,
        );
        write.send(tokio_tungstenite::tungstenite::Message::Text(connect_msg.to_json()?)).await?;
        handler.handle_connect(self.seat_id);

        Ok(())
    }

    pub async fn send_beam(&self, beam: String, target_seat_id: Option<u8>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(sender) = &self.sender {
            sender.send(MoifeuWebSocketMessage::new_beam(beam, self.seat_id, target_seat_id)).await?;
        }
        Ok(())
    }

    pub async fn send_zed_beam(&self, zed_beam: ZedMoifeuBeam, target_seat_id: Option<u8>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.send_beam(zed_beam.beam.raw.clone(), target_seat_id).await
    }

    pub async fn receive(&mut self) -> Option<MoifeuWebSocketMessage> {
        self.receiver.as_mut()?.recv().await
    }

    pub async fn disconnect(&mut self) {
        self.sender = None;
        self.receiver = None;
    }

    pub fn seat_id(&self) -> u8 {
        self.seat_id
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = MoifeuWebSocketMessage::new_beam("[0000000]|test".to_string(), 1, None);
        assert_eq!(msg.seat_id, 1);
        assert_eq!(msg.message_type, MoifeuWebSocketMessageType::Beam);
    }

    #[test]
    fn test_message_serialization() {
        let msg = MoifeuWebSocketMessage::new_beam("[0000000]|test".to_string(), 1, None);
        let json = msg.to_json().unwrap();
        let parsed = MoifeuWebSocketMessage::from_json(&json).unwrap();
        assert_eq!(parsed.seat_id, 1);
    }

    #[test]
    fn test_batch() {
        let beams = vec!["[0000000]|a".to_string(), "[0000000]|b".to_string()];
        let msg = MoifeuWebSocketMessage::new_batch(beams.clone(), 1, None);
        assert_eq!(msg.get_all_beams(), beams);
    }

    #[test]
    fn test_config() {
        let config = MoifeuWebSocketConfig::default();
        assert_eq!(config.port, 8080);
    }
}
