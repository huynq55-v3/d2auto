mod input;
mod memory;
mod models;

use memory::{GameOffsets, MemoryReader, find_pid_by_name, get_wine_base_address};
use models::PlayerInfo;
use std::os::unix::fs::FileExt;
use std::thread;
use std::time::Duration;

fn main() {
    println!("--- D2R Linux Memory Scanner & Parser ---");

    // 1. Tìm PID của game
    let process_target = "d2r";
    println!("Đang tìm tiến trình có tên: {}...", process_target);

    let pid = match find_pid_by_name(process_target) {
        Some(p) => {
            println!("[+] Tìm thấy PID: {}", p);
            p
        }
        None => {
            println!(
                "[-] Không tìm thấy tiến trình nào chứa từ khóa: '{}'",
                process_target
            );
            println!("Gợi ý: Hãy mở game lên trước khi chạy công cụ này.");
            return;
        }
    };

    // 3. Tìm Base Address
    println!("Đang quét Base Address theo chữ ký 'MZ'...");
    let (base_addr, base_size) = match get_wine_base_address(pid) {
        Some((addr, size)) => {
            println!(
                "[SUCCESS] Tìm thấy Base Address: 0x{:X} (Size: {} MB)",
                addr,
                size / 1024 / 1024
            );
            (addr, size)
        }
        None => {
            println!("[-] Không tìm thấy Base Address. Bạn đã chạy chương trình bằng 'sudo' chưa?");
            return;
        }
    };

    // 4. Khởi tạo MemoryReader
    let reader = match MemoryReader::new(pid) {
        Ok(r) => r,
        Err(e) => {
            println!("[-] Lỗi mở RAM: {}. Hãy chạy bằng sudo.", e);
            return;
        }
    };

    // 5. Khởi tạo InputController (X11 Native)
    println!("Đang kết nối tới X Server và tìm cửa sổ game...");
    let mut input = match input::InputController::new("Diablo II") {
        Ok(i) => i,
        Err(e) => {
            println!("[-] Lỗi khởi tạo Input: {}", e);
            return;
        }
    };

    // 6. Quét Game Offsets
    println!("Đang quét các Offset của Game từ bộ nhớ...");
    println!(
        "[*] Đang đọc {} MB RAM để quét Pattern...",
        base_size / 1024 / 1024
    );
    let mut module_buffer = vec![0u8; base_size];
    if let Err(e) = reader.mem_file.read_exact_at(&mut module_buffer, base_addr) {
        println!("[-] Lỗi đọc RAM: {}", e);
        return;
    }

    let mut offsets = GameOffsets::load_from_memory(&module_buffer);
    offsets.find_player_unit(&reader, base_addr);

    println!("[+] Đã tìm thấy các Offset chính:");
    println!("    - GameData:  0x{:X}", offsets.game_data);
    println!("    - UnitTable: 0x{:X}", offsets.unit_table);
    println!("    - PlayerUnitPtr: 0x{:X}", offsets.player_unit_ptr);

    // 7. Game Loop
    println!("[+] Bắt đầu Game Loop (Tick Rate: 30 FPS)...");
    loop {
        let players = PlayerInfo::get_local_players(&reader, base_addr, offsets.unit_table);

        for player in players {
            if player.x > 0 && player.y > 0 {
                println!(
                    "Nhân vật (ID: {}) đang đứng tại: X = {}, Y = {}",
                    player.id, player.x, player.y
                );
            }
        }

        thread::sleep(Duration::from_millis(33));
    }
}
