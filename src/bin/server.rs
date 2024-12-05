use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::{fs, thread};
use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::time::{SystemTime};
use chrono::{DateTime, Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use users::{get_user_by_uid, get_group_by_gid};

#[derive(Serialize, Deserialize)]
struct FileResponse {
    success: bool,
    message: String,
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:7878")?;
    println!("Server listening on port 7878...");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    handle_client(stream);
                    println!("Client closed connection");
                });
            }
            Err(e) => {
                eprintln!("Failed to accept client: {}", e);
            }
        }
    }
    Ok(())
}
fn handle_client(mut stream: TcpStream) {
    println!("New client connected: {}", stream.peer_addr().unwrap());


    let mut buffer = [0u8; 1000];
    loop {
        println!("Waiting for client");
        match stream.read(&mut buffer) {
            Ok(size) => {
                if size == 0 {
                    break;
                }
                let request = String::from_utf8_lossy(&buffer[..size]);
                eprintln!("Request: {}", request);
                if request.starts_with("GET_DIR") {
                    let path_and_visual_index = request.trim().strip_prefix("GET_DIR ").unwrap();

                    let split_index = path_and_visual_index.rfind('-').unwrap();
                    let path = &path_and_visual_index[..split_index];
                    let visual_index = &path_and_visual_index[split_index + 1..].parse::<i32>().unwrap();

                    // Remove any trailing \0 character
                    let path = path.trim_end_matches('\0');

                    let dir_info = load_and_parse_dir(path, *visual_index);
                    let response = serde_json::to_string(&dir_info).unwrap();

                    if let Err(e) = stream.write_all(response.as_bytes()) {
                        eprintln!("Failed to send response: {}", e);
                    }
                }
                else if request.starts_with("GET_FILE"){
                    let path_and_visual_index = request.trim().strip_prefix("GET_FILE ").unwrap();
                    // Remove any trailing \0 character
                    let path = path_and_visual_index.trim_end_matches('\0');

                    let response = match read_file_content(path){
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
                    if let Err(e) = stream.write_all(response_serialized.as_bytes()) {
                        eprintln!("Failed to send response: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to read from client: {}", e);
            }
        }
    }
}
fn convert_rwx_bits(mode: u32) -> String {
    let mut rwx :Vec<String> = vec![];

    let mut shift = 6;
    while shift >= 0{
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
fn load_and_parse_dir(path: &str, current_visual_menu_option: i32) -> Vec<Vec<String>>{
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

        let mut one_row : Vec<String> = Vec::new();
        if path.is_dir() {
            one_row.push(format!("/{}", file_name));
        }
        else{
            one_row.push(format!("{}", file_name));
        }

        one_row.push(format!(
            "{}/{}/{} {}:{}", formatted_day, formatted_month, year, formatted_hours, formatted_minutes
        ));

        if current_visual_menu_option == 1{
            let file_size = fs::metadata(&path).unwrap().len();

            let owner_id = fs::metadata(&path).unwrap().uid();
            let owner_name = get_user_by_uid(owner_id).unwrap().name().to_string_lossy().into_owned();

            let group_id = fs::metadata(&path).unwrap().gid();
            let group_name = get_group_by_gid(group_id).unwrap().name().to_string_lossy().into_owned();

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
    if let Err(_err) = file{
        Err("Unable to open file.".to_string())
    }
    else{
        let mut content = String::new();
        if let Err(_err) = file.unwrap().read_to_string(&mut content){
            Err("Unable to read file".to_string())
        }
        else{
            Ok(content)
        }
    }
}
