use reqwest::blocking::Client;
use std::collections::{HashMap, VecDeque};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

// Dùng rename_all để Rust tự map chữ "levelOrigin" thành "level_origin"
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapDataJSON {
    pub level_origin: Point,
    pub map_rows: Vec<Vec<i32>>,
    // (Tùy chọn) Nếu JSON API của bạn có trả về lối đi (warps) hoặc NPC,
    // bạn có thể thêm Option để hứng, nếu không có nó sẽ tự bỏ qua.
    // pub adjacent_levels: Option<Vec<ExitJSON>>,
}

// ==========================================
// 2. DATA STRUCTURES (OBJECTS CÓ Ý NGHĨA)
// ==========================================
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
// 2. CẤU TRÚC AREA (Khu vực)
// ==========================================
pub struct Area {
    pub id: u32,
    pub name: String,
    pub grid: HashMap<(i32, i32), bool>,
}

impl Area {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            grid: HashMap::new(),
        }
    }

    /// Máy hút bụi: Tìm mép sương mù gần nhân vật nhất
    pub fn find_closest_frontier(&self, player_x: i32, player_y: i32) -> Option<(i32, i32)> {
        let mut best_frontier = None;
        let mut min_distance = i32::MAX;

        let directions = [(0, 1), (1, 0), (0, -1), (-1, 0)];

        // Duyệt qua toàn bộ các ô đã biết trong Area
        for (&(x, y), &is_walkable) in &self.grid {
            if !is_walkable {
                continue;
            } // Bỏ qua tường

            // Kiểm tra xem ô đất trống này có nằm sát Sương Mù không
            let mut is_frontier = false;
            for &(dx, dy) in &directions {
                let neighbor = (x + dx, y + dy);
                // Nếu hàng xóm không tồn tại trong HashMap => Đó là sương mù chưa khám phá
                if !self.grid.contains_key(&neighbor) {
                    is_frontier = true;
                    break;
                }
            }

            if is_frontier {
                // Tính khoảng cách từ Nhân vật đến Sương mù này
                let dist = (x - player_x).abs() + (y - player_y).abs(); // Manhattan distance
                if dist < min_distance {
                    min_distance = dist;
                    best_frontier = Some((x, y));
                }
            }
        }

        best_frontier
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

    pub fn fetch_map_from_seed(&mut self, seed: u32, difficulty: u32, area_id: u32) -> Option<()> {
        if self.world_map.areas.contains_key(&area_id) {
            return Some(()); // Đã có map trong não, không cần gọi lại
        }

        println!(
            "[MAP-API] Đang gọi API lấy Map Area {} từ Seed {}...",
            area_id, seed
        );

        let url = format!(
            "http://localhost:5000/maps?mapid={}&area={}&difficulty={}",
            seed, area_id, difficulty
        );

        let client = Client::new();
        match client.get(&url).send() {
            Ok(res) if res.status().is_success() => {
                let json_text = res.text().unwrap_or_default();

                // Parse chuỗi JSON khổng lồ thành Struct
                if let Ok(map_data) = serde_json::from_str::<MapDataJSON>(&json_text) {
                    let mut new_area = Area::new(area_id, "API_Map");

                    let offset_x = map_data.level_origin.x;
                    let offset_y = map_data.level_origin.y;

                    let height = map_data.map_rows.len();
                    let width = if height > 0 {
                        map_data.map_rows[0].len()
                    } else {
                        0
                    };

                    // Duyệt qua từng ô của mảng 2D
                    for (y, row) in map_data.map_rows.iter().enumerate() {
                        for (x, &tile_val) in row.iter().enumerate() {
                            // Bỏ qua hư vô
                            if tile_val == -1 {
                                continue;
                            }

                            // Giải mã vật lý: Số chẵn = Đi được (True), Số lẻ = Tường (False)
                            let is_walkable = (tile_val % 2) == 0;

                            // Tọa độ thực tế trong Game = Tọa độ Gốc + Chỉ số mảng
                            let global_x = offset_x + (x as i32);
                            let global_y = offset_y + (y as i32);

                            new_area.grid.insert((global_x, global_y), is_walkable);
                        }
                    }

                    self.world_map.areas.insert(area_id, new_area);
                    println!("[MAP-API] Nạp thành công! Map {}x{} tiles.", width, height);
                    return Some(());
                } else {
                    println!("[MAP-API] Lỗi Parse JSON! Định dạng trả về không khớp Struct.");
                }
            }
            Ok(res) => println!("[MAP-API] HTTP Code: {}", res.status()),
            Err(e) => println!("[MAP-API] Lỗi kết nối server: {}", e),
        }
        None
    }
}
