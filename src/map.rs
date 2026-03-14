use std::collections::{HashMap, VecDeque};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
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
}
