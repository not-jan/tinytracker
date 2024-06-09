use std::{
    fmt::{Display, Formatter},
    io::Cursor,
    net::Ipv4Addr,
};

use anyhow::anyhow;
use binrw::{binrw, helpers::until_eof, BinRead, BinReaderExt, BinResult, BinWrite, BinWriterExt};
use tokio_util::{
    bytes::{Buf, BytesMut},
    codec::{Decoder, Encoder},
};

pub struct TrackerCodec {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackerPacket {
    ConnectRequest(ConnectRequest),
    ConnectResponse(ConnectResponse),
    AnnounceRequest(AnnounceRequest),
    AnnounceResponse(AnnounceResponse),
    ScrapeRequest(ScrapeRequest),
    ScrapeResponse(ScrapeResponse),
}

impl Encoder<TrackerPacket> for TrackerCodec {
    type Error = anyhow::Error;

    fn encode(
        &mut self,
        item: TrackerPacket,
        dst: &mut BytesMut,
    ) -> std::result::Result<(), Self::Error> {
        let mut cursor = Cursor::new(Vec::new());

        match item {
            TrackerPacket::ConnectRequest(req) => {
                cursor.write_be(&req).map_err(|e| anyhow!(e))?;
            }
            TrackerPacket::ConnectResponse(res) => {
                cursor.write_be(&res).map_err(|e| anyhow!(e))?;
            }
            TrackerPacket::AnnounceRequest(req) => {
                cursor.write_be(&req).map_err(|e| anyhow!(e))?;
            }
            TrackerPacket::AnnounceResponse(res) => {
                cursor.write_be(&res).map_err(|e| anyhow!(e))?;
            }
            TrackerPacket::ScrapeRequest(req) => {
                cursor.write_be(&req).map_err(|e| anyhow!(e))?;
            }
            TrackerPacket::ScrapeResponse(res) => {
                cursor.write_be(&res).map_err(|e| anyhow!(e))?;
            }
        }

        cursor.set_position(0);

        dst.extend_from_slice(cursor.get_ref().as_slice());

        Ok(())
    }
}

const CONNECT_MAGIC: u64 = 0x41727101980;

impl Decoder for TrackerCodec {
    type Item = TrackerPacket;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 16 {
            Ok(None)
        } else {
            let mut magic_bytes = [0u8; 8];
            magic_bytes.copy_from_slice(&src[..8]);
            let magic = u64::from_be_bytes(magic_bytes);

            let mut reader = Cursor::new(src.to_vec());

            if magic == CONNECT_MAGIC {
                // This is most certainly a ConnectRequest
                src.advance(src.len());
                return Ok(Some(TrackerPacket::ConnectRequest(reader.read_be()?)));
            }

            let mut request_action_bytes = [0u8; 4];
            request_action_bytes.copy_from_slice(&src[8..12]);
            let request_action = u32::from_be_bytes(request_action_bytes);
            src.advance(src.len());

            let response_action = (magic >> 32) as u32;

            match Action::try_from(request_action) {
                Ok(Action::Announce) => {
                    return Ok(Some(TrackerPacket::AnnounceRequest(reader.read_be()?)))
                }
                Ok(Action::Scrape) => {
                    return Ok(Some(TrackerPacket::ScrapeRequest(reader.read_be()?)))
                }
                _ => {}
            }

            match Action::try_from(response_action) {
                Ok(Action::Connect) => {
                    return Ok(Some(TrackerPacket::ConnectResponse(reader.read_be()?)))
                }
                Ok(Action::Announce) => {
                    return Ok(Some(TrackerPacket::AnnounceResponse(reader.read_be()?)))
                }
                Ok(Action::Scrape) => {
                    return Ok(Some(TrackerPacket::ScrapeResponse(reader.read_be()?)))
                }
                _ => {}
            }

            Ok(None)
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[binrw]
#[brw(big, repr = u32)]
#[repr(u32)]
pub enum Action {
    Connect = 0,
    Announce = 1,
    Scrape = 2,
}

impl TryFrom<u32> for Action {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Action::Connect),
            1 => Ok(Action::Announce),
            2 => Ok(Action::Scrape),
            _ => Err(anyhow!("Unknown action: {}", value)),
        }
    }
}

