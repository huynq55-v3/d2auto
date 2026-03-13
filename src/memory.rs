use std::convert::TryInto;
use std::fs::{self, File};
use std::io::{self, BufRead, Read, Seek, SeekFrom};
use std::os::unix::fs::FileExt; // Cực kỳ quan trọng để đọc file tối ưu trên Linux

/// Find PID by searching /proc/[pid]/comm and /proc/[pid]/cmdline
pub fn find_pid_by_name(target_name: &str) -> Option<i32> {
    let lower_target = target_name.to_lowercase();

    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Lấy tên thư mục an toàn
            let file_name = match path.file_name() {
                Some(name) => name,
                None => continue,
            };

            let pid_str = match file_name.to_str() {
                Some(s) => s,
                None => continue,
            };

            // Ép kiểu thành i32. Nếu LỖI (không phải số) thì BỎ QUA và đi tiếp
            let pid: i32 = match pid_str.parse() {
                Ok(p) => p,
                Err(_) => continue, // <-- Thay dấu ? bằng lệnh continue
            };

            // 1. Kiểm tra comm (thường ngắn và không có tham số)
            if let Ok(comm) = std::fs::read_to_string(path.join("comm")) {
                if comm.trim().to_lowercase().contains(&lower_target) {
                    return Some(pid);
                }
            }

            // 2. Kiểm tra cmdline (Dùng cho Wine/D2R.exe)
            if let Ok(cmdline_bytes) = std::fs::read(path.join("cmdline")) {
                // Chuyển NULL thành khoảng trắng để tìm kiếm dễ dàng
                let cmdline_clean: String = cmdline_bytes
                    .iter()
                    .map(|&b| if b == 0 { ' ' } else { b as char })
                    .collect();

                if cmdline_clean.to_lowercase().contains(&lower_target) {
                    return Some(pid);
                }
            }
        }
    }
    None
}

