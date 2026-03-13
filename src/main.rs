mod memory;

use memory::{find_pid_by_name, get_wine_base_address};

fn main() {
    println!("--- D2R Linux Process Scanner ---");

    // 1. Tìm PID của game (không phân biệt hoa thường)
    let process_target = "d2r";
    println!("Đang tìm tiến trình có tên: {}...", process_target);

    match find_pid_by_name(process_target) {
        Some(pid) => {
            println!("[+] Tìm thấy PID: {}", pid);

            // 2. Tìm Base Address của module chính theo chữ ký 'MZ'
            println!("Đang quét Base Address theo chữ ký 'MZ'...");

            match get_wine_base_address(pid) {
                Some(base_addr) => {
                    println!("[SUCCESS] Tìm thấy Base Address: 0x{:X}", base_addr);
                    println!(
                        "Bạn đã sẵn sàng để thực hiện các bước tiếp theo (Đọc RAM, Quét Pattern)!"
                    );
                }
                None => {
                    println!(
                        "[-] Không tìm thấy Base Address. Bạn đã chạy chương trình bằng 'sudo' chưa? Game đã thực sự chạy chưa?"
                    );
                }
            }
        }
        None => {
            println!(
                "[-] Không tìm thấy tiến trình nào chứa từ khóa: '{}'",
                process_target
            );
            println!("Gợi ý: Hãy mở game lên trước khi chạy công cụ này.");
        }
    }
}
