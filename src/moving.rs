use std::{thread, time::Duration};

use crate::{
    astar,
    input::{InputController, MouseButton},
    map::{ExitType, GameTopology, WorldMap}, // Import thêm ExitType
    memory::MemoryReader,
};

pub fn move_to_node_isometric(
    cur_x: f32,
    cur_y: f32,
    next_x: f32,
    next_y: f32,
    screen_center_x: i32,
    screen_center_y: i32,
    input: &mut InputController,
) {
    let delta_x = next_x - cur_x;
    let delta_y = next_y - cur_y;

    let scale_x = 32.0;
    let scale_y = 16.0;

    let screen_dx = (delta_x - delta_y) * scale_x;
    let screen_dy = (delta_x + delta_y) * scale_y;

    let length = (screen_dx.powi(2) + screen_dy.powi(2)).sqrt();

    if length > 0.0 {
        let click_radius = length.min(100.0);

        let click_x = (screen_center_x as f32) + (screen_dx / length) * click_radius;
        let click_y = (screen_center_y as f32) + (screen_dy / length) * click_radius;

        let _ = input.click_at(click_x as i32, click_y as i32, MouseButton::Left);
    }
}

pub fn click_object_isometric(
    cur_x: f32,
    cur_y: f32,
    obj_x: f32,
    obj_y: f32,
    screen_center_x: i32,
    screen_center_y: i32,
    input: &mut InputController,
) {
    let delta_x = obj_x - cur_x;
    let delta_y = obj_y - cur_y;

    let scale_x = 32.0;
    let scale_y = 16.0;

    let screen_dx = (delta_x - delta_y) * scale_x;
    let screen_dy = (delta_x + delta_y) * scale_y;

    let click_x = (screen_center_x as f32) + screen_dx;
    let click_y = (screen_center_y as f32) + screen_dy;

    println!(
        ">>> CLICK TƯƠNG TÁC TẠI ({}, {})",
        click_x as i32, click_y as i32
    );
    let _ = input.click_at(click_x as i32, click_y as i32, MouseButton::Left);
}

pub fn move_follow_astar(
    p_x: i32,
    p_y: i32,
    current_path: &mut Vec<(i32, i32)>,
    center_x: i32,
    center_y: i32,
    input: &mut InputController,
) {
    if current_path.is_empty() {
        return;
    }

    let scan_range = std::cmp::min(current_path.len(), 10);
    let mut closest_idx = 0;

    let mut min_dist =
        (((current_path[0].0 - p_x).pow(2) + (current_path[0].1 - p_y).pow(2)) as f32).sqrt();

    for i in 1..scan_range {
        let d =
            (((current_path[i].0 - p_x).pow(2) + (current_path[i].1 - p_y).pow(2)) as f32).sqrt();

        if d <= min_dist {
            min_dist = d;
            closest_idx = i;
        } else {
            break;
        }
    }

    for _ in 0..closest_idx {
        current_path.remove(0);
    }

    if min_dist <= 2.5 && current_path.len() > 1 {
        current_path.remove(0);
    }

    if current_path.is_empty() {
        return;
    }

    let look_ahead_idx = std::cmp::min(current_path.len() - 1, 4);
    let target_node = current_path[look_ahead_idx];

    move_to_node_isometric(
        p_x as f32,
        p_y as f32,
        target_node.0 as f32,
        target_node.1 as f32,
        center_x,
        center_y,
        input,
    );
}

