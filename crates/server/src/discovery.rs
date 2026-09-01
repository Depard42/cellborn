//! Отклик на поиск сервера в локальной сети.
//!
//! Отдельный UDP-сокет на своём порту: игровой занят netcode-протоколом
//! lightyear, и подмешивать туда свои пакеты — верный способ однажды сломать
//! рукопожатие ради удобства меню.
//!
//! Сокет неблокирующий и опрашивается раз в тик. Это не сервис, а вежливость:
//! если он не ответит, игрок просто введёт адрес руками.

use bevy::prelude::*;
use cellborn_common::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::Connected;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use crate::config::ServerConfig;

#[derive(Resource)]
pub struct Beacon {
    socket: Option<UdpSocket>,
}

pub fn open(config: &ServerConfig) -> Beacon {
    // Порт поиска считается от игрового: сервер на нестандартном порту должен
    // находиться так же, как и обычный.
    let port = config.port.wrapping_add(1);
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
    match UdpSocket::bind(addr) {
        Ok(socket) => {
            let _ = socket.set_nonblocking(true);
            info!("поиск в локальной сети слушает {addr}");
            Beacon { socket: Some(socket) }
        }
        // Не беда: сервер продолжит работать, его просто не найдут
        // автоматически. Чаще всего это второй сервер на той же машине.
        Err(error) => {
            warn!("порт поиска {port} занят ({error}); сервер придётся вводить руками");
            Beacon { socket: None }
        }
    }
}

/// Отвечает всем, кто спрашивает.
pub fn answer_probes(
    beacon: Res<Beacon>,
    config: Res<ServerConfig>,
    players: Query<(), (With<ClientOf>, With<Connected>)>,
    organisms: Query<(), With<PlayerGenome>>,
) {
    let Some(socket) = &beacon.socket else { return };
    let mut buffer = [0u8; 64];

    // Разбираем всё, что накопилось, но не больше горстки за тик: иначе поток
    // мусора на этот порт мог бы занять сервер целиком.
    for _ in 0..16 {
        let Ok((len, from)) = socket.recv_from(&mut buffer) else { break };
        if &buffer[..len.min(DISCOVERY_PROBE.len())] != DISCOVERY_PROBE {
            continue;
        }
        // Имя сервера, игроки и население: по этому игрок в меню поймёт, куда
        // он попадёт, ещё до подключения.
        let reply = format!(
            "{}{}|{}|{}|{}",
            String::from_utf8_lossy(DISCOVERY_REPLY),
            config.port,
            players.iter().count(),
            organisms.iter().count(),
            version::VERSION,
        );
        let _ = socket.send_to(reply.as_bytes(), from);
    }
}
