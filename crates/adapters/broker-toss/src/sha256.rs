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

fn compress_block(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut schedule = [0_u32; 64];
    let (words, _rest) = block.as_chunks::<4>();
    for (slot, bytes) in schedule.iter_mut().take(16).zip(words.iter()) {
        *slot = u32::from_be_bytes(*bytes);
    }
    for index in 16_usize..64 {
        let word_m15 = schedule.get(index.saturating_sub(15)).copied().unwrap_or(0);
        let word_m2 = schedule.get(index.saturating_sub(2)).copied().unwrap_or(0);
        let word_m16 = schedule.get(index.saturating_sub(16)).copied().unwrap_or(0);
        let word_m7 = schedule.get(index.saturating_sub(7)).copied().unwrap_or(0);
        let sigma0 = word_m15.rotate_right(7) ^ word_m15.rotate_right(18) ^ (word_m15 >> 3_u32);
        let sigma1 = word_m2.rotate_right(17) ^ word_m2.rotate_right(19) ^ (word_m2 >> 10_u32);
        if let Some(target) = schedule.get_mut(index) {
            *target = word_m16
                .wrapping_add(sigma0)
                .wrapping_add(word_m7)
                .wrapping_add(sigma1);
        }
    }
    let [
        mut va,
        mut vb,
        mut vc,
        mut vd,
        mut ve,
        mut vf,
        mut vg,
        mut vh,
    ] = *state;
    for (constant, word) in ROUND_CONSTANTS.iter().zip(schedule.iter()) {
        let sum1 = ve.rotate_right(6) ^ ve.rotate_right(11) ^ ve.rotate_right(25);
        let choice = (ve & vf) ^ ((!ve) & vg);
        let temp1 = vh
            .wrapping_add(sum1)
            .wrapping_add(choice)
            .wrapping_add(*constant)
            .wrapping_add(*word);
        let sum0 = va.rotate_right(2) ^ va.rotate_right(13) ^ va.rotate_right(22);
        let majority = (va & vb) ^ (va & vc) ^ (vb & vc);
        let temp2 = sum0.wrapping_add(majority);

        vh = vg;
        vg = vf;
        vf = ve;
        ve = vd.wrapping_add(temp1);
        vd = vc;
        vc = vb;
        vb = va;
        va = temp1.wrapping_add(temp2);
    }
    let updates = [va, vb, vc, vd, ve, vf, vg, vh];
    for (slot, delta) in state.iter_mut().zip(updates.iter()) {
        *slot = slot.wrapping_add(*delta);
    }
}

#[must_use]
pub fn sha256_digest(input: &[u8]) -> [u8; 32] {
    let mut state = INITIAL_STATE;
    let (blocks, remainder) = input.as_chunks::<64>();
    for block in blocks {
        compress_block(&mut state, block);
    }
    let mut tail = [0_u8; 128];
    for (dest, byte) in tail.iter_mut().zip(remainder.iter()) {
        *dest = *byte;
    }
    if let Some(marker) = tail.get_mut(remainder.len()) {
        *marker = 0x80;
    }
    let mut total_bits = 0_u64;
    for _block in blocks {
        total_bits = total_bits.wrapping_add(512);
    }
    for _byte in remainder {
        total_bits = total_bits.wrapping_add(8);
    }
    let block_count: usize = if remainder.len() < 56 { 1 } else { 2 };
    let length_start: usize = if block_count == 1 { 56 } else { 120 };
    if let Some(length_slice) = tail.get_mut(length_start..length_start.saturating_add(8)) {
        for (dest, byte) in length_slice.iter_mut().zip(total_bits.to_be_bytes().iter()) {
            *dest = *byte;
        }
    }
    let (tail_blocks, _tail_rest) = tail.as_chunks::<64>();
    if let Some(first) = tail_blocks.first() {
        compress_block(&mut state, first);
    }
    if block_count == 2
        && let Some(second) = tail_blocks.get(1)
    {
        compress_block(&mut state, second);
    }
    let mut output = [0_u8; 32];
    let (out_chunks, _out_rest) = output.as_chunks_mut::<4>();
    for (word, dest) in state.iter().zip(out_chunks.iter_mut()) {
        *dest = word.to_be_bytes();
    }
    output
}

#[must_use]
pub fn sha256_hex(input: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = sha256_digest(input);
    let mut out = String::with_capacity(64);
    for byte in digest {
        let high = usize::from(byte >> 4_u32);
        let low = usize::from(byte & 0x0f);
        if let Some(&first) = HEX.get(high) {
            out.push(char::from(first));
        }
        if let Some(&second) = HEX.get(low) {
            out.push(char::from(second));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    #[test]
    fn sha256_matches_nist_vectors_and_pinned_toss_spec() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let spec = std::fs::read("../../../docs/toss-openapi-1.2.28.json").expect("pinned spec");
        assert_eq!(sha256_hex(&spec), broker_toss_schema::SPEC_SHA256);
    }
}
