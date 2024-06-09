#![warn(rust_2018_idioms)]

use std::{
    net::{AddrParseError, Ipv4Addr, SocketAddrV4},
    str::FromStr,
};
use std::net::SocketAddr;

use anyhow::Result;

use log::{debug, info, warn};
use tokio::{net::UdpSocket, signal};
use tokio_stream::StreamExt;
use tokio_util::udp::UdpFramed;

mod codec;

use clap::Parser;
use futures::SinkExt;

use crate::codec::{
    Action, AnnounceResponse, ConnectResponse, Peer, ScrapeData, ScrapeResponse, TrackerCodec,
    TrackerPacket,
};

#[derive(Parser, Debug)]
#[clap(version = "1.0", author = "not-jan")]
/// Structure representing command line options for the program.
pub struct Opts {
    /// The IPv4 address that the program should listen on.
    #[clap(
        short = 'l',
        long = "listen-address",
        env = "LISTEN_ADDRESS",
        default_value = "0.0.0.0"
    )]
    pub listen_address: String,

    /// The port that the program should bind to.
    #[clap(short = 'p', long = "port", env = "LISTEN_PORT", default_value = "8080")]
    pub listen_port: u16,

    /// List of static peers that the tracker should advertise.
    #[clap(short = 's', env = "STATIC_PEERS", long = "static-peer")]
    pub static_peers: Vec<String>,

    /// Time between announces that this tracker asks for in seconds
    #[clap(
        short = 'i',
        env = "ANNOUNCE_INTERVAL",
        long = "announce-interval",
        default_value = "600"
    )]
    pub interval: u32,
}

struct Tracker {
    frame: UdpFramed<TrackerCodec>,
    static_peers: Vec<Peer>,
    interval: u32,
}

impl Tracker {
    pub fn new(socket: UdpSocket, static_peers: Vec<SocketAddrV4>, interval: u32) -> Self {
        let frame = UdpFramed::new(socket, TrackerCodec {});
        let static_peers = static_peers
            .into_iter()
            .map(|addr| Peer { ip_address: *addr.ip(), port: addr.port() })
            .collect();
        Tracker { frame, static_peers, interval }
    }

    async fn handle_packet(&self, packet: TrackerPacket, addr: SocketAddr) -> Result<Option<TrackerPacket>> {
        Ok(match packet {
            TrackerPacket::ConnectRequest(request) => {
                debug!("[{addr}] Received connect request");
                Some(TrackerPacket::ConnectResponse(ConnectResponse {
                    action: Action::Connect,
                    transaction_id: request.transaction_id,
                    connection_id: rand::random(),
                }))
            }
            TrackerPacket::AnnounceRequest(request) => {
                debug!("[{addr}] Received announce request: {}", request.info_hash);
                Some(TrackerPacket::AnnounceResponse(AnnounceResponse {
                    action: Action::Announce,
                    transaction_id: request.transaction_id,
                    interval: self.interval,
                    leechers: 0,
                    seeders: self.static_peers.len() as u32,
                    peers: self.static_peers.clone(),
                }))
            }
            TrackerPacket::ScrapeRequest(request) => {
                debug!("[{addr}] Received scrape request for {} hashes", request.hashes.len());
                Some(TrackerPacket::ScrapeResponse(ScrapeResponse {
                    action: Action::Scrape,
                    transaction_id: request.transaction_id,
                    data: request
                        .hashes
                        .into_iter()
                        .map(|_| ScrapeData {
                            seeders: self.static_peers.len() as u32,
                            completed: self.static_peers.len() as u32,
                            leechers: 0,
                        })
                        .collect::<Vec<_>>(),
                }))
            }
            _ => None,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                _ = signal::ctrl_c() => {
                    info!("Received Ctrl-C, shutting down");
                    break;
                },
                result = self.frame.next() => match result {
                    Some(Err(e)) => {
                        debug!("Failed to parse request: {}", e);
                        continue;
                    },
                    None => break,
                    Some(Ok((packet, addr))) => match self.handle_packet(packet, addr).await {
                        Ok(Some(response)) => if let Err(e) = self.frame.send((response, addr)).await {
                            warn!("[{}] Failed to send reply: {}", addr, e);
                        },
                        // Client sent a valid packet, but it doesn't warrant a reply.
                        Ok(None) => {}
                        // Client sent something invalid!
                        Err(e) => {
                            debug!("[{}] Received invalid packet: {}", addr, e);
                        }
                    },
                }
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Opts::parse();

    let ip = Ipv4Addr::from_str(&args.listen_address)?;
    let addr = SocketAddrV4::new(ip, args.listen_port);

    let socket = UdpSocket::bind(&addr).await?;
    info!("Listening on: {}", socket.local_addr()?);

    let static_peers = args
        .static_peers
        .into_iter()
        .map(|peer| SocketAddrV4::from_str(&peer))
        .collect::<Result<Vec<SocketAddrV4>, AddrParseError>>()?;

    info!("Loaded {} static peers", static_peers.len());
    let mut tracker = Tracker::new(socket, static_peers, args.interval);
    tracker.start().await?;
    Ok(())
}
