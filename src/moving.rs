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
    if current_area_id == 0 || current_area_id == target_area {
        if current_area_id == target_area {
            current_path.clear();
        }
        return;
    }

    // 1. Tìm Area tiếp theo
    let route = match topology.get_macro_route(current_area_id, target_area) {
        Some(r) if r.len() >= 2 => r,
        _ => return,
    };
    let next_area_id = route[1];

    // 2. Load Map
    if !world_map.areas.contains_key(&current_area_id) {
        world_map.fetch_map_from_seed(seed, difficulty, current_area_id);
    }

    // 3. Lấy thông tin Exit (Khớp 4 tham số)
    let (exit_x, exit_y, exit_type, normal_n) =
        match world_map.find_exit_position(current_area_id, next_area_id) {
            Some(pos) => pos,
            None => return,
        };

    // 4. Tìm đường A* nếu rỗng
    if current_path.is_empty() {
        let grid = world_map.get_astar_grid(current_area_id);
        if let Some(path) = astar::find_path(&grid, (p_x, p_y), (exit_x, exit_y)) {
            *current_path = path;
        }
    }

    // 5. Thực thi di chuyển
    if !current_path.is_empty() {
        if current_path.len() > 1 {
            move_follow_astar(p_x, p_y, current_path, center_x, center_y, input);
        } else {
            let target = current_path[0];
            let dist = (((target.0 - p_x).pow(2) + (target.1 - p_y).pow(2)) as f32).sqrt();

            if exit_type == ExitType::Boundary {
                // CHIẾN THUẬT "ĐÂM CHÍNH DIỆN - ĐÂM SIÊU SÂU"
                // Click vào điểm nằm tít bên kia biên giới theo Vectơ pháp tuyến chuẩn
                let thrust_x = target.0 as f32 + normal_n.0 * 150.0;
                let thrust_y = target.1 as f32 + normal_n.1 * 150.0;

                move_to_node_isometric(
                    p_x as f32, p_y as f32, thrust_x, thrust_y, center_x, center_y, input,
                );

                // Giữ path để Tick sau húc tiếp, không xóa path ở đây!
            } else {
                // --- CHIẾN THUẬT "CLICK LỲ LỢM" CHO CỬA HANG ---
                if dist > 4.0 {
                    // Nếu còn hơi xa, nhích thêm tí nữa
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
                    // ĐÃ ĐỨNG SÁT CỬA HANG:
                    // 1. Click tương tác
                    click_object_isometric(
                        p_x as f32,
                        p_y as f32,
                        target.0 as f32,
                        target.1 as f32,
                        center_x,
                        center_y,
                        input,
                    );

                    // 2. CHỈ NGỦ NGẮN (Ví dụ 500ms) để chờ hiệu ứng loading map bắt đầu
                    // Không dùng 3000ms vì nó quá lâu.
                    thread::sleep(Duration::from_millis(500));

                    // 3. QUAN TRỌNG: TUYỆT ĐỐI KHÔNG Clear Path ở đây!
                    // Nếu sau 500ms mà RAM vẫn báo ở Map cũ (Area 1),
                    // vòng lặp sau sẽ quay lại đây và CLICK TIẾP phát nữa.
                    // Bot sẽ click cho đến khi nào vào được hang thì mới thôi.
                }
            }
        }
    }
}
