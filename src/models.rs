use crate::memory::MemoryReader;

// ==========================================
// 2. DATA STRUCTURES (OBJECTS CÓ Ý NGHĨA)
// ==========================================
#[derive(Debug)]
pub struct PlayerInfo {
    pub id: u32,
    pub x: u16,
    pub y: u16,
}

impl PlayerInfo {
    // Trong D2R, UnitTable bao gồm 5 bảng Hash (Players, Monsters, Objects, Missiles, Items).
    // Mỗi bảng có 128 slot chứa các danh sách liên kết (Linked list).
    pub fn get_local_players(
        reader: &MemoryReader,
        base_address: u64,
        unit_table_offset: u64,
    ) -> Vec<PlayerInfo> {
        let mut players = Vec::new();
        if unit_table_offset == 0 {
            return players;
        }

        let unit_table_base = base_address + unit_table_offset;

        // Bảng đầu tiên (offset 0) chính là bảng của Type 0 (Players)
        for i in 0..128 {
            let mut unit_ptr = reader.read_u64(unit_table_base + i * 8).unwrap_or(0);

            // Duyệt qua danh sách liên kết trong Hash Bucket này
            while unit_ptr > 0 {
                let unit_type = reader.read_u32(unit_ptr).unwrap_or(99);

                if unit_type == 0 {
                    // 0 = Player
                    let unit_id = reader.read_u32(unit_ptr + 0x08).unwrap_or(0);

                    // Lấy con trỏ Path (chứa tọa độ)
                    let path_ptr = reader.read_u64(unit_ptr + 0x38).unwrap_or(0);
                    if path_ptr > 0 {
                        // Tọa độ tĩnh (Static X, Y) nằm ở offset 0x02 và 0x06
                        let x = reader.read_u16(path_ptr + 0x02).unwrap_or(0);
                        let y = reader.read_u16(path_ptr + 0x06).unwrap_or(0);

                        players.push(PlayerInfo { id: unit_id, x, y });
                    }
                }

                // Trỏ tới Unit tiếp theo trong LinkedList (Offset 0x150 của struct Unit)
                unit_ptr = reader.read_u64(unit_ptr + 0x150).unwrap_or(0);
            }
        }
        players
    }
}
