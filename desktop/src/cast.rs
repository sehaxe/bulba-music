//! Sending the stream to a speaker on the LAN.
//!
//! Two protocols cover nearly every device in a house: CASTV2 (Chromecast) and
//! DLNA/UPnP. Both boil down to "give it a URL and it plays", which is exactly
//! what the server offers: one URL per track. Discovery replies stream in while
//! both protocols are asked at once; the connect path keeps the socket alive on
//! a worker thread so a silent TV can never block the window.

// The sheet and the sender are wired by the window in a later change.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashSet};
use std::io::{Read as _, Write as _};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use gpui::{BoxShadow, CursorStyle, FontWeight, IntoElement, SharedString, div, point, prelude::*, px};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme, StreamOwned};
use serde_json::{Value, json};

use crate::api::Track;
use crate::theme::Palette;

const CAST_PORT: u16 = 8009;
const NS_CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
const NS_RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
const NS_MEDIA: &str = "urn:x-cast:com.google.cast.media";
const APP_DEFAULT_MEDIA_RECEIVER: &str = "CC1AD845";

const SSDP_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(239, 255, 255, 250)), 1900);
const MDNS_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 251)), 5353);

const MSEARCH: &str = "M-SEARCH * HTTP/1.1\r\n\
HOST: 239.255.255.250:1900\r\n\
MAN: \"ssdp:discover\"\r\n\
MX: 2\r\n\
ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\
\r\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Chromecast,
    Dlna,
    Both,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Chromecast => "chromecast",
            Self::Dlna => "dlna",
            Self::Both => "chromecast · dlna",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub host: String,
    pub port: u16,
    /// The device description URL. DLNA needs it; Chromecast ignores it.
    pub location: String,
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// SSDP for renderers and mDNS for Chromecasts, on every IPv4 interface.
/// Blocking: call it from a thread.
pub fn discover(timeout: Duration) -> Vec<Device> {
    let deadline = Instant::now() + timeout;
    let sockets = discovery_sockets();
    for sock in &sockets {
        send_queries(sock);
    }

    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut buf = vec![0u8; 9000];
    while Instant::now() < deadline {
        for sock in &sockets {
            loop {
                match sock.recv_from(&mut buf) {
                    Ok((n, from)) => {
                        let data = &buf[..n];
                        let from = match from.ip() {
                            IpAddr::V4(ip) => ip,
                            IpAddr::V6(_) => continue,
                        };
                        let parsed = if data.starts_with(b"HTTP/") {
                            std::str::from_utf8(data)
                                .ok()
                                .and_then(parse_ssdp_response)
                                .and_then(|reply| device_from_ssdp(&reply, from))
                                .into_iter()
                                .collect()
                        } else {
                            parse_mdns(data, from)
                        };
                        for device in parsed {
                            if seen.insert(format!("{}|{}", device.id, device.host)) {
                                found.push(device);
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
                    Err(_) => break,
                }
            }
        }
    }
    merge(found)
}

/// One socket per address is what makes `IP_MULTICAST_IF` unnecessary: the
/// kernel routes a datagram out of the interface its source address belongs to.
fn discovery_sockets() -> Vec<UdpSocket> {
    let mut sockets = Vec::new();
    for ip in local_ipv4s() {
        if let Ok(sock) = UdpSocket::bind(SocketAddr::new(IpAddr::V4(ip), 0)) {
            let _ = sock.set_read_timeout(Some(Duration::from_millis(50)));
            sockets.push(sock);
        }
    }
    sockets
}

fn send_queries(sock: &UdpSocket) {
    let _ = sock.set_multicast_ttl_v4(2);
    let _ = sock.send_to(MSEARCH.as_bytes(), SSDP_ADDR);
    // mDNS requires a hop limit of 255; SSDP is happy with either.
    let _ = sock.set_multicast_ttl_v4(255);
    let _ = sock.send_to(&mdns_query(), MDNS_ADDR);
}

fn local_ipv4s() -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    // getifaddrs is the only interface enum in the stdlib's reach; libc is
    // already a dependency for other parts of the player.
    unsafe {
        let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut list) != 0 {
            return out;
        }
        let mut cur = list;
        while !cur.is_null() {
            let ifa = &*cur;
            if !ifa.ifa_addr.is_null() && (*ifa.ifa_addr).sa_family as i32 == libc::AF_INET {
                let addr = (*(ifa.ifa_addr as *const libc::sockaddr_in)).sin_addr;
                let ip = Ipv4Addr::from(u32::from_be(addr.s_addr));
                if !ip.is_loopback() && !out.contains(&ip) {
                    out.push(ip);
                }
            }
            cur = ifa.ifa_next;
        }
        libc::freeifaddrs(list);
    }
    out
}

fn mdns_query() -> Vec<u8> {
    let mut q = vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in ["_googlecast", "_tcp", "local"] {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&12u16.to_be_bytes()); // PTR
    q.extend_from_slice(&0x8001u16.to_be_bytes()); // IN, unicast response
    q
}

#[derive(Debug, Default)]
struct SsdpReply {
    location: String,
    st: String,
    usn: String,
    server: String,
}

fn parse_ssdp_response(text: &str) -> Option<SsdpReply> {
    if !text.starts_with("HTTP/1.1 200") {
        return None;
    }
    let mut reply = SsdpReply::default();
    for line in text.lines().skip(1) {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim().to_ascii_lowercase().as_str() {
            "location" => reply.location = value.to_string(),
            "st" => reply.st = value.to_string(),
            "usn" => reply.usn = value.to_string(),
            "server" => reply.server = value.to_string(),
            _ => {}
        }
    }
    (!reply.location.is_empty()).then_some(reply)
}

fn device_from_ssdp(reply: &SsdpReply, from: Ipv4Addr) -> Option<Device> {
    let url = parse_http_url(&reply.location)?;
    let tags = format!("{} {} {}", reply.st, reply.usn, reply.server).to_ascii_lowercase();
    if tags.contains("googlecast") || tags.contains("chromecast") {
        Some(Device {
            id: reply.usn.split("::").next().unwrap_or(&reply.location).to_string(),
            name: format!("chromecast · {from}"),
            kind: Kind::Chromecast,
            host: from.to_string(),
            port: CAST_PORT,
            location: String::new(),
        })
    } else {
        // We only asked for MediaRenderer, so a 200 with a LOCATION is one.
        let name = url
            .path
            .rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or(&url.host);
        Some(Device {
            id: reply.usn.split("::").next().unwrap_or(&reply.location).to_string(),
            name: format!("{name} · {}", url.host),
            kind: Kind::Dlna,
            host: url.host,
            port: url.port,
            location: reply.location.clone(),
        })
    }
}

struct HttpUrl {
    host: String,
    port: u16,
    path: String,
    scheme: &'static str,
}

fn parse_http_url(s: &str) -> Option<HttpUrl> {
    let lower = s.to_ascii_lowercase();
    let (scheme, rest, default_port) = if lower.starts_with("http://") {
        ("http", &s[7..], 80)
    } else if lower.starts_with("https://") {
        ("https", &s[8..], 443)
    } else {
        return None;
    };
    let end = rest.find('/').unwrap_or(rest.len());
    let authority = rest[..end].rsplit('@').next()?;
    let (host, port) = if let Some(after_open) = authority.strip_prefix('[') {
        let close = after_open.find(']')?;
        let port = after_open[close + 1..]
            .strip_prefix(':')
            .and_then(|p| p.parse().ok())
            .unwrap_or(default_port);
        (after_open[..close].to_string(), port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        if !host.is_empty() && !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            (host.to_string(), port.parse().ok()?)
        } else {
            (authority.to_string(), default_port)
        }
    } else {
        (authority.to_string(), default_port)
    };
    if host.is_empty() {
        return None;
    }
    let path = if end < rest.len() { rest[end..].to_string() } else { "/".to_string() };
    Some(HttpUrl { host, port, path, scheme })
}

fn resolve_url(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    let Some(url) = parse_http_url(base) else {
        return href.to_string();
    };
    let root = format!("{}://{}:{}", url.scheme, url.host, url.port);
    if href.starts_with('/') {
        format!("{root}{href}")
    } else {
        let dir = &url.path[..url.path.rfind('/').map_or(0, |i| i + 1)];
        format!("{root}{dir}{href}")
    }
}

// --- mDNS ------------------------------------------------------------------

fn parse_mdns(buf: &[u8], from: Ipv4Addr) -> Vec<Device> {
    if buf.len() < 12 {
        return Vec::new();
    }
    let questions = u16::from_be_bytes([buf[4], buf[5]]) as usize;
    let answers = u16::from_be_bytes([buf[6], buf[7]]) as usize
        + u16::from_be_bytes([buf[8], buf[9]]) as usize
        + u16::from_be_bytes([buf[10], buf[11]]) as usize;
    if answers == 0 {
        return Vec::new();
    }

    let mut pos = 12;
    for _ in 0..questions {
        if dns_name(buf, &mut pos).is_none() {
            return Vec::new();
        }
        pos += 4;
    }

    let mut srv = Vec::new();
    let mut ptr = Vec::new();
    let mut txt: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut addrs: BTreeMap<String, Ipv4Addr> = BTreeMap::new();

    for _ in 0..answers {
        let Some(name) = dns_name(buf, &mut pos) else { break };
        if pos + 10 > buf.len() {
            break;
        }
        let kind = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let len = u16::from_be_bytes([buf[pos + 8], buf[pos + 9]]) as usize;
        pos += 10;
        if pos + len > buf.len() {
            break;
        }
        let data = &buf[pos..pos + len];
        match kind {
            1 if len == 4 => {
                addrs.insert(name.to_ascii_lowercase(), Ipv4Addr::new(data[0], data[1], data[2], data[3]));
            }
            12 => {
                let mut p = pos;
                if let Some(target) = dns_name(buf, &mut p) {
                    ptr.push(target);
                }
            }
            16 => {
                txt.insert(name.to_ascii_lowercase(), parse_txt(data));
            }
            33 if len >= 6 => {
                let mut p = pos + 6;
                if let Some(target) = dns_name(buf, &mut p) {
                    srv.push((name, target, u16::from_be_bytes([data[4], data[5]])));
                }
            }
            _ => {}
        }
        pos += len;
    }

    let mut candidates: Vec<(String, Option<String>, u16)> =
        ptr.iter().map(|name| (name.clone(), None, CAST_PORT)).collect();
    for (name, target, port) in srv {
        candidates.push((name, Some(target), port));
    }

    let suffix = "._googlecast._tcp.local";
    let mut out = Vec::new();
    for (instance, target, port) in candidates {
        let lower = instance.to_ascii_lowercase();
        if !lower.ends_with(suffix) {
            continue;
        }
        let host = target
            .and_then(|t| addrs.get(&t.to_ascii_lowercase()).copied())
            .unwrap_or(from);
        let meta = txt.get(&lower);
        let id = meta
            .and_then(|m| m.get("id"))
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| format!("chromecast-{host}-{port}"));
        let name = meta
            .and_then(|m| m.get("fn"))
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| {
                let base = &instance[..instance.len() - suffix.len()];
                if base.is_empty() {
                    format!("chromecast · {host}")
                } else {
                    base.replace('-', " ")
                }
            });
        out.push(Device {
            id,
            name,
            kind: Kind::Chromecast,
            host: host.to_string(),
            port,
            location: String::new(),
        });
    }
    out
}

