const MAP_HASH_DIVISOR: u32 = 1 << 16; // 65536

/// Đảo ngược hàm mã hóa (startValue * 0x6AC690C5 + 666) & 0xFFFFFFFF
fn reverse_map_seed_hash(hash: u32) -> Option<u32> {
    let mut incremental_value: u32 = 1;
    let mut start_value: u32 = 0;

    // Lặp Brute-force thông minh để tìm ra Seed gốc
    loop {
        // Mô phỏng chính xác tràn số 32-bit của C++/Go
        let seed_result = start_value.wrapping_mul(0x6AC690C5).wrapping_add(666);

        if seed_result == hash {
            return Some(start_value);
        }

        if incremental_value == 1 && (seed_result % MAP_HASH_DIVISOR) == (hash % MAP_HASH_DIVISOR) {
            incremental_value = MAP_HASH_DIVISOR;
        }

        // Tăng biến đếm, nếu vượt qua u32::MAX thì dừng lại (chống lặp vô tận)
        match start_value.checked_add(incremental_value) {
            Some(v) => start_value = v,
            None => break,
        }
    }

    None
}

/// Tính toán Seed cuối cùng dựa trên 2 mã Hash đọc được từ RAM
pub fn get_map_seed_from_hash(init_hash_seed: u32, end_hash_seed: u32) -> Option<u32> {
    if let Some(seed) = reverse_map_seed_hash(end_hash_seed) {
        let game_seed_xor = init_hash_seed ^ seed;

        if game_seed_xor != 0 {
            return Some(seed);
        }
    }
    None
}
