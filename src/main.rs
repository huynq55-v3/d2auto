mod astar;
mod input;
mod map;
mod memory;
mod scripting;
mod seed;

use map::{AreaManager, GameTopology};
use memory::{GameOffsets, MemoryReader, find_pid_by_name, get_wine_base_address};
use scripting::{BotEngine, ScriptParser};
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
    let mut module_buffer = vec![0u8; base_size];
    use std::os::unix::fs::FileExt;
    if let Err(e) = reader.mem_file.read_exact_at(&mut module_buffer, base_addr) {
        println!("[-] Lỗi đọc RAM: {}", e);
        return;
    }

    let mut offsets = GameOffsets::load_from_memory(&module_buffer);
    offsets.find_player_unit(&reader, base_addr);

    println!("[+] Đã tìm thấy các Offset chính:");
    println!("    - UnitTable: 0x{:X}", offsets.unit_table);
    println!("    - PlayerUnitPtr: 0x{:X}", offsets.player_unit_ptr);
    println!("    - GameData: 0x{:X}", offsets.game_data);

    let current_map_seed = seed::read_seed_from_memory(&reader, offsets.player_unit_ptr);
    println!("    - Current Map Seed: {}", current_map_seed.unwrap());

    // 7. Khởi tạo Bot Components
    let mut area_manager = AreaManager::new();
    let topology = GameTopology::new();
    let mut engine = BotEngine::new();
    let parser = ScriptParser::new();

    // Nạp script mặc định: Đến Den of Evil
    let script = "go to den of evil";
    let commands = parser.parse_script(script);
    engine.load_script(commands);

    // 8. Game Loop
    println!("[+] Bắt đầu Game Loop (Bot Active)...");
    loop {
        thread::sleep(Duration::from_millis(100)); // Sleep để chuột không giật lag
    }
}
