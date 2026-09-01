//! Поиск серверов в локальной сети.
//!
//! Клиент кричит в широковещательный адрес и слушает, кто отзовётся. Найденные
//! серверы показываются в меню подключения вместе с запомненными.
//!
//! **Почему широковещание, а не перебор адресов.** Перебирать /24 — это две
//! с половиной сотни пакетов на каждое обновление списка, и всё равно мимо,
//! если сеть не /24. Широковещание доходит до всех, кто слушает, одним
//! пакетом — ровно то, для чего оно есть.
//!
//! Сокет неблокирующий: сеть не должна задерживать кадр даже на миллисекунду.

use bevy::prelude::*;
use cellborn_common::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

/// Как часто рассылается запрос, пока открыт экран подключения.
const PROBE_INTERVAL: f32 = 1.2;

/// Найденный в сети сервер.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub address: SocketAddr,
    pub players: usize,
    pub organisms: usize,
    pub version: String,
    /// Сколько секунд назад он отзывался в последний раз.
    pub silent_for: f32,
}

impl Found {
    /// Строка для меню: адрес и то, что там происходит.
    pub fn label(&self) -> String {
        format!(
            "{}   игроков {}   особей {}   v{}",
            self.address, self.players, self.organisms, self.version
        )
    }
}

#[derive(Resource)]
pub struct Discovery {
    socket: Option<UdpSocket>,
    pub found: Vec<Found>,
    since_probe: f32,
}

impl Default for Discovery {
    fn default() -> Self {
        // Порт 0: система выдаст свободный. Слушать на фиксированном не нужно —
        // ответы приходят на тот же сокет, с которого ушёл запрос.
        let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))
            .ok()
            .and_then(|socket| {
                socket.set_nonblocking(true).ok()?;
                // Без этого широковещательный пакет не уйдёт вовсе.
                socket.set_broadcast(true).ok()?;
                Some(socket)
            });
        if socket.is_none() {
            warn!("не открылся сокет поиска: серверы придётся вводить адресом");
        }
        Self { socket, found: Vec::new(), since_probe: f32::MAX }
    }
}

impl Discovery {
    /// Забыть найденное: при входе на экран список начинается с чистого листа,
    /// иначе игрок увидит серверы, выключенные полчаса назад.
    pub fn reset(&mut self) {
        self.found.clear();
        self.since_probe = f32::MAX;
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Discovery>();
}

/// Рассылает запрос и собирает ответы. Зовётся, только пока открыт экран
/// подключения: молотить сеть во время игры незачем.
pub fn poll(discovery: &mut Discovery, dt: f32) {
    let Some(socket) = &discovery.socket else { return };

    discovery.since_probe += dt;
    if discovery.since_probe >= PROBE_INTERVAL {
        discovery.since_probe = 0.0;
        // Широковещательный адрес плюс явный localhost: сервер, запущенный на
        // этой же машине, до широковещания не всегда доходит.
        for target in [
            SocketAddr::from((Ipv4Addr::BROADCAST, DISCOVERY_PORT)),
            SocketAddr::from((Ipv4Addr::LOCALHOST, DISCOVERY_PORT)),
        ] {
            let _ = socket.send_to(DISCOVERY_PROBE, target);
        }
    }

    // Найденные серверы стареют: перестал отвечать — исчез из списка.
    for server in &mut discovery.found {
        server.silent_for += dt;
    }
    discovery.found.retain(|s| s.silent_for < DISCOVERY_TIMEOUT);

    let mut buffer = [0u8; 128];
    while let Ok((len, from)) = socket.recv_from(&mut buffer) {
        let Some(server) = parse_reply(&buffer[..len], from.ip()) else { continue };
        match discovery.found.iter_mut().find(|s| s.address == server.address) {
            Some(existing) => *existing = server,
            None => discovery.found.push(server),
        }
    }
    // Порядок стабильный, иначе строки в меню прыгают под курсором.
    discovery.found.sort_by_key(|s| s.address.to_string());
}

/// Разбирает ответ сервера. Мусор с этого порта игнорируется молча.
fn parse_reply(bytes: &[u8], from: IpAddr) -> Option<Found> {
    let text = std::str::from_utf8(bytes).ok()?;
    let rest = text.strip_prefix(std::str::from_utf8(DISCOVERY_REPLY).ok()?)?;
    let mut parts = rest.split('|');
    let port: u16 = parts.next()?.parse().ok()?;
    let players = parts.next()?.parse().unwrap_or(0);
    let organisms = parts.next()?.parse().unwrap_or(0);
    let version = parts.next().unwrap_or("?").to_string();
    Some(Found {
        // Порт берётся из ответа, а не из адреса отправителя: сервер отвечает
        // с порта поиска, а играть надо на игровом.
        address: SocketAddr::new(from, port),
        players,
        organisms,
        version,
        silent_for: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_is_understood_and_points_at_the_game_port() {
        let bytes = b"CELLBORN!15555|2|48|0.2.0";
        let server = parse_reply(bytes, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 7)))
            .expect("ответ не разобран");
        // Играть надо на игровом порту, а не на том, с которого пришёл ответ.
        assert_eq!(server.address.port(), 5555);
        assert_eq!(server.address.ip().to_string(), "192.168.1.7");
        assert_eq!(server.players, 2);
        assert_eq!(server.organisms, 48);
        assert_eq!(server.version, "0.2.0");
    }

    /// На этот порт может прилететь что угодно; падать от этого нельзя.
    #[test]
    fn rubbish_is_ignored_quietly() {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        for junk in [
            "привет".as_bytes(),
            &b"CELLBORN!1"[..],
            "CELLBORN!1не-число|1|1|x".as_bytes(),
            &[0xff, 0xfe, 0x00][..],
        ] {
            assert!(parse_reply(junk, ip).is_none(), "мусор принят за сервер: {junk:?}");
        }
    }
}
