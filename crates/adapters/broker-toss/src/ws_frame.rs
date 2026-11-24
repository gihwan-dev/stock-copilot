use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const MAX_CLIENT_PAYLOAD: usize = 1_048_576;
const MAX_SERVER_PAYLOAD: usize = 4_194_304;
const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) const OPCODE_CONTINUATION: u8 = 0x0;
pub(crate) const OPCODE_TEXT: u8 = 0x1;
pub(crate) const OPCODE_CLOSE: u8 = 0x8;
pub(crate) const OPCODE_PING: u8 = 0x9;
pub(crate) const OPCODE_PONG: u8 = 0xA;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WsWireFrame {
    Text(String),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close(Option<u16>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WsFrameError {
    Io,
    Protocol,
    FrameTooLarge,
    InvalidUtf8,
    ClosedWithoutCloseFrame,
}

pub(crate) fn generate_ws_key(counter: u64) -> String {
    let prefix = *b"toss-ws-";
    let tail = counter.to_be_bytes();
    let mut nonce = [0_u8; 16];
    for (slot, byte) in nonce.iter_mut().take(8).zip(prefix.iter()) {
        *slot = *byte;
    }
    for (slot, byte) in nonce.iter_mut().skip(8).zip(tail.iter()) {
        *slot = *byte;
    }
    base64_encode(&nonce)
}

pub(crate) fn ws_accept_key(ws_key: &str) -> String {
    let mut material = Vec::with_capacity(ws_key.len().saturating_add(WS_GUID.len()));
    material.extend_from_slice(ws_key.as_bytes());
    material.extend_from_slice(WS_GUID.as_bytes());
    let digest = sha1_digest(&material);
    base64_encode(&digest)
}

pub(crate) fn base64_encode(input: &[u8]) -> String {
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

fn push_base64_char(output: &mut String, index: usize) {
    if let Some(byte) = BASE64_TABLE.get(index) {
        output.push(char::from(*byte));
    }
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

pub(crate) fn sha1_digest(data: &[u8]) -> [u8; 20] {
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

pub(crate) async fn write_client_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    opcode: u8,
    payload: &[u8],
    mask_key: [u8; 4],
) -> Result<(), WsFrameError> {
    if payload.len() > MAX_CLIENT_PAYLOAD {
        return Err(WsFrameError::FrameTooLarge);
    }
    let mut frame = Vec::with_capacity(payload.len().saturating_add(14));
    frame.push(0x80_u8 | (opcode & 0x0F_u8));
    if let Ok(short_len) = u8::try_from(payload.len())
        && short_len <= 125_u8
    {
        frame.push(0x80_u8 | short_len);
    } else if let Ok(medium_len) = u16::try_from(payload.len()) {
        frame.push(0x80_u8 | 0x7E_u8);
        frame.extend_from_slice(&medium_len.to_be_bytes());
    } else {
        let wide_len =
            u64::try_from(payload.len()).map_err(|_error| WsFrameError::FrameTooLarge)?;
        frame.push(0x80_u8 | 0x7F_u8);
        frame.extend_from_slice(&wide_len.to_be_bytes());
    }
    frame.extend_from_slice(&mask_key);
    for (index, byte) in payload.iter().enumerate() {
        let mask_byte = mask_key.get(index & 3_usize).copied().unwrap_or(0);
        frame.push(*byte ^ mask_byte);
    }
    writer
        .write_all(&frame)
        .await
        .map_err(|_error| WsFrameError::Io)?;
    writer.flush().await.map_err(|_error| WsFrameError::Io)?;
    Ok(())
}

pub(crate) async fn write_client_text<W: AsyncWrite + Unpin>(
    writer: &mut W,
    text: &str,
    mask_key: [u8; 4],
) -> Result<(), WsFrameError> {
    write_client_frame(writer, OPCODE_TEXT, text.as_bytes(), mask_key).await
}

pub(crate) async fn write_client_pong<W: AsyncWrite + Unpin>(
    writer: &mut W,
    payload: &[u8],
    mask_key: [u8; 4],
) -> Result<(), WsFrameError> {
    write_client_frame(writer, OPCODE_PONG, payload, mask_key).await
}

pub(crate) async fn write_client_close<W: AsyncWrite + Unpin>(
    writer: &mut W,
    code: u16,
    mask_key: [u8; 4],
) -> Result<(), WsFrameError> {
    let payload = code.to_be_bytes();
    write_client_frame(writer, OPCODE_CLOSE, &payload, mask_key).await
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FrameFin {
    Final,
    Fragment,
}

struct RawServerFrame {
    fin: FrameFin,
    opcode: u8,
    payload: Vec<u8>,
}

fn map_read_error(error: &std::io::Error) -> WsFrameError {
    let kind = error.kind();
    if kind == std::io::ErrorKind::ConnectionReset
        || kind == std::io::ErrorKind::UnexpectedEof
        || kind == std::io::ErrorKind::BrokenPipe
    {
        WsFrameError::ClosedWithoutCloseFrame
    } else {
        WsFrameError::Io
    }
}

async fn read_exact_or_eof<R: AsyncRead + Unpin>(
    reader: &mut R,
    buffer: &mut [u8],
) -> Result<bool, WsFrameError> {
    let mut offset = 0_usize;
    while offset < buffer.len() {
        let target = buffer.get_mut(offset..).ok_or(WsFrameError::Protocol)?;
        let count = reader
            .read(target)
            .await
            .map_err(|error| map_read_error(&error))?;
        if count == 0_usize {
            if offset == 0_usize {
                return Ok(false);
            }
            return Err(WsFrameError::ClosedWithoutCloseFrame);
        }
        offset = offset.saturating_add(count);
    }
    Ok(true)
}

async fn read_payload_len<R: AsyncRead + Unpin>(
    reader: &mut R,
    len_tag: u8,
) -> Result<usize, WsFrameError> {
    let length = match len_tag {
        0..=125 => usize::from(len_tag),
        126 => {
            let mut ext = [0_u8; 2];
            if !read_exact_or_eof(reader, &mut ext).await? {
                return Err(WsFrameError::ClosedWithoutCloseFrame);
            }
            usize::from(u16::from_be_bytes(ext))
        }
        127 => {
            let mut ext = [0_u8; 8];
            if !read_exact_or_eof(reader, &mut ext).await? {
                return Err(WsFrameError::ClosedWithoutCloseFrame);
            }
            let wide = u64::from_be_bytes(ext);
            usize::try_from(wide).map_err(|_error| WsFrameError::FrameTooLarge)?
        }
        _ => return Err(WsFrameError::Protocol),
    };
    if length > MAX_SERVER_PAYLOAD {
        return Err(WsFrameError::FrameTooLarge);
    }
    Ok(length)
}

async fn read_raw_server_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<RawServerFrame, WsFrameError> {
    let mut header = [0_u8; 2];
    if !read_exact_or_eof(reader, &mut header).await? {
        return Err(WsFrameError::ClosedWithoutCloseFrame);
    }
    let [first_byte, second_byte] = header;
    let fin = if (first_byte & 0x80_u8) != 0_u8 {
        FrameFin::Final
    } else {
        FrameFin::Fragment
    };
    if (first_byte & 0x70_u8) != 0_u8 || (second_byte & 0x80_u8) != 0_u8 {
        return Err(WsFrameError::Protocol);
    }
    let opcode = first_byte & 0x0F_u8;
    let length = read_payload_len(reader, second_byte & 0x7F_u8).await?;
    let mut payload = vec![0_u8; length];
    if length > 0_usize && !read_exact_or_eof(reader, &mut payload).await? {
        return Err(WsFrameError::ClosedWithoutCloseFrame);
    }
    Ok(RawServerFrame {
        fin,
        opcode,
        payload,
    })
}

fn decode_control_frame(raw: RawServerFrame) -> Result<WsWireFrame, WsFrameError> {
    if raw.fin != FrameFin::Final || raw.payload.len() > 125_usize {
        return Err(WsFrameError::Protocol);
    }
    match raw.opcode {
        OPCODE_PING => Ok(WsWireFrame::Ping(raw.payload)),
        OPCODE_PONG => Ok(WsWireFrame::Pong(raw.payload)),
        OPCODE_CLOSE => {
            if raw.payload.len() == 1_usize {
                return Err(WsFrameError::Protocol);
            }
            let code = match (raw.payload.first(), raw.payload.get(1)) {
                (Some(high), Some(low)) => Some(u16::from_be_bytes([*high, *low])),
                _ => None,
            };
            Ok(WsWireFrame::Close(code))
        }
        _ => Err(WsFrameError::Protocol),
    }
}

pub(crate) struct WsFrameReader<R> {
    reader: R,
    fragment_buffer: Option<Vec<u8>>,
}

impl<R: AsyncRead + Unpin> WsFrameReader<R> {
    pub(crate) const fn new(reader: R) -> Self {
        Self {
            reader,
            fragment_buffer: None,
        }
    }

    fn handle_text_start(
        &mut self,
        raw: RawServerFrame,
    ) -> Result<Option<WsWireFrame>, WsFrameError> {
        if self.fragment_buffer.is_some() {
            return Err(WsFrameError::Protocol);
        }
        if raw.fin == FrameFin::Final {
            let text =
                String::from_utf8(raw.payload).map_err(|_error| WsFrameError::InvalidUtf8)?;
            return Ok(Some(WsWireFrame::Text(text)));
        }
        self.fragment_buffer = Some(raw.payload);
        Ok(None)
    }

    fn handle_continuation(
        &mut self,
        raw: &RawServerFrame,
    ) -> Result<Option<WsWireFrame>, WsFrameError> {
        let Some(buffer) = self.fragment_buffer.as_mut() else {
            return Err(WsFrameError::Protocol);
        };
        if buffer.len().saturating_add(raw.payload.len()) > MAX_SERVER_PAYLOAD {
            return Err(WsFrameError::FrameTooLarge);
        }
        buffer.extend_from_slice(&raw.payload);
        if raw.fin == FrameFin::Final {
            let finished = self.fragment_buffer.take().ok_or(WsFrameError::Protocol)?;
            let text = String::from_utf8(finished).map_err(|_error| WsFrameError::InvalidUtf8)?;
            return Ok(Some(WsWireFrame::Text(text)));
        }
        Ok(None)
    }

    pub(crate) async fn next_frame(&mut self) -> Result<WsWireFrame, WsFrameError> {
        loop {
            let raw = read_raw_server_frame(&mut self.reader).await?;
            let ready = match raw.opcode {
                OPCODE_PING | OPCODE_PONG | OPCODE_CLOSE => Some(decode_control_frame(raw)?),
                OPCODE_TEXT => self.handle_text_start(raw)?,
                OPCODE_CONTINUATION => self.handle_continuation(&raw)?,
                _ => return Err(WsFrameError::Protocol),
            };
            if let Some(frame) = ready {
                return Ok(frame);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FrameFin, OPCODE_CONTINUATION, OPCODE_PING, OPCODE_TEXT, WsFrameError, WsFrameReader,
        WsWireFrame, generate_ws_key, write_client_close, write_client_text, ws_accept_key,
    };

    fn server_frame(fin: FrameFin, opcode: u8, payload: &[u8]) -> Vec<u8> {
        let mut frame = Vec::new();
        let first = match fin {
            FrameFin::Final => 0x80_u8 | opcode,
            FrameFin::Fragment => opcode,
        };
        frame.push(first);
        if payload.len() <= 125_usize {
            frame.push(u8::try_from(payload.len()).expect("short len"));
        } else {
            frame.push(126_u8);
            frame.extend_from_slice(&u16::try_from(payload.len()).expect("u16 len").to_be_bytes());
        }
        frame.extend_from_slice(payload);
        frame
    }

    #[test]
    fn rfc_6455_accept_key_and_generated_key_match_specification() {
        assert_eq!(
            ws_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
        let key_one = generate_ws_key(1);
        let key_two = generate_ws_key(2);
        assert_eq!(key_one.len(), 24);
        assert_ne!(key_one, key_two);
    }

    #[test]
    fn client_frames_are_masked_and_fragmented_server_frames_reassemble() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let mut sink = Vec::new();
            write_client_text(&mut sink, "PING", [0x12, 0x34, 0x56, 0x78])
                .await
                .expect("write text");
            assert_eq!(sink.first().copied(), Some(0x81_u8));
            assert_eq!(sink.get(1).copied(), Some(0x84_u8));
            assert_eq!(sink.get(6).copied(), Some(b'P' ^ 0x12_u8));
            write_client_close(&mut sink, 1000, [0x01, 0x02, 0x03, 0x04])
                .await
                .expect("write close");

            let mut wire = Vec::new();
            wire.extend_from_slice(&server_frame(FrameFin::Fragment, OPCODE_TEXT, b"hello "));
            wire.extend_from_slice(&server_frame(FrameFin::Final, OPCODE_PING, b"ka"));
            wire.extend_from_slice(&server_frame(
                FrameFin::Final,
                OPCODE_CONTINUATION,
                b"world",
            ));
            let mut reader = WsFrameReader::new(wire.as_slice());
            assert_eq!(
                reader.next_frame().await.expect("ping"),
                WsWireFrame::Ping(b"ka".to_vec())
            );
            assert_eq!(
                reader.next_frame().await.expect("text"),
                WsWireFrame::Text("hello world".to_owned())
            );
            assert_eq!(
                reader.next_frame().await.expect_err("eof"),
                WsFrameError::ClosedWithoutCloseFrame
            );
        });
    }
}