fn dns_name(buf: &[u8], pos: &mut usize) -> Option<String> {
    let mut out = String::new();
    let mut p = *pos;
    let mut jumped = false;
    let mut hops = 0;
    loop {
        let len = *buf.get(p)? as usize;
        if len == 0 {
            p += 1;
            if !jumped {
                *pos = p;
            }
            return Some(out);
        }
        if len & 0xC0 == 0xC0 {
            if p + 1 >= buf.len() {
                return None;
            }
            let offset = ((len & 0x3F) << 8) | buf[p + 1] as usize;
            if !jumped {
                *pos = p + 2;
                jumped = true;
            }
            p = offset;
            hops += 1;
            if hops > 32 {
                return None;
            }
            continue;
        }
        if len & 0xC0 != 0 || p + 1 + len > buf.len() {
            return None;
        }
        if !out.is_empty() {
            out.push('.');
        }
        out.push_str(&String::from_utf8_lossy(&buf[p + 1..p + 1 + len]));
        p += 1 + len;
    }
}

fn parse_txt(data: &[u8]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut i = 0;
    while i < data.len() {
        let len = data[i] as usize;
        i += 1;
        if i + len > data.len() {
            break;
        }
        let text = String::from_utf8_lossy(&data[i..i + len]);
        i += len;
        if let Some((key, value)) = text.split_once('=') {
            out.insert(key.to_ascii_lowercase(), value.to_string());
        }
    }
    out
}

