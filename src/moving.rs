use crate::input::{InputController, MouseButton};

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
        // Lấy bán kính click là 50.0, NHƯNG nếu khoảng cách thực tế (length)
        // tới Node ngắn hơn 50.0, thì click thẳng vào Node đó luôn.
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

    // Không dùng radius, click thẳng vào vị trí pixel của vật thể/cổng
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

    // 1. Dọn dẹp Node: Nếu nhân vật đã đứng đủ gần Node 0 (<= 3.0), ăn Node đó.
    let first_node = current_path[0];
    let dist_to_first = (((first_node.0 - p_x).pow(2) + (first_node.1 - p_y).pow(2)) as f32).sqrt();

    if dist_to_first <= 3.0 {
        current_path.remove(0);
    }

    if current_path.is_empty() {
        return; // Đã tới đích cuối cùng
    }

    // 2. Lấy đích theo 5 node một. Nếu path còn ít hơn 5 node, lấy node cuối cùng (lẻ 1, 2, 3, 4).
    let look_ahead_idx = std::cmp::min(current_path.len() - 1, 5);
    let target_node = current_path[look_ahead_idx];

    // 3. Gọi hàm di chuyển (Bên trong hàm này đã xử lý radius = min(100.0, length))
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
