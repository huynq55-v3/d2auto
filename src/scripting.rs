use crate::astar;
use crate::input::InputController;
use crate::map::{AreaManager, GameTopology};
use crate::memory::MemoryReader;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    GoToArea(u32),
}

pub struct ScriptParser {
    area_dict: HashMap<String, u32>,
}

impl ScriptParser {
    pub fn new() -> Self {
        let mut dict = HashMap::new();
        dict.insert("rogue encampment".to_string(), 1);
        dict.insert("blood moor".to_string(), 2);
        dict.insert("cold plains".to_string(), 3);
        dict.insert("den of evil".to_string(), 8);

        Self { area_dict: dict }
    }

    pub fn parse_script(&self, script_text: &str) -> Vec<Command> {
        let mut commands = Vec::new();

        for line in script_text.lines() {
            let clean_line = line.trim().to_lowercase();
            if clean_line.is_empty() || clean_line.starts_with("//") {
                continue;
            }

            if clean_line.starts_with("go to area ") {
                let id_str = clean_line.replace("go to area ", "");
                if let Ok(id) = id_str.parse::<u32>() {
                    commands.push(Command::GoToArea(id));
                }
            } else if clean_line.starts_with("go to ") {
                let map_name = clean_line.replace("go to ", "");
                if let Some(&id) = self.area_dict.get(&map_name) {
                    commands.push(Command::GoToArea(id));
                } else {
                    println!("[LỖI SCRIPT] Không tìm thấy tên map: {}", map_name);
                }
            } else {
                println!("[LỖI SCRIPT] Cú pháp không hợp lệ: {}", line);
            }
        }

        commands
    }
}

#[derive(Debug, PartialEq)]
pub enum BotState {
    Idle,
    MovingToArea(u32),
}

pub struct BotEngine {
    pub command_queue: VecDeque<Command>,
    pub state: BotState,
    pub macro_route: Vec<u32>,
}

impl BotEngine {
    pub fn new() -> Self {
        Self {
            command_queue: VecDeque::new(),
            state: BotState::Idle,
            macro_route: Vec::new(),
        }
    }

    pub fn load_script(&mut self, commands: Vec<Command>) {
        self.command_queue = VecDeque::from(commands);
        println!(
            "[ENGINE] Đã nạp {} lệnh vào hàng đợi.",
            self.command_queue.len()
        );
    }

    pub fn tick(
        &mut self,
        reader: &MemoryReader,
        input: &mut InputController,
        area_manager: &mut AreaManager,
        topology: &GameTopology,
        current_area_id: u32,
        player_x: i32,
        player_y: i32,
    ) {
        if self.state == BotState::Idle {
            if let Some(next_cmd) = self.command_queue.pop_front() {
                match next_cmd {
                    Command::GoToArea(target_id) => {
                        println!("[SM] Nhận lệnh: Đi đến Area {}", target_id);
                        if let Some(route) = topology.get_macro_route(current_area_id, target_id) {
                            self.macro_route = route;
                            self.state = BotState::MovingToArea(target_id);
                            println!("[SM] Lộ trình vĩ mô: {:?}", self.macro_route);
                        } else {
                            println!("[SM] Lỗi: Không có đường đi đến Area {}", target_id);
                        }
                    }
                }
            }
            return;
        }

        if let BotState::MovingToArea(target_area) = self.state {
            if current_area_id == target_area {
                println!("[SM] HOÀN THÀNH: Đã đến đích Area {}", target_area);
                self.state = BotState::Idle;
                self.macro_route.clear();
                return;
            }

            let current_area = match area_manager.world_map.areas.get(&current_area_id) {
                Some(a) => a,
                None => return,
            };

            // IN DEBUG ĐỂ KIỂM TRA LƯỚI VA CHẠM
            if current_area.grid.is_empty() {
                println!(
                    "[SM-CẢNH BÁO] Grid của Area {} đang trống! Bạn đã gọi area_manager.update() chưa?",
                    current_area_id
                );
                return;
            }

            // LỖI 1 ĐƯỢC SỬA Ở ĐÂY: Lấy trạm dừng chân tiếp theo
            // VD: Route là [1, 2, 8]. Đang ở 1 -> Next là 2.
            let next_area = self
                .macro_route
                .iter()
                .skip_while(|&&a| a != current_area_id)
                .nth(1) // Lấy phần tử ngay sau current_area_id
                .cloned()
                .unwrap_or(target_area); // Nếu không tìm thấy thì fallback về target cuối

            // Tìm cửa đi sang NEXT_AREA (ví dụ Area 2), KHÔNG phải target_area (Area 8)
            let target_door_pos = self.find_door_to_area(reader, next_area);

            let target_tile = if let Some(door_pos) = target_door_pos {
                println!(
                    "[SM] Radar phát hiện lối sang Area {} tại {:?}",
                    next_area, door_pos
                );
                door_pos
            } else {
                if let Some(frontier) = current_area.find_closest_frontier(player_x, player_y) {
                    println!("[SM] Đang chạy ra mép sương mù tại {:?}", frontier);
                    frontier
                } else {
                    println!(
                        "[SM] Kẹt! Đã quét {} ô map nhưng không thấy cửa sang Area {}.",
                        current_area.grid.len(),
                        next_area
                    );
                    self.state = BotState::Idle;
                    return;
                }
            };

            // Tìm đường A*
            if let Some(path) =
                astar::find_path(&current_area.grid, (player_x, player_y), target_tile)
            {
                if path.len() > 1 {
                    let step_index = usize::min(path.len() - 1, 3);
                    let next_step = path[step_index];
                    input
                        .click_to_move(player_x, player_y, next_step.0, next_step.1)
                        .ok();
                }
            } else {
                println!(
                    "[SM] A* báo lỗi: Không có đường vật lý tới {:?}",
                    target_tile
                );
            }
        }
    }

    pub fn find_door_to_area(
        &self,
        _reader: &MemoryReader,
        _target_area_id: u32,
    ) -> Option<(i32, i32)> {
        // Placeholder for now
        None
    }
}
