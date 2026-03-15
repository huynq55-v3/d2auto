use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ExitType {
    Object,   // Cổng, Hang (Cần click đích danh)
    Boundary, // Ranh giới đất liền (Chạy xuyên qua)
}

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
    #[serde(default)]
    pub adjacent_levels: HashMap<String, AdjacentLevel>,
}

pub struct Area {
    pub id: u32,
    pub name: String,
    pub origin: Point,
    pub grid: HashMap<(i32, i32), i32>,
    // Lưu: (X, Y, Loại, Vectơ_Pháp_Tuyến)
    pub exits: HashMap<u32, (i32, i32, ExitType, (f32, f32))>,
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

            // 1. Nạp Grid
            for (y, row) in map_data.map_rows.iter().enumerate() {
                for (x, &tile_val) in row.iter().enumerate() {
                    if tile_val == -1 {
                        continue;
                    }
                    area.grid.insert(
                        (
                            map_data.level_origin.x + x as i32,
                            map_data.level_origin.y + y as i32,
                        ),
                        tile_val,
                    );
                }
            }

            // 2. Nạp Lối đi (Duyệt toàn bộ các Map lân cận)
            for (target_id_str, level_info) in map_data.adjacent_levels {
                if let Ok(target_id) = target_id_str.parse::<u32>() {
                    // Tính Vectơ pháp tuyến chuẩn
                    let raw_dx = (level_info.level_origin.x - area.origin.x) as f32;
                    let raw_dy = (level_info.level_origin.y - area.origin.y) as f32;
                    let mag = (raw_dx * raw_dx + raw_dy * raw_dy).sqrt();
                    let normal = if mag > 0.0 {
                        let nx = raw_dx / mag;
                        let ny = raw_dy / mag;
                        if nx.abs() > ny.abs() {
                            (nx.signum(), 0.0)
                        } else {
                            (0.0, ny.signum())
                        }
                    } else {
                        (1.0, 0.0)
                    };

                    if let Some(first_exit) = level_info.exits.first() {
                        // TRƯỜNG HỢP: OBJECT (Cửa/Hang)
                        area.exits.insert(
                            target_id,
                            (first_exit.x, first_exit.y, ExitType::Object, normal),
                        );
                        println!(
                            "[MAP-API] Tìm thấy cửa (Object) sang Area {} tại ({}, {}) | Normal: {:?}",
                            target_id, first_exit.x, first_exit.y, normal
                        );
                    } else {
                        // TRƯỜNG HỢP: BOUNDARY (Biên giới đất liền)
                        // BỎ HẲN TÌNH TRẠNG MARGIN QUÉT NHẦM RÁC BÊN NGOÀI
                        // Thay vào đó, tính Toán học Hình chữ nhật Giao nhau giữa 2 Map

                        let m1_w = if map_data.map_rows.is_empty() {
                            0
                        } else {
                            map_data.map_rows[0].len() as i32
                        };
                        let m1_h = map_data.map_rows.len() as i32;

                        let m1_min_x = area.origin.x;
                        let m1_max_x = area.origin.x + m1_w;
                        let m1_min_y = area.origin.y;
                        let m1_max_y = area.origin.y + m1_h;

                        let m2_min_x = level_info.level_origin.x;
                        let m2_max_x = level_info.level_origin.x + level_info.width;
                        let m2_min_y = level_info.level_origin.y;
                        let m2_max_y = level_info.level_origin.y + level_info.height;

                        // Tính Hình chữ nhật Giao Nhau (Overlap Rectangle)
                        let overlap_min_x = m1_min_x.max(m2_min_x);
                        let overlap_max_x = m1_max_x.min(m2_max_x);
                        let overlap_min_y = m1_min_y.max(m2_min_y);
                        let overlap_max_y = m1_max_y.min(m2_max_y);

                        // TÂM VẬT LÝ CỦA GIAO DIỆN (Đây CHẮC CHẮN là điểm chính giữa cây cầu)
                        let overlap_cx = (overlap_min_x + overlap_max_x) / 2;
                        let overlap_cy = (overlap_min_y + overlap_max_y) / 2;

                        let mut real_exit = (overlap_cx, overlap_cy);
                        let mut min_dist = f32::MAX;
                        let mut found = false;

                        // Quét Area 1 để tìm viên gạch đi được GẦN TÂM CẦU NHẤT
                        for (&(x, y), &val) in &area.grid {
                            if val % 2 == 0 {
                                let dx = (x - overlap_cx) as f32;
                                let dy = (y - overlap_cy) as f32;
                                let dist = (dx * dx + dy * dy).sqrt();

                                if dist < min_dist {
                                    min_dist = dist;
                                    real_exit = (x, y);
                                    found = true;
                                }
                            }
                        }

                        if found {
                            area.exits.insert(
                                target_id,
                                (real_exit.0, real_exit.1, ExitType::Boundary, normal),
                            );
                            println!(
                                "[MAP-API] Giao diện Area {}: Tâm Cầu VẬT LÝ ({}, {}) -> Chốt điểm E thật tại ({}, {})",
                                target_id, overlap_cx, overlap_cy, real_exit.0, real_exit.1
                            );
                        } else {
                            println!(
                                "\x1b[31m[MAP-API] LỖI: Không có điểm walkable nào gần giao diện Area {}\x1b[0m",
                                target_id
                            );
                        }
                    }
                }
            }
            self.areas.insert(area_id, area);
            println!("[MAP-API] Nạp thành công Area {}.", area_id);
            return Some(());
        }
        None
    }

    pub fn find_exit_position(
        &self,
        cur_id: u32,
        target_id: u32,
    ) -> Option<(i32, i32, ExitType, (f32, f32))> {
        self.areas.get(&cur_id)?.exits.get(&target_id).copied()
    }

    pub fn get_astar_grid(
        &self,
        area_id: u32,
        next_area_id: Option<u32>,
    ) -> HashMap<(i32, i32), bool> {
        let mut g = HashMap::new();

        // 1. Nạp lưới Area hiện tại
        if let Some(area) = self.areas.get(&area_id) {
            for (&pos, &val) in &area.grid {
                g.insert(pos, val % 2 == 0);
            }
            for ex in area.exits.values() {
                g.insert((ex.0, ex.1), true);
            }
        }

        // 2. VÁ CẦU: Nạp lưới Area đích vào chung một bản đồ
        if let Some(target_id) = next_area_id {
            if let Some(target_area) = self.areas.get(&target_id) {
                for (&pos, &val) in &target_area.grid {
                    if val % 2 == 0 {
                        g.insert(pos, true);
                    }
                }
            }
        }
        g
    }

    /// Trả về DANH SÁCH tất cả các điểm E thỏa mãn định nghĩa:
    /// Thuộc A1, Walkable, và chạm vào ít nhất 1 điểm Walkable của A2.
    pub fn find_true_boundaries(&self, a1_id: u32, a2_id: u32) -> Vec<(i32, i32)> {
        let mut valid_exits = Vec::new();
        let a1 = match self.areas.get(&a1_id) {
            Some(area) => area,
            None => return valid_exits,
        };
        let a2 = match self.areas.get(&a2_id) {
            Some(area) => area,
            None => return valid_exits,
        };

        for (&(x1, y1), &val1) in &a1.grid {
            if val1 % 2 == 0 {
                let mut found_adjacent = false;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        if let Some(&val2) = a2.grid.get(&(x1 + dx, y1 + dy)) {
                            if val2 % 2 == 0 {
                                found_adjacent = true;
                                break;
                            }
                        }
                    }
                    if found_adjacent {
                        break;
                    }
                }
                if found_adjacent {
                    valid_exits.push((x1, y1));
                }
            }
        }
        valid_exits
    }
}

pub struct GameTopology {
    pub connections: HashMap<u32, Vec<u32>>,
}
impl GameTopology {
    pub fn new() -> Self {
        let mut t = HashMap::new();
        t.insert(1, vec![2]);
        t.insert(2, vec![1, 3, 8]);
        t.insert(3, vec![2, 4, 17]);
        t.insert(8, vec![2]);
        Self { connections: t }
    }
    pub fn get_macro_route(&self, start: u32, target: u32) -> Option<Vec<u32>> {
        let mut q = VecDeque::from([start]);
        let mut visited = HashMap::from([(start, start)]);
        while let Some(curr) = q.pop_front() {
            if curr == target {
                let (mut r, mut n) = (vec![target], target);
                while n != start {
                    n = *visited.get(&n)?;
                    r.push(n);
                }
                r.reverse();
                return Some(r);
            }
            if let Some(neighbors) = self.connections.get(&curr) {
                for &nb in neighbors {
                    if !visited.contains_key(&nb) {
                        visited.insert(nb, curr);
                        q.push_back(nb);
                    }
                }
            }
        }
        None
    }
}
