use std::io::{Read, Write};
use std::{fs};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::sync::Mutex;
use std::time::{SystemTime};
use chrono::{DateTime, Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use users::{get_user_by_uid};
use tokio::net::TcpListener;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};

//TODO MUTEX CONTROL
/*
#[derive(Debug)]
struct MutexLogic{
    locked_entities : Mutex<HashMap<String, Option<i32>>>,
}
impl MutexLogic{
    fn new() -> Self{
        MutexLogic{
            locked_entities: Mutex::new(HashMap::new()),
        }
    }
    fn lock_entity(&self, entity_name : String, thread_id : i32) -> Result<(), String>{
        let mut locked_entities = self.locked_entities.lock().unwrap();
        if let Some(locked_by) = locked_entities.get(&entity_name) {
            if locked_by.is_some() {
                return Err(format!("File {} is already locked by another user", file_name));
            }
        }
    }
}

 */
#[derive(Serialize, Deserialize)]
struct FileResponse {
    success: bool,
    message: String,
}
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("Server listening on port 8080...");

    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream).await {
                eprintln!("Error handling client: {}", e);
            }
        });
    }
}

async fn handle_get_dir(stream: &mut tokio::net::TcpStream, path_info: String) {
    // TODO REFACTOR TO THIS let parts: Vec<&str> = path_and_visual_index.split('|').collect();
    let split_index = path_info.rfind('|').unwrap();
    let path = &path_info[..split_index];
    let visual_index = &path_info[split_index + 1..].parse::<i32>().unwrap();

    // Remove any trailing \0 character
    let path = path.trim_end_matches('\0');

    let dir_info = load_and_parse_dir(path, *visual_index);
    let response = serde_json::to_string(&dir_info).unwrap();

    if let Err(e) = stream.write_all(response.as_bytes()).await {
        eprintln!("Failed to send response: {}", e);
    }
    eprintln!("Request sent");
}
async fn handle_get_file(stream: &mut tokio::net::TcpStream, path_info: String) {
    let response = match read_file_content(path_info.as_str()) {
        Ok(content) => FileResponse {
            success: true,
            message: content,
        },
        Err(error_message) => FileResponse {
            success: false,
            message: error_message,
        }
    };
    let response_serialized = serde_json::to_string(&response).unwrap();
    if let Err(e) = stream.write_all(response_serialized.as_bytes()).await {
        eprintln!("Failed to send response: {}", e);
    }
}
async fn handle_filter_dir(stream: &mut tokio::net::TcpStream, path_info: String) {
    let parts: Vec<String> = path_info.split('|')
        .map(|s| s.to_string())
        .collect();

    let path = &parts[0];
    let visual_index = &parts[1].parse::<i32>().unwrap();
    let filter_keyword = &parts[2];

    let dir_info = load_and_parse_dir(path, *visual_index);
    let filtered_dir_info = filter_dir(&dir_info, filter_keyword);

    let response = serde_json::to_string(&filtered_dir_info).unwrap();

    if let Err(e) = stream.write_all(response.as_bytes()).await {
        eprintln!("Failed to send response: {}", e);
    }
}
async fn handle_save_file(path_info: String) {
    let split_index = path_info.rfind(';').unwrap();
    let path = &path_info[..split_index];
    let file_content = &path_info[split_index + 1..];
    if let Err(e) = update_file_content(path, file_content) {
        eprintln!("Failed to save file: {}", e);
    }
}
async fn handle_create_dir(stream: &mut tokio::net::TcpStream, path_info: String) {
    if let Err(_e) = fs::create_dir(path_info) {
        if let Err(e) = stream.write_all(b"Error").await {
            eprintln!("Failed to send response: {}", e);
        }
    } else if let Err(e) = stream.write_all(b"Ok").await {
        eprintln!("Failed to send response: {}", e);
    }
}
async fn handle_remove_dir(stream: &mut tokio::net::TcpStream, path_info: String) {
    if let Err(_e) = fs::remove_dir(path_info) {
        if let Err(e) = stream.write_all(b"Error").await {
            eprintln!("Failed to send response: {}", e);
        }
    } else if let Err(e) = stream.write_all(b"Ok").await {
        eprintln!("Failed to send response: {}", e);
    }
}
async fn handle_create_file(stream: &mut tokio::net::TcpStream, path_info: String) {
    if let Err(_e) = fs::File::create(path_info) {
        if let Err(e) = stream.write_all(b"Error").await {
            eprintln!("Failed to send response: {}", e);
        }
    } else if let Err(e) = stream.write_all(b"Ok").await {
        eprintln!("Failed to send response: {}", e);
    }
}
async fn handle_remove_file(stream: &mut tokio::net::TcpStream, path_info: String) {
    if let Err(_e) = fs::remove_file(path_info) {
        if let Err(e) = stream.write_all(b"Error").await {
            eprintln!("Failed to send response: {}", e);
        }
    } else if let Err(e) = stream.write_all(b"Ok").await {
        eprintln!("Failed to send response: {}", e);
    }
}
async fn handle_move_file(stream: &mut tokio::net::TcpStream, path_info: String) {
    // Split to source | destination
    let split_source_index = path_info.rfind('|').unwrap();
    let source_path = &path_info[..split_source_index];

    //Extract the file name
    let split_filename_index = source_path.rfind('/').unwrap();
    let file_name = &source_path[split_filename_index + 1..];

    let destination_path = &path_info[split_source_index + 1..];
    // Add the filename to destination path
    let full_destination_path = format!("{}/{}", destination_path, file_name);

    if let Err(_e) = fs::rename(source_path, full_destination_path) {
        if let Err(e) = stream.write_all(b"Error").await {
            eprintln!("Failed to send response: {}", e);
        }
    } else if let Err(e) = stream.write_all(b"Ok").await {
        eprintln!("Failed to send response: {}", e);
    }
}
async fn handle_rename_file(stream: &mut tokio::net::TcpStream, path_info: String) {
    // Split to source | destination
    let split_source_index = path_info.rfind('|').unwrap();
    let source_path = &path_info[..split_source_index];

    //Extract the file name
    let split_filename_index = source_path.rfind('/').unwrap();
    let source_path_without_filename = &source_path[..split_filename_index];

    let new_filename = &path_info[split_source_index + 1..];
    // Add the filename to destination path
    let full_destination_path = format!("{}/{}", source_path_without_filename, new_filename);

    if let Err(_e) = fs::rename(source_path, full_destination_path) {
        if let Err(e) = stream.write_all(b"Error").await {
            eprintln!("Failed to send response: {}", e);
        }
    } else if let Err(e) = stream.write_all(b"Ok").await {
        eprintln!("Failed to send response: {}", e);
    }
}
async fn handle_client(mut stream: tokio::net::TcpStream) -> io::Result<()>{
    println!("New client connected: {}", stream.peer_addr().unwrap());
    let mut buffer = [0u8; 1000];
    loop {
        println!("Waiting for client");
        match stream.read(&mut buffer).await {
            Ok(size) => {
                if size == 0 {
                    //Connection closed
                    eprintln!("Client closed connection");
                    return Ok(());
                }
                let request = String::from_utf8_lossy(&buffer[..size]);
                eprintln!("Request: {}", request);

                if request.starts_with("GetDir") {
                    let path_info = request.trim().strip_prefix("GetDir ").unwrap().to_string();
                    handle_get_dir(&mut stream, path_info).await;
                } else if request.starts_with("GetFile") {
                    let path_info = parse_path("GetFile ".to_string(), request);
                    handle_get_file(&mut stream, path_info).await;
                } else if request.starts_with("FilterDir") {
                    let path_info = request.trim().strip_prefix("FilterDir ").unwrap().to_string();
                    handle_filter_dir(&mut stream, path_info).await;
                } else if request.starts_with("SaveFile") {
                    let path_info = request.trim().strip_prefix("SaveFile ").unwrap().to_string();
                    handle_save_file(path_info).await;
                } else if request.starts_with("CreateDir") {
                    let path_info = parse_path("CreateDir ".to_string(), request);
                    handle_create_dir(&mut stream, path_info).await;
                } else if request.starts_with("RemoveDir") {
                    let path_info = parse_path("RemoveDir ".to_string(), request);
                    handle_remove_dir(&mut stream, path_info).await;
                } else if request.starts_with("CreateFile") {
                    let path_info = parse_path("CreateFile ".to_string(), request);
                    handle_create_file(&mut stream, path_info).await;
                } else if request.starts_with("RemoveFile") {
                    let path_info = parse_path("RemoveFile ".to_string(), request);
                    handle_remove_file(&mut stream, path_info).await;
                } else if request.starts_with("MoveFile") {
                    let path_info = request.trim().strip_prefix("MoveFile ").unwrap().to_string();
                    handle_move_file(&mut stream, path_info).await;
                } else if request.starts_with("RenameFile") {
                    let path_info = request.trim().strip_prefix("RenameFile ").unwrap().to_string();
                    handle_rename_file(&mut stream, path_info).await;
                }
            }
            Err(e) => {
                eprintln!("Failed to read from client: {}", e);
            }
        }
    }
}
fn convert_rwx_bits(mode: u32) -> String {
    let mut rwx: Vec<String> = vec![];

    let mut shift = 6;
    while shift >= 0 {
        let mut one_category: String = String::new();

        if mode & (0o400 >> shift) != 0 {
            one_category.push('r');
        } else {
            one_category.push('-');
        }

        if mode & (0o200 >> shift) != 0 {
            one_category.push('w');
        } else {
            one_category.push('-');
        }

        if mode & (0o100 >> shift) != 0 {
            one_category.push('x');
        } else {
            one_category.push('-');
        }
        shift -= 3;
        rwx.push(one_category)
    }
    format!("{}{}{}", rwx[0], rwx[1], rwx[2])
}
fn load_and_parse_dir(path: &str, current_visual_menu_option: i32) -> Vec<Vec<String>> {
    let mut entries_info = Vec::new();
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        let file_name = path.file_name().unwrap().to_string_lossy();
        let last_modified = fs::metadata(&path).unwrap().modified().unwrap();
        //TODO Control that last_modified is accessible - that i have permissions

        // Datetime manipulation
        let time_duration = last_modified.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        let datetime = DateTime::from_timestamp(time_duration.as_secs() as i64, 0).unwrap();

        // Convert UTC to local time due to time differences (-1 h)
        let datetime_local: DateTime<Local> = datetime.with_timezone(&Local);

        let minutes = datetime_local.minute();
        let hours = datetime_local.hour();
        let day = datetime_local.day();
        let month = datetime_local.month();
        let year = datetime_local.year();

        //Add 0 if for better visual representation if there is only one digit (9:03 -> 09:03)
        let formatted_minutes = format!("{:02}", minutes);
        let formatted_hours = format!("{:02}", hours);
        let formatted_day = format!("{:02}", day);
        let formatted_month = format!("{:02}", month);

        let mut one_row: Vec<String> = Vec::new();
        if path.is_dir() {
            one_row.push(format!("/{}", file_name));
        } else {
            one_row.push(format!("{}", file_name));
        }

        one_row.push(format!(
            "{}/{}/{} {}:{}", formatted_day, formatted_month, year, formatted_hours, formatted_minutes
        ));

        if current_visual_menu_option == 1 {
            let file_size = fs::metadata(&path).unwrap().len();

            let owner_id = fs::metadata(&path).unwrap().uid();
            let owner_name = match get_user_by_uid(owner_id) {
                Some(user) => user.name().to_string_lossy().into_owned(),
                None => {
                    "Unknown".to_string()
                }
            };
            let group_id = fs::metadata(&path).unwrap().gid();
            let group_name = match get_user_by_uid(group_id) {
                Some(group) => group.name().to_string_lossy().into_owned(),
                None => {
                    "Unknown".to_string()
                }
            };
            let mode = fs::metadata(&path).unwrap().mode();
            let transformed_permissions = convert_rwx_bits(mode);

            one_row.push(format!(
                "{}", file_size
            ));
            one_row.push(owner_name);
            one_row.push(group_name);
            one_row.push(transformed_permissions);
        }

        entries_info.push(one_row);
    }
    entries_info
}
fn read_file_content(path: &str) -> Result<String, String> {
    let file = File::open(path);
    if let Err(_err) = file {
        Err("Unable to open file.".to_string())
    } else {
        let mut content = String::new();
        if let Err(_err) = file.unwrap().read_to_string(&mut content) {
            Err("Unable to read file".to_string())
        } else {
            Ok(content)
        }
    }
}
fn update_file_content(path: &str, file_content: &str) -> Result<String, String> {
    //Rewrite the current file
    let file = File::create(path);
    if let Err(_err) = file {
        Err("Unable to open file.".to_string())
    } else if let Err(_err) = file.unwrap().write_all(file_content.as_ref()) {
        Err("Unable to write to file".to_string())
    } else {
        Ok("".to_string())
    }
}
fn parse_path(prefix: String, request: Cow<str>) -> String {
    let path_and_visual_index = request.trim().strip_prefix(prefix.as_str()).unwrap();
    // Remove any trailing \0 character
    path_and_visual_index.trim_end_matches('\0').to_string()
}
fn filter_dir(dir_info: &[Vec<String>], filter_keyword: &str) -> Vec<Vec<String>> {
    // Get all that contain the filter_keyword in ignore lower/upper manner
    dir_info.iter()
        .filter(|vec| vec
            .iter().any(|s| {s.to_lowercase().contains(&filter_keyword.to_lowercase()) } ))
        .cloned()
        .collect()
}