impl From<Action> for u32 {
    fn from(value: Action) -> Self {
        value as u32
    }
}

// Offset  Size            Name            Value
// 0       64-bit integer  protocol_id     0x41727101980 // magic constant
// 8       32-bit integer  action          0 // connect
// 12      32-bit integer  transaction_id
// 16

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[binrw(magic = 0x41727101980)]
#[brw(big)]
#[repr(C)]
pub struct ConnectRequest {
    pub protocol_id: u64,
    pub action: Action,
    pub transaction_id: u32,
}

// Offset  Size            Name            Value
// 0       32-bit integer  action          0 // connect
// 4       32-bit integer  transaction_id
// 8       64-bit integer  connection_id
// 16

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[binrw]
#[brw(big)]
#[repr(C)]
pub struct ConnectResponse {
    pub action: Action,
    pub transaction_id: u32,
    pub connection_id: u64,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[binrw]
#[brw(big, repr = u32)]
#[repr(u32)]
pub enum AnnounceEvent {
    None = 0,
    Completed = 1,
    Started = 2,
    Stopped = 3,
}

impl TryFrom<u32> for AnnounceEvent {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(AnnounceEvent::None),
            1 => Ok(AnnounceEvent::Completed),
            2 => Ok(AnnounceEvent::Started),
            3 => Ok(AnnounceEvent::Stopped),
            _ => Err(anyhow!("Unsupported announce event: {}!", value)),
        }
    }
}

impl From<AnnounceEvent> for u32 {
    fn from(value: AnnounceEvent) -> Self {
        value as u32
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[binrw]
pub struct PeerId {
    pub inner: [u8; 20],
}

impl Display for PeerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match std::str::from_utf8(self.inner.as_slice()) {
            Ok(id) => f.write_str(id),
            Err(_) => write!(f, "{:#02X?}", self.inner),
        }
    }
}

// Offset  Size    Name    Value
// 0       64-bit integer  connection_id
// 8       32-bit integer  action          1 // announce
// 12      32-bit integer  transaction_id
// 16      20-byte string  info_hash
// 36      20-byte string  peer_id
// 56      64-bit integer  downloaded
// 64      64-bit integer  left
// 72      64-bit integer  uploaded
// 80      32-bit integer  event           0 // 0: none; 1: completed; 2: started; 3: stopped
// 84      32-bit integer  IP address      0 // default
// 88      32-bit integer  key
// 92      32-bit integer  num_want        -1 // default
// 96      16-bit integer  port
// 98

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[binrw]
#[brw(big)]
#[repr(C)]
pub struct AnnounceRequest {
    pub connection_id: u64,
    pub action: Action,
    pub transaction_id: u32,
    pub info_hash: InfoHash,
    pub peer_id: PeerId,
    pub downloaded: u64,
    pub left: u64,
    pub uploaded: u64,
    pub event: AnnounceEvent,
    #[br(parse_with = ip_addr_parser)]
    #[bw(write_with = ip_addr_writer)]
    pub ip_address: Ipv4Addr,
    pub key: u32,
    pub peers_wanted: u32,
    pub port: Option<u16>,
}

// Offset      Size            Name            Value
// 0           32-bit integer  action          1 // announce
// 4           32-bit integer  transaction_id
// 8           32-bit integer  interval
// 12          32-bit integer  leechers
// 16          32-bit integer  seeders
// 20 + 6 * n  32-bit integer  IP address
// 24 + 6 * n  16-bit integer  TCP port
// 20 + 6 * N

#[derive(Debug, Clone, Eq, PartialEq)]
#[binrw]
#[brw(big)]
#[repr(C)]
pub struct AnnounceResponse {
    pub action: Action,
    pub transaction_id: u32,
    pub interval: u32,
    pub leechers: u32,
    pub seeders: u32,
    #[br(parse_with = until_eof)]
    pub peers: Vec<Peer>,
}

// Offset          Size            Name            Value
// 0               64-bit integer  connection_id
// 8               32-bit integer  action          2 // scrape
// 12              32-bit integer  transaction_id
// 16 + 20 * n     20-byte string  info_hash
// 16 + 20 * N

