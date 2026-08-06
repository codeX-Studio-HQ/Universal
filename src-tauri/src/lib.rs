use tauri::Manager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};
use walkdir::WalkDir;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use rdev::{listen, Event, EventType, Key};
use chrono::prelude::*;

struct AppInfo {
    name: String,
    path: PathBuf,
    icon: String,
    exec: String,
}

struct AppState {
    app_index: Mutex<HashMap<String, AppInfo>>,
    file_index: Mutex<HashMap<String, PathBuf>>,
    recent_items: Mutex<Vec<String>>,
}

#[derive(serde::Serialize, Clone)]
struct SearchResult {
    name: String,
    desc: String,
    icon: String,
    result_type: String,
    path: String,
}

fn parse_desktop_file(path: &PathBuf) -> Option<AppInfo> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut name = String::new();
    let mut icon = String::new();
    let mut exec = String::new();

    for line in content.lines() {
        if line.starts_with("Name=") { name = line[5..].trim().to_string(); }
        else if line.starts_with("Icon=") { icon = line[5..].trim().to_string(); }
        else if line.starts_with("Exec=") { exec = line[5..].trim().to_string(); }
    }

    if name.is_empty() { return None; }

    exec = exec.replace("%u", "").replace("%U", "").replace("%f", "").replace("%F", "")
        .replace("%d", "").replace("%D", "").replace("%n", "").replace("%N", "")
        .replace("%i", "").replace("%c", "").replace("%k", "").replace("%v", "")
        .replace("%m", "").trim().to_string();

    if icon.is_empty() || icon == "unknown" { icon = name.to_lowercase(); }

    Some(AppInfo { name, path: path.clone(), icon, exec })
}

