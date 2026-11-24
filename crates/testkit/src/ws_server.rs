use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[derive(Debug, Clone)]
pub enum WsUpgradeReply {
    Accept,
    Reject {
        status: u16,
        reason: &'static str,
        body: String,
    },
}

#[derive(Debug, Clone)]
pub enum FakeWsStep {
    ExpectText,
    SendText(String),
    SendPing(Vec<u8>),
    ExpectPong(Vec<u8>),
    SendClose(Option<u16>),
    AbruptClose,
    WaitClientClose,
    ServeUntilClientClose {
        ack_json: Option<String>,
        push_frames: Vec<String>,
        expected_pings: usize,
    },
}

#[derive(Debug, Clone)]
pub struct FakeWsSessionPlan {
    pub upgrade: WsUpgradeReply,
    pub steps: Vec<FakeWsStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PriorState {
    Closed,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsRecordedConnection {
    pub path: String,
    pub authorization: Option<String>,
    pub prior_closed_at_accept: bool,
    pub received_texts: Vec<String>,
    pub received_pongs: Vec<Vec<u8>>,
    pub client_closed: bool,
}

#[derive(Debug)]
pub struct FakeWsServer {
    port: u16,
    active: Arc<AtomicUsize>,
    max_concurrent: Arc<AtomicUsize>,
    accepted: Arc<AtomicUsize>,
    task: JoinHandle<io::Result<Vec<WsRecordedConnection>>>,
}

fn push_base64_char(output: &mut String, index: usize) {
    if let Some(byte) = BASE64_TABLE.get(index) {
        output.push(char::from(*byte));
    }
}

fn base64_encode(input: &[u8]) -> String {
    let (chunks, remainder) = input.as_chunks::<3>();
    let mut output = String::with_capacity(input.len().saturating_mul(2));
    for chunk in chunks {
        let [first, second, third] = *chunk;
        let packed =
            (usize::from(first) << 16_u32) | (usize::from(second) << 8_u32) | usize::from(third);
        push_base64_char(&mut output, (packed >> 18_u32) & 0x3F);
        push_base64_char(&mut output, (packed >> 12_u32) & 0x3F);
        push_base64_char(&mut output, (packed >> 6_u32) & 0x3F);
        push_base64_char(&mut output, packed & 0x3F);
    }
    match remainder {
        [only] => {
            let packed = usize::from(*only) << 16_u32;
            push_base64_char(&mut output, (packed >> 18_u32) & 0x3F);
            push_base64_char(&mut output, (packed >> 12_u32) & 0x3F);
            output.push('=');
            output.push('=');
        }
        [first, second] => {
            let packed = (usize::from(*first) << 16_u32) | (usize::from(*second) << 8_u32);
            push_base64_char(&mut output, (packed >> 18_u32) & 0x3F);
            push_base64_char(&mut output, (packed >> 12_u32) & 0x3F);
            push_base64_char(&mut output, (packed >> 6_u32) & 0x3F);
            output.push('=');
        }
        _ => {}
    }
    output
}

fn word_at(words: &[u32; 80], index: usize) -> u32 {
    words.get(index).copied().unwrap_or(0)
}

fn sha1_compress(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut words = [0_u32; 80];
    let (word_chunks, _rest) = block.as_chunks::<4>();
    for (slot, chunk) in words.iter_mut().take(16).zip(word_chunks.iter()) {
        *slot = u32::from_be_bytes(*chunk);
    }
    for index in 16_usize..80_usize {
        let mixed = word_at(&words, index.saturating_sub(3))
            ^ word_at(&words, index.saturating_sub(8))
            ^ word_at(&words, index.saturating_sub(14))
            ^ word_at(&words, index.saturating_sub(16));
        if let Some(slot) = words.get_mut(index) {
            *slot = mixed.rotate_left(1);
        }
    }
    let [mut reg_a, mut reg_b, mut reg_c, mut reg_d, mut reg_e] = *state;
    for (step, word) in words.iter().enumerate() {
        let (func, constant) = match step {
            0..=19 => ((reg_b & reg_c) | ((!reg_b) & reg_d), 0x5A82_7999_u32),
            20..=39 => (reg_b ^ reg_c ^ reg_d, 0x6ED9_EBA1_u32),
            40..=59 => (
                (reg_b & reg_c) | (reg_b & reg_d) | (reg_c & reg_d),
                0x8F1B_BCDC_u32,
            ),
            _ => (reg_b ^ reg_c ^ reg_d, 0xCA62_C1D6_u32),
        };
        let next_a = reg_a
            .rotate_left(5)
            .wrapping_add(func)
            .wrapping_add(reg_e)
            .wrapping_add(constant)
            .wrapping_add(*word);
        reg_e = reg_d;
        reg_d = reg_c;
        reg_c = reg_b.rotate_left(30);
        reg_b = reg_a;
        reg_a = next_a;
    }
    *state = [
        state[0].wrapping_add(reg_a),
        state[1].wrapping_add(reg_b),
        state[2].wrapping_add(reg_c),
        state[3].wrapping_add(reg_d),
        state[4].wrapping_add(reg_e),
    ];
}

fn sha1_digest(data: &[u8]) -> [u8; 20] {
    let mut state: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let (blocks, remainder) = data.as_chunks::<64>();
    for block in blocks {
        sha1_compress(&mut state, block);
    }
    let mut tail = [0_u8; 128];
    for (slot, byte) in tail.iter_mut().zip(remainder.iter()) {
        *slot = *byte;
    }
    if let Some(marker) = tail.get_mut(remainder.len()) {
        *marker = 0x80;
    }
    let total_len = if remainder.len() < 56 {
        64_usize
    } else {
        128_usize
    };
    let mut bit_len = 0_u64;
    for _byte in data {
        bit_len = bit_len.wrapping_add(8);
    }
    let len_bytes = bit_len.to_be_bytes();
    let len_offset = total_len.saturating_sub(8);
    for (offset, byte) in len_bytes.iter().enumerate() {
        if let Some(slot) = tail.get_mut(len_offset.saturating_add(offset)) {
            *slot = *byte;
        }
    }
    let (tail_blocks, _tail_rest) = tail.get(..total_len).unwrap_or(&[]).as_chunks::<64>();
    for block in tail_blocks {
        sha1_compress(&mut state, block);
    }
    let mut output = [0_u8; 20];
    let (out_chunks, _out_rest) = output.as_chunks_mut::<4>();
    for (slot, word) in out_chunks.iter_mut().zip(state.iter()) {
        *slot = word.to_be_bytes();
    }
    output
}

fn accept_key(ws_key: &str) -> String {
    let mut material = Vec::with_capacity(ws_key.len().saturating_add(WS_GUID.len()));
    material.extend_from_slice(ws_key.as_bytes());
    material.extend_from_slice(WS_GUID.as_bytes());
    base64_encode(&sha1_digest(&material))
}

struct ParsedUpgrade {
    path: String,
    authorization: Option<String>,
    ws_key: Option<String>,
}

fn read_upgrade_head(stream: &mut TcpStream) -> io::Result<ParsedUpgrade> {
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        raw.extend_from_slice(chunk.get(..count).ok_or(io::ErrorKind::InvalidData)?);
        if String::from_utf8_lossy(&raw).contains("\r\n\r\n") {
            break;
        }
    }
    let text = String::from_utf8_lossy(&raw);
    let mut lines = text.lines();
    let start_line = lines.next().unwrap_or("");
    let path = start_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_owned();
    let mut authorization = None;
    let mut ws_key = None;
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("authorization") {
                authorization = Some(value.trim().to_owned());
            } else if key.eq_ignore_ascii_case("sec-websocket-key") {
                ws_key = Some(value.trim().to_owned());
            }
        }
    }
    Ok(ParsedUpgrade {
        path,
        authorization,
        ws_key,
    })
}