fn merge(mut devices: Vec<Device>) -> Vec<Device> {
    let mut out: Vec<Device> = Vec::new();
    for device in devices.drain(..) {
        if let Some(existing) = out.iter_mut().find(|d| {
            !d.host.is_empty() && d.host == device.host && d.kind != device.kind && d.kind != Kind::Both
        }) {
            // Both protocols on one box: CASTV2 is the better remote, DLNA
            // keeps its description URL as the fallback.
            let location = if existing.location.is_empty() {
                device.location.clone()
            } else {
                existing.location.clone()
            };
            if device.kind == Kind::Chromecast {
                existing.name = device.name.clone();
                existing.id = device.id.clone();
                existing.host = device.host.clone();
                existing.port = device.port;
                existing.location = location;
            }
            existing.kind = Kind::Both;
            continue;
        }
        out.push(device);
    }
    out.sort_by(|a, b| {
        let rank = |k: Kind| match k {
            Kind::Chromecast | Kind::Both => 0,
            Kind::Dlna => 1,
        };
        rank(a.kind)
            .cmp(&rank(b.kind))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

// ---------------------------------------------------------------------------
// The sender
// ---------------------------------------------------------------------------

pub struct Cast {
    target: Option<Target>,
}

enum Target {
    Chromecast(Chromecast),
    Dlna(Dlna),
}

impl Target {
    fn device(&self) -> &Device {
        match self {
            Self::Chromecast(c) => &c.device,
            Self::Dlna(d) => &d.device,
        }
    }
}

impl Default for Cast {
    fn default() -> Self {
        Self::new()
    }
}

impl Cast {
    pub fn new() -> Self {
        Self { target: None }
    }

    /// The device currently receiving, if any.
    pub fn connected(&self) -> Option<&Device> {
        self.target.as_ref().map(Target::device)
    }

    /// Human-readable state, and the connected device's name so the sheet can
    /// light the matching row.
    pub fn status(&self) -> String {
        match &self.target {
            Some(target) => format!("играет на {}", target.device().name),
            None => "не подключено".to_string(),
        }
    }

    pub fn connect(&mut self, device: &Device) -> Result<()> {
        self.target = None;
        let target = match device.kind {
            Kind::Dlna => Target::Dlna(Dlna::connect(device)?),
            Kind::Chromecast => Target::Chromecast(Chromecast::connect(device)?),
            Kind::Both => match Chromecast::connect(device) {
                Ok(cast) => Target::Chromecast(cast),
                Err(cast_err) => Target::Dlna(
                    Dlna::connect(device).with_context(|| format!("ни chromecast, ни dlna: {cast_err}"))?,
                ),
            },
        };
        self.target = Some(target);
        Ok(())
    }

    pub fn load(&mut self, track: &Track, stream_url: &str, position_secs: f64, playing: bool) -> Result<()> {
        let mime = stream_mime(stream_url, track);
        let position = position_secs.max(0.0);
        match &self.target {
            Some(Target::Chromecast(cast)) => cast.send(Cmd::Load {
                url: stream_url.to_string(),
                mime,
                title: track.title.clone(),
                artist: track.artist.clone(),
                position,
                playing,
            }),
            Some(Target::Dlna(dlna)) => dlna.load(track, stream_url, position, playing, &mime),
            None => bail!("нет подключённого устройства"),
        }
    }

    pub fn play(&mut self) -> Result<()> {
        match &self.target {
            Some(Target::Chromecast(cast)) => cast.send(Cmd::Play),
            Some(Target::Dlna(dlna)) => dlna.play(),
            None => bail!("нет подключённого устройства"),
        }
    }

    pub fn pause(&mut self) -> Result<()> {
        match &self.target {
            Some(Target::Chromecast(cast)) => cast.send(Cmd::Pause),
            Some(Target::Dlna(dlna)) => dlna.pause(),
            None => bail!("нет подключённого устройства"),
        }
    }

    pub fn seek(&mut self, secs: f64) -> Result<()> {
        match &self.target {
            Some(Target::Chromecast(cast)) => cast.send(Cmd::Seek(secs.max(0.0))),
            Some(Target::Dlna(dlna)) => dlna.seek(secs),
            None => bail!("нет подключённого устройства"),
        }
    }

    pub fn stop(&mut self) -> Result<()> {
        match &self.target {
            Some(Target::Chromecast(cast)) => cast.send(Cmd::Stop),
            Some(Target::Dlna(dlna)) => dlna.stop(),
            None => bail!("нет подключённого устройства"),
        }
    }

    pub fn volume(&mut self, v: f32) -> Result<()> {
        match &self.target {
            Some(Target::Chromecast(cast)) => cast.send(Cmd::Volume(v.clamp(0.0, 1.0))),
            Some(Target::Dlna(dlna)) => dlna.volume(v),
            None => bail!("нет подключённого устройства"),
        }
    }
}

/// The server's stream policy, not the source file, decides the bytes. A HEAD
/// asks it; the codec is only a guess for when the server is unreachable.
fn stream_mime(url: &str, track: &Track) -> String {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(1500)))
        .build()
        .new_agent();
    agent
        .head(url)
        .call()
        .ok()
        .and_then(|res| res.headers().get("content-type").and_then(|v| v.to_str().ok()).map(str::to_string))
        .map(|ct| ct.split(';').next().unwrap_or("").trim().to_ascii_lowercase())
        .filter(|ct| !ct.is_empty())
        .unwrap_or_else(|| mime_for_codec(track.codec.as_deref()).to_string())
}

fn mime_for_codec(codec: Option<&str>) -> &'static str {
    match codec.unwrap_or("") {
        "flac" => "audio/flac",
        "mp3" => "audio/mpeg",
        "m4a" | "alac" => "audio/mp4",
        "aac" => "audio/aac",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "aiff" | "aif" => "audio/aiff",
        _ => "audio/ogg",
    }
}

