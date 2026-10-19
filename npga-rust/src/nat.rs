//! RFC 5389/3489 NAT type discovery through the local SOCKS5 proxy port.
//!
//! Faithful port of the C# `Socks5ServerTestUtils.DiscoveryNatTypeAsync` flow:
//! the STUN probe travels through the proxy, so the result describes the exit
//! the acceleration traffic uses, not the user's own LAN.

use anyhow::{Context, Result, bail};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

const STUN_COOKIE: u32 = 0x2112A442;
const ATTR_MAPPED: u16 = 0x0001;
const ATTR_CHANGED: u16 = 0x0005;
const ATTR_XOR_MAPPED: u16 = 0x0020;
const ATTR_CHANGE_REQUEST: u16 = 0x0003;
const CHANGE_PORT: u32 = 0x02;
const TRANSACTION_TIMEOUT: Duration = Duration::from_millis(1800);

#[derive(Debug, Clone)]
pub struct NatResult {
    /// Simple code mirroring the C# client: 1/2/3/NoUDP or an error string.
    pub result: String,
    pub mapping: String,
    pub filtering: String,
    pub public_end: Option<String>,
    pub local_end: Option<String>,
}

struct SocksUdp {
    _control: TcpStream,
    socket: UdpSocket,
    relay: SocketAddr,
}

impl SocksUdp {
    /// Opens a SOCKS5 UDP ASSOCIATE on an ephemeral control connection.
    fn associate(proxy: SocketAddr, user: &str, pass: &str) -> Result<Self> {
        let mut control = TcpStream::connect_timeout(&proxy, Duration::from_millis(2500))
            .with_context(|| format!("连接本地 SOCKS 代理 {proxy} 失败"))?;
        control.set_read_timeout(Some(Duration::from_millis(2500)))?;
        control.set_write_timeout(Some(Duration::from_millis(2500)))?;
        if user.is_empty() {
            control.write_all(&[0x05, 0x01, 0x00])?;
            let mut reply = [0u8; 2];
            control.read_exact(&mut reply)?;
            if reply[0] != 0x05 || reply[1] != 0x00 {
                bail!("SOCKS 代理拒绝了无认证方式");
            }
        } else {
            control.write_all(&[0x05, 0x02, 0x00, 0x02])?;
            let mut reply = [0u8; 2];
            control.read_exact(&mut reply)?;
            if reply[0] != 0x05 || reply[1] != 0x02 {
                bail!("SOCKS 代理不支持用户名认证");
            }
            let mut request = vec![0x01, user.len() as u8];
            request.extend(user.bytes());
            request.push(pass.len() as u8);
            request.extend(pass.bytes());
            control.write_all(&request)?;
            let mut status = [0u8; 2];
            control.read_exact(&mut status)?;
            if status[1] != 0x00 {
                bail!("SOCKS 代理用户名认证失败");
            }
        }
        // UDP ASSOCIATE with an unspecified client endpoint.
        control.write_all(&[0x05, 0x03, 0x00, 0x01, 0, 0, 0, 0, 0, 0])?;
        let mut head = [0u8; 4];
        control.read_exact(&mut head)?;
        if head[0] != 0x05 || head[1] != 0x00 {
            bail!("SOCKS UDP ASSOCIATE 被拒绝");
        }
        let relay = match head[3] {
            0x01 => {
                let mut addr = [0u8; 6];
                control.read_exact(&mut addr)?;
                SocketAddr::new(
                    std::net::Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3]).into(),
                    u16::from_be_bytes([addr[4], addr[5]]),
                )
            }
            0x04 => {
                let mut addr = [0u8; 18];
                control.read_exact(&mut addr)?;
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&addr[..16]);
                SocketAddr::new(
                    std::net::Ipv6Addr::from(octets).into(),
                    u16::from_be_bytes([addr[16], addr[17]]),
                )
            }
            0x03 => {
                let mut len = [0u8; 1];
                control.read_exact(&mut len)?;
                let mut rest = vec![0u8; len[0] as usize + 2];
                control.read_exact(&mut rest)?;
                let host = String::from_utf8_lossy(&rest[..len[0] as usize]).into_owned();
                let port = u16::from_be_bytes([rest[len[0] as usize], rest[len[0] as usize + 1]]);
                (host.as_str(), port)
                    .to_socket_addrs()?
                    .next()
                    .context("无法解析 SOCKS 中继地址")?
            }
            other => bail!("未知 SOCKS 地址类型 {other}"),
        };
        // Bind a local UDP socket; the relay accepts encapsulated datagrams from it.
        let socket = UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0))
            .context("绑定本地 UDP 套接字失败")?;
        socket.set_read_timeout(Some(TRANSACTION_TIMEOUT))?;
        Ok(Self {
            _control: control,
            socket,
            relay,
        })
    }

    /// Sends a STUN request to `target` through the relay, retransmitting once.
    fn stun_transaction(&self, target: SocketAddr, message: &[u8]) -> Result<Vec<u8>> {
        let mut datagram = vec![0x00, 0x00, 0x00, 0x01];
        match target.ip() {
            std::net::IpAddr::V4(v4) => datagram.extend(v4.octets()),
            std::net::IpAddr::V6(v6) => {
                datagram[3] = 0x04;
                datagram.extend(v6.octets());
            }
        }
        datagram.extend(target.port().to_be_bytes());
        datagram.extend_from_slice(message);
        let begin = Instant::now();
        let mut buffer = [0u8; 2048];
        loop {
            self.socket
                .send_to(&datagram, self.relay)
                .context("发送 STUN 请求失败")?;
            loop {
                match self.socket.recv_from(&mut buffer) {
                    Ok((len, _)) => {
                        // Strip the SOCKS UDP header (RSV RSV FRAG ATYP + addr + port).
                        let payload = &buffer[..len];
                        if payload.len() < 10 {
                            continue;
                        }
                        return Ok(payload[10..].to_vec());
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        break;
                    }
                    Err(e) => return Err(e).context("接收 STUN 响应失败"),
                }
            }
            if begin.elapsed() > TRANSACTION_TIMEOUT * 2 {
                bail!("STUN 服务器无响应");
            }
        }
    }
}