fn write_server_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> io::Result<()> {
    let mut frame = Vec::with_capacity(payload.len().saturating_add(10));
    frame.push(0x80_u8 | (opcode & 0x0F_u8));
    if let Ok(short_len) = u8::try_from(payload.len())
        && short_len <= 125_u8
    {
        frame.push(short_len);
    } else if let Ok(medium_len) = u16::try_from(payload.len()) {
        frame.push(126_u8);
        frame.extend_from_slice(&medium_len.to_be_bytes());
    } else {
        let wide_len = u64::try_from(payload.len()).map_err(|_error| io::ErrorKind::InvalidData)?;
        frame.push(127_u8);
        frame.extend_from_slice(&wide_len.to_be_bytes());
    }
    frame.extend_from_slice(payload);
    stream.write_all(&frame)?;
    stream.flush()
}

enum ClientFrame {
    Text(String),
    Pong(Vec<u8>),
    Close,
    Eof,
}

fn read_client_frame(stream: &mut TcpStream) -> io::Result<ClientFrame> {
    let mut header = [0_u8; 2];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(error)
            if error.kind() == io::ErrorKind::UnexpectedEof
                || error.kind() == io::ErrorKind::ConnectionReset =>
        {
            return Ok(ClientFrame::Eof);
        }
        Err(error) => return Err(error),
    }
    let [first_byte, second_byte] = header;
    let opcode = first_byte & 0x0F_u8;
    if (second_byte & 0x80_u8) == 0_u8 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    let len_tag = second_byte & 0x7F_u8;
    let length = match len_tag {
        0..=125 => usize::from(len_tag),
        126 => {
            let mut ext = [0_u8; 2];
            stream.read_exact(&mut ext)?;
            usize::from(u16::from_be_bytes(ext))
        }
        127 => {
            let mut ext = [0_u8; 8];
            stream.read_exact(&mut ext)?;
            usize::try_from(u64::from_be_bytes(ext))
                .map_err(|_error| io::Error::from(io::ErrorKind::InvalidData))?
        }
        _ => return Err(io::Error::from(io::ErrorKind::InvalidData)),
    };
    let mut mask = [0_u8; 4];
    stream.read_exact(&mut mask)?;
    let mut payload = vec![0_u8; length];
    if length > 0 {
        stream.read_exact(&mut payload)?;
    }
    for (index, byte) in payload.iter_mut().enumerate() {
        let mask_byte = mask.get(index & 3_usize).copied().unwrap_or(0);
        *byte ^= mask_byte;
    }
    match opcode {
        0x1 => {
            let text = String::from_utf8(payload)
                .map_err(|_error| io::Error::from(io::ErrorKind::InvalidData))?;
            Ok(ClientFrame::Text(text))
        }
        0x8 => Ok(ClientFrame::Close),
        0xA => Ok(ClientFrame::Pong(payload)),
        _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
    }
}

