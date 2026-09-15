use std::collections::{BTreeMap, HashMap};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::{Duration, Instant};

use crate::config::ServerConfig;
use crate::epoll::Epoll;
use crate::http::parse_request;
use crate::sessions::SessionStore;

const MAX_EVENTS: usize = 128;
const TIMEOUT: Duration = Duration::from_secs(10);

struct Connection {
    stream: TcpStream,
    server_index: usize,
    server_indices: Vec<usize>,
    received: Vec<u8>,
    response: Vec<u8>,
    sent: usize,
    last_active: Instant,
}

pub fn run(servers: Vec<ServerConfig>, sessions: &mut SessionStore) -> Result<(), String> {
    let mut additional_sessions: Vec<SessionStore> = servers
        .iter()
        .skip(1)
        .map(|_| SessionStore::new())
        .collect();
    let mut server_sessions: Vec<&mut SessionStore> = std::iter::once(sessions)
        .chain(additional_sessions.iter_mut())
        .collect();
    let epoll = Epoll::new().map_err(|error| error.to_string())?;
    let mut listeners = Vec::new();
    let mut listener_servers = HashMap::new();
    let mut connections: HashMap<RawFd, Connection> = HashMap::new();

    for ((host, port), server_indices) in group_servers(&servers) {
        let address = format!("{host}:{port}");
        let listener = TcpListener::bind(&address)
            .map_err(|error| format!("Could not listen on {address}: {error}"))?;

        listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;

        let fd = listener.as_raw_fd();
        epoll
            .add(fd, libc::EPOLLIN as u32)
            .map_err(|error| error.to_string())?;

        println!("Server running at http://{address}");
        listener_servers.insert(fd, server_indices);
        listeners.push(listener);
    }

    let mut events: Vec<libc::epoll_event> = (0..MAX_EVENTS)
        .map(|_| unsafe { std::mem::zeroed() })
        .collect();

    loop {
        remove_timed_out_connections(&epoll, &mut connections);

        let event_count = epoll
            .wait(&mut events, 1000)
            .map_err(|error| error.to_string())?;

        for event in events.iter().take(event_count) {
            let fd = event.u64 as RawFd;

            if let Some(server_indices) = listener_servers.get(&fd) {
                let listener = listeners
                    .iter()
                    .find(|listener| listener.as_raw_fd() == fd)
                    .ok_or("Listener was not found")?;

                accept_clients(listener, server_indices, &epoll, &mut connections)?;

                continue;
            }

            if event.events & (libc::EPOLLIN as u32) != 0 {
                read_client(fd, &servers, &mut server_sessions, &epoll, &mut connections);
            }

            if event.events & (libc::EPOLLOUT as u32) != 0 {
                write_client(fd, &epoll, &mut connections);
            }
        }
    }
}

fn accept_clients(
    listener: &TcpListener,
    server_indices: &[usize],
    epoll: &Epoll,
    connections: &mut HashMap<RawFd, Connection>,
) -> Result<(), String> {
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(true)
                    .map_err(|error| error.to_string())?;

                let fd = stream.as_raw_fd();

                epoll
                    .add(fd, (libc::EPOLLIN | libc::EPOLLRDHUP) as u32)
                    .map_err(|error| error.to_string())?;

                connections.insert(
                    fd,
                    Connection {
                        stream,
                        server_index: server_indices[0],
                        server_indices: server_indices.to_vec(),
                        received: Vec::new(),
                        response: Vec::new(),
                        sent: 0,
                        last_active: Instant::now(),
                    },
                );
            }

            Err(error) if error.kind() == ErrorKind::WouldBlock => break,
            Err(error) => return Err(error.to_string()),
        }
    }

    Ok(())
}

fn read_client(
    fd: RawFd,
    servers: &[ServerConfig],
    sessions: &mut [&mut SessionStore],
    epoll: &Epoll,
    connections: &mut HashMap<RawFd, Connection>,
) {
    let mut request = None;
    let mut closed = false;

    if let Some(connection) = connections.get_mut(&fd) {
        let mut buffer = [0; 4096];

        match connection.stream.read(&mut buffer) {
            Ok(0) => closed = true,
            Ok(size) => {
                connection.received.extend_from_slice(&buffer[..size]);
                connection.last_active = Instant::now();

                match parse_request(&connection.received) {
                    Ok(parsed) => {
                        connection.server_index = select_server(
                            servers,
                            &connection.server_indices,
                            parsed.headers.get("host").map(String::as_str),
                        );
                        request = Some(parsed);
                    }
                    Err(error) if crate::is_incomplete_request(&error) => {}
                    Err(_) => {
                        connection.response = crate::make_error_response(
                            &servers[connection.server_index],
                            400,
                            "400 Bad Request",
                        );
                    }
                }
            }
            Err(error)
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
            Err(_) => closed = true,
        }
    }

    if closed {
        let _ = epoll.delete(fd);
        connections.remove(&fd);
        return;
    }

    if let Some(request) = request {
        if let Some(connection) = connections.get_mut(&fd) {
            connection.response = crate::handle_request(
                request,
                &servers[connection.server_index],
                sessions[connection.server_index],
            );
        }
    }

    if let Some(connection) = connections.get(&fd) {
        if !connection.response.is_empty() {
            if epoll
                .modify(fd, (libc::EPOLLOUT | libc::EPOLLRDHUP) as u32)
                .is_err()
            {
                let _ = epoll.delete(fd);
                connections.remove(&fd);
            }
        }
    }
}