fn index_apps(apps: &mut HashMap<String, AppInfo>) {
    #[cfg(target_os = "windows")]
    {
        // Windows: Program Files ve Program Files (x86) tara
        let program_dirs = vec![
            "C:\\Program Files",
            "C:\\Program Files (x86)",
            "C:\\Users\\{}\\AppData\\Local\\Programs",
        ];
        
        for dir_template in program_dirs {
            let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".to_string());
            let dir = dir_template.replace("{}", &home);
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        // .exe dosyalarını ara
                        if let Ok(exe_entries) = std::fs::read_dir(&path) {
                            for exe in exe_entries.flatten() {
                                let exe_path = exe.path();
                                if exe_path.extension().map_or(false, |e| e == "exe") {
                                    if let Some(name) = exe_path.file_stem().map(|s| s.to_string_lossy().to_string()) {
                                        if !name.is_empty() && !name.contains("uninstall") && !name.contains("installer") {
                                            apps.insert(name.to_lowercase(), AppInfo {
                                                name: name.clone(),
                                                path: exe_path.clone(),
                                                icon: "📱".to_string(),
                                                exec: exe_path.to_string_lossy().to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: mevcut kodun aynısı
        let mut dirs = vec![
            "/usr/share/applications".to_string(),
            "/usr/local/share/applications".to_string(),
        ];
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(format!("{}/.local/share/applications", home));
            dirs.push(format!("{}/.local/share/flatpak/exports/share/applications", home));
            dirs.push("/var/lib/flatpak/exports/share/applications".to_string());
        }
        for dir in &dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "desktop") {
                        if let Some(app) = parse_desktop_file(&path) {
                            apps.insert(app.name.to_lowercase(), app);
                        }
                    }
                }
            }
        }
    }
}

fn index_files(files: &mut HashMap<String, PathBuf>) {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| {
            if cfg!(target_os = "linux") { "/home".to_string() }
            else { "C:\\Users".to_string() }
        });

    let dirs = if cfg!(target_os = "linux") {
        vec![
            format!("{}/Masaüstü", home),
            format!("{}/Belgeler", home),
            format!("{}/İndirilenler", home),
            format!("{}/Resimler", home),
            format!("{}/Müzik", home),
            format!("{}/Videolar", home),
            format!("{}/Desktop", home),
            format!("{}/Documents", home),
            format!("{}/Downloads", home),
            format!("{}/Pictures", home),
            format!("{}/Music", home),
            format!("{}/Videos", home),
            format!("{}/Schreibtisch", home),
            format!("{}/Dokumente", home),
            format!("{}/Bilder", home),
            format!("{}/Musik", home),
        ]
    } else {
        vec![
            format!("{}\\Desktop", home),
            format!("{}\\Documents", home),
            format!("{}\\Downloads", home),
            format!("{}\\Pictures", home),
            format!("{}\\Music", home),
            format!("{}\\Videos", home),
        ]
    };

    for dir in &dirs {
        if std::fs::read_dir(dir).is_ok() {
            for entry in WalkDir::new(dir).max_depth(10).into_iter().flatten() {
                if entry.file_type().is_file() && !entry.file_name().to_string_lossy().starts_with('.') {
                    if let Some(name) = entry.file_name().to_str() {
                        files.insert(name.to_lowercase(), entry.path().to_path_buf());
                    }
                }
            }
        }
    }
    
    println!("Indexed {} files", files.len());
}

fn calculate(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() { return None; }
    
    let valid_chars: Vec<char> = input.chars().filter(|c| !c.is_whitespace()).collect();
    if valid_chars.is_empty() { return None; }
    if !valid_chars.iter().any(|c| c.is_digit(10)) { return None; }
    
    let expr: String = valid_chars.iter().collect();
    
    if let Ok(val) = evalexpr::eval(&expr) {
        return match val {
            evalexpr::Value::Int(n) => Some(format!("{}", n)),
            evalexpr::Value::Float(n) => Some(format!("{}", n)),
            _ => None,
        };
    }
    
    None
}

#[tauri::command]
fn search(state: tauri::State<AppState>, query: String) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let matcher = SkimMatcherV2::default();
    let query_lower = query.to_lowercase();

    // Calculator
    let calc_result = calculate(&query);
    if let Some(result) = calc_result {
        results.push(SearchResult {
            name: format!("{} = {}", query, result),
            desc: "Calculator".to_string(),
            icon: "🧮".to_string(),
            result_type: "calculator".to_string(),
            path: String::new(),
        });
    }

        // Komut paleti (> ile başlayanlar)
    if query.starts_with('>') {
        let cmd = query[1..].trim().to_string();
        if !cmd.is_empty() {
            results.push(SearchResult {
                name: format!("Run: {}", cmd),
                desc: "Execute command".to_string(),
                icon: "🖥️".to_string(),
                result_type: "command".to_string(),
                path: cmd,
            });
        }
    }

    if !query.is_empty() {
        results.push(SearchResult {
            name: format!("Search web for \"{}\"", query),
            desc: "Open in browser".to_string(),
            icon: "🌐".to_string(),
            result_type: "web_search".to_string(),
            path: query.clone(),
        });
    }

        if !query.is_empty() {
        results.push(SearchResult {
            name: format!("Search YouTube for \"{}\"", query),
            desc: "Open in YouTube".to_string(),
            icon: "▶️".to_string(),
            result_type: "youtube_search".to_string(),
            path: query.clone(),
        });

    }

        let sys_cmds = vec![
        ("lock", "Lock Screen", "🔒"),
        ("shutdown", "Shutdown Computer", "⏻"),
        ("reboot", "Restart Computer", "🔄"),
        ("sleep", "Suspend Computer", "😴"),
        ("logout", "Log Out", "🚪"),
        ("settings", "System Settings", "⚙️"),
        ("volumeup", "Volume Up", "🔊"),
        ("volumedown", "Volume Down", "🔉"),
        ("volumemute", "Mute/Unmute", "🔇"),
        ("brightnessup", "Brightness Up", "🔆"),
        ("brightnessdown", "Brightness Down", "🔅"),
        ("wifi", "Toggle Wi-Fi", "📶"),
        ("bluetooth", "Toggle Bluetooth", "🟦"),
        ("screenshot", "Take Screenshot", "📸"),
        ("taskmanager", "Task Manager", "📊"),
        ("calculator", "Open Calculator", "🔢"),
        ("filemanager", "Open File Manager", "📁"),
        ("terminal", "Open Terminal", "💻"),
    ];
    for (cmd, desc, icon) in &sys_cmds {
        if matcher.fuzzy_match(cmd, &query_lower).unwrap_or(0) > 30 {
            results.push(SearchResult {
                name: cmd.to_string(), desc: desc.to_string(),
                icon: icon.to_string(), result_type: "system".to_string(), path: cmd.to_string(),
            });
        }
    }

    let recents = state.recent_items.lock().unwrap();
    for item in recents.iter().rev().take(5) {
        if matcher.fuzzy_match(&item.to_lowercase(), &query_lower).unwrap_or(0) > 30 {
            results.push(SearchResult {
                name: item.clone(), desc: "Recently used".to_string(),
                icon: "🕐".to_string(), result_type: "recent".to_string(), path: item.clone(),
            });
        }
    }

    let app_index = state.app_index.lock().unwrap();
    for (key, app) in app_index.iter() {
        if matcher.fuzzy_match(key, &query_lower).unwrap_or(0) > 20
            || matcher.fuzzy_match(&app.name.to_lowercase(), &query_lower).unwrap_or(0) > 20 {
            results.push(SearchResult {
                name: app.name.clone(), desc: "Application".to_string(),
                icon: get_app_icon_name(&app.icon), result_type: "app".to_string(),
                path: app.path.to_string_lossy().to_string(),
            });
        }
    }

    let file_index = state.file_index.lock().unwrap();
    for (name, path) in file_index.iter() {
        if matcher.fuzzy_match(name, &query_lower).unwrap_or(0) > 40 {
            results.push(SearchResult {
                name: name.clone(),
                desc: get_file_info(path),
                icon: get_file_icon(path),
                result_type: "file".to_string(),
                path: path.to_string_lossy().to_string(),
            });
        }
    }

    results.truncate(20);
    results
}

fn get_app_icon_name(icon: &str) -> String {
    let lower = icon.to_lowercase();
    if lower.contains("firefox") || lower.contains("browser") { return "🦊".into(); }
    if lower.contains("chrome") || lower.contains("chromium") { return "🌐".into(); }
    if lower.contains("terminal") || lower.contains("konsole") || lower.contains("kitty") || lower.contains("alacritty") { return "💻".into(); }
    if lower.contains("code") || lower.contains("vscode") { return "📝".into(); }
    if lower.contains("files") || lower.contains("nautilus") || lower.contains("dolphin") || lower.contains("thunar") { return "📁".into(); }
    if lower.contains("settings") || lower.contains("preferences") { return "⚙️".into(); }
    if lower.contains("calculator") { return "🔢".into(); }
    if lower.contains("mail") || lower.contains("thunderbird") || lower.contains("evolution") || lower.contains("geary") { return "📧".into(); }
    if lower.contains("music") || lower.contains("spotify") || lower.contains("rhythmbox") { return "🎵".into(); }
    if lower.contains("video") || lower.contains("vlc") || lower.contains("mpv") { return "🎬".into(); }
    if lower.contains("image") || lower.contains("gimp") || lower.contains("eog") || lower.contains("gwenview") { return "🖼️".into(); }
    if lower.contains("discord") || lower.contains("telegram") || lower.contains("slack") || lower.contains("signal") { return "💬".into(); }
    if lower.contains("steam") { return "🎮".into(); }
    if lower.contains("torrent") || lower.contains("transmission") { return "📥".into(); }
    if lower.contains("calendar") { return "📅".into(); }
    if lower.contains("pdf") || lower.contains("document") || lower.contains("libreoffice") { return "📃".into(); }
    "📱".into()
}

fn get_file_icon(path: &PathBuf) -> String {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) {
        Some(ref ext) if ext == "pdf" => "📕".into(),
        Some(ref ext) if matches!(ext.as_str(), "jpg"|"jpeg"|"png"|"gif"|"bmp"|"webp"|"svg") => "🖼️".into(),
        Some(ref ext) if matches!(ext.as_str(), "mp3"|"wav"|"flac"|"ogg"|"aac") => "🎵".into(),
        Some(ref ext) if matches!(ext.as_str(), "mp4"|"mkv"|"avi"|"mov"|"webm") => "🎬".into(),
        Some(ref ext) if matches!(ext.as_str(), "zip"|"tar"|"gz"|"rar"|"7z") => "📦".into(),
        Some(ref ext) if matches!(ext.as_str(), "txt"|"md"|"log"|"conf") => "📝".into(),
        Some(ref ext) if matches!(ext.as_str(), "doc"|"docx") => "📃".into(),
        Some(ref ext) if matches!(ext.as_str(), "xls"|"xlsx"|"csv") => "📊".into(),
        Some(ref ext) if matches!(ext.as_str(), "html"|"css"|"js"|"ts"|"json"|"xml") => "💻".into(),
        Some(ref ext) if matches!(ext.as_str(), "py"|"rs"|"cpp"|"c"|"java"|"go") => "⚡".into(),
        Some(ref ext) if matches!(ext.as_str(), "ttf"|"otf") => "🔤".into(),
        Some(ref ext) if matches!(ext.as_str(), "iso"|"img"|"dmg") => "💿".into(),
        Some(ref ext) if matches!(ext.as_str(), "deb"|"rpm"|"appimage") => "📥".into(),
        _ => "📁".into(),
    }
}

