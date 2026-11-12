use std::fmt::Write;

const ROUND_CONSTANTS: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

const INITIAL_STATE: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const BLOCK_BYTES: usize = 64;
const LENGTH_OFFSET: usize = 56;

fn earlier(schedule: &[u32], back: usize) -> u32 {
    schedule
        .len()
        .checked_sub(back)
        .and_then(|index| schedule.get(index))
        .copied()
        .unwrap_or(0)
}

fn message_schedule(block: &[u8; BLOCK_BYTES]) -> Vec<u32> {
    let mut schedule: Vec<u32> = block
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| u32::from_be_bytes(*bytes))
        .collect();
    while schedule.len() < ROUND_CONSTANTS.len() {
        let before_two = earlier(&schedule, 2);
        let before_fifteen = earlier(&schedule, 15);
        let small_one =
            before_two.rotate_right(17) ^ before_two.rotate_right(19) ^ before_two.wrapping_shr(10);
        let small_zero = before_fifteen.rotate_right(7)
            ^ before_fifteen.rotate_right(18)
            ^ before_fifteen.wrapping_shr(3);
        schedule.push(
            small_one
                .wrapping_add(earlier(&schedule, 7))
                .wrapping_add(small_zero)
                .wrapping_add(earlier(&schedule, 16)),
        );
    }
    schedule
}

fn compress(state: &mut [u32; 8], block: &[u8; BLOCK_BYTES]) {
    let mut work = *state;
    for (constant, word) in ROUND_CONSTANTS.iter().zip(message_schedule(block)) {
        let [w0, w1, w2, w3, w4, w5, w6, w7] = work;
        let big_one = w4.rotate_right(6) ^ w4.rotate_right(11) ^ w4.rotate_right(25);
        let choose = (w4 & w5) ^ (!w4 & w6);
        let first = w7
            .wrapping_add(big_one)
            .wrapping_add(choose)
            .wrapping_add(*constant)
            .wrapping_add(word);
        let big_zero = w0.rotate_right(2) ^ w0.rotate_right(13) ^ w0.rotate_right(22);
        let majority = (w0 & w1) ^ (w0 & w2) ^ (w1 & w2);
        let second = big_zero.wrapping_add(majority);
        work = [
            first.wrapping_add(second),
            w0,
            w1,
            w2,
            w3.wrapping_add(first),
            w4,
            w5,
            w6,
        ];
    }
    for (slot, value) in state.iter_mut().zip(work) {
        *slot = slot.wrapping_add(value);
    }
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    let bit_length = u64::try_from(data.len()).map_or(u64::MAX, |length| length.wrapping_mul(8));
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len().checked_rem(BLOCK_BYTES) != Some(LENGTH_OFFSET) {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());
    let mut state = INITIAL_STATE;
    for block in message.as_chunks::<BLOCK_BYTES>().0 {
        compress(&mut state, block);
    }
    let mut text = String::new();
    for word in state {
        if write!(text, "{word:08x}").is_err() {
            return String::new();
        }
    }
    text
}