#[derive(Debug, Clone, Eq, PartialEq)]
#[binrw]
#[brw(big)]
#[repr(C)]
pub struct ScrapeRequest {
    pub connection_id: u64,
    pub action: Action,
    pub transaction_id: u32,
    #[br(parse_with = until_eof)]
    pub hashes: Vec<InfoHash>,
}

// Offset      Size            Name            Value
// 0           32-bit integer  action          2 // scrape
// 4           32-bit integer  transaction_id
// 8 + 12 * n  32-bit integer  seeders
// 12 + 12 * n 32-bit integer  completed
// 16 + 12 * n 32-bit integer  leechers
// 8 + 12 * N

#[derive(Debug, Clone, Eq, PartialEq)]
#[binrw]
#[brw(big)]
#[repr(C)]
pub struct ScrapeResponse {
    pub action: Action,
    pub transaction_id: u32,
    #[br(parse_with = until_eof)]
    pub data: Vec<ScrapeData>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[binrw]
#[brw(big)]
#[repr(C)]
pub struct Peer {
    #[br(parse_with = ip_addr_parser)]
    #[bw(write_with = ip_addr_writer)]
    pub ip_address: Ipv4Addr,
    pub port: u16,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[binrw]
#[brw(big)]
#[repr(C)]
pub struct InfoHash {
    pub hash: [u8; 20],
}

impl Display for InfoHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.hash.iter().try_for_each(|b| write!(f, "{:02X}", b))
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
#[binrw]
#[brw(big)]
#[repr(C)]
pub struct ScrapeData {
    pub seeders: u32,
    pub completed: u32,
    pub leechers: u32,
}

#[binrw::parser(reader, endian)]
fn ip_addr_parser() -> BinResult<Ipv4Addr> {
    let ip: u32 = <_>::read_options(reader, endian, ())?;
    Ok(Ipv4Addr::from(ip))
}

#[binrw::writer(writer, endian)]
fn ip_addr_writer(ip: &Ipv4Addr) -> BinResult<()> {
    let ip: u32 = (*ip).into();
    ip.write_options(writer, endian, ())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use bytes::BytesMut;
    use rand::Rng;
    use tokio_util::codec::{Decoder, Encoder};

    use super::*;

    #[tokio::test]
    async fn test_decode_tracker_codec() {
        let mut tracker_codec = TrackerCodec {};
        let connect_request = ConnectRequest {
            protocol_id: CONNECT_MAGIC,
            action: Action::Connect,
            transaction_id: 123,
        };
        let mut bytes = BytesMut::with_capacity(16);
        let mut cursor = Cursor::new(Vec::new());
        cursor.write_be(&connect_request).unwrap();

        cursor.set_position(0);

        bytes.extend_from_slice(cursor.get_ref().as_slice());

        let decode_result = tracker_codec.decode(&mut bytes);

        assert!(decode_result.is_ok());
        assert_eq!(decode_result.unwrap().unwrap(), TrackerPacket::ConnectRequest(connect_request));
    }

    #[tokio::test]
    async fn test_random_bytes() {


        for _ in 0..100 {
            let mut tracker_codec = TrackerCodec {};
            let mut buf = [0u8; 32];

            rand::thread_rng().fill(&mut buf);

            let mut bytes = BytesMut::from(buf.as_slice());

            assert!(tracker_codec.decode(&mut bytes).is_ok());
        }

    }

    #[tokio::test]
    async fn test_encode_tracker_codec() {
        let mut tracker_codec = TrackerCodec {};
        let connect_request = ConnectRequest {
            protocol_id: CONNECT_MAGIC,
            action: Action::Connect,
            transaction_id: 123,
        };
        let mut bytes = BytesMut::with_capacity(16);
        let encode_result =
            tracker_codec.encode(TrackerPacket::ConnectRequest(connect_request), &mut bytes);

        let mut cursor = Cursor::new(Vec::new());
        cursor.write_be(&connect_request).unwrap();
        cursor.set_position(0);

        let correct_bytes = cursor.get_ref().as_slice();

        assert!(encode_result.is_ok());
        assert_eq!(bytes, correct_bytes);
    }
}
