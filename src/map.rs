use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

// 1. Cấu trúc mới khớp 100% với JSON của D2 Map API
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdjacentLevel {
    pub exits: Vec<Point>,
    pub level_origin: Point,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapDataJSON {
    pub level_origin: Point,
    pub map_rows: Vec<Vec<i32>>,
    // Dùng HashMap<String, ...> vì key trong JSON là chuỗi ("1", "8", "3"...)
    #[serde(default)]
    pub adjacent_levels: HashMap<String, AdjacentLevel>,
}

pub struct Area {
    pub id: u32,
    pub name: String,
    pub origin: Point,
    pub grid: HashMap<(i32, i32), i32>,
    pub exits: HashMap<u32, (i32, i32)>, // Lưu tọa độ cổng (Target ID -> x, y)
}

impl Area {
    pub fn new(id: u32, name: &str, origin: Point) -> Self {
        Self {
            id,
            name: name.to_string(),
            origin,
            grid: HashMap::new(),
            exits: HashMap::new(),
        }
    }
}

pub struct WorldMap {
    pub areas: HashMap<u32, Area>,
}

impl WorldMap {
    pub fn new() -> Self {
        Self {
            areas: HashMap::new(),
        }
    }

    pub fn fetch_map_from_seed(&mut self, seed: u32, difficulty: u32, area_id: u32) -> Option<()> {
        println!(
            "[MAP-API] Đang tải Map Area {} từ Seed {}...",
            area_id, seed
        );

        let url = format!(
            "http://localhost:5000/maps?mapid={}&area={}&difficulty={}",
            seed, area_id, difficulty
        );

        let client = Client::new();
        let response = client.get(&url).send().ok()?;

        if response.status().is_success() {
            let map_data: MapDataJSON = response.json().ok()?;
            let mut area = Area::new(area_id, "API_Map", map_data.level_origin);

            let offset_x = map_data.level_origin.x;
            let offset_y = map_data.level_origin.y;

            // 1. Nạp Grid (bản đồ vật cản)
            for (y, row) in map_data.map_rows.iter().enumerate() {
                for (x, &tile_val) in row.iter().enumerate() {
                    if tile_val == -1 {
                        continue;
                    }
                    let global_x = offset_x + (x as i32);
                    let global_y = offset_y + (y as i32);
                    area.grid.insert((global_x, global_y), tile_val);
                }
            }

            // 2. Nạp Lối đi (Exits) - XỬ LÝ CHUẨN TỪ JSON!
            for (target_id_str, level_info) in map_data.adjacent_levels {
                // Ép kiểu String ("8") về số u32 (8)
                if let Ok(target_id) = target_id_str.parse::<u32>() {
                    // Trong JSON, mảng exits có thể rỗng hoặc có nhiều điểm trùng nhau.
                    // Chúng ta chỉ cần lấy điểm ĐẦU TIÊN làm tọa độ click.
                    if let Some(first_exit) = level_info.exits.first() {
                        area.exits.insert(target_id, (first_exit.x, first_exit.y));
                        println!(
                            "[MAP-API] Tìm thấy cửa sang Area {} tại tọa độ ({}, {})",
                            target_id, first_exit.x, first_exit.y
                        );
                    }
                }
            }

            self.areas.insert(area_id, area);
            println!("[MAP-API] Nạp thành công Area {}.", area_id);
            return Some(());
        }
        None
    }

    /// Trả về trực tiếp tọa độ của cổng dịch chuyển
    pub fn find_exit_position(
        &self,
        current_area_id: u32,
        target_area_id: u32,
    ) -> Option<(i32, i32)> {
        if let Some(area) = self.areas.get(&current_area_id) {
            if let Some(&pos) = area.exits.get(&target_area_id) {
                return Some(pos);
            }
        }
        None
    }

    /// Chuyển đổi thành Grid để A* tìm đường
    pub fn get_astar_grid(&self, area_id: u32) -> HashMap<(i32, i32), bool> {
        let mut grid_for_astar = HashMap::new();

        if let Some(area) = self.areas.get(&area_id) {
            for (&(x, y), &tile_val) in &area.grid {
                // LOGIC CHUẨN: Chỉ quan tâm chẵn (đi được) và lẻ (tường)
                let is_walkable = tile_val % 2 == 0;
                grid_for_astar.insert((x, y), is_walkable);
            }

            // MẸO CỦA BOT: Điểm cửa (Exit) thường bị kẹt vào tường (is_walkable = false).
            // Ta bắt buộc gán nó bằng True để A* có thể chạm tới sát mép cửa!
            for &exit_pos in area.exits.values() {
                grid_for_astar.insert(exit_pos, true);
            }
        }
        grid_for_astar
    }
}