fn get_file_info(path: &PathBuf) -> String {
    let size = human_size(path);
    let date = modified_date(path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("File").to_uppercase();
    format!("{} • {} • {}", ext, size, date)
}

fn human_size(path: &PathBuf) -> String {
    if let Ok(meta) = std::fs::metadata(path) {
        let size = meta.len();
        if size < 1024 { return format!("{} B", size); }
        if size < 1024*1024 { return format!("{:.1} KB", size as f64/1024.0); }
        if size < 1024*1024*1024 { return format!("{:.1} MB", size as f64/(1024.0*1024.0)); }
        format!("{:.1} GB", size as f64/(1024.0*1024.0*1024.0))
    } else { "?".into() }
}

fn modified_date(path: &PathBuf) -> String {
    if let Ok(meta) = std::fs::metadata(path) {
        if let Ok(modified) = meta.modified() {
            let dt: DateTime<Local> = modified.into();
            return dt.format("%d/%m/%y %H:%M").to_string();
        }
    }
    "?".into()
}

#[tauri::command]
fn open_file(path: String) -> Result<(), String> {
    open::that(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_app(state: tauri::State<AppState>, path: String) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);
    if let Some(app) = parse_desktop_file(&path_buf) {
        let mut recents = state.recent_items.lock().unwrap();
        recents.push(app.name.clone());

        if !app.exec.is_empty() {
            let parts: Vec<&str> = app.exec.split_whitespace().collect();
            if !parts.is_empty() {
                let mut cmd = std::process::Command::new(parts[0]);
                for arg in &parts[1..] { cmd.arg(arg); }
                cmd.spawn().map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
    }
    if let Some(filename) = path_buf.file_name() {
        std::process::Command::new("gtk-launch")
            .arg(filename.to_string_lossy().to_string())
            .spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(&text).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn web_search(query: String) -> Result<(), String> {
    let url = format!("https://www.google.com/search?q={}", query.replace(' ', "+"));
    open::that(&url).map_err(|e| e.to_string())
}

#[tauri::command]
fn youtube_search(query: String) -> Result<(), String> {
    let url = format!("https://www.youtube.com/results?search_query={}", query.replace(' ', "+"));
    open::that(&url).map_err(|e| e.to_string())
}

#[tauri::command]
fn system_command(cmd: String) -> Result<String, String> {
    match cmd.as_str() {
        "lock" => {
            #[cfg(target_os = "linux")]
            {
                let result = std::process::Command::new("cinnamon-screensaver-command").arg("--lock").spawn();
                if result.is_err() {
                    std::process::Command::new("xdg-screensaver").arg("lock").spawn().map_err(|e| e.to_string())?;
                }
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("rundll32.exe")
                    .args(["user32.dll", "LockWorkStation"])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Ok("Screen locked".to_string())
        }
        "shutdown" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("shutdown").arg("-h").arg("now").spawn().map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("shutdown").args(["/s", "/t", "0"]).spawn().map_err(|e| e.to_string())?;
            }
            Ok("Shutting down...".to_string())
        }
        "reboot" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("shutdown").arg("-r").arg("now").spawn().map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("shutdown").args(["/r", "/t", "0"]).spawn().map_err(|e| e.to_string())?;
            }
            Ok("Rebooting...".to_string())
        }
        "sleep" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("systemctl").arg("suspend").spawn().map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("rundll32.exe")
                    .args(["powrprof.dll", "SetSuspendState", "0", "1", "0"])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Ok("Sleeping...".to_string())
        }
        "logout" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("cinnamon-session-quit").arg("--logout").spawn()
                    .or_else(|_| std::process::Command::new("gnome-session-quit").arg("--logout").spawn())
                    .map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("shutdown").args(["/l"]).spawn().map_err(|e| e.to_string())?;
            }
            Ok("Logging out...".to_string())
        }
        "settings" => {
            #[cfg(target_os = "linux")]
            {
                let result = std::process::Command::new("cinnamon-settings").spawn();
                if result.is_err() {
                    std::process::Command::new("gnome-control-center").spawn().map_err(|e| e.to_string())?;
                }
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("ms-settings:").spawn()
                    .or_else(|_| std::process::Command::new("start").arg("ms-settings:").spawn())
                    .map_err(|e| e.to_string())?;
            }
            Ok("Settings opened".to_string())
        }
        "volumeup" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("pactl").args(["set-sink-volume", "@DEFAULT_SINK@", "+5%"]).spawn().map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("powershell")
                    .args(["-Command", "(New-Object -ComObject WScript.Shell).SendKeys([char]175)"])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Ok("Volume up".to_string())
        }
        "volumedown" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("pactl").args(["set-sink-volume", "@DEFAULT_SINK@", "-5%"]).spawn().map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("powershell")
                    .args(["-Command", "(New-Object -ComObject WScript.Shell).SendKeys([char]174)"])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Ok("Volume down".to_string())
        }
        "volumemute" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("pactl").args(["set-sink-mute", "@DEFAULT_SINK@", "toggle"]).spawn().map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("powershell")
                    .args(["-Command", "(New-Object -ComObject WScript.Shell).SendKeys([char]173)"])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Ok("Mute toggled".to_string())
        }
        "brightnessup" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("brightnessctl").args(["set", "+10%"]).spawn().map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("powershell")
                    .args(["-Command", "(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightnessMethods).WmiSetBrightness(1, [Math]::Min((Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightness).CurrentBrightness + 10, 100))"])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Ok("Brightness up".to_string())
        }
        "brightnessdown" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("brightnessctl").args(["set", "10%-"]).spawn().map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("powershell")
                    .args(["-Command", "(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightnessMethods).WmiSetBrightness(1, [Math]::Max((Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightness).CurrentBrightness - 10, 0))"])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Ok("Brightness down".to_string())
        }
        "wifi" => {
            #[cfg(target_os = "linux")]
            {
                let status = std::process::Command::new("nmcli").args(["radio", "wifi"]).output().map_err(|e| e.to_string())?;
                let status_str = String::from_utf8_lossy(&status.stdout);
                if status_str.contains("enabled") {
                    std::process::Command::new("nmcli").args(["radio", "wifi", "off"]).spawn().map_err(|e| e.to_string())?;
                    Ok("Wi-Fi disabled".to_string())
                } else {
                    std::process::Command::new("nmcli").args(["radio", "wifi", "on"]).spawn().map_err(|e| e.to_string())?;
                    Ok("Wi-Fi enabled".to_string())
                }
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("explorer")
                    .arg("ms-settings:network-wifi")
                    .spawn()
                    .map_err(|e| e.to_string())?;
                Ok("Wi-Fi settings opened".to_string())
            }
        }
        "bluetooth" => {
            #[cfg(target_os = "linux")]
            {
                let status = std::process::Command::new("rfkill").args(["list", "bluetooth"]).output().map_err(|e| e.to_string())?;
                let status_str = String::from_utf8_lossy(&status.stdout);
                if status_str.contains("Soft blocked: yes") {
                    std::process::Command::new("rfkill").args(["unblock", "bluetooth"]).spawn().map_err(|e| e.to_string())?;
                    Ok("Bluetooth enabled".to_string())
                } else {
                    std::process::Command::new("rfkill").args(["block", "bluetooth"]).spawn().map_err(|e| e.to_string())?;
                    Ok("Bluetooth disabled".to_string())
                }
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("explorer")
                    .arg("ms-settings:bluetooth")
                    .spawn()
                    .map_err(|e| e.to_string())?;
                Ok("Bluetooth settings opened".to_string())
            }
        }
        "screenshot" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("gnome-screenshot").arg("--interactive").spawn()
                    .or_else(|_| std::process::Command::new("xfce4-screenshooter").spawn())
                    .map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("snippingtool").spawn()
                    .or_else(|_| std::process::Command::new("explorer").arg("ms-screenclip:").spawn())
                    .map_err(|e| e.to_string())?;
            }
            Ok("Screenshot taken".to_string())
        }
        "taskmanager" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("gnome-system-monitor").spawn()
                    .or_else(|_| std::process::Command::new("xfce4-taskmanager").spawn())
                    .or_else(|_| std::process::Command::new("htop").arg("-t").spawn())
                    .map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("taskmgr").spawn().map_err(|e| e.to_string())?;
            }
            Ok("Task manager opened".to_string())
        }
        "calculator" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("gnome-calculator").spawn()
                    .or_else(|_| std::process::Command::new("galculator").spawn())
                    .map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("calc").spawn().map_err(|e| e.to_string())?;
            }
            Ok("Calculator opened".to_string())
        }
        "filemanager" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("nemo").spawn()
                    .or_else(|_| std::process::Command::new("nautilus").spawn())
                    .or_else(|_| std::process::Command::new("thunar").spawn())
                    .map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("explorer").spawn().map_err(|e| e.to_string())?;
            }
            Ok("File manager opened".to_string())
        }
        "terminal" => {
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("gnome-terminal").spawn()
                    .or_else(|_| std::process::Command::new("xfce4-terminal").spawn())
                    .or_else(|_| std::process::Command::new("x-terminal-emulator").spawn())
                    .map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("cmd").arg("/c").arg("start").arg("cmd").spawn()
                    .or_else(|_| std::process::Command::new("powershell").spawn())
                    .map_err(|e| e.to_string())?;
            }
            Ok("Terminal opened".to_string())
        }
        _ => Err("Unknown command".to_string())
    }
}

