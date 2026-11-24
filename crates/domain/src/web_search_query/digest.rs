use std::fmt;

use crate::{UntrustedText, WebSearchCandidateQuery};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSearchDigestError {
    InvalidFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WebSearchQueryDigest(String);

impl WebSearchQueryDigest {
    pub fn try_new(value: &str) -> Result<Self, WebSearchDigestError> {
        if value.len() != 64_usize
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(WebSearchDigestError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn of_query(query: &WebSearchCandidateQuery) -> Self {
        sha256_bytes(&ByteBuf::new(query.as_str().as_bytes()))
    }

    #[must_use]
    pub fn of_untrusted(text: &UntrustedText) -> Self {
        sha256_bytes(&ByteBuf::new(text.as_untrusted_str().as_bytes()))
    }
}

impl fmt::Display for WebSearchQueryDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy)]
struct Word(u32);

impl Word {
    const fn new(value: u32) -> Self {
        Self(value)
    }

    const fn as_u32(&self) -> &u32 {
        &self.0
    }
}

struct HashState {
    h0: Word,
    h1: Word,
    h2: Word,
    h3: Word,
    h4: Word,
    h5: Word,
    h6: Word,
    h7: Word,
}

struct RoundWork {
    va: Word,
    vb: Word,
    vc: Word,
    vd: Word,
    ve: Word,
    vf: Word,
    vg: Word,
    vh: Word,
    w0: Word,
    w1: Word,
    w2: Word,
    w3: Word,
    w4: Word,
    w5: Word,
    w6: Word,
    w7: Word,
    w8: Word,
    w9: Word,
    w10: Word,
    w11: Word,
    w12: Word,
    w13: Word,
    w14: Word,
    w15: Word,
}

#[derive(Clone, Copy)]
enum ScheduleMode {
    Initial,
    Expand,
}

impl RoundWork {
    fn step(&mut self, round_k: Word, mode: ScheduleMode) {
        let w0 = *self.w0.as_u32();
        let w1 = *self.w1.as_u32();
        let w9 = *self.w9.as_u32();
        let w14 = *self.w14.as_u32();
        let schedule_word = match mode {
            ScheduleMode::Initial => w0,
            ScheduleMode::Expand => {
                let sig0 = w1.rotate_right(7_u32) ^ w1.rotate_right(18_u32) ^ (w1 >> 3_u32);
                let sig1 = w14.rotate_right(17_u32) ^ w14.rotate_right(19_u32) ^ (w14 >> 10_u32);
                w0.wrapping_add(sig0).wrapping_add(w9).wrapping_add(sig1)
            }
        };
        self.w0 = self.w1;
        self.w1 = self.w2;
        self.w2 = self.w3;
        self.w3 = self.w4;
        self.w4 = self.w5;
        self.w5 = self.w6;
        self.w6 = self.w7;
        self.w7 = self.w8;
        self.w8 = self.w9;
        self.w9 = self.w10;
        self.w10 = self.w11;
        self.w11 = self.w12;
        self.w12 = self.w13;
        self.w13 = self.w14;
        self.w14 = self.w15;
        self.w15 = Word::new(schedule_word);
        let va = *self.va.as_u32();
        let vb = *self.vb.as_u32();
        let vc = *self.vc.as_u32();
        let vd = *self.vd.as_u32();
        let ve = *self.ve.as_u32();
        let vf = *self.vf.as_u32();
        let vg = *self.vg.as_u32();
        let vh = *self.vh.as_u32();
        let big_s1 = ve.rotate_right(6_u32) ^ ve.rotate_right(11_u32) ^ ve.rotate_right(25_u32);
        let choice = (ve & vf) ^ ((!ve) & vg);
        let temp1 = vh
            .wrapping_add(big_s1)
            .wrapping_add(choice)
            .wrapping_add(*round_k.as_u32())
            .wrapping_add(schedule_word);
        let big_s0 = va.rotate_right(2_u32) ^ va.rotate_right(13_u32) ^ va.rotate_right(22_u32);
        let majority = (va & vb) ^ (va & vc) ^ (vb & vc);
        let temp2 = big_s0.wrapping_add(majority);
        self.vh = self.vg;
        self.vg = self.vf;
        self.vf = self.ve;
        self.ve = Word::new(vd.wrapping_add(temp1));
        self.vd = self.vc;
        self.vc = self.vb;
        self.vb = self.va;
        self.va = Word::new(temp1.wrapping_add(temp2));
    }

    fn run_quarter_initial(&mut self) {
        for rk in [
            0x428a_2f98_u32,
            0x7137_4491_u32,
            0xb5c0_fbcf_u32,
            0xe9b5_dba5_u32,
            0x3956_c25b_u32,
            0x59f1_11f1_u32,
            0x923f_82a4_u32,
            0xab1c_5ed5_u32,
            0xd807_aa98_u32,
            0x1283_5b01_u32,
            0x2431_85be_u32,
            0x550c_7dc3_u32,
            0x72be_5d74_u32,
            0x80de_b1fe_u32,
            0x9bdc_06a7_u32,
            0xc19b_f174_u32,
        ] {
            self.step(Word::new(rk), ScheduleMode::Initial);
        }
    }

    fn run_quarter_second(&mut self) {
        for rk in [
            0xe49b_69c1_u32,
            0xefbe_4786_u32,
            0x0fc1_9dc6_u32,
            0x240c_a1cc_u32,
            0x2de9_2c6f_u32,
            0x4a74_84aa_u32,
            0x5cb0_a9dc_u32,
            0x76f9_88da_u32,
            0x983e_5152_u32,
            0xa831_c66d_u32,
            0xb003_27c8_u32,
            0xbf59_7fc7_u32,
            0xc6e0_0bf3_u32,
            0xd5a7_9147_u32,
            0x06ca_6351_u32,
            0x1429_2967_u32,
        ] {
            self.step(Word::new(rk), ScheduleMode::Expand);
        }
    }

    fn run_quarter_third(&mut self) {
        for rk in [
            0x27b7_0a85_u32,
            0x2e1b_2138_u32,
            0x4d2c_6dfc_u32,
            0x5338_0d13_u32,
            0x650a_7354_u32,
            0x766a_0abb_u32,
            0x81c2_c92e_u32,
            0x9272_2c85_u32,
            0xa2bf_e8a1_u32,
            0xa81a_664b_u32,
            0xc24b_8b70_u32,
            0xc76c_51a3_u32,
            0xd192_e819_u32,
            0xd699_0624_u32,
            0xf40e_3585_u32,
            0x106a_a070_u32,
        ] {
            self.step(Word::new(rk), ScheduleMode::Expand);
        }
    }

    fn run_quarter_fourth(&mut self) {
        for rk in [
            0x19a4_c116_u32,
            0x1e37_6c08_u32,
            0x2748_774c_u32,
            0x34b0_bcb5_u32,
            0x391c_0cb3_u32,
            0x4ed8_aa4a_u32,
            0x5b9c_ca4f_u32,
            0x682e_6ff3_u32,
            0x748f_82ee_u32,
            0x78a5_636f_u32,
            0x84c8_7814_u32,
            0x8cc7_0208_u32,
            0x90be_fffa_u32,
            0xa450_6ceb_u32,
            0xbef9_a3f7_u32,
            0xc671_78f2_u32,
        ] {
            self.step(Word::new(rk), ScheduleMode::Expand);
        }
    }
}

impl HashState {
    const fn initial() -> Self {
        Self {
            h0: Word::new(0x6a09_e667_u32),
            h1: Word::new(0xbb67_ae85_u32),
            h2: Word::new(0x3c6e_f372_u32),
            h3: Word::new(0xa54f_f53a_u32),
            h4: Word::new(0x510e_527f_u32),
            h5: Word::new(0x9b05_688c_u32),
            h6: Word::new(0x1f83_d9ab_u32),
            h7: Word::new(0x5be0_cd19_u32),
        }
    }

    fn compress(&mut self, block: &MessageBlock) {
        let mut work = RoundWork {
            va: self.h0,
            vb: self.h1,
            vc: self.h2,
            vd: self.h3,
            ve: self.h4,
            vf: self.h5,
            vg: self.h6,
            vh: self.h7,
            w0: block.w0,
            w1: block.w1,
            w2: block.w2,
            w3: block.w3,
            w4: block.w4,
            w5: block.w5,
            w6: block.w6,
            w7: block.w7,
            w8: block.w8,
            w9: block.w9,
            w10: block.w10,
            w11: block.w11,
            w12: block.w12,
            w13: block.w13,
            w14: block.w14,
            w15: block.w15,
        };
        work.run_quarter_initial();
        work.run_quarter_second();
        work.run_quarter_third();
        work.run_quarter_fourth();
        self.h0 = Word::new(self.h0.as_u32().wrapping_add(*work.va.as_u32()));
        self.h1 = Word::new(self.h1.as_u32().wrapping_add(*work.vb.as_u32()));
        self.h2 = Word::new(self.h2.as_u32().wrapping_add(*work.vc.as_u32()));
        self.h3 = Word::new(self.h3.as_u32().wrapping_add(*work.vd.as_u32()));
        self.h4 = Word::new(self.h4.as_u32().wrapping_add(*work.ve.as_u32()));
        self.h5 = Word::new(self.h5.as_u32().wrapping_add(*work.vf.as_u32()));
        self.h6 = Word::new(self.h6.as_u32().wrapping_add(*work.vg.as_u32()));
        self.h7 = Word::new(self.h7.as_u32().wrapping_add(*work.vh.as_u32()));
    }

    fn finish(self) -> WebSearchQueryDigest {
        WebSearchQueryDigest(format!(
            "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
            self.h0.as_u32(),
            self.h1.as_u32(),
            self.h2.as_u32(),
            self.h3.as_u32(),
            self.h4.as_u32(),
            self.h5.as_u32(),
            self.h6.as_u32(),
            self.h7.as_u32(),
        ))
    }
}

struct MessageBlock {
    w0: Word,
    w1: Word,
    w2: Word,
    w3: Word,
    w4: Word,
    w5: Word,
    w6: Word,
    w7: Word,
    w8: Word,
    w9: Word,
    w10: Word,
    w11: Word,
    w12: Word,
    w13: Word,
    w14: Word,
    w15: Word,
}

struct ByteBuf(Vec<u8>);

impl ByteBuf {
    fn new(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }

    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

fn sha256_bytes(input: &ByteBuf) -> WebSearchQueryDigest {
    let bytes = input.as_bytes();
    let mut bit_len = 0_u64;
    for _byte in bytes {
        bit_len = bit_len.wrapping_add(8_u64);
    }
    let mut padded = Vec::with_capacity(bytes.len().saturating_add(72_usize));
    padded.extend_from_slice(bytes);
    padded.push(0x80_u8);
    while (padded.len() & 63_usize) != 56_usize {
        padded.push(0x00_u8);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = HashState::initial();
    let (blocks, _rem) = padded.as_chunks::<64_usize>();
    for chunk in blocks {
        let mut raw = [0_u32; 16];
        let (quads, _quad_rem) = chunk.as_chunks::<4_usize>();
        for (slot, quad) in raw.iter_mut().zip(quads) {
            *slot = u32::from_be_bytes(*quad);
        }
        let [
            r0,
            r1,
            r2,
            r3,
            r4,
            r5,
            r6,
            r7,
            r8,
            r9,
            r10,
            r11,
            r12,
            r13,
            r14,
            r15,
        ] = raw;
        let block = MessageBlock {
            w0: Word::new(r0),
            w1: Word::new(r1),
            w2: Word::new(r2),
            w3: Word::new(r3),
            w4: Word::new(r4),
            w5: Word::new(r5),
            w6: Word::new(r6),
            w7: Word::new(r7),
            w8: Word::new(r8),
            w9: Word::new(r9),
            w10: Word::new(r10),
            w11: Word::new(r11),
            w12: Word::new(r12),
            w13: Word::new(r13),
            w14: Word::new(r14),
            w15: Word::new(r15),
        };
        state.compress(&block);
    }
    state.finish()
}
