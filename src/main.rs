mod astar;
mod input;
mod map;
mod memory;
mod moving;

use crate::input::MouseButton;
use crate::map::WorldMap;
use device_query::{DeviceQuery, DeviceState, Keycode};
use memory::{GameOffsets, MemoryReader, find_pid_by_name, get_wine_base_address};
use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() {
    println!("--- D2R Linux Bot Controller ---");
    println!("[HOTKEY] Bấm F7 để TẠM DỪNG / TIẾP TỤC Bot.");

    // 1. Tìm PID & Base Address (Giữ nguyên logic của bạn)
    let process_target = "d2r";
    let pid = find_pid_by_name(process_target).expect("Không tìm thấy game d2r");
    let (base_addr, base_size) = get_wine_base_address(pid).expect("Không tìm thấy Base Address");
    let reader = MemoryReader::new(pid).expect("Lỗi mở RAM");
    let mut input = input::InputController::new("Diablo II").expect("Lỗi khởi tạo Input");

    // 2. Quét Offsets
    let mut module_buffer = vec![0u8; base_size];
    use std::os::unix::fs::FileExt;
    reader
        .mem_file
        .read_exact_at(&mut module_buffer, base_addr)
        .ok();
    let mut offsets = GameOffsets::load_from_memory(&module_buffer);
    offsets.find_player_unit(&reader, base_addr);

    // 4. Trạng thái điều khiển
    let device_state = DeviceState::new();
    let mut is_bot_enabled = false; // Mặc định vào game chưa chạy ngay
    let mut last_f7_state = false;

    println!("[+] Bot đã sẵn sàng. Bấm F7 để bắt đầu.");

    let seed = 12345678;
    let difficulty = 0;

    let area_id = 2; // Blood Moor
    let target_area = 8; // Den of Evil

    let mut world_map = WorldMap::new();
    world_map.fetch_map_from_seed(seed, difficulty, area_id);
    let astar_grid = world_map.get_astar_grid(area_id);
    let target_pos = world_map.find_exit_position(area_id, target_area);

    println!("Target Position: {:?}", target_pos);

    let mut current_path: Vec<(i32, i32)> = Vec::new();

    // Không cần get_d2r_window_position nữa!
    // Tâm chuột TƯƠNG ĐỐI của cửa sổ game (bạn có thể thay đổi nếu chơi độ phân giải khác)
    let center_x = 640;
    let center_y = 352;

    loop {
        let keys = device_state.get_keys();
        let f7_pressed = keys.contains(&Keycode::F7);

        if f7_pressed && !last_f7_state {
            is_bot_enabled = !is_bot_enabled;
            println!(
                "{}",
                if is_bot_enabled {
                    "\x1b[32m[STATUS] BOT: RUNNING\x1b[0m"
                } else {
                    "\x1b[31m[STATUS] BOT: PAUSED\x1b[0m"
                }
            );
            if !is_bot_enabled {
                current_path.clear();
            }
        }
        last_f7_state = f7_pressed;

        if is_bot_enabled {
            let player_ptr = offsets.player_unit_ptr;
            let path_ptr = reader.read_u64(player_ptr + 0x38).unwrap_or(0);

            if path_ptr != 0 {
                let p_x = reader.read_u16(path_ptr + 0x02).unwrap_or(0) as i32;
                let p_y = reader.read_u16(path_ptr + 0x06).unwrap_or(0) as i32;

                // 1. Tính A*
                if current_path.is_empty() {
                    if let Some(dest) = target_pos {
                        if let Some(path) = astar::find_path(&astar_grid, (p_x, p_y), dest) {
                            current_path = path;
                            println!("Đường đi mới: {} bước", current_path.len());
                        }
                    }
                }

                // 2. Logic bám đường
                if !current_path.is_empty() {
                    // --- Dọn dẹp Node ---
                    let scan_range = std::cmp::min(current_path.len(), 10);
                    let mut closest_idx = 0;
                    let mut min_dist = f32::MAX;

                    for i in 0..scan_range {
                        let d = (((current_path[i].0 - p_x).pow(2)
                            + (current_path[i].1 - p_y).pow(2))
                            as f32)
                            .sqrt();
                        if d < min_dist {
                            min_dist = d;
                            closest_idx = i;
                        }
                    }

                    for _ in 0..closest_idx {
                        current_path.remove(0);
                    }

                    if min_dist <= 2.5 && current_path.len() > 1 {
                        current_path.remove(0);
                    }

                    // --- Di chuyển ---
                    if current_path.len() > 1 {
                        // Chạy bộ
                        let look_ahead = std::cmp::min(current_path.len() - 1, 4);
                        let target_node = current_path[look_ahead];

                        moving::move_to_node_isometric(
                            p_x as f32,
                            p_y as f32,
                            target_node.0 as f32,
                            target_node.1 as f32,
                            center_x,
                            center_y, // CHỈ TRUYỀN TÂM TƯƠNG ĐỐI
                            &mut input,
                        );
                    } else {
                        // Tương tác cổng
                        let target = current_path[0];
                        let dist_to_door =
                            (((target.0 - p_x).pow(2) + (target.1 - p_y).pow(2)) as f32).sqrt();

                        if dist_to_door > 3.0 {
                            moving::move_to_node_isometric(
                                p_x as f32,
                                p_y as f32,
                                target.0 as f32,
                                target.1 as f32,
                                center_x,
                                center_y,
                                &mut input,
                            );
                        } else {
                            println!("[ACTION] Đã đến sát cổng, tiến hành click để vào hầm...");
                            moving::click_object_isometric(
                                p_x as f32,
                                p_y as f32,
                                target.0 as f32,
                                target.1 as f32,
                                center_x,
                                center_y,
                                &mut input,
                            );

                            thread::sleep(Duration::from_millis(1500));
                            current_path.clear();
                        }
                    }
                }
            }

            // input.click_at(center_x, center_y, MouseButton::Left);
        }
        thread::sleep(Duration::from_millis(150));
    }
}
