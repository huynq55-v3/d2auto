#[derive(Debug, Clone)]
pub struct MonsterData {
    pub unit_id: u32,
    pub class_id: u32, // ID của loại quái (TxtFileNo) - Dùng để xem là Fallen, Zombie hay Boss
    pub mode: u32,     // Trạng thái (0: Đang chết, 12: Xác chết, 1-11: Đang sống/chiến đấu)
    pub x: u16,
    pub y: u16,
    pub ptr: u64, // Lưu pointer gốc để sau này đọc máu (HP) và thuộc tính (Stats)
}
