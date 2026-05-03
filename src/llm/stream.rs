/// A parsed Server-Sent Events line pair.
#[derive(Debug, Default)]
pub struct SseLine {
    pub event: Option<String>,
    pub data: Option<String>,
}

/// Parse raw bytes from an SSE stream into complete (`event`, `data`) pairs.
/// Yields one `SseLine` per blank-line-terminated SSE message.
pub struct SseParser {
    buf: String,
    current: SseLine,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            current: SseLine::default(),
        }
    }

    /// Feed raw bytes; returns any complete SSE messages found.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<SseLine> {
        self.buf.push_str(&String::from_utf8_lossy(bytes));
        let mut out = Vec::new();

        while let Some(newline_pos) = self.buf.find('\n') {
            let line = self.buf[..newline_pos].trim_end_matches('\r').to_string();
            self.buf = self.buf[newline_pos + 1..].to_string();

            if line.is_empty() {
                // Blank line = end of SSE message
                let msg = std::mem::take(&mut self.current);
                if msg.data.is_some() || msg.event.is_some() {
                    out.push(msg);
                }
            } else if let Some(rest) = line.strip_prefix("event:") {
                self.current.event = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("data:") {
                self.current.data = Some(rest.trim().to_string());
            }
            // ignore `id:`, `retry:`, comments
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_event() {
        let mut p = SseParser::new();
        let msgs = p.feed(b"event: content_block_delta\ndata: {\"type\":\"text\"}\n\n");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].event.as_deref(), Some("content_block_delta"));
        assert_eq!(msgs[0].data.as_deref(), Some("{\"type\":\"text\"}"));
    }

    #[test]
    fn parses_data_only_event() {
        let mut p = SseParser::new();
        let msgs = p.feed(b"data: hello\n\n");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].data.as_deref(), Some("hello"));
    }

    #[test]
    fn handles_chunked_delivery() {
        let mut p = SseParser::new();
        let mut msgs = p.feed(b"data: hel");
        assert!(msgs.is_empty());
        msgs.extend(p.feed(b"lo\n\n"));
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].data.as_deref(), Some("hello"));
    }

    #[test]
    fn parses_multiple_messages() {
        let mut p = SseParser::new();
        let msgs = p.feed(b"data: a\n\ndata: b\n\n");
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn skips_empty_messages() {
        let mut p = SseParser::new();
        let msgs = p.feed(b"\n\ndata: x\n\n");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].data.as_deref(), Some("x"));
    }
}
