mod astar;
mod input;
mod map;
mod memory;
mod moving;

use device_query::{DeviceQuery, DeviceState, Keycode};
use memory::{GameOffsets, MemoryReader, find_pid_by_name, get_wine_base_address};
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

    // hardcode den of evil door position
    let (den_of_evil_x, den_of_evil_y) = (5215, 5940);

    // 5. Game Loop
    loop {
        // --- KIỂM TRA PHÍM F7 (TOGGLE) ---
        let keys = device_state.get_keys();
        let f7_pressed = keys.contains(&Keycode::F7);

        if f7_pressed && !last_f7_state {
            is_bot_enabled = !is_bot_enabled;
            if is_bot_enabled {
                println!("\x1b[32m[STATUS] BOT: RUNNING\x1b[0m"); // In màu xanh
            } else {
                println!("\x1b[31m[STATUS] BOT: PAUSED\x1b[0m"); // In màu đỏ
            }
        }
        last_f7_state = f7_pressed;

        if is_bot_enabled {
            let player_ptr = offsets.player_unit_ptr;

            let path_ptr = reader.read_u64(player_ptr + 0x38).unwrap_or(0);

            let player_x = reader.read_u16(path_ptr + 0x02).unwrap_or(0) as i32;
            let player_y = reader.read_u16(path_ptr + 0x06).unwrap_or(0) as i32;

            println!("Player Position: ({}, {})", player_x, player_y);
        }

        // Tần số quét 25ms/lần (40 FPS cho não Bot)
        thread::sleep(Duration::from_millis(25));
    }
}
