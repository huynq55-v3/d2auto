use crate::memory::MemoryReader;

const MAP_HASH_DIVISOR: u32 = 1 << 16;

pub fn read_seed_from_memory(reader: &MemoryReader, player_unit_ptr: u64) -> Option<u32> {
    // --- 1. LẤY MAP SEED ---
    let act_ptr = reader.read_u64(player_unit_ptr + 0x20).unwrap_or(0);
    if act_ptr != 0 {
        let act_misc_ptr = reader.read_u64(act_ptr + 0x78).unwrap_or(0);
        if act_misc_ptr != 0 {
            let init_seed_hash = reader.read_u32(act_misc_ptr + 0x840).unwrap_or(0);
            let end_seed_hash = reader.read_u32(act_misc_ptr + 0x868).unwrap_or(0);

            if init_seed_hash != 0 && end_seed_hash != 0 {
                if let Some(map_seed) = get_map_seed(init_seed_hash, end_seed_hash) {
                    println!("[+] BÙM! Lấy được Game Seed: {}", map_seed);
                    return Some(map_seed);
                }
            }
        }
    }
    None
}

pub fn get_map_seed(init_hash_seed: u32, end_hash_seed: u32) -> Option<u32> {
    if let Some(seed) = reverse_map_seed_hash(end_hash_seed) {
        let game_seed_xor = init_hash_seed ^ seed;
        if game_seed_xor != 0 {
            return Some(seed);
        }
    }
    None
}

fn reverse_map_seed_hash(hash: u32) -> Option<u32> {
    let mut incremental_value: u32 = 1;
    let mut start_value: u32 = 0;

    while start_value < u32::MAX {
        let seed_result = start_value.wrapping_mul(0x6AC690C5).wrapping_add(666);

        if seed_result == hash {
            return Some(start_value);
        }

        if incremental_value == 1 && (seed_result % MAP_HASH_DIVISOR) == (hash % MAP_HASH_DIVISOR) {
            incremental_value = MAP_HASH_DIVISOR;
        }

        if let Some(next_val) = start_value.checked_add(incremental_value) {
            start_value = next_val;
        } else {
            break;
        }
    }
    None
}
