# Minecraft Server Status Client (Rust)

This Rust crate provides functionality to perform Minecraft server status queries using the official Minecraft protocol handshake and status query packets.

It supports:
- Establishing TCP connections (with optional DNS resolving).
- Sending Minecraft handshake packets.
- Sending status query packets.
- Parsing the server's JSON status response into a Rust struct.

---

## Features

- Async/await based using [Tokio](https://tokio.rs/).
- Optional DNS resolving feature (enabled via `resolve` feature flag).
- Parses JSON server status into typed Rust structs.
- Timeout support on connections.

---

## Usage Example

```rust
use tokio::time::Duration;
use mc_ping::connection::Connection;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = ("example.com".to_string(), 25565);

    // Connect to the server with a timeout of 5 seconds
    let mut connection = Connection::new(addr)
    connection = connection.timeout(5000)?.connect().await?

    // Perform handshake and status query
    let status = connection.ping().await?;

    println!("Server Status: {:?}", status);

    Ok(())
}
