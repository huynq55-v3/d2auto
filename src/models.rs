use std::collections::{HashMap, HashSet, VecDeque};

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

pub struct GameTopology {
    // Lưu trữ đồ thị liên kết: Area ID -> Danh sách Area ID liền kề
    pub connections: HashMap<u32, Vec<u32>>,
}

impl GameTopology {
    pub fn new() -> Self {
        let mut topo = HashMap::new();
        // Act 1: Làng (1) -> Blood Moor (2) -> Cold Plains (3)
        // Blood Moor (2) -> Den of Evil (8)
        topo.insert(1, vec![2]);
        topo.insert(2, vec![1, 3, 8]);
        topo.insert(3, vec![2, 4, 17]);
        topo.insert(8, vec![2]); // Ngõ cụt, chỉ có thể quay lại Blood Moor

        Self { connections: topo }
    }

    /// Trả về mảng các Area cần đi qua. VD: Từ 1 đến 8 trả về [1, 2, 8]
    pub fn get_macro_route(&self, start: u32, target: u32) -> Option<Vec<u32>> {
        let mut queue = VecDeque::new();
        let mut visited = HashMap::new();

        queue.push_back(start);
        visited.insert(start, start);

        while let Some(current) = queue.pop_front() {
            if current == target {
                let mut route = Vec::new();
                let mut node = target;
                while node != start {
                    route.push(node);
                    node = *visited.get(&node).unwrap();
                }
                route.push(start);
                route.reverse();
                return Some(route);
            }

            if let Some(neighbors) = self.connections.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains_key(&neighbor) {
                        visited.insert(neighbor, current);
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        None
    }
}

// ==========================================
// 1. CẤU TRÚC ROOM
// ==========================================
#[derive(Debug, Clone)]
pub struct Room {
    pub ptr: u64,           // Địa chỉ Room1 trong RAM (để dùng cho quét lân cận)
    pub x: i32,             // room_x * 5
    pub y: i32,             // room_y * 5
    pub width: i32,         // room_width * 5
    pub height: i32,        // room_height * 5
    pub collision_ptr: u64, // p_collision_grid
    pub area_id: u32,       // ID khu vực (rất quan trọng để chống đi lạc)
}

impl Room {
    pub fn from_reader(reader: &MemoryReader, room1_ptr: u64) -> Option<Self> {
        if room1_ptr == 0 {
            return None;
        }

        let rx = reader.read_u32(room1_ptr + 0x1E0)? as i32;
        let ry = reader.read_u32(room1_ptr + 0x1E4)? as i32;
        let rw = reader.read_u32(room1_ptr + 0x1E8)? as i32;
        let rh = reader.read_u32(room1_ptr + 0x1EC)? as i32;

        let room2_ptr = reader.read_u64(room1_ptr + 0x18)?;
        let col_grid = reader.read_u64(room2_ptr + 0xA8)?;

        Some(Room {
            ptr: room1_ptr,
            x: rx * 5,
            y: ry * 5,
            width: rw * 5,
            height: rh * 5,
            collision_ptr: col_grid,
            area_id: 0,
        })
    }

    pub fn get_neighbor_pointers(&self, reader: &MemoryReader) -> Vec<u64> {
        let mut ptrs = Vec::new();
        let p_rooms_near = reader.read_u64(self.ptr + 0x78).unwrap_or(0);
        let count = reader.read_u32(self.ptr + 0x80).unwrap_or(0);

        for i in 0..count {
            if let Some(ptr) = reader.read_u64(p_rooms_near + (i as u64 * 8)) {
                if ptr != 0 {
                    ptrs.push(ptr);
                }
            }
        }
        ptrs
    }
}

// ==========================================
// 2. CẤU TRÚC AREA (Khu vực)
// ==========================================
pub struct Area {
    pub id: u32,
    pub name: String,
    pub scanned_rooms: HashSet<u64>,
    pub grid: HashMap<(i32, i32), bool>,
}

impl Area {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            scanned_rooms: HashSet::new(),
            grid: HashMap::new(),
        }
    }

