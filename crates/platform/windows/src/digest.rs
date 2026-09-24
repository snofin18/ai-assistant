//! SHA-256（**纯函数、零依赖**）—— `Fingerprint` 的摘要算法（架构 v2 §7.3 / §15.3）。
//!
//! 职责：把规范化后的树形状字符串压成 32 字节摘要，再编码成 `Fingerprint` 要求的
//! `sha256:<64 位小写 hex>`（见 `assistant_platform_api::Fingerprint::parse`）。
//! 边界：**只做摘要**；不做规范化（规范化在 `uia::fingerprint`）、不做比较、不做审计链。
//!
//! ## 为什么不依赖 `sha2`
//! `sha2` 已登记为 `crates/storage` 的使用方，但**本卡 write scope 只允许改
//! `docs/DEPENDENCIES.md` 的 `windows` 一行**；把 `crates/platform/windows` 加进 `sha2`
//! 的使用方列属于「write scope 外」+ 漂移触发器 ①（加依赖使用方要按登记规则 1 先登记）。
//! 替代方案（改用 CNG `BCryptHashData`）要多开一个 `windows` feature 并多 6 处 FFI，
//! 且**无法在 macOS / ubuntu 的 CI 上单测**；本实现是纯函数，三平台 CI 都能跑 FIPS 向量。
//! 该取舍已登记在 `tasks/TASK-017-*.md` §5（DRIFT-017-3）。
//!
//! ## 不变量
//! 1. 输出恒为 64 个**小写** hex 字符（`Fingerprint::parse` 会拒绝其它形态）。
//! 2. 实现遵循 FIPS 180-4；正确性由本文件 §「测试」的官方向量（空串 / `abc` / 填充边界）锁定。
//! 3. 纯函数：同样输入必得同样输出，不碰 IO、不碰平台 API。
//!
//! 相关：架构 v2 §7.3（指纹）、§15.3（内容寻址与 hash chain 同口径）、ADR-0023（EOL 规范化）。

/// SHA-256 的 64 个轮常量（FIPS 180-4 §4.2.2，前 64 位素数的立方根小数部分）。
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

/// 初始状态（FIPS 180-4 §5.3.3，前 8 位素数平方根的小数部分）。
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

/// 计算 `input` 的 SHA-256，返回 `sha256:` 前缀 + 64 位小写 hex。
///
/// 返回值**必然**满足 `assistant_platform_api::Fingerprint::parse` 的格式要求
/// （见本模块不变量 1），因此调用方无需再处理格式失败。
#[must_use]
pub fn sha256_fingerprint(input: &str) -> String {
    let digest = sha256(input.as_bytes());
    let mut encoded = String::with_capacity(71);
    encoded.push_str("sha256:");
    for byte in digest {
        push_lower_hex(&mut encoded, byte);
    }
    encoded
}

/// SHA-256 压缩函数（FIPS 180-4 §6.2），返回 32 字节摘要。
fn sha256(input: &[u8]) -> [u8; 32] {
    let mut state = INITIAL_STATE;
    // 消息长度以**比特**计；`wrapping_mul` 是规范要求的 mod 2^64 行为，不是错误吞掉。
    let bit_length = (input.len() as u64).wrapping_mul(8);

    // 填充：0x80 + 若干个 0，使长度 ≡ 56 (mod 64)，再接 8 字节大端比特长度。
    let mut padded = Vec::with_capacity(input.len() + 72);
    padded.extend_from_slice(input);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());

    for block in padded.as_chunks::<64>().0 {
        let schedule = message_schedule(block);
        state = compress(state, &schedule);
    }

    let mut digest = [0_u8; 32];
    for (index, word) in state.iter().enumerate() {
        let bytes = word.to_be_bytes();
        let offset = index * 4;
        if let Some(slot) = digest.get_mut(offset..offset + 4) {
            slot.copy_from_slice(&bytes);
        }
    }
    digest
}

/// 把 64 字节消息块扩展成 64 个 32 位字（FIPS 180-4 §6.2.2 第 1 步）。
fn message_schedule(block: &[u8]) -> [u32; 64] {
    let mut schedule = [0_u32; 64];
    for (index, word) in block.as_chunks::<4>().0.iter().enumerate() {
        let mut bytes = [0_u8; 4];
        bytes.copy_from_slice(word);
        set_word(&mut schedule, index, u32::from_be_bytes(bytes));
    }
    for index in 16..64 {
        let word_15 = get_word(&schedule, index - 15);
        let word_2 = get_word(&schedule, index - 2);
        let small_sigma_0 = word_15.rotate_right(7) ^ word_15.rotate_right(18) ^ (word_15 >> 3);
        let small_sigma_1 = word_2.rotate_right(17) ^ word_2.rotate_right(19) ^ (word_2 >> 10);
        let value = get_word(&schedule, index - 16)
            .wrapping_add(small_sigma_0)
            .wrapping_add(get_word(&schedule, index - 7))
            .wrapping_add(small_sigma_1);
        set_word(&mut schedule, index, value);
    }
    schedule
}