// ---------------------------------------------------------------------------
// Chromecast: CASTV2 over TLS
// ---------------------------------------------------------------------------

struct Chromecast {
    device: Device,
    tx: mpsc::Sender<Cmd>,
    _worker: JoinHandle<()>,
}

enum Cmd {
    Load {
        url: String,
        mime: String,
        title: String,
        artist: String,
        position: f64,
        playing: bool,
    },
    Play,
    Pause,
    Seek(f64),
    Stop,
    Volume(f32),
}

impl Chromecast {
    fn connect(device: &Device) -> Result<Self> {
        let addr = format!("{}:{}", device.host, device.port)
            .to_socket_addrs()
            .with_context(|| format!("адрес {}:{}", device.host, device.port))?
            .next()
            .ok_or_else(|| anyhow!("{} не резолвится", device.host))?;
        let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
            .with_context(|| format!("tcp {}:{}", device.host, device.port))?;
        let _ = tcp.set_nodelay(true);
        // The handshake happens on the first write and needs room; the reader
        // loop relaxes this to 100 ms once the tunnel is up.
        tcp.set_read_timeout(Some(Duration::from_secs(5)))?;
        tcp.set_write_timeout(Some(Duration::from_secs(5)))?;

        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyCert { provider }))
            .with_no_client_auth();
        let name = ServerName::try_from(device.host.clone())
            .unwrap_or_else(|_| ServerName::from(IpAddr::from(Ipv4Addr::LOCALHOST)));
        let conn = ClientConnection::new(Arc::new(config), name).context("tls: cast")?;
        let mut tls = StreamOwned::new(conn, tcp);

        send_cast(&mut tls, "receiver-0", NS_CONNECTION, json!({"type": "CONNECT"}))
            .with_context(|| format!("connect {}", device.name))?;
        tls.sock.set_read_timeout(Some(Duration::from_millis(100)))?;

        let (tx, rx) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("castv2".to_string())
            .spawn(move || run_session(tls, rx))
            .context("cast thread")?;
        Ok(Self {
            device: device.clone(),
            tx,
            _worker: worker,
        })
    }

    fn send(&self, cmd: Cmd) -> Result<()> {
        self.tx
            .send(cmd)
            .map_err(|_| anyhow!("соединение с {} потеряно", self.device.name))
    }
}

