use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    cost: i32,
    position: (i32, i32),
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .cmp(&self.cost)
            .then_with(|| self.position.cmp(&other.position))
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Tìm đường A* trên một Grid cụ thể. Trả về danh sách các Tile cần đi qua.
pub fn find_path(
    grid: &HashMap<(i32, i32), bool>,
    start: (i32, i32),
    goal: (i32, i32),
) -> Option<Vec<(i32, i32)>> {
    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

    g_score.insert(start, 0);

    let heuristic =
        |p: (i32, i32), g: (i32, i32)| -> i32 { (p.0 - g.0).abs().max((p.1 - g.1).abs()) * 10 };

    open_set.push(State {
        cost: heuristic(start, goal),
        position: start,
    });

    let directions = [
        (0, 1),
        (1, 0),
        (0, -1),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];

    while let Some(State { position, .. }) = open_set.pop() {
        if position == goal {
            // Đã tới đích, truy ngược lại đường đi
            let mut path = vec![goal];
            let mut current = goal;
            while let Some(&prev) = came_from.get(&current) {
                path.push(prev);
                current = prev;
                if current == start {
                    break;
                }
            }
            path.reverse();
            return Some(path);
        }

        let current_g = *g_score.get(&position).unwrap_or(&i32::MAX);

        for &(dx, dy) in &directions {
            let neighbor = (position.0 + dx, position.1 + dy);

            // LUẬT TÌM ĐƯỜNG: Ô đó phải tồn tại trong map VÀ phải là đất trống (true)
            if let Some(&is_walkable) = grid.get(&neighbor) {
                if is_walkable {
                    let move_cost = if dx != 0 && dy != 0 { 14 } else { 10 };
                    let tentative_g = current_g + move_cost;

                    let neighbor_g = *g_score.get(&neighbor).unwrap_or(&i32::MAX);
                    if tentative_g < neighbor_g {
                        came_from.insert(neighbor, position);
                        g_score.insert(neighbor, tentative_g);

                        let f_score = tentative_g + heuristic(neighbor, goal);
                        open_set.push(State {
                            cost: f_score,
                            position: neighbor,
                        });
                    }
                }
            }
        }
    }

    None // Không tìm thấy đường
}
