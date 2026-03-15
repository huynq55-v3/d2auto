mod astar;
mod input;
mod map;
mod memory;
mod models;
mod moving;

use crate::map::WorldMap;
use memory::{GameOffsets, MemoryReader, find_pid_by_name, get_wine_base_address};
use rdev::{Event, EventType, Key, listen};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

fn main() {
    println!("--- D2R Linux Bot Controller ---");
    println!("[HOTKEY] Bấm F7 để TẠM DỪNG / TIẾP TỤC Bot. (GLOBAL HOTKEY)");

    // 1. Tìm PID & Base Address
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

    // ==========================================
    // 3. LUỒNG BÀN PHÍM GLOBAL (DÙNG RDEV)
    // ==========================================
    let is_bot_enabled = Arc::new(AtomicBool::new(false));
    let is_bot_enabled_hotkey = Arc::clone(&is_bot_enabled);

    thread::spawn(move || {
        let callback = move |event: Event| {
            if let EventType::KeyPress(key) = event.event_type {
                if key == Key::F7 {
                    let current_state = is_bot_enabled_hotkey.load(Ordering::SeqCst);
                    let new_state = !current_state;
                    is_bot_enabled_hotkey.store(new_state, Ordering::SeqCst);

                    println!(
                        "{}",
                        if new_state {
                            "\x1b[32m[STATUS] BOT: RUNNING\x1b[0m"
                        } else {
                            "\x1b[31m[STATUS] BOT: PAUSED\x1b[0m"
                        }
                    );
                }
            }
        };

        if let Err(error) = listen(callback) {
            println!("[-] Lỗi khởi tạo Global Hotkey: {:?}", error);
        }
    });
    // ==========================================

    println!("[+] Bot đã sẵn sàng. Bấm F7 ở bất kỳ đâu để bắt đầu.");

    let seed = 1234567;
    let difficulty = 0;
    let target_area = 8; // Đích đến: Den of Evil

    let mut world_map = WorldMap::new();
    let topology = crate::map::GameTopology::new();
    let mut current_path: Vec<(i32, i32)> = Vec::new();

    let center_x = 640;
    let center_y = 352;

    let mut main_thread_was_enabled = false;
    let mut last_area_id = 0; // Để theo dõi lúc đổi map

    // VÒNG LẶP CHÍNH
    loop {
        let currently_enabled = is_bot_enabled.load(Ordering::SeqCst);

        if !currently_enabled && main_thread_was_enabled {
            current_path.clear();
        }
        main_thread_was_enabled = currently_enabled;

        if currently_enabled {
            // Giả sử bạn đang ở main loop, đã có base_address và offsets
            let all_monsters = reader.get_all_monsters(base_addr, offsets.unit_table);

            let mut alive_count = 0;
            for m in &all_monsters {
                // Mode 0 và 12 trong D2R thường là trạng thái Đã Chết (Corpse) / Đang Ngã Xuống
                // (Lưu ý: Bạn có thể cần đối chiếu chính xác bảng Mode của D2R sau này)
                if m.mode != 0 && m.mode != 12 {
                    alive_count += 1;
                    // println!("Quái sống ID: {} | Class: {} | Tọa độ: ({}, {})", m.unit_id, m.class_id, m.x, m.y);
                }
            }
            println!(
                "[RADAR] Quét thấy {} con quái ({} con còn sống)",
                all_monsters.len(),
                alive_count
            );
        }

        // Tốc độ nhả lệnh chuẩn 150ms để xdotool tiêu hóa kịp, chống chuột ma
        thread::sleep(Duration::from_millis(150));
    }
}