fn transaction_id() -> [u8; 12] {
    // 96 bits of address-space randomness is unnecessary; a time/salt mix suffices
    // for single-flight local probes. STUN servers only echo it back.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut id = [0u8; 12];
    // `as_nanos` returns u128, so only the low eight bytes fit here.
    id[..8].copy_from_slice(&nanos.to_le_bytes()[..8]);
    id[8..].copy_from_slice(&(std::process::id() as u64).to_le_bytes()[..4]);
    id
}

fn stun_request(change_request: Option<u32>) -> Vec<u8> {
    let mut message = Vec::with_capacity(28);
    message.extend(0x0001u16.to_be_bytes()); // Binding Request
    let id = transaction_id();
    let mut attributes = vec![];
    if let Some(flags) = change_request {
        attributes.extend(ATTR_CHANGE_REQUEST.to_be_bytes());
        attributes.extend(4u16.to_be_bytes());
        attributes.extend(flags.to_be_bytes());
    }
    message.extend((attributes.len() as u16).to_be_bytes());
    message.extend(STUN_COOKIE.to_be_bytes());
    message.extend(id);
    message.extend(attributes);
    message
}

struct StunResponse {
    mapped: Option<SocketAddr>,
    changed: Option<SocketAddr>,
}

fn parse_stun_response(data: &[u8]) -> Result<StunResponse> {
    if data.len() < 20 {
        bail!("STUN 响应过短");
    }
    let message_type = u16::from_be_bytes([data[0], data[1]]);
    let length = u16::from_be_bytes([data[2], data[3]]) as usize;
    if message_type != 0x0101 {
        bail!("STUN 响应类型不是 Binding Success");
    }
    let end = 20 + length;
    if data.len() < end {
        bail!("STUN 响应长度字段越界");
    }
    let mut mapped = None;
    let mut changed = None;
    let mut offset = 20;
    while offset + 4 <= end {
        let attr_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let attr_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
        let value_start = offset + 4;
        let value_end = value_start + attr_len;
        if value_end > end {
            bail!("STUN 属性越界");
        }
        let value = &data[value_start..value_end];
        if let Some(parsed) = parse_address_attribute(attr_type, value, &data[4..20]) {
            match attr_type {
                ATTR_MAPPED | ATTR_XOR_MAPPED => mapped = Some(parsed),
                ATTR_CHANGED | 0x802C => changed = Some(parsed),
                _ => {}
            }
        }
        offset = value_end + ((4 - attr_len % 4) % 4);
    }
    Ok(StunResponse { mapped, changed })
}

