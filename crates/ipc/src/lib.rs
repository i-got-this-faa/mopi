use lss_types::{
    DaemonStats, DaemonStatus, DoctorReport, FailureRecord, QueryId, RootSummary, SearchQuery,
    SearchResult,
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
    GetFailures {
        limit: usize,
    },
    Search {
        query_id: QueryId,
        query: SearchQuery,
    },
    CancelSearch {
        query_id: QueryId,
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
    ListFailures(Vec<FailureRecord>),
    Error {
        message: String,
    },
    SearchResults {
        query_id: QueryId,
        results: Vec<SearchResult>,
    },
    SearchResultChunk {
        query_id: QueryId,
        results: Vec<SearchResult>,
        is_final: bool,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio::io::BufWriter;

    #[tokio::test]
    async fn write_and_read_roundtrip_ping() {
        let envelope = RequestEnvelope::new(Request::Ping);
        let mut buf = Vec::new();
        write_frame(&mut buf, &envelope).await.expect("write failed");

        let mut cursor = Cursor::new(buf);
        let decoded: RequestEnvelope = read_frame(&mut cursor).await.expect("read failed");
        assert_eq!(decoded, envelope);
    }

    #[tokio::test]
    async fn write_and_read_roundtrip_search() {
        let query = SearchQuery::new("hello world");
        let envelope = RequestEnvelope::new(Request::Search {
            query_id: QueryId::new(),
            query,
        });
        let mut buf = Vec::new();
        write_frame(&mut buf, &envelope).await.expect("write failed");

        let mut cursor = Cursor::new(buf);
        let decoded: RequestEnvelope = read_frame(&mut cursor).await.expect("read failed");
        assert_eq!(decoded.protocol_version, envelope.protocol_version);
        assert!(matches!(decoded.request, Request::Search { .. }));
    }

    #[tokio::test]
    async fn write_and_read_roundtrip_response() {
        let envelope = ResponseEnvelope::new(Response::Pong {
            protocol_version: PROTOCOL_VERSION,
        });
        let mut buf = Vec::new();
        write_frame(&mut buf, &envelope).await.expect("write failed");

        let mut cursor = Cursor::new(buf);
        let decoded: ResponseEnvelope = read_frame(&mut cursor).await.expect("read failed");
        assert_eq!(decoded, envelope);
    }

    #[tokio::test]
    async fn oversized_frame_is_rejected_on_write() {
        let huge = "x".repeat((MAX_FRAME_BYTES as usize) + 1);
        let envelope = ResponseEnvelope::new(Response::Error { message: huge });
        let mut buf = Vec::new();
        let result = write_frame(&mut buf, &envelope).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("exceeded maximum size"));
    }

    #[tokio::test]
    async fn oversized_frame_is_rejected_on_read() {
        let mut buf = Vec::new();
        let bad_len = MAX_FRAME_BYTES + 1;
        buf.extend_from_slice(&bad_len.to_be_bytes());
        buf.extend_from_slice(b"{}");

        let mut cursor = Cursor::new(buf);
        let result: anyhow::Result<RequestEnvelope> = read_frame(&mut cursor).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("invalid size"));
    }

    #[tokio::test]
    async fn malformed_json_fails_gracefully() {
        let mut buf = Vec::new();
        let bad_json = b"not json at all";
        buf.extend_from_slice(&(bad_json.len() as u32).to_be_bytes());
        buf.extend_from_slice(bad_json);

        let mut cursor = Cursor::new(buf);
        let result: anyhow::Result<RequestEnvelope> = read_frame(&mut cursor).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn multiple_frames_in_sequence() {
        let ping = RequestEnvelope::new(Request::Ping);
        let status = RequestEnvelope::new(Request::GetStatus);

        let mut buf = Vec::new();
        write_frame(&mut buf, &ping).await.expect("write ping failed");
        write_frame(&mut buf, &status).await.expect("write status failed");

        let mut cursor = Cursor::new(buf);
        let decoded1: RequestEnvelope = read_frame(&mut cursor).await.expect("read 1 failed");
        let decoded2: RequestEnvelope = read_frame(&mut cursor).await.expect("read 2 failed");

        assert!(matches!(decoded1.request, Request::Ping));
        assert!(matches!(decoded2.request, Request::GetStatus));
    }

    #[tokio::test]
    async fn protocol_version_is_preserved() {
        let envelope = RequestEnvelope::new(Request::Doctor);
        assert_eq!(envelope.protocol_version, PROTOCOL_VERSION);

        let response = ResponseEnvelope::new(Response::Ack {
            message: "ok".into(),
        });
        assert_eq!(response.protocol_version, PROTOCOL_VERSION);
    }

    #[tokio::test]
    async fn write_frame_with_bufwriter() {
        let envelope = RequestEnvelope::new(Request::Ping);
        let inner = Vec::new();
        let mut writer = BufWriter::new(inner);
        write_frame(&mut writer, &envelope).await.expect("write failed");
        let buf = writer.into_inner();
        assert!(!buf.is_empty());
    }

    #[tokio::test]
    async fn cancel_search_roundtrip() {
        let envelope = RequestEnvelope::new(Request::CancelSearch {
            query_id: QueryId::new(),
        });
        let mut buf = Vec::new();
        write_frame(&mut buf, &envelope).await.expect("write failed");

        let mut cursor = Cursor::new(buf);
        let decoded: RequestEnvelope = read_frame(&mut cursor).await.expect("read failed");
        assert!(matches!(decoded.request, Request::CancelSearch { .. }));
    }

    #[tokio::test]
    async fn search_result_chunk_roundtrip() {
        let chunk = ResponseEnvelope::new(Response::SearchResultChunk {
            query_id: QueryId::new(),
            results: vec![],
            is_final: false,
        });
        let mut buf = Vec::new();
        write_frame(&mut buf, &chunk).await.expect("write failed");

        let mut cursor = Cursor::new(buf);
        let decoded: ResponseEnvelope = read_frame(&mut cursor).await.expect("read failed");
        assert!(matches!(
            decoded.response,
            Response::SearchResultChunk { is_final: false, .. }
        ));
    }

    #[tokio::test]
    async fn search_result_chunk_final_roundtrip() {
        let chunk = ResponseEnvelope::new(Response::SearchResultChunk {
            query_id: QueryId::new(),
            results: vec![],
            is_final: true,
        });
        let mut buf = Vec::new();
        write_frame(&mut buf, &chunk).await.expect("write failed");

        let mut cursor = Cursor::new(buf);
        let decoded: ResponseEnvelope = read_frame(&mut cursor).await.expect("read failed");
        assert!(matches!(
            decoded.response,
            Response::SearchResultChunk { is_final: true, .. }
        ));
    }

    #[tokio::test]
    async fn streaming_sequence_assembles_correctly() {
        let chunk1 = Response::SearchResultChunk {
            query_id: QueryId::new(),
            results: vec![],
            is_final: false,
        };
        let chunk2 = Response::SearchResultChunk {
            query_id: QueryId::new(),
            results: vec![],
            is_final: true,
        };

        let mut buf = Vec::new();
        write_frame(&mut buf, &ResponseEnvelope::new(chunk1.clone()))
            .await
            .expect("write failed");
        write_frame(&mut buf, &ResponseEnvelope::new(chunk2.clone()))
            .await
            .expect("write failed");

        let mut cursor = Cursor::new(buf);
        let decoded1: ResponseEnvelope = read_frame(&mut cursor).await.expect("read 1 failed");
        let decoded2: ResponseEnvelope = read_frame(&mut cursor).await.expect("read 2 failed");

        assert!(matches!(decoded1.response, Response::SearchResultChunk { is_final: false, .. }));
        assert!(matches!(decoded2.response, Response::SearchResultChunk { is_final: true, .. }));
    }
}