#[tauri::command]
fn get_recent_files(state: tauri::State<AppState>) -> Vec<SearchResult> {
    let recents = state.recent_items.lock().unwrap();
    let app_index = state.app_index.lock().unwrap();
    let mut results = Vec::new();

    for item in recents.iter().rev().take(8) {
        let lower = item.to_lowercase();
        if let Some(app) = app_index.get(&lower) {
            results.push(SearchResult {
                name: app.name.clone(),
                desc: "Recently opened".to_string(),
                icon: get_app_icon_name(&app.icon),
                result_type: "recent".to_string(),
                path: app.path.to_string_lossy().to_string(),
            });
        }
    }

    results
}

#[tauri::command]
fn get_autostart() -> Result<bool, String> {
    let auto = auto_launch::AutoLaunchBuilder::new()
        .set_app_name("universal-launcher")
        .set_app_path(&std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy())
        .build()
        .map_err(|e| e.to_string())?;
    Ok(auto.is_enabled().unwrap_or(true))
}

#[tauri::command]
fn set_autostart(enable: bool) -> Result<(), String> {
    let auto = auto_launch::AutoLaunchBuilder::new()
        .set_app_name("universal-launcher")
        .set_app_path(&std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy())
        .build()
        .map_err(|e| e.to_string())?;
    
    if enable {
        auto.enable().map_err(|e| e.to_string())?;
    } else {
        auto.disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_shortcut() -> Result<String, String> {
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("universal-launcher")
        .join("shortcut.txt");
    
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        Ok(content.trim().to_string())
    } else {
        Ok("ctrl+space".to_string())
    }
}

#[tauri::command]
fn set_shortcut(shortcut: String) -> Result<(), String> {
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("universal-launcher")
        .join("shortcut.txt");
    
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&config_path, &shortcut).map_err(|e| e.to_string())?;
    Ok(())
}