/// Parses MAPPED-ADDRESS style attributes; XOR variants unmask with the cookie
/// and the first four transaction-id bytes.
fn parse_address_attribute(
    attr_type: u16,
    value: &[u8],
    transaction_id: &[u8],
) -> Option<SocketAddr> {
    if value.len() < 4 {
        return None;
    }
    let family = value[1];
    let port_raw = u16::from_be_bytes([value[2], value[3]]);
    // Only XOR-MAPPED-ADDRESS is obfuscated with the cookie. MAPPED-ADDRESS,
    // CHANGED-ADDRESS and the other attributes carry the plain address.
    let xor = attr_type == ATTR_XOR_MAPPED;
    let (ip, port) = match (family, xor) {
        (0x01, false) => {
            if value.len() < 8 {
                return None;
            }
            (
                std::net::Ipv4Addr::new(value[4], value[5], value[6], value[7]).into(),
                port_raw,
            )
        }
        (0x01, true) => {
            if value.len() < 8 {
                return None;
            }
            let mask = STUN_COOKIE.to_be_bytes();
            let port = port_raw ^ u16::from_be_bytes([mask[0], mask[1]]);
            let mut octets = [0u8; 4];
            for i in 0..4 {
                octets[i] = value[4 + i] ^ mask[i];
            }
            (std::net::Ipv4Addr::from(octets).into(), port)
        }
        (0x02, false) => {
            if value.len() < 20 {
                return None;
            }
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&value[4..20]);
            (std::net::Ipv6Addr::from(octets).into(), port_raw)
        }
        (0x02, true) => {
            if value.len() < 20 {
                return None;
            }
            let mut mask = STUN_COOKIE.to_be_bytes().to_vec();
            mask.extend_from_slice(&transaction_id[..4]);
            let port = port_raw ^ u16::from_be_bytes([mask[0], mask[1]]);
            let mut octets = [0u8; 16];
            for i in 0..16 {
                octets[i] = value[4 + i] ^ mask[i];
            }
            (std::net::Ipv6Addr::from(octets).into(), port)
        }
        _ => return None,
    };
    Some(SocketAddr::new(ip, port))
}

