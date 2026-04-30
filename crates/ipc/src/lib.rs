use mopi_types::{
    DaemonStats, DaemonStatus, DoctorReport, QueryId, RootSummary, SearchQuery, SearchResult,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTOCOL_VERSION: u32 = 1;
const MAX_FRAME_BYTES: u32 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub protocol_version: u32,
    pub request: Request,
}

impl RequestEnvelope {
    #[must_use]
    pub fn new(request: Request) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub protocol_version: u32,
    pub response: Response,
}

impl ResponseEnvelope {
    #[must_use]
    pub fn new(response: Response) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            response,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Request {
    Ping,
    GetStatus,
    GetStats,
    ListRoots,
    ReloadConfig,
    RefreshChanged,
    Doctor,
    Search {
        query_id: QueryId,
        query: SearchQuery,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Response {
    Pong {
        protocol_version: u32,
    },
    Status(DaemonStatus),
    Stats(DaemonStats),
    Roots(Vec<RootSummary>),
    Ack {
        message: String,
    },
    Doctor(DoctorReport),
    Error {
        message: String,
    },
    SearchResults {
        query_id: QueryId,
        results: Vec<SearchResult>,
    },
}

pub async fn write_frame<T>(writer: &mut (impl AsyncWrite + Unpin), value: &T) -> anyhow::Result<()>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_FRAME_BYTES as usize {
        anyhow::bail!("IPC frame exceeded maximum size of {MAX_FRAME_BYTES} bytes");
    }
    writer.write_u32(bytes.len() as u32).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_frame<T>(reader: &mut (impl AsyncRead + Unpin)) -> anyhow::Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let len = reader.read_u32().await?;
    if len > MAX_FRAME_BYTES {
        anyhow::bail!("IPC frame declared invalid size: {len} bytes");
    }
    let mut bytes = vec![0_u8; len as usize];
    reader.read_exact(&mut bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}
