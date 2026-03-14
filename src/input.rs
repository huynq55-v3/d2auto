use std::io::Write;
use std::process::{Command, Stdio, ChildStdin};
use std::error::Error;

pub enum MouseButton {
    Left = 1,
    Right = 3,
}

pub struct InputController {
    window_id: String,
    xdotool_stdin: ChildStdin,
    pub window_width: i32,
    pub window_height: i32,
}

impl InputController {
    pub fn new(window_name: &str) -> Result<Self, Box<dyn Error>> {
        // 1. Tìm Window ID bằng xdotool
        let output = Command::new("xdotool")
            .args(&["search", "--name", window_name])
            .output()?;
        
        let id_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let first_id = id_str.lines().next().ok_or("Could not find window ID")?.to_string();

        // 2. Khởi chạy xdotool ở chế độ listener (tham số "-")
        let mut child = Command::new("xdotool")
            .arg("-")
            .stdin(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().ok_or("Failed to open stdin for xdotool")?;

        println!("[DEBUG] Target Window ID (xdotool): {}", first_id);

        // Mặc định cho D2R (Bạn có thể viết hàm lấy size thật sau)
        let window_width = 1280; 
        let window_height = 720;

        Ok(Self { 
            window_id: first_id,
            xdotool_stdin: stdin,
            window_width,
            window_height,
        })
    }

    /// Tuyệt kỹ Toán học: Game Tile -> Screen Pixel
    pub fn click_to_move(
        &mut self,
        player_x: i32,
        player_y: i32,
        target_x: i32,
        target_y: i32,
    ) -> Result<(), Box<dyn Error>> {
        let diff_x = target_x - player_x;
        let diff_y = target_y - player_y;

        // Công thức Isometric chuẩn của Diablo
        // Tỷ lệ scale: Có thể cần chỉnh sửa (1.0, 1.5, 2.0) tùy vào độ phân giải cửa sổ
        let scale = 1.0; 
        
        // 1 Tile X = Di chuyển xuống góc dưới bên phải
        // 1 Tile Y = Di chuyển xuống góc dưới bên trái
        let offset_x = ((diff_x as f32 - diff_y as f32) * 16.0 * scale) as i32;
        let offset_y = ((diff_x as f32 + diff_y as f32) * 8.0 * scale) as i32;

        let center_x = self.window_width / 2;
        let center_y = self.window_height / 2;

        // Nhân vật D2R thường bị lệch lên trên một chút so với tâm màn hình
        let final_x = center_x + offset_x;
        let final_y = center_y + offset_y - 40; 

        // Giới hạn không click văng ra ngoài cửa sổ
        let safe_x = final_x.clamp(10, self.window_width - 10);
        let safe_y = final_y.clamp(10, self.window_height - 10);

        self.click_at(safe_x, safe_y, MouseButton::Left)
    }

    /// Di chuyển chuột tới tọa độ (x, y) trong cửa sổ game
    pub fn warp_to(&mut self, x: i32, y: i32) -> Result<(), Box<dyn Error>> {
        let command = format!("mousemove --window {} {} {}\n", self.window_id, x, y);
        self.xdotool_stdin.write_all(command.as_bytes())?;
        self.xdotool_stdin.flush()?;
        Ok(())
    }

    /// Click tại tọa độ (x, y) với nút tương ứng
    pub fn click_at(&mut self, x: i32, y: i32, button: MouseButton) -> Result<(), Box<dyn Error>> {
        let btn = button as u8;
        let commands = format!(
            "mousemove --window {} {} {}\nsleep 0.03\nclick --window {} {}\n",
            self.window_id, x, y, self.window_id, btn
        );

        self.xdotool_stdin.write_all(commands.as_bytes())?;
        self.xdotool_stdin.flush()?;
        Ok(())
    }

    /// Kiểm tra xem cửa sổ game có đang được focus hay không
    pub fn is_window_focused(&self) -> bool {
        let output = Command::new("xdotool")
            .arg("getactivewindow")
            .output();

        match output {
            Ok(out) => {
                let active_id = String::from_utf8_lossy(&out.stdout).trim().to_string();
                active_id == self.window_id
            }
            Err(_) => false,
        }
    }

    /// Lấy kích thước cửa sổ game (width, height)
    pub fn get_window_size(&self) -> Result<(i32, i32), Box<dyn Error>> {
        let output = Command::new("xdotool")
            .args(&["getwindowgeometry", "--shell", &self.window_id])
            .output()?;

        let out_str = String::from_utf8_lossy(&output.stdout);
        let mut width = 0;
        let mut height = 0;

        for line in out_str.lines() {
            if line.starts_with("WIDTH=") {
                width = line.split('=').nth(1).unwrap_or("0").parse()?;
            } else if line.starts_with("HEIGHT=") {
                height = line.split('=').nth(1).unwrap_or("0").parse()?;
            }
        }

        if width == 0 || height == 0 {
            return Err("Could not determine window size".into());
        }

        Ok((width, height))
    }
}