/// 对单个消息块做 64 轮压缩（FIPS 180-4 §6.2.2 第 2~3 步）。
///
/// 8 个工作寄存器对应 FIPS 规范里的 a..h，但带上 `register_` 前缀：AGENTS.md §5.1 禁止单字母名，
/// clippy 的 `many_single_char_names` 也会拦（本 crate 禁止 `#[allow]`）。
fn compress(state: [u32; 8], schedule: &[u32; 64]) -> [u32; 8] {
    let mut register_a = get_word(&state, 0);
    let mut register_b = get_word(&state, 1);
    let mut register_c = get_word(&state, 2);
    let mut register_d = get_word(&state, 3);
    let mut register_e = get_word(&state, 4);
    let mut register_f = get_word(&state, 5);
    let mut register_g = get_word(&state, 6);
    let mut register_h = get_word(&state, 7);

    for index in 0..64 {
        let big_sigma_1 =
            register_e.rotate_right(6) ^ register_e.rotate_right(11) ^ register_e.rotate_right(25);
        let choice = (register_e & register_f) ^ ((!register_e) & register_g);
        let temp_1 = register_h
            .wrapping_add(big_sigma_1)
            .wrapping_add(choice)
            .wrapping_add(get_word(schedule, index))
            .wrapping_add(get_word(&ROUND_CONSTANTS, index));
        let big_sigma_0 =
            register_a.rotate_right(2) ^ register_a.rotate_right(13) ^ register_a.rotate_right(22);
        let majority =
            (register_a & register_b) ^ (register_a & register_c) ^ (register_b & register_c);
        let temp_2 = big_sigma_0.wrapping_add(majority);

        register_h = register_g;
        register_g = register_f;
        register_f = register_e;
        register_e = register_d.wrapping_add(temp_1);
        register_d = register_c;
        register_c = register_b;
        register_b = register_a;
        register_a = temp_1.wrapping_add(temp_2);
    }

    let mut updated = state;
    let deltas = [
        register_a, register_b, register_c, register_d, register_e, register_f, register_g,
        register_h,
    ];
    for (index, delta) in deltas.into_iter().enumerate() {
        let previous = get_word(&updated, index);
        set_word(&mut updated, index, previous.wrapping_add(delta));
    }
    updated
}

/// 读第 `index` 个字（越界返回 0）。
///
/// 用 `.get()` 而不是 `w[index]`：`indexing_slicing` 在本 workspace 是 **deny**
/// （AGENTS.md §5.3），而索引越界 panic 也违反铁律 1「无静默失败」的同一条精神。
fn get_word(words: &[u32], index: usize) -> u32 {
    words.get(index).copied().unwrap_or(0)
}

/// 写第 `index` 个字（越界静默忽略；调用点的 index 恒在范围内，此分支只为不 panic）。
fn set_word(words: &mut [u32], index: usize, value: u32) {
    if let Some(slot) = words.get_mut(index) {
        *slot = value;
    }
}

/// 追加一个字节的两位小写 hex。
fn push_lower_hex(output: &mut String, byte: u8) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let high = usize::from(byte >> 4);
    let low = usize::from(byte & 0x0f);
    if let (Some(high_char), Some(low_char)) = (DIGITS.get(high), DIGITS.get(low)) {
        output.push(char::from(*high_char));
        output.push(char::from(*low_char));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FIPS 180-4 / NIST 官方示例向量：这些值**不能**被"调通为止"地改。
    #[test]
    fn test_sha256_matches_nist_vectors() {
        assert_eq!(
            sha256_fingerprint(""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_fingerprint("abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_fingerprint("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "sha256:248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    /// 55 / 56 / 63 / 64 / 65 字节 = 填充边界（56 字节起长度字段独占一个新块；65 字节进入第二块）。
    ///
    /// 期望值全部用 `CPython` 的 `hashlib.sha256` **独立复算**过（DRIFT-017-6）：实现与期望值
    /// 不一致时先怀疑期望值，但改期望值必须有独立来源，不得"调到通过为止"。
    #[test]
    fn test_sha256_handles_padding_boundaries() {
        assert_eq!(
            sha256_fingerprint(&"a".repeat(55)),
            "sha256:9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
        );
        assert_eq!(
            sha256_fingerprint(&"a".repeat(56)),
            "sha256:b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
        );
        assert_eq!(
            sha256_fingerprint(&"a".repeat(63)),
            "sha256:7d3e74a05d7db15bce4ad9ec0658ea98e3f06eeecf16b4c6fff2da457ddc2f34"
        );
        assert_eq!(
            sha256_fingerprint(&"a".repeat(64)),
            "sha256:ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
        );
        assert_eq!(
            sha256_fingerprint(&"a".repeat(65)),
            "sha256:635361c48bb9eab14198e76ea8ab7f1a41685d6ad62aa9146d301d4f17eb0ae0"
        );
    }

    /// 输出形态必须被 `Fingerprint::parse` 接受 —— 这是本模块对外的唯一契约。
    #[test]
    fn test_output_shape_is_parseable_as_fingerprint() {
        use assistant_platform_api::Fingerprint;
        let value = sha256_fingerprint("契约");
        let parsed = Fingerprint::parse(value.clone());
        assert!(
            parsed.is_ok(),
            "sha256_fingerprint 的输出必须能通过 Fingerprint::parse"
        );
        assert_eq!(
            parsed.map(|fingerprint| fingerprint.as_str().to_string()),
            Ok(value)
        );
    }

    /// 大小写敏感：不同输入必得不同摘要（不是"同长度就算一样"）。
    #[test]
    fn test_different_inputs_produce_different_digests() {
        assert_ne!(sha256_fingerprint("Notepad"), sha256_fingerprint("notepad"));
    }
}