#[tauri::command]
fn get_system_info() -> Result<serde_json::Value, String> {
    let mut info = serde_json::json!({
        "os": "Unknown",
        "kernel": "Unknown",
        "hostname": "Unknown",
        "uptime": "0h 0m",
        "loadavg": "0.00 0.00 0.00",
        "cpu": {"name": "Unknown", "cores": 0, "usage": "?%"},
        "ram": {"total": "?", "available": "?", "percent": 0},
        "swap": {"total": "?", "free": "?", "percent": 0},
        "disk": {"total": "?", "free": "?", "percent": 0},
        "network": "Bağlı",
        "temperature": "—"
    });

    #[cfg(target_os = "linux")]
    {
        // OS & Kernel
        if let Ok(os) = std::fs::read_to_string("/etc/os-release") {
            for line in os.lines() {
                if line.starts_with("PRETTY_NAME=") {
                    info["os"] = serde_json::json!(line.split('=').nth(1).unwrap_or("Unknown").trim_matches('"'));
                }
            }
        }
        if let Ok(kernel) = std::fs::read_to_string("/proc/version") {
            let k = kernel.split_whitespace().nth(2).unwrap_or("Unknown");
            info["kernel"] = serde_json::json!(k);
        }

        // Hostname
        if let Ok(host) = std::fs::read_to_string("/etc/hostname") {
            info["hostname"] = serde_json::json!(host.trim());
        }

        // Uptime + Load Average
        if let Ok(up) = std::fs::read_to_string("/proc/uptime") {
            if let Some(seconds_str) = up.split('.').next() {
                if let Ok(seconds) = seconds_str.parse::<u64>() {
                    let hours = seconds / 3600;
                    let mins = (seconds % 3600) / 60;
                    info["uptime"] = serde_json::json!(format!("{}h {}m", hours, mins));
                }
            }
        }
        if let Ok(load) = std::fs::read_to_string("/proc/loadavg") {
            let parts: Vec<&str> = load.split_whitespace().collect();
            if !parts.is_empty() {
                info["loadavg"] = serde_json::json!(parts[0..3].join(" "));
            }
        }

        // CPU
        if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
            let cores = cpuinfo.lines().filter(|l| l.starts_with("processor")).count();
            let model = cpuinfo.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .unwrap_or("Unknown")
                .trim();
            info["cpu"]["name"] = serde_json::json!(model);
            info["cpu"]["cores"] = serde_json::json!(cores);
        }

        // RAM
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            let get_kb = |key: &str| -> f64 {
                meminfo.lines()
                    .find(|l| l.starts_with(key))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0)
            };
            let total = get_kb("MemTotal");
            let avail = get_kb("MemAvailable");
            let percent = if total > 0.0 { ((total - avail) / total * 100.0) as u32 } else { 0 };

            info["ram"] = serde_json::json!({
                "total": format!("{:.1} GB", total / 1024.0 / 1024.0),
                "available": format!("{:.1} GB", avail / 1024.0 / 1024.0),
                "percent": percent
            });
        }

        // Swap
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            let get_kb = |key: &str| -> f64 {
                meminfo.lines()
                    .find(|l| l.starts_with(key))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0)
            };
            let sw_total = get_kb("SwapTotal");
            let sw_free = get_kb("SwapFree");
            let sw_percent = if sw_total > 0.0 { ((sw_total - sw_free) / sw_total * 100.0) as u32 } else { 0 };

            info["swap"] = serde_json::json!({
                "total": format!("{:.1} GB", sw_total / 1024.0 / 1024.0),
                "free": format!("{:.1} GB", sw_free / 1024.0 / 1024.0),
                "percent": sw_percent
            });
        }

        // Disk
        if let Ok(output) = std::process::Command::new("df").arg("-h").arg("/").output() {
            let df = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = df.lines().nth(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let percent = parts[4].trim_end_matches('%').parse::<u32>().unwrap_or(0);
                    info["disk"] = serde_json::json!({
                        "total": parts[1],
                        "free": parts[3],
                        "percent": percent
                    });
                }
            }
        }

        // Temperature
        if let Ok(output) = std::process::Command::new("sensors").output() {
            let temp_str = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = temp_str.lines().find(|l| l.contains("Core 0") || l.contains("CPU")) {
                if let Some(temp) = line.split(':').nth(1).and_then(|s| s.split('°').next()) {
                    info["temperature"] = serde_json::json!(temp.trim());
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use winapi::um::sysinfoapi::{GetComputerNameExW, ComputerNameDnsHostname, GetSystemInfo, SYSTEM_INFO};
use winapi::um::winbase::{GlobalMemoryStatus, MEMORYSTATUS};
use winapi::um::sysinfoapi::GetTickCount;
use winapi::um::fileapi::GetDiskFreeSpaceExW;
use winapi::um::winnt::ULARGE_INTEGER;

        // OS
        info["os"] = serde_json::json!("Windows");

        // Kernel (Windows sürümü)
        if let Ok(version) = std::process::Command::new("cmd").args(["/c", "ver"]).output() {
            let ver_str = String::from_utf8_lossy(&version.stdout);
            let kernel = ver_str.lines().next().unwrap_or("Unknown").trim();
            info["kernel"] = serde_json::json!(kernel);
        }

        // Hostname
        unsafe {
            let mut buffer: [u16; 256] = [0; 256];
            let mut size = 256;
            if GetComputerNameExW(ComputerNameDnsHostname, buffer.as_mut_ptr(), &mut size) != 0 {
                let hostname = String::from_utf16_lossy(&buffer[..size as usize]);
                info["hostname"] = serde_json::json!(hostname);
            }
        }

        // Uptime
        unsafe {
            let uptime_ms = GetTickCount() as u64;
            let uptime_sec = uptime_ms / 1000;
            let hours = uptime_sec / 3600;
            let mins = (uptime_sec % 3600) / 60;
            info["uptime"] = serde_json::json!(format!("{}h {}m", hours, mins));
        }

        // CPU
        unsafe {
            let mut sys_info: SYSTEM_INFO = std::mem::zeroed();
            GetSystemInfo(&mut sys_info);
            let cores = sys_info.dwNumberOfProcessors;
            info["cpu"]["cores"] = serde_json::json!(cores);
            
            // CPU adını registry'den al
            if let Ok(cpu_name) = std::process::Command::new("wmic")
                .args(["cpu", "get", "name", "/format:csv"])
                .output() {
                let cpu_str = String::from_utf8_lossy(&cpu_name.stdout);
                for line in cpu_str.lines() {
                    if line.contains("Name") || line.is_empty() { continue; }
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() >= 2 {
                        info["cpu"]["name"] = serde_json::json!(parts[1].trim());
                        break;
                    }
                }
            }
            if info["cpu"]["name"] == "Unknown" {
                info["cpu"]["name"] = serde_json::json!("Windows CPU");
            }
        }

        // RAM
        unsafe {
    let mut mem_status: MEMORYSTATUS = std::mem::zeroed();
    mem_status.dwLength = std::mem::size_of::<MEMORYSTATUS>() as u32;
    GlobalMemoryStatus(&mut mem_status);
    let total = mem_status.dwTotalPhys as f64 / (1024.0 * 1024.0 * 1024.0);
    let avail = mem_status.dwAvailPhys as f64 / (1024.0 * 1024.0 * 1024.0);
    let used = total - avail;
    let percent = (used / total * 100.0) as u32;
    info["ram"] = serde_json::json!({
        "total": format!("{:.1} GB", total),
        "available": format!("{:.1} GB", avail),
        "percent": percent
    });
}

        // Swap (Windows'ta pagefile olarak)
        if let Ok(output) = std::process::Command::new("wmic")
            .args(["pagefile", "get", "AllocatedBaseSize,CurrentUsage"])
            .output() {
            let swap_str = String::from_utf8_lossy(&output.stdout);
            for line in swap_str.lines() {
                if line.contains("AllocatedBaseSize") || line.is_empty() { continue; }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let (Ok(total), Ok(used)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                        let total_gb = total / 1024.0;
                        let free_gb = (total - used) / 1024.0;
                        let percent = (used / total * 100.0) as u32;
                        info["swap"] = serde_json::json!({
                            "total": format!("{:.1} GB", total_gb),
                            "free": format!("{:.1} GB", free_gb),
                            "percent": percent
                        });
                    }
                }
            }
        }
        if info["swap"]["total"] == "?" {
            info["swap"] = serde_json::json!({
                "total": "0 GB",
                "free": "0 GB",
                "percent": 0
            });
        }

        // Disk (C: sürücüsü)
        unsafe {
            let drive = "C:\\";
            let drive_w: Vec<u16> = drive.encode_utf16().chain(Some(0)).collect();
            let mut free_bytes: ULARGE_INTEGER = std::mem::zeroed();
            let mut total_bytes: ULARGE_INTEGER = std::mem::zeroed();
            let mut total_free_bytes: ULARGE_INTEGER = std::mem::zeroed();
            
            if GetDiskFreeSpaceExW(drive_w.as_ptr(), &mut free_bytes, &mut total_bytes, &mut total_free_bytes) != 0 {
                let total = *total_bytes.QuadPart() as f64 / (1024.0 * 1024.0 * 1024.0);
                let free = *total_free_bytes.QuadPart() as f64 / (1024.0 * 1024.0 * 1024.0);
                let used = total - free;
                let percent = (used / total * 100.0) as u32;
                info["disk"] = serde_json::json!({
                    "total": format!("{:.1} GB", total),
                    "free": format!("{:.1} GB", free),
                    "percent": percent
                });
            }
        }

        // Network
        info["network"] = serde_json::json!("Bağlı");

        // Temperature (Windows'ta genelde yok)
        info["temperature"] = serde_json::json!("—");
    }

    Ok(info)
}