/// Runs the three-test discovery and maps the outcome to the simple result
/// codes used by the NPGA clients.
pub fn discover_nat_type(
    socks_endpoint: &str,
    stun_host: &str,
    stun_port: u16,
    socks_user: &str,
    socks_pass: &str,
) -> Result<NatResult> {
    let proxy: SocketAddr = socks_endpoint
        .to_socket_addrs()
        .with_context(|| format!("解析本地 SOCKS 端口 {socks_endpoint} 失败"))?
        .next()
        .context("本地 SOCKS 端口无效")?;
    let stun: SocketAddr = (stun_host, stun_port)
        .to_socket_addrs()
        .with_context(|| format!("解析 STUN 服务器 {stun_host} 失败"))?
        .next()
        .context("STUN 服务器地址无效")?;
    let channel = SocksUdp::associate(proxy, socks_user, socks_pass)?;

    // Test I: binding request to the primary server.
    let test1 = channel.stun_transaction(stun, &stun_request(None));
    let first = match test1 {
        Ok(data) => parse_stun_response(&data)?,
        Err(_) => {
            return Ok(NatResult {
                result: "NoUDP".into(),
                mapping: "UDP 不可用".into(),
                filtering: "UDP 不可用".into(),
                public_end: None,
                local_end: None,
            });
        }
    };
    let mapped = first.mapped.context("STUN 响应缺少映射地址")?;
    let changed = first.changed;

    // Test II: binding request to the changed address (mapping behaviour).
    let mapping: String = match changed {
        Some(alt) => match channel.stun_transaction(alt, &stun_request(None)) {
            Ok(data) => {
                let response = parse_stun_response(&data)?;
                match response.mapped {
                    Some(second) if second == mapped => {
                        "Endpoint Independent Mapping（全锥形映射）".into()
                    }
                    Some(_) => "Address Dependent Mapping（地址相关映射）".into(),
                    None => "映射行为无法判定".into(),
                }
            }
            Err(_) => "Address And Port Dependent Mapping（端口相关映射）".into(),
        },
        None => "服务器未提供 CHANGED-ADDRESS，映射行为无法判定".into(),
    };

    // Test III: request to the changed address asking the response to leave from
    // a different port (filtering behaviour).
    let filtering: String = match changed {
        Some(alt) => match channel.stun_transaction(alt, &stun_request(Some(CHANGE_PORT))) {
            Ok(data) if parse_stun_response(&data).is_ok() => {
                "Endpoint Independent Filtering（不过滤）".into()
            }
            _ => "Address Dependent Filtering（地址相关过滤）".into(),
        },
        None => "服务器未提供 CHANGED-ADDRESS，过滤行为无法判定".into(),
    };

    let result = if mapping.starts_with("Endpoint Independent") {
        if filtering.starts_with("Endpoint Independent") {
            "1"
        } else {
            "2"
        }
    } else if mapping.starts_with("Address") || mapping.starts_with("服务器") {
        "3"
    } else {
        "2"
    };
    Ok(NatResult {
        result: result.into(),
        mapping,
        filtering,
        public_end: Some(mapped.to_string()),
        local_end: Some(proxy.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a STUN Binding Success response carrying one address attribute.
    fn success_response(attr_type: u16, value: &[u8]) -> Vec<u8> {
        let mut message = vec![0x01, 0x01, 0x00, 0x00];
        message.extend_from_slice(&STUN_COOKIE.to_be_bytes());
        message.extend_from_slice(&[0u8; 12]);
        message.extend_from_slice(&attr_type.to_be_bytes());
        message.extend_from_slice(&(value.len() as u16).to_be_bytes());
        message.extend_from_slice(value);
        let length = (message.len() - 20) as u16;
        message[2..4].copy_from_slice(&length.to_be_bytes());
        message
    }

    /// Encodes a MAPPED-ADDRESS style value for an IPv4 address.
    fn mapped_ipv4(ip: [u8; 4], port: u16) -> Vec<u8> {
        let mut value = vec![0, 0x01];
        value.extend_from_slice(&port.to_be_bytes());
        value.extend_from_slice(&ip);
        value
    }

    #[test]
    fn binding_request_is_well_formed() {
        let request = stun_request(None);
        assert_eq!(request.len(), 20);
        // Message type 0x0001 is a Binding Request.
        assert_eq!(&request[0..2], &[0x00, 0x01]);
        // No attributes, so the length field is zero.
        assert_eq!(&request[2..4], &[0x00, 0x00]);
        assert_eq!(&request[4..8], &STUN_COOKIE.to_be_bytes());
    }

    #[test]
    fn change_request_attribute_is_encoded() {
        let request = stun_request(Some(CHANGE_PORT));
        // 20 byte header plus a 4 byte attribute value.
        assert_eq!(request.len(), 28);
        assert_eq!(&request[2..4], &[0x00, 0x08]);
        assert_eq!(&request[20..22], &ATTR_CHANGE_REQUEST.to_be_bytes());
        assert_eq!(&request[22..24], &[0x00, 0x04]);
        assert_eq!(&request[24..28], &CHANGE_PORT.to_be_bytes());
    }

    #[test]
    fn transaction_ids_differ_between_requests() {
        let first = transaction_id();
        let second = transaction_id();
        assert_ne!(first, second);
        assert_eq!(first.len(), 12);
    }

    #[test]
    fn mapped_address_is_parsed() {
        let response = success_response(ATTR_MAPPED, &mapped_ipv4([203, 0, 113, 7], 51234));
        let parsed = parse_stun_response(&response).unwrap();
        let mapped = parsed.mapped.expect("应解析出映射地址");
        assert_eq!(mapped.ip().to_string(), "203.0.113.7");
        assert_eq!(mapped.port(), 51234);
        assert!(parsed.changed.is_none());
    }

    #[test]
    fn xor_mapped_address_is_unmasked() {
        let ip = [203u8, 0, 113, 7];
        let port = 51234u16;
        let mask = STUN_COOKIE.to_be_bytes();
        let mut value = vec![0, 0x01];
        value.extend_from_slice(&(port ^ u16::from_be_bytes([mask[0], mask[1]])).to_be_bytes());
        for (index, byte) in ip.iter().enumerate() {
            value.push(byte ^ mask[index]);
        }
        let response = success_response(ATTR_XOR_MAPPED, &value);
        let parsed = parse_stun_response(&response).unwrap();
        let mapped = parsed.mapped.expect("应解析出异或映射地址");
        assert_eq!(mapped.ip().to_string(), "203.0.113.7");
        assert_eq!(mapped.port(), port);
    }

    #[test]
    fn changed_address_is_parsed() {
        let response = success_response(ATTR_CHANGED, &mapped_ipv4([198, 51, 100, 9], 3478));
        let parsed = parse_stun_response(&response).unwrap();
        let changed = parsed.changed.expect("应解析出变更地址");
        assert_eq!(changed.ip().to_string(), "198.51.100.9");
        assert_eq!(changed.port(), 3478);
    }

    #[test]
    fn malformed_responses_are_rejected() {
        // Too short to be a STUN message.
        assert!(parse_stun_response(&[0u8; 8]).is_err());
        // Length field claims more data than is present.
        let mut truncated = success_response(ATTR_MAPPED, &mapped_ipv4([1, 2, 3, 4], 1));
        truncated.truncate(24);
        assert!(parse_stun_response(&truncated).is_err());
        // Not a Binding Success response.
        let mut wrong_type = success_response(ATTR_MAPPED, &mapped_ipv4([1, 2, 3, 4], 1));
        wrong_type[1] = 0x02;
        assert!(parse_stun_response(&wrong_type).is_err());
    }

    #[test]
    fn attribute_values_shorter_than_the_address_are_ignored() {
        let response = success_response(ATTR_MAPPED, &[0, 0x01, 0x00]);
        let parsed = parse_stun_response(&response).unwrap();
        assert!(parsed.mapped.is_none());
    }
}
