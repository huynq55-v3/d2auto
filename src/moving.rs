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
        let click_radius = length.min(50.0);

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