pub fn get_wine_base_address(pid: i32) -> Option<u64> {
    let maps_path = format!("/proc/{}/maps", pid);
    let mem_path = format!("/proc/{}/mem", pid);

    let maps_file = File::open(maps_path).ok()?;
    let reader = std::io::BufReader::new(maps_file);

    // Mở trực tiếp RAM của tiến trình (Yêu cầu sudo)
    let mut mem_file = match File::open(&mem_path) {
        Ok(f) => f,
        Err(_) => {
            println!("[-] Lỗi mở file mem. Bạn đã chạy chương trình bằng 'sudo' chưa?");
            return None;
        }
    };

    for line in reader.lines().flatten() {
        let mut parts = line.split_whitespace();

        let addr_range = match parts.next() {
            Some(r) => r,
            None => continue,
        };

        let perms = match parts.next() {
            Some(p) => p,
            None => continue,
        };

        // CHỈ KIỂM TRA CHỮ 'r' Ở ĐẦU.
        // Chấp nhận cả private ('p') lẫn shared ('s')
        if !perms.starts_with('r') {
            continue;
        }

        if let Some(start_addr_str) = addr_range.split('-').next() {
            if let Ok(base_addr) = u64::from_str_radix(start_addr_str, 16) {
                // Nhảy đến địa chỉ bắt đầu của vùng nhớ này trong RAM
                if mem_file.seek(SeekFrom::Start(base_addr)).is_ok() {
                    let mut magic = [0u8; 2];

                    // Đọc 2 byte đầu tiên
                    if mem_file.read_exact(&mut magic).is_ok() {
                        // Kiểm tra chữ ký MZ
                        if &magic == b"MZ" {
                            // 0x140000000 là địa chỉ tĩnh kinh điển của D2R 64-bit
                            if base_addr == 0x140000000 || base_addr == 0x400000 {
                                return Some(base_addr);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

// ==========================================
// 1. MODULE ĐỌC BỘ NHỚ (MEMORY READER)
// ==========================================
pub struct MemoryReader {
    pub mem_file: File,
}

impl MemoryReader {
    pub fn new(pid: i32) -> Result<Self, std::io::Error> {
        let mem_file = File::open(format!("/proc/{}/mem", pid))?;
        Ok(Self { mem_file })
    }

    /// Hàm đọc tổng quát cho bất kỳ kiểu dữ liệu nào (Generic)
    /// Điều này đáp ứng nguyên tắc DRY
    pub fn read<T: Copy>(&self, address: u64) -> Option<T> {
        let size = std::mem::size_of::<T>();
        let mut buf = vec![0u8; size];

        if self.mem_file.read_exact_at(&mut buf, address).is_ok() {
            unsafe {
                Some(std::ptr::read_unaligned(buf.as_ptr() as *const T))
            }
        } else {
            None
        }
    }

    // Các hàm helper nhanh cho các kiểu dữ liệu phổ biến
    pub fn read_u64(&self, address: u64) -> Option<u64> {
        self.read::<u64>(address)
    }

    pub fn read_u32(&self, address: u64) -> Option<u32> {
        self.read::<u32>(address)
    }

    pub fn read_u16(&self, address: u64) -> Option<u16> {
        self.read::<u16>(address)
    }
}

#[derive(Debug, Clone)]
pub enum ExtractMode {
    Raw,                 // Không tính toán, offset = giá trị đọc được
    RipRelative(isize),  // RIP-Relative = pattern_index + instruction_length + displacement
}

#[derive(Debug, Clone)]
pub struct ExtractionRule {
    pub add_offset: usize, // Vị trí đọc 4 bytes (tính từ đầu Pattern)
    pub read_size: usize,
    pub mode: ExtractMode, // Chọn thuật toán tương ứng
}

pub struct Signature<'a> {
    pub name: &'a str,
    pub pattern: &'a [u8],
    pub mask: &'a str,
    pub rule: ExtractionRule,
}

impl<'a> Signature<'a> {
    /// Compiles pattern and mask into a vector of Option<u8> for easier matching
    pub fn compile(&self) -> Vec<Option<u8>> {
        self.pattern
            .iter()
            .zip(self.mask.chars())
            .map(
                |(&byte, mask_char)| {
                    if mask_char == 'x' { Some(byte) } else { None }
                },
            )
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct GameOffsets {
    pub game_data: u64,
    pub unit_table: u64,
    pub ui: u64,
}

impl GameOffsets {
    pub fn load_from_memory(module_memory: &[u8]) -> Self {
        let mut offsets = Self::default();
        for sig in SIGNATURES {
            if let Some(val) = extract_offset(module_memory, sig) {
                match sig.name {
                    "GameData" => offsets.game_data = val,
                    "UnitTable" => offsets.unit_table = val,
                    "UI" => offsets.ui = val,
                    _ => {}
                }
            }
        }
        offsets
    }
}

// Khai báo chính xác 100% logic từ d2go/pkg/memory/offset.go
pub const SIGNATURES: &[Signature] = &[
    Signature {
        name: "GameData",
        pattern: b"\x44\x88\x25\x00\x00\x00\x00\x66\x44\x89\x25\x00\x00\x00\x00",
        mask: "xxx????xxxx????",
        // D2go: (pattern - Base) - 0x121 + offsetInt
        rule: ExtractionRule { add_offset: 3, read_size: 4, mode: ExtractMode::RipRelative(-0x121) },
    },
    Signature {
        name: "UnitTable",
        pattern: b"\x48\x03\xC7\x49\x8B\x8C\xC6",
        mask: "xxxxxxx",
        // D2go: unitTableOffset := offsetInt (Hoàn toàn thô)
        rule: ExtractionRule { add_offset: 7, read_size: 4, mode: ExtractMode::Raw },
    },
    Signature {
        name: "UI",
        pattern: b"\x40\x84\xed\x0f\x94\x05",
        mask: "xxxxxx",
        // D2go: pattern + 10 + offsetInt
        rule: ExtractionRule { add_offset: 6, read_size: 4, mode: ExtractMode::RipRelative(10) },
    },
];

/// Basic pattern scanning function
pub fn find_pattern(memory_buffer: &[u8], pattern: &[Option<u8>]) -> Option<usize> {
    if pattern.is_empty() || memory_buffer.len() < pattern.len() {
        return None;
    }

    for i in 0..=(memory_buffer.len() - pattern.len()) {
        let mut match_found = true;
        for (j, &pat_byte) in pattern.iter().enumerate() {
            if let Some(b) = pat_byte {
                if memory_buffer[i + j] != b {
                    match_found = false;
                    break;
                }
            }
        }
        if match_found {
            return Some(i);
        }
    }
    None
}

pub fn extract_offset(module_memory: &[u8], sig: &Signature) -> Option<u64> {
    let compiled_pattern = sig.compile();

    if let Some(pattern_index) = find_pattern(module_memory, &compiled_pattern) {
        let start_idx = pattern_index + sig.rule.add_offset;
        let end_idx = start_idx + sig.rule.read_size;

        if end_idx <= module_memory.len() {
            let bytes_slice = &module_memory[start_idx..end_idx];

            let parsed_value: i64 = match sig.rule.read_size {
                4 => i32::from_le_bytes(bytes_slice.try_into().unwrap()) as i64,
                8 => i64::from_le_bytes(bytes_slice.try_into().unwrap()),
                _ => return None,
            };

            // Ép kiểu dựa trên ExtractMode
            match sig.rule.mode {
                ExtractMode::Raw => {
                    return Some(parsed_value as u64); // Lấy raw y xì
                }
                ExtractMode::RipRelative(instruction_len) => {
                    // Địa chỉ = Vị trí tìm thấy + Chiều dài lệnh + Độ lệch
                    let final_offset = pattern_index as i64 + instruction_len as i64 + parsed_value;
                    return Some(final_offset as u64);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_table_extraction() {
        let mut fake_ram: Vec<u8> = vec![0; 100];
        let pattern = b"\x48\x03\xC7\x49\x8B\x8C\xC6";
        let offset: u32 = 0xDDCCBBAA;
        let offset_bytes = offset.to_le_bytes();

        // Place pattern at index 10
        for (i, &b) in pattern.iter().enumerate() {
            fake_ram[10 + i] = b;
        }
        // Place offset at index 10 + 7
        for (i, &b) in offset_bytes.iter().enumerate() {
            fake_ram[17 + i] = b;
        }

        let sig = SIGNATURES.iter().find(|s| s.name == "UnitTable").unwrap();
        let result = extract_offset(&fake_ram, sig);
        assert_eq!(result, Some(0xDDCCBBAA as u64));
    }
}
