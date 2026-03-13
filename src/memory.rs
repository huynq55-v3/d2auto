use std::convert::TryInto;
use std::fs::{self, File};
use std::io::{self, BufRead, Read, Seek, SeekFrom};

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

// 1. Struct for extraction rules
#[derive(Debug, Clone)]
pub struct ExtractionRule {
    pub add_offset: usize, // Offset from the start of the pattern match
    pub read_size: usize,  // Number of bytes to read (4 for u32, 8 for u64)
}

// 2. Struct for signatures
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

// 3. Struct for game offsets
#[derive(Debug, Default)]
pub struct GameOffsets {
    pub game_data: u64,
    pub unit_table: u64,
    pub ui: u64,
    pub hover: u64,
    pub expansion: u64,
    pub roster_offset: u64,
    pub panel_manager_container_offset: u64,
    pub widget_states_offset: u64,
    pub waypoints_offset: u64,
    pub fps: u64,
    pub key_bindings_offset: u64,
    pub key_bindings_skills_offset: u64,
    pub tz: u64, // Terror Zones
    pub quests: u64,
    pub ping: u64,
    pub legacy_graphics: u64,
}

// 4. SIGNATURES constant
pub const SIGNATURES: &[Signature] = &[
    // GameData
    Signature {
        name: "GameData",
        pattern: b"\x44\x88\x25\x00\x00\x00\x00\x66\x44\x89\x25\x00\x00\x00\x00",
        mask: "xxx????xxxx????",
        rule: ExtractionRule { add_offset: 3, read_size: 4 },
    },
    // UnitTable
    Signature {
        name: "UnitTable",
        pattern: b"\x48\x03\xC7\x49\x8B\x8C\xC6",
        mask: "xxxxxxx",
        rule: ExtractionRule { add_offset: 7, read_size: 4 },
    },
    // UI
    Signature {
        name: "UI",
        pattern: b"\x40\x84\xed\x0f\x94\x05",
        mask: "xxxxxx",
        rule: ExtractionRule { add_offset: 6, read_size: 4 },
    },
    // Hover
    Signature {
        name: "Hover",
        pattern: b"\xc6\x84\xc2\x00\x00\x00\x00\x00\x48\x8b\x74",
        mask: "xxx?????xxx",
        rule: ExtractionRule { add_offset: 3, read_size: 4 },
    },
    // Expansion
    Signature {
        name: "Expansion",
        pattern: b"\x48\x8B\x05\x00\x00\x00\x00\x48\x8B\xD9\xF3\x0F\x10\x50\x00",
        mask: "xxx????xxxxxxx?",
        rule: ExtractionRule { add_offset: 3, read_size: 4 },
    },
    // RosterOffset
    Signature {
        name: "RosterOffset",
        pattern: b"\x02\x45\x33\xD2\x4D\x8B",
        mask: "xxxxxx",
        rule: ExtractionRule { add_offset: 2, read_size: 4 }, // Placeholder rule, check d2go
    },
    // PanelManagerContainer
    Signature {
        name: "PanelManagerContainer",
        pattern: b"\x48\x89\x05\x00\x00\x00\x00\x48\x85\xDB\x74\x1E",
        mask: "xxx????xxxxx",
        rule: ExtractionRule { add_offset: 3, read_size: 4 },
    },
    // WidgetStates
    Signature {
        name: "WidgetStates",
        pattern: b"\x48\x8B\x0D\x00\x00\x00\x00\x4C\x8D\x44\x24\x00\x48\x03\xC2",
        mask: "xxx????xxxx?xxx",
        rule: ExtractionRule { add_offset: 3, read_size: 4 },
    },
    // Waypoints
    Signature {
        name: "Waypoints",
        pattern: b"\x48\x89\x05\x00\x00\x00\x00\x0F\x11\x00",
        mask: "xxx????xxx",
        rule: ExtractionRule { add_offset: 3, read_size: 4 },
    },
    // FPS
    Signature {
        name: "FPS",
        pattern: b"\x8B\x1D\x00\x00\x00\x00\x48\x8D\x05\x00\x00\x00\x00\x48\x8D\x4C\x24\x40",
        mask: "xx????xxx????xxxxx",
        rule: ExtractionRule { add_offset: 2, read_size: 4 },
    },
    // KeyBindings
    Signature {
        name: "KeyBindings",
        pattern: b"\x48\x8D\x05\xAF\xEE",
        mask: "xxxxx",
        rule: ExtractionRule { add_offset: 3, read_size: 4 },
    },
    // KeyBindingsSkills
    Signature {
        name: "KeyBindingsSkills",
        pattern: b"\x0F\x10\x04\x24\x48\x6B\xC8\x1C\x48\x8D\x05",
        mask: "xxxxxxxxxxx",
        rule: ExtractionRule { add_offset: 11, read_size: 4 },
    },
    // TerrorZones
    Signature {
        name: "TerrorZones",
        pattern: b"\x48\x89\x05\xCC\xCC\xCC\xCC\x48\x8D\x05\xCC\xCC\xCC\xCC\x48\x89\x05\xCC\xCC\xCC\xCC\x48\x8D\x05\xCC\xCC\xCC\xCC\x48\x89\x15\xCC\xCC\xCC\xCC\x48\x89\x15",
        mask: "xxx????xxx????xxx????xxx????xxx????xxx",
        rule: ExtractionRule { add_offset: 3, read_size: 4 },
    },
    // Quests
    Signature {
        name: "Quests",
        pattern: b"\x42\xc6\x84\x28\x00\x00\x00\x00\x00\x49\xff\xc5\x49\x83\xfd\x29",
        mask: "xxxx?????xxxxxxx",
        rule: ExtractionRule { add_offset: 4, read_size: 4 },
    },
    // Ping
    Signature {
        name: "Ping",
        pattern: b"\x48\x8B\x0D\xCC\xCC\xCC\xCC\x49\x2B\xC7",
        mask: "xxx????xxx",
        rule: ExtractionRule { add_offset: 3, read_size: 4 },
    },
    // LegacyGraphics
    Signature {
        name: "LegacyGraphics",
        pattern: b"\x80\x3D\x00\x00\x00\x00\x00\x48\x8D\x54\x24\x30",
        mask: "xx?????xxxxx",
        rule: ExtractionRule { add_offset: 2, read_size: 4 },
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

/// Generic offset extraction function
pub fn extract_offset(module_memory: &[u8], sig: &Signature) -> Option<u64> {
    let compiled_pattern = sig.compile();

    if let Some(pattern_index) = find_pattern(module_memory, &compiled_pattern) {
        let start_idx = pattern_index + sig.rule.add_offset;
        let end_idx = start_idx + sig.rule.read_size;

        if end_idx <= module_memory.len() {
            let bytes_slice = &module_memory[start_idx..end_idx];

            match sig.rule.read_size {
                4 => {
                    let val_u32 = u32::from_le_bytes(bytes_slice.try_into().unwrap());
                    return Some(val_u32 as u64);
                }
                8 => {
                    let val_u64 = u64::from_le_bytes(bytes_slice.try_into().unwrap());
                    return Some(val_u64);
                }
                _ => {
                    return None;
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
