# Moifeu WebSocket Module

A real-time WebSocket transport layer for the Moifeu protocol, enabling WebSocket-based communication between Zed agents.

## Overview

This module provides:
- WebSocket message format for Moifeu beam transport
- Message serialization/deserialization
- Configuration for WebSocket servers/clients
- Handler trait for processing incoming beams
- Integration helpers for ZedMoifeuBeam

## Features

### Message Format

```json
{
  "message_type": "beam",
  "beam": "[0000000]|hello",
  "seat_id": 1,
  "target_seat_id": 2,
  "message_id": "msg_123456789",
  "timestamp": "1970-01-01T12:34:56.789000Z"
}
```

### Core Types

- `MoifeuWebSocketMessage` - Main message wrapper
- `MoifeuWebSocketConfig` - Server configuration
- `MoifeuWebSocketHandler` - Trait for beam handlers
- `DefaultMoifeuWebSocketHandler` - Default logging handler

### Integration with Zed

The module integrates seamlessly with the existing Moifeu protocol:

```rust
use crate::moifeu::{MoifeuBeam, ZedMoifeuBeam};
use crate::moifeu_websocket::{MoifeuWebSocketMessage, MoifeuWebSocketConfig};

// Create a message from a ZedMoifeuBeam
let zed_beam = ZedMoifeuBeam::from_moifeu(beam);
let message = MoifeuWebSocketMessage::new_zed_beam(&zed_beam, 1, None);

// Serialize to JSON for WebSocket transmission
let json = message.to_json()?;

// Parse incoming message
let msg: MoifeuWebSocketMessage = serde_json::from_str(&json)?;
```

## Usage

### Basic Usage

```rust
// Create a message
let msg = MoifeuWebSocketMessage::new_beam(
    "[0000000]|test message".to_string(),
    1,  // source seat
    Some(2),  // target seat (optional)
);

// Serialize
let json = msg.to_json().unwrap();

// Deserialize
let parsed = MoifeuWebSocketMessage::from_json(&json).unwrap();
```

### With Handler

```rust
use crate::moifeu_websocket::{MoifeuWebSocketHandler, DefaultMoifeuWebSocketHandler};

let handler = DefaultMoifeuWebSocketHandler;

// Handle incoming beam (called by WebSocket server)
handler.handle_beam(beam, source_seat, target_seat);
handler.handle_connect(seat_id);
handler.handle_disconnect(seat_id);
```

### Custom Handler

```rust
use crate::moifeu::{MoifeuBeam, ZedMoifeuBeam};
use crate::moifeu_websocket::MoifeuWebSocketHandler;

struct MyHandler;

impl MoifeuWebSocketHandler for MyHandler {
    fn handle_beam(&self, beam: MoifeuBeam, source_seat: u8, target_seat: Option<u8>) {
        // Custom beam handling logic
        println!("Got beam from {}: {}", source_seat, beam.raw);
    }
    
    fn handle_connect(&self, seat_id: u8) {
        println!("Seat {} connected", seat_id);
    }
    
    fn handle_disconnect(&self, seat_id: u8) {
        println!("Seat {} disconnected", seat_id);
    }
}
```

## Configuration

```rust
use crate::moifeu_websocket::MoifeuWebSocketConfig;

let config = MoifeuWebSocketConfig {
    host: "127.0.0.1".to_string(),
    port: 8080,
    max_connections: 100,
    message_buffer_size: 1000,
};

// Or use defaults
let config = MoifeuWebSocketConfig::default();
```

## SpacetimeDB Integration

The `spacetime_integration` submodule provides helpers for working with SpacetimeDB:

```rust
use crate::moifeu_websocket::spacetime_integration::{beam_to_websocket_message, raw_beam_to_websocket_message};

// Convert ZedMoifeuBeam to WebSocket message
let ws_msg = beam_to_websocket_message(&zed_beam, seat_id);

// Convert raw beam string to WebSocket message
let ws_msg = raw_beam_to_websocket_message(&raw_beam, seat_id);
```

## Testing

```bash
cargo test --features websocket
```

## Feature Flag

This module is available when the `websocket` feature is enabled:

```toml
[features]
websocket = []
```

To use:

```bash
cargo build --features websocket
```

## Dependencies

- serde (for serialization)
- serde_json (for JSON encoding)
- std::time (for timestamps)

No external crates are required beyond the standard library and serde.

## License

MIT