fn write_client(fd: RawFd, epoll: &Epoll, connections: &mut HashMap<RawFd, Connection>) {
    let mut finished = false;

    if let Some(connection) = connections.get_mut(&fd) {
        if connection.sent < connection.response.len() {
            match connection
                .stream
                .write(&connection.response[connection.sent..])
            {
                Ok(0) => finished = true,
                Ok(size) => {
                    connection.sent += size;
                    connection.last_active = Instant::now();
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) =>
                {
                    return;
                }
                Err(_) => finished = true,
            }
        }

        if connection.sent == connection.response.len() {
            finished = true;
        }
    }

    if finished {
        let _ = epoll.delete(fd);
        connections.remove(&fd);
    }
}

fn remove_timed_out_connections(epoll: &Epoll, connections: &mut HashMap<RawFd, Connection>) {
    let now = Instant::now();

    let timed_out: Vec<RawFd> = connections
        .iter()
        .filter(|(_, connection)| now.duration_since(connection.last_active) > TIMEOUT)
        .map(|(fd, _)| *fd)
        .collect();

    for fd in timed_out {
        let _ = epoll.delete(fd);
        connections.remove(&fd);
    }
}

// Every listener group is nonempty, with server indices in configuration order.
fn group_servers(servers: &[ServerConfig]) -> BTreeMap<(String, u16), Vec<usize>> {
    let mut groups: BTreeMap<(String, u16), Vec<usize>> = BTreeMap::new();
    for (index, server) in servers.iter().enumerate() {
        for &port in &server.ports {
            let group = groups.entry((server.host.clone(), port)).or_default();
            if !group.contains(&index) {
                group.push(index);
            }
        }
    }
    groups
}

fn select_server(servers: &[ServerConfig], group: &[usize], host: Option<&str>) -> usize {
    if let Some(host) = host {
        let host = host.trim();
        // Brackets keep IPv6 colons distinct from the optional port separator.
        let name = if host.starts_with('[') {
            host.find(']').map(|end| &host[..=end]).unwrap_or(host)
        } else {
            host.split_once(':').map_or(host, |(name, _)| name)
        };
        for &index in group {
            if servers[index]
                .server_names
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(name))
            {
                return index;
            }
        }
    }
    group[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_config;

    fn servers() -> Vec<ServerConfig> {
        parse_config("server {\nhost 127.0.0.1\nport 8080\nport 8081\nroot .\nserver_name main.local\n}\nserver {\nhost 127.0.0.1\nport 8080\nroot .\nserver_name second.local alias.local\n}\nserver {\nhost 127.0.0.1\nport 8082\nroot .\nserver_name outside.local\n}").unwrap()
    }

    #[test]
    fn matches_host_names_and_aliases_ignoring_port_and_case() {
        let servers = servers();
        for host in ["second.local", "SECOND.LOCAL:8080", "alias.local:8080"] {
            assert_eq!(select_server(&servers, &[0, 1], Some(host)), 1);
        }
        assert_eq!(select_server(&servers, &[0, 1], Some("main.local")), 0);
    }

    #[test]
    fn defaults_to_first_server_in_listener_group() {
        let servers = servers();
        for host in [None, Some("unknown.local"), Some("outside.local:8080")] {
            assert_eq!(select_server(&servers, &[0, 1], host), 0);
            assert_eq!(select_server(&servers, &[1, 0], host), 1);
        }
    }

    #[test]
    fn groups_shared_addresses_and_preserves_multiple_ports() {
        let mut servers = servers();
        servers[0].ports.push(8080);
        let groups = group_servers(&servers);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[&("127.0.0.1".into(), 8080)], [0, 1]);
        assert_eq!(groups[&("127.0.0.1".into(), 8081)], [0]);
        assert_eq!(groups[&("127.0.0.1".into(), 8082)], [2]);
    }
}