#[tauri::command]
fn get_index_stats(state: tauri::State<AppState>) -> Result<serde_json::Value, String> {
    let app_count = state.app_index.lock().unwrap().len();
    let file_count = state.file_index.lock().unwrap().len();
    Ok(serde_json::json!({
        "apps": app_count,
        "files": file_count
    }))
}

#[tauri::command]
fn execute_command(cmd: String) -> Result<String, String> {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut apps = HashMap::new();
    let mut files = HashMap::new();
    index_apps(&mut apps);
    index_files(&mut files);
    println!("Indexed {} apps and {} files", apps.len(), files.len());

    tauri::Builder::default()
        .manage(AppState {
    app_index: Mutex::new(apps),
    file_index: Mutex::new(files),
    recent_items: Mutex::new(Vec::new()),
})
       .invoke_handler(tauri::generate_handler![search, open_file, open_app, web_search, system_command, get_recent_files, get_autostart, set_autostart, set_shortcut, youtube_search, copy_to_clipboard, get_system_info, get_shortcut, get_index_stats, execute_command])
        .setup(|app| {
            use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
            use tauri::menu::{MenuBuilder, MenuItemBuilder};

            let window = app.get_webview_window("main").unwrap();
            window.center().unwrap();
            window.show().unwrap();

            // Tray ikonu
            let toggle_item = MenuItemBuilder::with_id("toggle", "Show/Hide").build(app).unwrap();
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app).unwrap();
            let menu = MenuBuilder::new(app)
                .item(&toggle_item)
                .item(&quit_item)
                .build()
                .unwrap();

            let _tray = TrayIconBuilder::new()
            .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Universal Launcher")
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "toggle" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)
                .unwrap();

            // Kısayol dinleyici
            let app_handle = app.handle().clone();
            thread::spawn(move || {
                let config_path = dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("universal-launcher")
                    .join("shortcut.txt");
                
                let shortcut_str = std::fs::read_to_string(&config_path)
                    .unwrap_or_else(|_| "ctrl+space".to_string())
                    .trim()
                    .to_string();
                
                let parts: Vec<&str> = shortcut_str.split('+').collect();
                let use_ctrl = parts[0] == "ctrl";
                let key_char = parts.get(1).map(|s| s.to_lowercase()).unwrap_or_else(|| "space".to_string());
                
                let target_key = match key_char.as_str() {
                    "space" => Key::Space,
                    "l" => Key::KeyL,
                    "p" => Key::KeyP,
                    _ => Key::Space,
                };
                
                let ctrl_down = AtomicBool::new(false);
                let alt_down = AtomicBool::new(false);
                
                listen(move |event: Event| {
                    match event.event_type {
                        EventType::KeyPress(Key::Alt) => { alt_down.store(true, Ordering::SeqCst); }
                        EventType::KeyRelease(Key::Alt) => { alt_down.store(false, Ordering::SeqCst); }
                        EventType::KeyPress(Key::ControlLeft) | EventType::KeyPress(Key::ControlRight) => { 
                            ctrl_down.store(true, Ordering::SeqCst); 
                        }
                        EventType::KeyRelease(Key::ControlLeft) | EventType::KeyRelease(Key::ControlRight) => { 
                            ctrl_down.store(false, Ordering::SeqCst); 
                        }
                        EventType::KeyPress(key) if key == target_key => {
                            let modifier_active = if use_ctrl { 
                                ctrl_down.load(Ordering::SeqCst) 
                            } else { 
                                alt_down.load(Ordering::SeqCst) 
                            };
                            
                            if modifier_active {
                                if let Some(window) = app_handle.get_webview_window("main") {
                                    if window.is_visible().unwrap_or(false) {
                                        let _ = window.hide();
                                    } else {
                                        let _ = window.show();
                                        let _ = window.center();
                                        let _ = window.set_focus();
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }).ok();
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}