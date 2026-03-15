use std::error::Error;
use std::io::Write;
use std::process::{ChildStdin, Command, Stdio};

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
        let first_id = id_str
            .lines()
            .next()
            .ok_or("Could not find window ID")?
            .to_string();

        // 2. Khởi chạy xdotool ở chế độ listener (tham số "-")
        let mut child = Command::new("xdotool")
            .arg("-")
            .stdin(Stdio::piped())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or("Failed to open stdin for xdotool")?;

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

    pub fn click_at(&mut self, x: i32, y: i32, button: MouseButton) -> Result<(), Box<dyn Error>> {
        let btn = button as u8;

        // --- BÙ TRỪ WINDOW DECORATIONS CỦA LINUX ---
        // x, y được truyền vào là tọa độ tinh khiết của Game (1280x720)
        // Lệnh xdotool --window sẽ lấy mốc (0,0) bao gồm cả khung viền cửa sổ của OS.
        let border_left = 6;
        let title_bar_top = 32;

        let real_x = x + border_left;
        let real_y = y + title_bar_top;

        let commands = format!(
            "mousemove --window {} {} {}\nsleep 0.03\nclick --window {} {}\n",
            self.window_id, real_x, real_y, self.window_id, btn
        );

        self.xdotool_stdin.write_all(commands.as_bytes())?;
        self.xdotool_stdin.flush()?;
        Ok(())
    }
}
