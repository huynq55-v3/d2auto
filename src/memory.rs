use std::convert::TryInto;
use std::fs::File;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::os::unix::fs::FileExt;

use crate::map_seed::get_map_seed_from_hash;
use crate::models::MonsterData; // Cực kỳ quan trọng để đọc file tối ưu trên Linux

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

pub fn get_wine_base_address(pid: i32) -> Option<(u64, usize)> {
    let maps_path = format!("/proc/{}/maps", pid);
    let maps_file = File::open(maps_path).ok()?;
    let reader = std::io::BufReader::new(maps_file);

    let mut base_start: Option<u64> = None;
    let mut current_end: u64 = 0;

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

        let mut addr_parts = addr_range.split('-');
        let start_addr_str = match addr_parts.next() {
            Some(s) => s,
            None => continue,
        };
        let end_addr_str = match addr_parts.next() {
            Some(e) => e,
            None => continue,
        };

        if let (Ok(start), Ok(end)) = (
            u64::from_str_radix(start_addr_str, 16),
            u64::from_str_radix(end_addr_str, 16),
        ) {
            if let Some(_s_found) = base_start {
                // Nếu vùng nhớ này liên kết trực tiếp với vùng trước đó, cộng dồn size
                if start == current_end {
                    current_end = end;
                } else {
                    // Hết các vùng nhớ liên tiếp của module
                    break;
                }
            } else {
                // Tìm Base Address bằng chữ ký MZ
                if (start == 0x140000000 || start == 0x400000) && perms.starts_with('r') {
                    let mem_path = format!("/proc/{}/mem", pid);
                    if let Ok(mut mem_file) = File::open(&mem_path) {
                        if mem_file.seek(SeekFrom::Start(start)).is_ok() {
                            let mut magic = [0u8; 2];
                            if mem_file.read_exact(&mut magic).is_ok() && &magic == b"MZ" {
                                base_start = Some(start);
                                current_end = end;
                            }
                        }
                    }
                }
            }
        }
    }

    base_start.map(|start| (start, (current_end - start) as usize))
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
            unsafe { Some(std::ptr::read_unaligned(buf.as_ptr() as *const T)) }
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

    pub fn read_current_area_id(&self, player_unit_ptr: u64) -> u32 {
        let path_ptr = self.read_u64(player_unit_ptr + 0x38).unwrap_or(0);
        if path_ptr == 0 {
            return 0;
        }

        let room1_ptr = self.read_u64(path_ptr + 0x20).unwrap_or(0);
        if room1_ptr == 0 || room1_ptr == 0x100000000 {
            return 0;
        }

        let room2_ptr = self.read_u64(room1_ptr + 0x18).unwrap_or(0);
        if room2_ptr == 0 {
            return 0;
        }

        let level_ptr = self.read_u64(room2_ptr + 0x90).unwrap_or(0);
        if level_ptr == 0 {
            return 0;
        }

        // Area ID (Level No) nằm ở offset 0x1F8 của pLevel
        self.read_u32(level_ptr + 0x1F8).unwrap_or(0)
    }

    /// Quét bảng UnitTable để lấy toàn bộ danh sách Quái vật/NPC đang tồn tại trong RAM
    pub fn get_all_monsters(&self, base_address: u64, unit_table_offset: u64) -> Vec<MonsterData> {
        let mut monsters = Vec::new();

        if unit_table_offset == 0 {
            return monsters;
        }

        // Type 1 (Monster) bắt đầu sau 1024 bytes (1 * 128 * 8) từ gốc UnitTable
        let monster_table_start = base_address + unit_table_offset + 1024;

        for i in 0..128 {
            let mut unit_ptr = self.read_u64(monster_table_start + (i * 8)).unwrap_or(0);

            while unit_ptr > 0 {
                // 1. Kiểm tra cờ Xác chết (Corpse) ở offset 0x1AE
                let is_corpse = self.read::<u8>(unit_ptr + 0x1AE).unwrap_or(0);

                // Nếu là xác chết -> Bỏ qua, chỉ đọc con trỏ Next (0x158) để đi tiếp
                if is_corpse != 0 {
                    unit_ptr = self.read_u64(unit_ptr + 0x158).unwrap_or(0);
                    continue;
                }

                let txt_file_no = self.read_u32(unit_ptr + 0x04).unwrap_or(0);
                let unit_id = self.read_u32(unit_ptr + 0x08).unwrap_or(0);
                let mode = self.read_u32(unit_ptr + 0x0C).unwrap_or(0);

                let path_ptr = self.read_u64(unit_ptr + 0x38).unwrap_or(0);
                let mut x = 0;
                let mut y = 0;

                if path_ptr > 0 {
                    // D2go chỉ đọc tọa độ tĩnh ở 0x02 và 0x06
                    x = self.read_u16(path_ptr + 0x02).unwrap_or(0);
                    y = self.read_u16(path_ptr + 0x06).unwrap_or(0);
                }

                // (Tùy chọn) Lọc bớt các NPC vô hại như Gà, Chuột, Lửa, Trap...
                // Ở đây ta cứ lấy hết, lát nữa lúc code AI ta sẽ lọc bằng txt_file_no sau

                monsters.push(MonsterData {
                    unit_id,
                    class_id: txt_file_no,
                    mode,
                    x,
                    y,
                    ptr: unit_ptr,
                });

                // VÁ LỖI CHÍ MẠNG: Con trỏ tới Unit tiếp theo nằm ở 0x158
                unit_ptr = self.read_u64(unit_ptr + 0x158).unwrap_or(0);
            }
        }

        monsters
    }

    /// Trích xuất Map Seed từ RAM thông qua việc giải mã Hash (Hoạt động cho cả Online và Offline)
    pub fn read_map_seed(&self, player_unit_ptr: u64) -> Option<u32> {
        // 1. Từ Player Unit (0x20) trỏ tới cấu trúc Act
        let p_act = self.read_u64(player_unit_ptr + 0x20)?;

        // 2. Từ Act (0x78) trỏ tới cấu trúc ActMisc
        let p_act_misc = self.read_u64(p_act + 0x70)?;

        // 3. Đọc 2 giá trị Hash đã bị mã hóa
        let init_seed_hash = self.read_u32(p_act_misc + 0x838)?;
        let end_seed_hash = self.read_u32(p_act_misc + 0x860)?;

        println!("player_unit_ptr: {:x}", player_unit_ptr);
        println!("p_act: {:x}", p_act);
        println!("p_act_misc: {:x}", p_act_misc);
        println!("init: {}", init_seed_hash);
        println!("end: {}", end_seed_hash);

        // 4. Giải mã Hash để lấy Map Seed 32-bit gốc
        get_map_seed_from_hash(init_seed_hash, end_seed_hash)
    }

    pub fn find_correct_act_misc_offset(&self, player_unit_ptr: u64) {
        println!("\x1b[33m--- BẮT ĐẦU QUÉT OFFSET ACT_MISC ---\x1b[0m");

        // 1. Lấy p_act (Thường ổn định ở 0x20)
        let p_act = match self.read_u64(player_unit_ptr + 0x20) {
            Some(ptr) if ptr > 0 => ptr,
            _ => {
                println!("[!] Không tìm thấy p_act tại offset 0x20");
                return;
            }
        };

        println!("[DEBUG] p_act found at: {:x}", p_act);

        // 2. Quét dải offset từ 0x00 đến 0x300 (mỗi bước 8 bytes cho con trỏ 64-bit)
        for offset in (0..0x300).step_by(8) {
            if let Some(potential_p_act_misc) = self.read_u64(p_act + offset) {
                // Loại bỏ các địa chỉ tĩnh hoặc rác (như 0x1400... mà bạn gặp)
                // ActMisc thật phải nằm trong vùng Heap động (thường là địa chỉ lớn hoặc nhỏ hơn hẳn vùng module base)
                if potential_p_act_misc == 0
                    || (potential_p_act_misc >= 0x140000000 && potential_p_act_misc <= 0x150000000)
                {
                    continue;
                }

                // Thử đọc Hash1 và Hash2 dựa trên cấu trúc d2go/koolo
                // Offset Hash1: 0x840, Hash2: 0x868
                if let (Some(init_hash), Some(end_hash)) = (
                    self.read_u32(potential_p_act_misc + 0x840),
                    self.read_u32(potential_p_act_misc + 0x868),
                ) {
                    // Nếu đọc ra 0 cả hai thì khả năng cao là sai chỗ
                    if init_hash == 0 && end_hash == 0 {
                        continue;
                    }

                    // Thử bẻ khóa thử xem có ra Seed không
                    if let Some(seed) = get_map_seed_from_hash(init_hash, end_hash) {
                        println!(
                            "\x1b[32m[FOUND!] Tìm thấy Offset khả thi: 0x{:x}\x1b[0m",
                            offset
                        );
                        println!("   -> p_act_misc: {:x}", potential_p_act_misc);
                        println!("   -> Map Seed: {}", seed);
                        println!("   -> InitHash: {}, EndHash: {}", init_hash, end_hash);
                        println!("-------------------------------------------");
                    }
                }
            }
        }
        println!("\x1b[33m--- KẾT THÚC QUÉT ---\x1b[0m");
    }

    pub fn deep_scan_act_misc(&self, player_unit_ptr: u64, target_seed: u32) {
        // 1. Tính toán Hash mà chúng ta đang đi tìm
        let target_end_hash = target_seed.wrapping_mul(0x6AC690C5).wrapping_add(666);
        println!("\x1b[33m--- DEEP SCAN ACT_MISC ---\x1b[0m");
        println!(
            "[INFO] Đang tìm EndHash: {} (tương ứng Seed {})",
            target_end_hash, target_seed
        );

        let p_act = self.read_u64(player_unit_ptr + 0x20).unwrap_or(0);
        if p_act == 0 {
            return;
        }

        // Quét qua các pointer khả nghi trong Act (0x70, 0x78, 0x80...)
        for act_offset in [0x70, 0x78, 0x80] {
            if let Some(p_act_misc) = self.read_u64(p_act + act_offset) {
                if p_act_misc == 0 || p_act_misc > 0x00007FFFFFFFFFFF {
                    continue;
                }

                // Quét sâu vào bên trong ActMisc (tầm 2000 bytes) để tìm target_end_hash
                for internal_offset in (0..0xA00).step_by(4) {
                    if let Some(val) = self.read_u32(p_act_misc + internal_offset) {
                        if val == target_end_hash {
                            println!("\x1b[32m[BINGO!] Đã tìm thấy vị trí chuẩn:\x1b[0m");
                            println!("   -> act_ptr + 0x{:x} (p_act_misc)", act_offset);
                            println!("   -> act_misc + 0x{:x} (EndHashOffset)", internal_offset);

                            // Thông thường InitHash nằm cách EndHash -0x28 (40 bytes)
                            let potential_init_offset = internal_offset - 0x28;
                            if let Some(init_val) =
                                self.read_u32(p_act_misc + potential_init_offset as u64)
                            {
                                println!(
                                    "   -> act_misc + 0x{:x} (InitHashOffset): val={}",
                                    potential_init_offset, init_val
                                );
                            }
                            println!("-------------------------------------------");
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExtractMode {
    Raw,                // Không tính toán, offset = giá trị đọc được
    RipRelative(isize), // RIP-Relative = pattern_index + instruction_length + displacement
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
    pub unit_table: u64,
    pub player_unit_ptr: u64,
    pub game_data: u64,
}

impl GameOffsets {
    pub fn load_from_memory(module_memory: &[u8]) -> Self {
        let mut offsets = Self::default();
        for sig in SIGNATURES {
            if let Some(val) = extract_offset(module_memory, sig) {
                match sig.name {
                    "UnitTable" => offsets.unit_table = val,
                    "GameData" => offsets.game_data = val,
                    _ => {}
                }
            }
        }
        offsets
    }

    /// Hàm mới: Dùng UnitTable để tìm địa chỉ của nhân vật chính (Local Player)
    pub fn find_player_unit(&mut self, reader: &MemoryReader, base_address: u64) -> bool {
        if self.unit_table == 0 {
            return false;
        }

        let unit_table_base = base_address + self.unit_table;

        for i in 0..128 {
            let mut unit_ptr = reader.read_u64(unit_table_base + (i * 8)).unwrap_or(0);

            while unit_ptr > 0 {
                let unit_type = reader.read_u32(unit_ptr).unwrap_or(99);

                if unit_type == 0 {
                    // --- ĐÃ THÊM LOGIC KIỂM TRA CHÉO (VALIDATION) ---

                    // 1. Kiểm tra Class ID (Nằm ở offset 0x04 của Unit)
                    let txt_file_no = reader.read_u32(unit_ptr + 0x04).unwrap_or(99);

                    // 2. Kiểm tra Path Pointer (Nằm ở offset 0x38)
                    let path_ptr = reader.read_u64(unit_ptr + 0x38).unwrap_or(0);

                    // 3. Kiểm tra Inventory Pointer (Nằm ở offset 0x90)
                    let inv_ptr = reader.read_u64(unit_ptr + 0x90).unwrap_or(0);

                    // Điều kiện kiện kiên quyết để là 1 Player hợp lệ:
                    // Class từ 0-6 VÀ phải có Path VÀ phải có Inventory
                    if txt_file_no <= 6 && path_ptr > 0 && inv_ptr > 0 {
                        self.player_unit_ptr = unit_ptr;
                        return true;
                    }
                }

                // Nhảy tới Unit tiếp theo trong Linked List
                unit_ptr = reader.read_u64(unit_ptr + 0x158).unwrap_or(0);
            }
        }
        false
    }
}

// Khai báo chính xác 100% logic từ d2go/pkg/memory/offset.go
pub const SIGNATURES: &[Signature] = &[
    Signature {
        name: "GameData",
        pattern: b"\x44\x88\x25\x00\x00\x00\x00\x66\x44\x89\x25\x00\x00\x00\x00",
        mask: "xxx????xxxx????",
        // D2go: (pattern - Base) - 0x121 + offsetInt
        rule: ExtractionRule {
            add_offset: 3,
            read_size: 4,
            mode: ExtractMode::RipRelative(-0x121),
        },
    },
    Signature {
        name: "UnitTable",
        pattern: b"\x48\x03\xC7\x49\x8B\x8C\xC6",
        mask: "xxxxxxx",
        // D2go: unitTableOffset := offsetInt (Hoàn toàn thô)
        rule: ExtractionRule {
            add_offset: 7,
            read_size: 4,
            mode: ExtractMode::Raw,
        },
    },
    Signature {
        name: "UI",
        pattern: b"\x40\x84\xed\x0f\x94\x05",
        mask: "xxxxxx",
        // D2go: pattern + 10 + offsetInt
        rule: ExtractionRule {
            add_offset: 6,
            read_size: 4,
            mode: ExtractMode::RipRelative(10),
        },
    },
    Signature {
        name: "Hover",
        pattern: b"\xc6\x84\xc2\x00\x00\x00\x00\x00\x48\x8b\x74",
        mask: "xxx?????xxx",
        rule: ExtractionRule {
            add_offset: 3,
            read_size: 4,
            mode: ExtractMode::Raw,
        },
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