    /// Đưa hàm stitch_collision về đúng cấu trúc của Area
    pub fn stitch_collision(&mut self, reader: &MemoryReader, room: &Room) {
        if room.collision_ptr == 0 {
            return;
        }

        let width = room.width;
        let height = room.height;
        let total_tiles = (width * height) as usize;
        let mut buffer = vec![0u16; total_tiles];
        let bytes_to_read = total_tiles * 2;

        if unsafe {
            use std::os::unix::fs::FileExt;
            reader
                .mem_file
                .read_exact_at(
                    std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u8, bytes_to_read),
                    room.collision_ptr,
                )
                .is_ok()
        } {
            for row in 0..height {
                for col in 0..width {
                    let index = (row * width + col) as usize;
                    let collision_value = buffer[index];
                    let is_walkable = (collision_value & 0x01) == 0;

                    let global_x = room.x + col;
                    let global_y = room.y + row;

                    self.grid.insert((global_x, global_y), is_walkable);
                }
            }
        }
    }

    /// Đưa hàm khám phá đệ quy vào Area để giải quyết lỗi Borrow Checker
    pub fn recursive_explore(
        &mut self,
        reader: &MemoryReader,
        room_ptr: u64,
        current_area_id: u32,
    ) {
        if room_ptr == 0 || self.scanned_rooms.contains(&room_ptr) {
            return;
        }

        if let Some(room) = Room::from_reader(reader, room_ptr) {
            // (Tùy chọn) Chống đi lạc: nếu room.area_id != current_area_id thì return;

            self.stitch_collision(reader, &room);
            self.scanned_rooms.insert(room_ptr);

            let neighbors = room.get_neighbor_pointers(reader);
            for n_ptr in neighbors {
                self.recursive_explore(reader, n_ptr, current_area_id);
            }
        }
    }
}

// ==========================================
// 3. CẤU TRÚC WORLDMAP
// ==========================================
pub struct WorldMap {
    pub areas: HashMap<u32, Area>,
}

impl WorldMap {
    pub fn new() -> Self {
        Self {
            areas: HashMap::new(),
        }
    }

    pub fn get_or_create_area(&mut self, area_id: u32, name: &str) -> &mut Area {
        self.areas
            .entry(area_id)
            .or_insert_with(|| Area::new(area_id, name))
    }
}

// ==========================================
// 4. CẤU TRÚC AREA MANAGER (Người quản lý tổng)
// ==========================================
pub struct AreaManager {
    pub world_map: WorldMap,
}

impl AreaManager {
    pub fn new() -> Self {
        Self {
            world_map: WorldMap::new(),
        }
    }

    /// Hàm cập nhật chính
    pub fn update(&mut self, reader: &MemoryReader, player_unit_ptr: u64, current_area_id: u32) {
        // Lấy con trỏ phòng hiện tại
        let current_room_ptr = Self::get_current_room_ptr(reader, player_unit_ptr);

        // Trích xuất area từ world_map
        let area = self
            .world_map
            .get_or_create_area(current_area_id, "Unknown");

        // Gọi hàm khám phá TRÊN ĐỐI TƯỢNG AREA
        // Điều này tách biệt Area khỏi WorldMap, không bị mượn (borrow) lặp lại
        area.recursive_explore(reader, current_room_ptr, current_area_id);
    }

    pub fn get_current_room_ptr(reader: &MemoryReader, player_unit_ptr: u64) -> u64 {
        if player_unit_ptr == 0 {
            return 0;
        }
        let path_ptr = reader.read_u64(player_unit_ptr + 0x38).unwrap_or(0);
        if path_ptr == 0 {
            return 0;
        }
        reader.read_u64(path_ptr + 0x20).unwrap_or(0)
    }
}