fn bump_max(max_concurrent: &AtomicUsize, current: usize) {
    let mut observed = max_concurrent.load(Ordering::SeqCst);
    while current > observed {
        match max_concurrent.compare_exchange(observed, current, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => break,
            Err(actual) => observed = actual,
        }
    }
}

#[derive(PartialEq, Eq)]
enum PushState {
    Pending,
    Sent,
}

fn send_ack_and_pushes(
    stream: &mut TcpStream,
    ack_json: Option<&str>,
    push_frames: &[String],
    push_state: &mut PushState,
) -> io::Result<()> {
    if let Some(ack) = ack_json {
        write_server_frame(stream, 0x1, ack.as_bytes())?;
    }
    if *push_state == PushState::Pending {
        *push_state = PushState::Sent;
        for frame in push_frames {
            write_server_frame(stream, 0x1, frame.as_bytes())?;
        }
    }
    Ok(())
}

fn run_serve_loop(
    stream: &mut TcpStream,
    ack_json: Option<&str>,
    push_frames: &[String],
    expected_pings: usize,
    record: &mut WsRecordedConnection,
) -> io::Result<()> {
    let mut push_state = PushState::Pending;
    let mut ping_count = 0_usize;
    loop {
        match read_client_frame(stream)? {
            ClientFrame::Text(text) => {
                record.received_texts.push(text.clone());
                if text == "PING" {
                    ping_count = ping_count.saturating_add(1);
                    write_server_frame(stream, 0x1, br#"{"type":"pong"}"#)?;
                } else {
                    send_ack_and_pushes(stream, ack_json, push_frames, &mut push_state)?;
                }
                if expected_pings > 0 && ping_count >= expected_pings {
                    return Ok(());
                }
            }
            ClientFrame::Pong(payload) => record.received_pongs.push(payload),
            ClientFrame::Close | ClientFrame::Eof => {
                record.client_closed = true;
                return Ok(());
            }
        }
    }
}

fn expect_pong_step(
    stream: &mut TcpStream,
    expected: &[u8],
    record: &mut WsRecordedConnection,
) -> io::Result<()> {
    loop {
        match read_client_frame(stream)? {
            ClientFrame::Pong(payload) => {
                let matched = payload == expected;
                record.received_pongs.push(payload);
                if !matched {
                    return Err(io::Error::from(io::ErrorKind::InvalidData));
                }
                return Ok(());
            }
            ClientFrame::Text(text) => record.received_texts.push(text),
            ClientFrame::Close | ClientFrame::Eof => {
                record.client_closed = true;
                return Ok(());
            }
        }
    }
}

fn wait_client_close_step(
    stream: &mut TcpStream,
    record: &mut WsRecordedConnection,
) -> io::Result<()> {
    loop {
        match read_client_frame(stream)? {
            ClientFrame::Close | ClientFrame::Eof => {
                record.client_closed = true;
                return Ok(());
            }
            ClientFrame::Text(text) => {
                let is_ping = text == "PING";
                record.received_texts.push(text);
                if is_ping {
                    let _pong = write_server_frame(stream, 0x1, br#"{"type":"pong"}"#);
                }
            }
            ClientFrame::Pong(payload) => record.received_pongs.push(payload),
        }
    }
}

fn execute_steps(
    stream: &mut TcpStream,
    steps: &[FakeWsStep],
    record: &mut WsRecordedConnection,
) -> io::Result<()> {
    for step in steps {
        match step {
            FakeWsStep::ExpectText => match read_client_frame(stream)? {
                ClientFrame::Text(text) => record.received_texts.push(text),
                ClientFrame::Close | ClientFrame::Eof => {
                    record.client_closed = true;
                    break;
                }
                ClientFrame::Pong(payload) => record.received_pongs.push(payload),
            },
            FakeWsStep::SendText(text) => write_server_frame(stream, 0x1, text.as_bytes())?,
            FakeWsStep::SendPing(payload) => write_server_frame(stream, 0x9, payload)?,
            FakeWsStep::ExpectPong(expected) => expect_pong_step(stream, expected, record)?,
            FakeWsStep::SendClose(code) => {
                let payload = code.map_or_else(Vec::new, |val| val.to_be_bytes().to_vec());
                write_server_frame(stream, 0x8, &payload)?;
                break;
            }
            FakeWsStep::AbruptClose => {
                let _shutdown = stream.shutdown(std::net::Shutdown::Both);
                break;
            }
            FakeWsStep::WaitClientClose => wait_client_close_step(stream, record)?,
            FakeWsStep::ServeUntilClientClose {
                ack_json,
                push_frames,
                expected_pings,
            } => run_serve_loop(
                stream,
                ack_json.as_deref(),
                push_frames,
                *expected_pings,
                record,
            )?,
        }
    }
    Ok(())
}

fn run_session(
    mut stream: TcpStream,
    plan: FakeWsSessionPlan,
    active: &AtomicUsize,
    max_concurrent: &AtomicUsize,
    prior: PriorState,
) -> io::Result<WsRecordedConnection> {
    let now_active = active.fetch_add(1, Ordering::SeqCst).saturating_add(1);
    bump_max(max_concurrent, now_active);
    let outcome = (|| -> io::Result<WsRecordedConnection> {
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let parsed = read_upgrade_head(&mut stream)?;
        let mut record = WsRecordedConnection {
            path: parsed.path,
            authorization: parsed.authorization,
            prior_closed_at_accept: prior == PriorState::Closed,
            received_texts: Vec::new(),
            received_pongs: Vec::new(),
            client_closed: false,
        };
        match plan.upgrade {
            WsUpgradeReply::Accept => {
                let ws_key = parsed
                    .ws_key
                    .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
                let reply = format!(
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
                    accept_key(&ws_key)
                );
                stream.write_all(reply.as_bytes())?;
                stream.flush()?;
                execute_steps(&mut stream, &plan.steps, &mut record)?;
            }
            WsUpgradeReply::Reject {
                status,
                reason,
                body,
            } => {
                let reply = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(reply.as_bytes())?;
                stream.flush()?;
            }
        }
        Ok(record)
    })();
    active.fetch_sub(1, Ordering::SeqCst);
    outcome
}

fn spawn_session(
    stream: TcpStream,
    plan: FakeWsSessionPlan,
    active: Arc<AtomicUsize>,
    max_concurrent: Arc<AtomicUsize>,
    prior: PriorState,
) -> JoinHandle<io::Result<WsRecordedConnection>> {
    thread::spawn(move || run_session(stream, plan, &active, &max_concurrent, prior))
}

fn prior_state(active: &AtomicUsize) -> PriorState {
    if active.load(Ordering::SeqCst) == 0 {
        PriorState::Closed
    } else {
        PriorState::Open
    }
}

fn run_accept_loop(
    listener: &TcpListener,
    plans: Vec<FakeWsSessionPlan>,
    active: &Arc<AtomicUsize>,
    max_concurrent: &Arc<AtomicUsize>,
    accepted: &Arc<AtomicUsize>,
) -> io::Result<Vec<WsRecordedConnection>> {
    let mut workers = Vec::with_capacity(plans.len());
    for plan in plans {
        let (stream, _) = listener.accept()?;
        accepted.fetch_add(1, Ordering::SeqCst);
        let prior = prior_state(active);
        workers.push(spawn_session(
            stream,
            plan,
            Arc::clone(active),
            Arc::clone(max_concurrent),
            prior,
        ));
    }
    let mut records = Vec::with_capacity(workers.len());
    for worker in workers {
        let record = worker
            .join()
            .map_err(|_error| io::Error::from(io::ErrorKind::Other))??;
        records.push(record);
    }
    Ok(records)
}

impl FakeWsServer {
    pub fn start(plans: Vec<FakeWsSessionPlan>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let active = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let accepted = Arc::new(AtomicUsize::new(0));
        let active_clone = Arc::clone(&active);
        let max_clone = Arc::clone(&max_concurrent);
        let accepted_clone = Arc::clone(&accepted);
        let task = thread::spawn(move || {
            run_accept_loop(&listener, plans, &active_clone, &max_clone, &accepted_clone)
        });
        Ok(Self {
            port,
            active,
            max_concurrent,
            accepted,
            task,
        })
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    #[must_use]
    pub fn active_connections(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn max_concurrent_connections(&self) -> usize {
        self.max_concurrent.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn accepted_connections(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }

    pub fn finish(self) -> io::Result<Vec<WsRecordedConnection>> {
        self.task
            .join()
            .map_err(|_error| io::Error::from(io::ErrorKind::Other))?
    }
}
