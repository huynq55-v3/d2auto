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

            // 2. Nạp Lối đi
            for (target_id_str, level_info) in map_data.adjacent_levels {
                if let Ok(target_id) = target_id_str.parse::<u32>() {
                    // Tính Vectơ pháp tuyến chuẩn (Hướng từ Map hiện tại sang Map mới)
                    let raw_dx = (level_info.level_origin.x - area.origin.x) as f32;
                    let raw_dy = (level_info.level_origin.y - area.origin.y) as f32;
                    let mag = (raw_dx * raw_dx + raw_dy * raw_dy).sqrt();
                    let normal = if mag > 0.0 {
                        let nx = raw_dx / mag;
                        let ny = raw_dy / mag;
                        // Làm tròn để đâm vuông góc (N, S, E, W)
                        if nx.abs() > ny.abs() {
                            (nx.signum(), 0.0)
                        } else {
                            (0.0, ny.signum())
                        }
                    } else {
                        (1.0, 0.0)
                    };

                    if let Some(first_exit) = level_info.exits.first() {
                        area.exits.insert(
                            target_id,
                            (first_exit.x, first_exit.y, ExitType::Object, normal),
                        );
                    } else {
                        // FALLBACK: Tính vùng giao trung bình cộng cho Boundary
                        let mut pts = Vec::new();
                        for (&(x, y), &val) in &area.grid {
                            if val % 2 == 0
                                && x >= level_info.level_origin.x - 2
                                && x <= level_info.level_origin.x + level_info.width + 2
                                && y >= level_info.level_origin.y - 2
                                && y <= level_info.level_origin.y + level_info.height + 2
                            {
                                pts.push((x, y));
                            }
                        }
                        if !pts.is_empty() {
                            let count = pts.len() as i32;
                            let cx = pts.iter().map(|p| p.0).sum::<i32>() / count;
                            let cy = pts.iter().map(|p| p.1).sum::<i32>() / count;
                            area.exits
                                .insert(target_id, (cx, cy, ExitType::Boundary, normal));
                        }
                    }
                }
            }
            self.areas.insert(area_id, area);
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

    pub fn get_astar_grid(&self, area_id: u32) -> HashMap<(i32, i32), bool> {
        let mut g = HashMap::new();
        if let Some(area) = self.areas.get(&area_id) {
            for (&pos, &val) in &area.grid {
                g.insert(pos, val % 2 == 0);
            }
            for ex in area.exits.values() {
                g.insert((ex.0, ex.1), true);
            }
        }
        g
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