pub fn move_to_act(
    target_area: u32,
    player_ptr: u64,
    p_x: i32,
    p_y: i32,
    reader: &MemoryReader,
    topology: &GameTopology,
    world_map: &mut WorldMap,
    current_path: &mut Vec<(i32, i32)>,
    center_x: i32,
    center_y: i32,
    input: &mut InputController,
    seed: u32,
    difficulty: u32,
) {
    let current_area_id = reader.read_current_area_id(player_ptr);

    // LOG 1: Bắt đầu tick
    println!(
        "\n--- [DEBUG-FLOW] Tick bắt đầu | current_area: {}, target: {}, pos: ({}, {})",
        current_area_id, target_area, p_x, p_y
    );

    if current_area_id == 0 || current_area_id == target_area {
        if current_area_id == target_area {
            println!(
                "[DEBUG-FLOW] Đã đến đích (Area {}). Clear path.",
                target_area
            );
            current_path.clear();
        } else {
            println!("[DEBUG-FLOW] current_area_id == 0. Bỏ qua tick.");
        }
        return;
    }

    // 1. Tìm Lộ trình Macro
    let route = match topology.get_macro_route(current_area_id, target_area) {
        Some(r) if r.len() >= 2 => {
            println!("[DEBUG-FLOW] Tìm thấy route: {:?}", r);
            r
        }
        _ => {
            println!(
                "[DEBUG-FLOW] LỖI: Topology không tìm ra route từ {} đến {}",
                current_area_id, target_area
            );
            return;
        }
    };

    // 2. TẢI TRƯỚC (Pre-load)
    for &area_id in &route {
        if !world_map.areas.contains_key(&area_id) {
            println!("[PRE-LOAD] Đang tải trước Map cho Area {}...", area_id);
            world_map.fetch_map_from_seed(seed, difficulty, area_id);
        }
    }

    let next_area_id = route[1];
    println!("[DEBUG-FLOW] next_area_id: {}", next_area_id);

    // 3. Lấy thông tin Exit (Tạm thời lấy Normal để lát nữa húc biên giới)
    let (exit_x, exit_y, exit_type, normal_n) =
        match world_map.find_exit_position(current_area_id, next_area_id) {
            Some(pos) => pos,
            None => return,
        };

    // 4. Tìm đường A* (Với vị trí xuất phát được bảo kê)
    if current_path.is_empty() {
        // Lấy lưới đã được ghép từ 2 map
        let mut grid = world_map.get_astar_grid(current_area_id, Some(next_area_id));

        grid.insert((p_x, p_y), true);
        grid.insert((p_x + 1, p_y), true);
        grid.insert((p_x - 1, p_y), true);
        grid.insert((p_x, p_y + 1), true);
        grid.insert((p_x, p_y - 1), true);

        if exit_type == ExitType::Boundary {
            // Lấy TẤT CẢ ứng viên thỏa mãn
            let mut candidates = world_map.find_true_boundaries(current_area_id, next_area_id);

            if candidates.is_empty() {
                println!("\x1b[31m[MACRO] LỖI: A1 và A2 không có điểm nào chạm nhau!\x1b[0m");
                return;
            }

            // Ưu tiên điểm gần nhân vật nhất
            candidates.sort_by(|a, b| {
                let dist_a = (a.0 - p_x).pow(2) + (a.1 - p_y).pow(2);
                let dist_b = (b.0 - p_x).pow(2) + (b.1 - p_y).pow(2);
                dist_a.cmp(&dist_b)
            });

            let mut found_path = false;
            // Cho A* tự chọn điểm đích nó thích nhất
            for &e in &candidates {
                if let Some(path) = astar::find_path(&grid, (p_x, p_y), e) {
                    *current_path = path;
                    println!(
                        "\x1b[32m[MACRO] A* ĐÃ TÌM THẤY ĐƯỜNG TỚI ĐIỂM E ({}, {}). Số bước: {}\x1b[0m",
                        e.0,
                        e.1,
                        current_path.len()
                    );
                    found_path = true;
                    break;
                }
            }

            if !found_path {
                println!(
                    "\x1b[31m[MACRO-FAIL] Thử {} điểm E nhưng A* đều chịu thua!\x1b[0m",
                    candidates.len()
                );
            }
        } else {
            // Logic cho Hang động (Object)
            if let Some(path) = astar::find_path(&grid, (p_x, p_y), (exit_x, exit_y)) {
                *current_path = path;
                println!(
                    "\x1b[32m[MACRO] Đã vẽ xong đường A* tới Object ({} bước)\x1b[0m",
                    current_path.len()
                );
            } else {
                println!(
                    "\x1b[31m[MACRO-FAIL] A* tịt ngòi với Object tại ({}, {})\x1b[0m",
                    exit_x, exit_y
                );
            }
        }
    }

    // 5. Thực thi di chuyển
    if !current_path.is_empty() {
        println!("[DEBUG-FLOW] Bắt đầu gọi lệnh di chuyển...");
        if current_path.len() > 1 {
            move_follow_astar(p_x, p_y, current_path, center_x, center_y, input);
        } else {
            let target = current_path[0];
            let dist = (((target.0 - p_x).pow(2) + (target.1 - p_y).pow(2)) as f32).sqrt();

            if exit_type == ExitType::Boundary {
                let thrust_x = target.0 as f32 + normal_n.0 * 150.0;
                let thrust_y = target.1 as f32 + normal_n.1 * 150.0;
                println!(
                    "[DEBUG-FLOW] Thực thi HÚC BIÊN GIỚI tới ({}, {})",
                    thrust_x, thrust_y
                );
                move_to_node_isometric(
                    p_x as f32, p_y as f32, thrust_x, thrust_y, center_x, center_y, input,
                );
            } else {
                if dist > 4.0 {
                    println!("[DEBUG-FLOW] Chạy lại gần OBJECT (dist={})", dist);
                    move_to_node_isometric(
                        p_x as f32,
                        p_y as f32,
                        target.0 as f32,
                        target.1 as f32,
                        center_x,
                        center_y,
                        input,
                    );
                } else {
                    println!("[DEBUG-FLOW] Click OBJECT và Sleep 500ms");
                    click_object_isometric(
                        p_x as f32,
                        p_y as f32,
                        target.0 as f32,
                        target.1 as f32,
                        center_x,
                        center_y,
                        input,
                    );
                    thread::sleep(std::time::Duration::from_millis(500));
                }
            }
        }
    } else {
        println!("\x1b[31m[DEBUG-FLOW] KHÔNG THỂ DI CHUYỂN: current_path rỗng ở cuối hàm.\x1b[0m");
    }
}
