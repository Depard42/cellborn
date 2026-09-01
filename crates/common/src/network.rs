//! Network constants shared by the client and the server.
//!
//! Both binaries must agree on these values: the netcode handshake fails if the
//! protocol id or the private key differ.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

/// Rate at which the `FixedUpdate` simulation runs.
pub const FIXED_TIMESTEP_HZ: f64 = 64.0;

/// How often the server sends replication updates to the clients.
pub const SEND_INTERVAL: Duration = Duration::from_millis(20);

/// Default port the server listens on.
pub const SERVER_PORT: u16 = 5555;

/// Identifies this game (and its protocol version) during the netcode handshake.
pub const PROTOCOL_ID: u64 = 0x0C_E1_1B_02;

/// Prototype-only key: a real deployment must keep this secret on the server and
/// hand out connect tokens from a backend instead.
pub const PRIVATE_KEY: [u8; 32] = [0; 32];

pub fn tick_duration() -> Duration {
    Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ)
}

/// Address the client connects to by default.
pub fn server_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), SERVER_PORT)
}

/// Address the server binds its UDP socket to.
pub fn server_bind_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), SERVER_PORT)
}

/// Address the client binds its UDP socket to (any interface, OS-assigned port).
pub fn client_bind_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
}