/// The cast device carries a Google-signed certificate whose hostname is never
/// the LAN address, so the only sane check is "this is the box I dialed".
#[derive(Debug)]
struct AcceptAnyCert {
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn run_session(mut tls: StreamOwned<ClientConnection, TcpStream>, rx: mpsc::Receiver<Cmd>) {
    let mut request_id = 0u64;
    let mut session: Option<i64> = None;
    let mut pending = Vec::new();
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        loop {
            match rx.try_recv() {
                Ok(cmd) => handle_cmd(&mut tls, cmd, &mut request_id, &mut session),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        match tls.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => {
                pending.extend_from_slice(&buf[..n]);
                while pending.len() >= 4 {
                    let len = u32::from_be_bytes([pending[0], pending[1], pending[2], pending[3]]) as usize;
                    if len > 4 * 1024 * 1024 || pending.len() < 4 + len {
                        if len > 4 * 1024 * 1024 {
                            pending.clear();
                        }
                        break;
                    }
                    if let Some(message) = decode_cast_message(&pending[4..4 + len])
                        && let Some(id) = message
                            .payload
                            .get("status")
                            .and_then(Value::as_array)
                            .and_then(|s| s.first())
                            .and_then(|s| s.get("mediaSessionId"))
                            .and_then(Value::as_i64)
                    {
                        session = Some(id);
                    }
                    pending.drain(..4 + len);
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => return,
        }
    }
}

fn handle_cmd(
    tls: &mut StreamOwned<ClientConnection, TcpStream>,
    cmd: Cmd,
    request_id: &mut u64,
    session: &mut Option<i64>,
) {
    match cmd {
        Cmd::Load {
            url,
            mime,
            title,
            artist,
            position,
            playing,
        } => {
            // An idle device ignores LOAD until its receiver app is running.
            let _ = send_cast(
                tls,
                "receiver-0",
                NS_RECEIVER,
                json!({"type": "LAUNCH", "appId": APP_DEFAULT_MEDIA_RECEIVER, "requestId": next(request_id)}),
            );
            std::thread::sleep(Duration::from_millis(900));
            let _ = send_cast(tls, "receiver-0", NS_CONNECTION, json!({"type": "CONNECT"}));
            let _ = send_cast(
                tls,
                "receiver-0",
                NS_MEDIA,
                json!({
                    "type": "LOAD",
                    "requestId": next(request_id),
                    "media": {
                        "contentId": url,
                        "streamType": "BUFFERED",
                        "contentType": mime,
                        "metadata": {
                            "metadataType": 0,
                            "title": title,
                            "artist": artist,
                        },
                    },
                    "currentTime": position,
                    "autoplay": playing,
                }),
            );
        }
        Cmd::Play => media_cmd(tls, "PLAY", request_id, session, json!({})),
        Cmd::Pause => media_cmd(tls, "PAUSE", request_id, session, json!({})),
        Cmd::Seek(secs) => media_cmd(tls, "SEEK", request_id, session, json!({"currentTime": secs})),
        Cmd::Stop => {
            media_cmd(tls, "STOP", request_id, session, json!({}));
            let _ = send_cast(
                tls,
                "receiver-0",
                NS_RECEIVER,
                json!({"type": "STOP", "requestId": next(request_id)}),
            );
        }
        Cmd::Volume(level) => {
            let _ = send_cast(
                tls,
                "receiver-0",
                NS_RECEIVER,
                json!({"type": "SET_VOLUME", "volume": {"level": level}, "requestId": next(request_id)}),
            );
        }
    }
}

fn next(id: &mut u64) -> u64 {
    *id += 1;
    *id
}

fn media_cmd(
    tls: &mut StreamOwned<ClientConnection, TcpStream>,
    kind: &str,
    request_id: &mut u64,
    session: &mut Option<i64>,
    extra: Value,
) {
    let mut payload = json!({"type": kind, "requestId": next(request_id)});
    if let Some(id) = *session {
        payload["mediaSessionId"] = json!(id);
    }
    if let (Value::Object(payload), Value::Object(extra)) = (&mut payload, extra) {
        payload.extend(extra);
    }
    let _ = send_cast(tls, "receiver-0", NS_MEDIA, payload);
}

fn send_cast(
    tls: &mut StreamOwned<ClientConnection, TcpStream>,
    destination: &str,
    namespace: &str,
    payload: Value,
) -> Result<()> {
    let bytes = encode_cast_message("sender-0", destination, namespace, &payload);
    tls.write_all(&bytes)?;
    tls.flush()?;
    Ok(())
}

// --- protobuf ---------------------------------------------------------------

fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn put_string(out: &mut Vec<u8>, field: u8, value: &str) {
    if value.is_empty() {
        return;
    }
    out.push((field << 3) | 2);
    put_varint(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

/// `CastMessage` with the four string fields, the enum and the payload, plus
/// the 4-byte big-endian prefix the receiver expects. Hand-rolled because the
/// whole schema is six fields and protobuf is the one part of casting that can
/// be checked without a device.
fn encode_cast_message(source: &str, destination: &str, namespace: &str, payload: &Value) -> Vec<u8> {
    let mut body = Vec::new();
    body.push(1 << 3);
    body.push(1); // protocol_version = CASTV2_1_0
    put_string(&mut body, 2, source);
    put_string(&mut body, 3, destination);
    put_string(&mut body, 4, namespace);
    body.push(5 << 3);
    body.push(0); // payload_type = STRING
    let payload = serde_json::to_string(payload).unwrap_or_else(|_| "null".to_string());
    put_string(&mut body, 6, &payload);

    let mut framed = Vec::with_capacity(4 + body.len());
    framed.extend_from_slice(&(body.len() as u32).to_be_bytes());
    framed.extend_from_slice(&body);
    framed
}

#[derive(Debug, PartialEq)]
struct CastMessage {
    source: String,
    destination: String,
    namespace: String,
    payload: Value,
}

fn decode_cast_message(body: &[u8]) -> Option<CastMessage> {
    let mut i = 0;
    let mut fields: BTreeMap<u64, Value> = BTreeMap::new();
    let read_varint = |i: &mut usize| -> Option<u64> {
        let mut shift = 0;
        let mut value = 0u64;
        loop {
            let byte = *body.get(*i)?;
            *i += 1;
            value |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
            shift += 7;
            if shift > 63 {
                return None;
            }
        }
    };

    while i < body.len() {
        let key = read_varint(&mut i)?;
        let field = key >> 3;
        match key & 7 {
            0 => {
                fields.insert(field, json!(read_varint(&mut i)?));
            }
            2 => {
                let len = read_varint(&mut i)? as usize;
                let end = i.checked_add(len)?;
                if end > body.len() {
                    return None;
                }
                fields.insert(field, json!(String::from_utf8_lossy(&body[i..end]).to_string()));
                i = end;
            }
            _ => return None,
        }
    }

    Some(CastMessage {
        source: fields.get(&2)?.as_str()?.to_string(),
        destination: fields.get(&3)?.as_str()?.to_string(),
        namespace: fields.get(&4)?.as_str()?.to_string(),
        payload: serde_json::from_str(fields.get(&6)?.as_str()?).ok()?,
    })
}

// ---------------------------------------------------------------------------
// DLNA: description XML + SOAP
// ---------------------------------------------------------------------------

struct Dlna {
    device: Device,
    control: String,
    render_control: Option<String>,
    agent: ureq::Agent,
}

impl Dlna {
    fn connect(device: &Device) -> Result<Self> {
        if device.location.is_empty() {
            bail!("у {} нет адреса описания", device.name);
        }
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .new_agent();
        let xml = agent
            .get(&device.location)
            .call()
            .with_context(|| format!("GET {}", device.location))?
            .body_mut()
            .read_to_string()
            .with_context(|| format!("описание {}", device.name))?;
        let control = service_control(&xml, "avtransport")
            .ok_or_else(|| anyhow!("нет AVTransport у {}", device.name))?;
        let render_control =
            service_control(&xml, "renderingcontrol").map(|c| resolve_url(&device.location, &c));
        Ok(Self {
            device: device.clone(),
            control: resolve_url(&device.location, &control),
            render_control,
            agent,
        })
    }

    fn soap(&self, control: &str, service: &str, action: &str, args: &str) -> Result<()> {
        let envelope = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
<s:Body><u:{action} xmlns:u=\"urn:schemas-upnp-org:service:{service}:1\">\
{args}</u:{action}></s:Body></s:Envelope>"
        );
        let soap_action = format!("\"urn:schemas-upnp-org:service:{service}:1#{action}\"");
        let res = self
            .agent
            .post(control)
            .header("content-type", "text/xml; charset=utf-8")
            .header("SOAPACTION", &soap_action)
            .send(envelope.as_str())
            .with_context(|| format!("{action} → {}", self.device.name))?;
        if res.status().as_u16() >= 400 {
            bail!("{action} → {} у {}", res.status(), self.device.name);
        }
        Ok(())
    }

    fn load(&self, track: &Track, url: &str, position: f64, playing: bool, mime: &str) -> Result<()> {
        let metadata = didl(url, &track.title, &track.artist, mime);
        let args = format!(
            "<InstanceID>0</InstanceID><CurrentURI>{}</CurrentURI>\
<CurrentURIMetaData>{}</CurrentURIMetaData>",
            xml_escape(url),
            xml_escape(&metadata)
        );
        self.soap(&self.control, "AVTransport", "SetAVTransportURI", &args)?;
        if position > 0.0 {
            self.seek(position)?;
        }
        if playing {
            self.play()
        } else {
            // Some renderers refuse Pause before the first Play; leaving it
            // loaded-and-idle is still the requested state.
            let _ = self.pause();
            Ok(())
        }
    }

    fn play(&self) -> Result<()> {
        self.soap(
            &self.control,
            "AVTransport",
            "Play",
            "<InstanceID>0</InstanceID><Speed>1</Speed>",
        )
    }

    fn pause(&self) -> Result<()> {
        self.soap(&self.control, "AVTransport", "Pause", "<InstanceID>0</InstanceID>")
    }

    fn seek(&self, secs: f64) -> Result<()> {
        let args = format!(
            "<InstanceID>0</InstanceID><Unit>REL_TIME</Unit><Target>{}</Target>",
            hms(secs)
        );
        self.soap(&self.control, "AVTransport", "Seek", &args)
    }

    fn stop(&self) -> Result<()> {
        self.soap(&self.control, "AVTransport", "Stop", "<InstanceID>0</InstanceID>")
    }

    fn volume(&self, v: f32) -> Result<()> {
        let Some(control) = &self.render_control else {
            bail!("у {} нет RenderingControl", self.device.name);
        };
        let level = (v.clamp(0.0, 1.0) * 100.0).round() as u8;
        let args = format!(
            "<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredVolume>{level}</DesiredVolume>"
        );
        self.soap(control, "RenderingControl", "SetVolume", &args)
    }
}

fn didl(url: &str, title: &str, artist: &str, mime: &str) -> String {
    format!(
        "<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\">\
<item id=\"1\" parentID=\"0\" restricted=\"1\">\
<dc:title>{}</dc:title><dc:creator>{}</dc:creator>\
<upnp:class>object.item.audioItem.musicTrack</upnp:class>\
<res protocolInfo=\"http-get:*:{mime}:*\">{}</res>\
</item></DIDL-Lite>",
        xml_escape(title),
        xml_escape(artist),
        xml_escape(url)
    )
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn hms(secs: f64) -> String {
    let total = secs.max(0.0) as u64;
    format!("{:02}:{:02}:{:02}", total / 3600, (total / 60) % 60, total % 60)
}

/// First `<service>` block that mentions `needle`, then its controlURL.
fn service_control(xml: &str, needle: &str) -> Option<String> {
    let lower = xml.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find("<service>") {
        let start = from + rel;
        let end = start + lower[start..].find("</service>")?;
        if lower[start..end].contains(needle) {
            return tag_text(&xml[start..end], "controlURL");
        }
        from = end + "</service>".len();
    }
    None
}

fn tag_text(xml: &str, tag: &str) -> Option<String> {
    let lower = xml.to_ascii_lowercase();
    let open = format!("<{}", tag.to_ascii_lowercase());
    let close = format!("</{}", tag.to_ascii_lowercase());
    let start = lower.find(&open)?;
    let text = start + lower[start..].find('>')? + 1;
    let end = text + lower[text..].find(&close)?;
    Some(xml[text..end].trim().to_string())
}

// ---------------------------------------------------------------------------
// The sheet
// ---------------------------------------------------------------------------

/// A small glass list of reachable speakers. `status` doubles as the footer
/// ("ищу…", "ничего не нашлось") and as the marker of the connected device: the
/// row whose name appears in it takes the accent.
pub fn render_sheet(
    p: Palette,
    devices: &[Device],
    status: &str,
    on_pick: impl Fn(usize) + Clone + 'static,
) -> impl IntoElement {
    let mut sheet = div()
        .flex()
        .flex_col()
        .w(px(244.))
        .p(px(6.))
        .rounded(px(16.))
        .bg(p.card)
        .border_1()
        .border_color(p.hairline)
        .shadow(vec![BoxShadow {
            color: p.shadow.opacity(0.5),
            offset: point(px(0.), px(18.)),
            blur_radius: px(40.),
            spread_radius: px(-8.),
        }]);

    sheet = sheet.child(
        div()
            .h(px(34.))
            .px(px(8.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .child(div().size(px(5.)).rounded(px(999.)).bg(p.accent))
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.ink)
                            .child("искать"),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_size(px(11.))
                    .text_color(p.ash)
                    .child(status.to_string()),
            ),
    );
    sheet = sheet.child(div().h(px(1.)).mx(px(8.)).bg(p.hairline_soft));

    if devices.is_empty() {
        sheet = sheet.child(
            div()
                .px(px(10.))
                .py(px(12.))
                .text_size(px(11.))
                .text_color(p.ash)
                .child("устройств не видно"),
        );
    }

    for (i, device) in devices.iter().enumerate() {
        let pick = on_pick.clone();
        let connected = !device.name.is_empty() && status.contains(device.name.as_str());
        sheet = sheet.child(
            div()
                .id(SharedString::from(format!("cast-device-{i}")))
                .h(px(44.))
                .px(px(10.))
                .rounded(px(10.))
                .flex()
                .items_center()
                .gap(px(9.))
                .cursor(CursorStyle::PointingHand)
                .when(connected, |el| el.bg(p.accent_soft))
                .hover(|el| el.bg(p.card_hover))
                .child(
                    div()
                        .w(px(3.))
                        .h(px(18.))
                        .rounded(px(999.))
                        .bg(if connected { p.accent } else { p.hairline }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(
                            div()
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if connected { p.accent } else { p.ink })
                                .child(device.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(p.ash)
                                .child(device.kind.label()),
                        ),
                )
                .on_click(move |_, _, _| pick(i)),
        );
    }
    sheet
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_envelope_round_trips() {
        let framed = encode_cast_message(
            "sender-0",
            "receiver-0",
            "urn:x-cast:com.google.cast.tp.connection",
            &json!({"type": "CONNECT"}),
        );
        let len = u32::from_be_bytes(framed[..4].try_into().unwrap()) as usize;
        assert_eq!(len, framed.len() - 4, "the length prefix must match");

        let back = decode_cast_message(&framed[4..]).expect("decodes");
        assert_eq!(back.source, "sender-0");
        assert_eq!(back.destination, "receiver-0");
        assert_eq!(back.namespace, "urn:x-cast:com.google.cast.tp.connection");
        assert_eq!(back.payload["type"], "CONNECT");
    }

    #[test]
    fn a_long_payload_is_still_framed_correctly() {
        let pad = "x".repeat(300);
        let framed = encode_cast_message("s", "d", "n", &json!({"type": "LOAD", "pad": pad}));
        let len = u32::from_be_bytes(framed[..4].try_into().unwrap()) as usize;
        assert_eq!(len, framed.len() - 4);

        let back = decode_cast_message(&framed[4..]).expect("decodes");
        assert_eq!(back.payload["pad"], "x".repeat(300));
    }

    #[test]
    fn an_ssdp_response_gives_the_description_url() {
        let raw = "HTTP/1.1 200 OK\r\n\
CACHE-CONTROL: max-age=1800\r\n\
DATE: Sun, 08 Nov 2026 00:00:00 GMT\r\n\
EXT:\r\n\
LOCATION: http://192.168.1.10:9197/dmr\r\n\
SERVER: Linux/4.4 UPnP/1.0 MiniDLNA/1.3\r\n\
ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\
USN: uuid:4d696e69-444c-164e-9d41-6c1f5b3f3e7a::urn:schemas-upnp-org:device:MediaRenderer:1\r\n\
\r\n";
        let reply = parse_ssdp_response(raw).expect("parses");
        assert_eq!(reply.location, "http://192.168.1.10:9197/dmr");
        assert!(reply.st.contains("MediaRenderer"));

        let device = device_from_ssdp(&reply, Ipv4Addr::new(192, 168, 1, 10)).expect("a device");
        assert_eq!(device.kind, Kind::Dlna);
        assert_eq!(device.host, "192.168.1.10");
        assert_eq!(device.port, 9197);
        assert_eq!(device.location, "http://192.168.1.10:9197/dmr");
    }

    #[test]
    fn the_avtransport_control_url_is_relative_to_the_description() {
        let xml = "<root><device><serviceList><service>\
<serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType>\
<controlURL>/rc</controlURL></service><service>\
<serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType>\
<controlURL>/upnp/control/avt</controlURL></service></serviceList></device></root>";
        let control = service_control(xml, "avtransport").expect("found");
        assert_eq!(control, "/upnp/control/avt");
        assert_eq!(
            resolve_url("http://192.168.1.10:9197/dmr", &control),
            "http://192.168.1.10:9197/upnp/control/avt"
        );
    }

    #[test]
    #[ignore = "needs a LAN with speakers"]
    fn probe_discovery() {
        let found = discover(Duration::from_secs(2));
        println!("найдено устройств: {}", found.len());
        for device in &found {
            println!(
                "  {} · {} · {}:{} · {}",
                device.kind.label(),
                device.name,
                device.host,
                device.port,
                device.id
            );
        }
    }
}
