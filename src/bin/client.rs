use std::io::{Read, Write};
use std::net::TcpStream;
use std::{fs, thread, io};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::format;
use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};
use ratatui::{crossterm::event::{self, KeyCode}, DefaultTerminal, Frame};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use chrono::{DateTime, Datelike, Local, Timelike};
use ratatui::style::{Color, Style};
use users::{get_user_by_uid, get_group_by_gid};
use serde_json::from_str;
use serde::Deserialize;

fn main() {
    match TcpStream::connect("127.0.0.1:7878") {
        Ok(mut stream) => {
            println!("Connected to server!");


            // Receive the response from the server
            /*
            let mut buffer = [0; 512];
            match stream.read(&mut buffer) {
                Ok(size) => {
                    if size > 0 {
                        println!("Received from server: {}", String::from_utf8_lossy(&buffer[..size]));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read from server: {}", e);
                }
            }

             */

            let mut terminal = ratatui::init();
            terminal.clear();

            let initial_path = "/mnt/c/Users/sisin/OneDrive/Plocha/VSB-ING1";

            let entries_info = get_dir_content_from_server(&stream, initial_path, 0);

            let _app_result = run(terminal, entries_info, initial_path.to_string(), &stream);


            ratatui::restore();
            /*
            let result = entries_info
                .iter()
                .map(|inner_vec| inner_vec.join(" ")) // Join each inner Vec<String> by spaces
                .collect::<Vec<String>>()
                .join("\n"); // Join all the strings by newlines

            println!("{}", result);

             */
        }
        Err(e) => {
            eprintln!("Failed to connect to server: {}", e);
        }
    }
}
fn get_dir_content_from_server(mut stream:&TcpStream, initial_path: &str, index: i32) -> Vec<Vec<String>>{
    let request = format!("GET_DIR {}-{}", initial_path, index);
    stream.write_all(request.as_bytes()).expect("Failed to send request");

    let mut buffer = Vec::new();  // Vector to accumulate the response
    let mut temp_buffer = [0u8; 1000];  // Temporary buffer for reading chunks

    loop {
        match stream.read(&mut temp_buffer) {
            // Stream closed
            Ok(0) => break,
            Ok(n) => {
                buffer.extend_from_slice(&temp_buffer[..n]);
                // Condition to read-all-until-empty
                if n != 1000{
                    break;
                }
            },
            Err(e) => {
                eprintln!("Failed to read from server: {}", e);
                break;
            }
        }
    }

    // Convert the response bytes to a UTF-8 string
    let response = String::from_utf8_lossy(&buffer);
    // Keep this commented
    //let response = response.trim_end_matches('\0');

    // Deserialize response
    from_str::<Vec<Vec<String>>>(&response).unwrap_or_else(|e| {
        eprintln!("Failed to deserialize response: {}", e);
        let mut outer_vec:Vec<Vec<String>> = vec![];
        let mut vec:Vec<String>= vec![];
        vec.push("Neco".to_string());
        vec.push("Tady".to_string());
        vec.push("Bude".to_string());

        outer_vec.push(vec);
        outer_vec
    })
}
#[derive(Deserialize)]
struct FileResponse {
    success: bool,
    message: String,
}
fn get_file_content_from_server(mut stream:&TcpStream, path: &str) -> Option<String>{
    let request = format!("GET_FILE {}", path);
    if let Err(e) = stream.write_all(request.as_bytes()) {
        eprintln!("Failed to send request: {}", e);
    }

    let mut buffer = Vec::new();
    let mut temp_buffer = [0u8; 1000];

    loop {
        match stream.read(&mut temp_buffer) {
            // Stream closed
            Ok(0) => break,
            Ok(n) => {
                buffer.extend_from_slice(&temp_buffer[..n]);
                // Condition to read-all-until-empty
                if n != 1000{
                    break;
                }
            },
            Err(e) => {
                eprintln!("Failed to read from server: {}", e);
                break;
            }
        }
    }

    // Convert the response bytes to a UTF-8 string
    let response_json = String::from_utf8_lossy(&buffer);
    let response : FileResponse = serde_json::from_str(&response_json).unwrap();

    //Handle if the server couldn't open/read file
    if response.success{
        Some(response.message)
    }
    else{
        None
    }
}


fn transform_and_render(f : &mut Frame, layout : Rc<[Rect]>, mut state: ListState, title_names : Vec<String>, current_entries : &Vec<Vec<String>>){
    for i in 0..title_names.len(){

        let mut atribute = List::new(current_entries.iter()
            .map(|entry| entry[i].as_str())
            .collect::<Vec<_>>());
        let right_border_index = title_names.len()-1;
        let borders = match i {
            0 => Borders::TOP | Borders::LEFT | Borders::BOTTOM,
            // Guard due to runtime check
            _ if i == right_border_index => Borders::TOP | Borders::BOTTOM | Borders::RIGHT,
            _ => Borders::TOP | Borders::BOTTOM,
        };
        atribute = atribute.block(Block::default().borders(borders).title(title_names[i].clone()))
            .highlight_style(Style::default().bg(Color::Yellow));

        // Add a >> for first
        if i == 0 {
            atribute = atribute.highlight_symbol(">>");
        }

        // Render
        f.render_stateful_widget(atribute, layout[i+1], &mut state);
    }
}
fn key_up_logic(current_state: &mut ListState, length:usize){
    // Circular behavior
    if Some(current_state.selected().unwrap()) == Option::from(0) {
        // select_last was not working correctly
        current_state.select(Some(length - 1));
    } else {
        current_state.select_previous();
    }
}
fn key_down_logic(current_state: &mut ListState, length:usize) {
    // Circular behavior
    if Some(current_state.selected().unwrap()) == Option::from(length-1) {
        current_state.select(Some(0));
    } else {
        current_state.select_next();
    }

}
fn handle_input_field_operations<F, T>(operation: F, current_path: &str, input_field: &str, current_entries: &mut Vec<Vec<String>>, map: &mut HashMap<String, Vec<Vec<String>>>,
                                       current_app_state: &mut AppStates, wrong_input: &mut bool, current_visual_menu_option: &i32, mut stream : &TcpStream) -> Result<(), String>
where
    F: FnOnce(&str) -> Result<T, io::Error>,
{
    if !input_field.is_empty() {
        let new_dir_path = format!("{}/{}", current_path, input_field);

        if let Err(_err) = operation(&new_dir_path) {
            *wrong_input = true;
            Err("Some error.".to_string())
        } else {
            //load_and_parse_dir(current_path, *current_visual_menu_option);
            *current_entries = get_dir_content_from_server(&stream, current_path, *current_visual_menu_option);
            map.insert(current_path.to_string(), current_entries.clone());

            *current_app_state = AppStates::Browsing;

            *wrong_input = false;
            Ok(())
        }
    } else {
        *wrong_input = true;
        Err("Input field is empty.".to_string())
    }
}
enum AppStates{
    Browsing,
    Creating,
    Viewing,
}
#[derive(Clone, Copy)]
enum MenuOption {
    BasicInfo,
    MoreInfo,
    NewDir,
    DeleteDir,
    NewFile,
    DeleteFile,
    ViewFile,
    Exit,
}
impl MenuOption {
    fn to_string(&self) -> &str {
        match self {
            MenuOption::BasicInfo => "Basic information",
            MenuOption::MoreInfo => "Extended information",
            MenuOption::NewDir => "Create directory",
            MenuOption::DeleteDir => "Delete directory",
            MenuOption::NewFile => "Create file",
            MenuOption::DeleteFile => "Delete file",
            MenuOption::ViewFile => "View file",
            MenuOption::Exit => "Exit",
        }
    }
    fn get_visual_titles(&self, title:String) -> Vec<String>{
        match &self{
            MenuOption::BasicInfo =>{
                let title_names_basic = [title.clone(), "Last Modified".to_string()];
                Vec::from(title_names_basic)
            }
            MenuOption::MoreInfo => {
                let title_names_advanced = [title.clone(), "Last Modified".to_string(), "File Size".to_string(), "Owner".to_string(), "Group".to_string(), "Permissions".to_string()];
                Vec::from(title_names_advanced)
            }
            _ => {Vec::new()}
        }
    }
    // Custom iterator
    fn all() -> &'static [MenuOption] {
        &[
            MenuOption::BasicInfo,
            MenuOption::MoreInfo,
            MenuOption::NewDir,
            MenuOption::DeleteDir,
            MenuOption::NewFile,
            MenuOption::DeleteFile,
            MenuOption::ViewFile,
            MenuOption::Exit,
        ]
    }
}

fn create_menu_items<'a>() -> Vec<(MenuOption, ListItem<'a>)> {
    let mut output: Vec<(MenuOption, ListItem)> = Vec::new();
    for option in MenuOption::all(){
        output.push((*option, ListItem::new(option.to_string())))
    }
    output
}
fn run(mut terminal: DefaultTerminal, entries_info: Vec<Vec<String>>, initial_path: String, mut stream:&TcpStream) -> io::Result<()> {
    let mut state = ListState::default();
    state.select(Some(0));

    let mut menu_state = ListState::default();
    let mut current_entries = entries_info.clone();
    let mut current_path = initial_path;

    // HashMap to "cache" most recent directories
    let mut map = HashMap::new();
    map.insert(current_path.clone(), current_entries.clone());

    let mut cached_flag: bool = false;
    let mut start = Instant::now();
    let mut refresh_entries = false;
    let mut terminate_app = false;
    let mut current_visual_menu_option = 0;

    // For menu functionality
    let mut current_menu_item_chosen : MenuOption = MenuOption::BasicInfo;

    // Input fields
    let mut input_field = String::new();
    let mut current_app_state = AppStates::Browsing;
    let mut wrong_input = false;

    //View file
    let mut file_content = String::new();
    let mut scroll_offset = 0;
    let mut view_height: usize = 0;
    let mut lines = 0;

    //Menu
    let menu_items: Vec<(MenuOption, ListItem)> = create_menu_items();

    let menu = List::new(menu_items.iter().map(|(_, item)| item.clone()).collect::<Vec<_>>())
        .block(Block::default().title("Main Menu").borders(Borders::ALL))
        .highlight_style(Style::default().fg(Color::Yellow))
        .highlight_symbol(">> ");

    loop {
        // Exit gracefully when ESC is pressed
        if terminate_app{
            break;
        }

        // No input in the last 50 seconds -> refresh entries
        if refresh_entries {
            // Refresh the entries
            //load_and_parse_dir(current_path.as_str(), current_visual_menu_option);
            current_entries = get_dir_content_from_server(&stream, current_path.as_str(), current_visual_menu_option);

            // Update hashmap if different entries found
            if let Some(value) = map.get(&current_path){
                if *value != current_entries{
                    map.insert(current_path.clone(), current_entries.clone());
                }
            }
        }

        terminal.draw(|f| {
            let size = f.area();

            let grid_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(85),
                    Constraint::Percentage(15),
                ])
                .split(size);


            let basic_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(15),
                    Constraint::Percentage(30),
                    Constraint::Percentage(15),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                ])
                .split(grid_layout[0]);

            let view_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(15),
                    Constraint::Percentage(85),
                ])
                .split(grid_layout[0]);
            view_height = view_layout[1].height as usize;

            let input_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(grid_layout[1]);
            let title ;
            if cached_flag {
                title = format!("{} (Cached)", current_path);
            }
            else{
                title = format!("{}", current_path);
            }

            match current_menu_item_chosen{
                //TODO Handling download folderu - velky folder
                MenuOption::BasicInfo =>{
                    transform_and_render(f, basic_layout.clone(), state.clone(), MenuOption::BasicInfo.get_visual_titles(title.clone()), &current_entries);
                }
                MenuOption::MoreInfo =>{
                    transform_and_render(f, basic_layout.clone(), state.clone(), MenuOption::MoreInfo.get_visual_titles(title.clone()), &current_entries);
                }
                // All other MenuOptions
                _ =>{
                    let text_for_prompt = match current_menu_item_chosen {
                        MenuOption::NewDir => "Enter new directory name: ".to_string(),
                        MenuOption::DeleteDir => "Enter directory name to delete: ".to_string(),
                        MenuOption::NewFile => "Enter new file name: ".to_string(),
                        MenuOption::DeleteFile => "Enter file name to delete: ".to_string(),
                        MenuOption::ViewFile => "Enter file to view: ".to_string(),
                        _ => {"".to_string()},
                    };

                    match current_app_state{
                        AppStates::Browsing | AppStates::Creating=>{
                            let menu_option = if current_visual_menu_option == 0{
                                MenuOption::BasicInfo
                            }
                            else{
                                MenuOption::MoreInfo
                            };

                            transform_and_render(f, basic_layout.clone(), state.clone(), menu_option.get_visual_titles(title.clone()), &current_entries);

                            // If we are creating add input_box
                            if let AppStates::Creating = current_app_state{
                                // Notify user - change background
                                let input_box;
                                if wrong_input{
                                    input_box = Paragraph::new(input_field.clone())
                                        .block(Block::default().borders(Borders::ALL).title(text_for_prompt.clone()))
                                        .style(Style::default().fg(Color::White).bg(Color::Red));
                                }
                                else{
                                    input_box = Paragraph::new(input_field.clone())
                                        .block(Block::default().borders(Borders::ALL).title(text_for_prompt.clone()))
                                        .style(Style::default().fg(Color::White));
                                }

                                f.render_widget(input_box, input_layout[0]);
                            }

                        }
                        AppStates::Viewing =>{
                            //TODO
                            let input_box;
                            if wrong_input{
                                input_box = Paragraph::new(input_field.clone())
                                    .block(Block::default().borders(Borders::ALL).title(text_for_prompt.clone()))
                                    .style(Style::default().fg(Color::White).bg(Color::Red));
                            }
                            else{
                                input_box = Paragraph::new(input_field.clone())
                                    .block(Block::default().borders(Borders::ALL).title(text_for_prompt.clone()))
                                    .style(Style::default().fg(Color::White));
                            }

                            f.render_widget(input_box, input_layout[0]);

                            let paragraph = Paragraph::new(file_content.clone())
                                .block(Block::default().borders(Borders::ALL).title("File Content"))
                                .style(Style::default().fg(Color::White))
                                .scroll((scroll_offset, 0));

                            f.render_widget(paragraph, view_layout[1]);
                        }
                    }

                }
            }

            // Render the menu at the left (first 10% of the screen)
            f.render_stateful_widget(menu.clone(), basic_layout[0], &mut menu_state);

        })?;

        // Listen for events for 20 seconds, if no event occurs then exit the loop -> which reloads entries
        refresh_entries = true;
        while start.elapsed().as_secs() < 60{
            if event::poll(Duration::from_millis(100))? {
                if let event::Event::Key(key) = event::read()? {
                    match current_app_state{
                        AppStates::Browsing =>{
                            match key.code {
                                KeyCode::Up => {
                                    if state.selected() == None {
                                        key_up_logic(&mut menu_state, menu.len());
                                    } else {
                                        key_up_logic(&mut state, current_entries.len());
                                    }
                                    refresh_entries = false;
                                }
                                KeyCode::Down => {
                                    if state.selected() == None {
                                        key_down_logic(&mut menu_state, menu.len());
                                    } else {
                                        key_down_logic(&mut state, current_entries.len());
                                    }
                                    refresh_entries = false;
                                }
                                KeyCode::Right => {
                                    if state.selected() != None {
                                        let selected_index = state.selected().unwrap();
                                        let selected_row = current_entries.get(selected_index).unwrap();

                                        // Parse the row
                                        let parsed_directory: String = selected_row[0].clone();

                                        let new_path = format!("{}{}", current_path, parsed_directory);

                                        if let Ok(metadata) = fs::metadata(&new_path) {
                                            if metadata.is_dir() {
                                                let new_entries;
                                                if let Some(entries) = map.get(&new_path) {
                                                    // Cloning a reference to hashmap value
                                                    new_entries = entries.clone();
                                                    cached_flag = true;
                                                } else {
                                                    //load_and_parse_dir(new_path.as_str(), current_visual_menu_option);
                                                    new_entries = get_dir_content_from_server(&stream, new_path.as_str(), current_visual_menu_option);
                                                    map.insert(new_path.clone(), new_entries.clone());
                                                    cached_flag = false;
                                                }

                                                current_entries = new_entries;
                                                current_path = new_path;

                                                state.select(Some(0));
                                            }
                                        }
                                        refresh_entries = false;
                                    }
                                }
                                KeyCode::Left => {
                                    if state.selected() != None {
                                        if let Some(position) = current_path.rfind('/') {
                                            current_path.truncate(position);

                                            if let Some(entries) = map.get(&current_path) {
                                                // Cloning a reference to hashmap value
                                                current_entries = entries.clone();
                                                cached_flag = true;
                                            } else {
                                                //load_and_parse_dir(current_path.as_str(), current_visual_menu_option);
                                                current_entries = get_dir_content_from_server(&stream, current_path.as_str(), current_visual_menu_option);
                                                map.insert(current_path.clone(), current_entries.clone());
                                                cached_flag = false;
                                            }

                                            state.select(Some(0));
                                        }
                                    }
                                    refresh_entries = false;
                                }
                                KeyCode::Esc => {
                                    terminate_app = true;
                                }
                                KeyCode::Char('f') => {
                                    if state.selected() == None {
                                        menu_state.select(None);
                                        state.select(Some(0));
                                    } else {
                                        menu_state.select(Some(0));
                                        state.select(None);
                                    }
                                    refresh_entries = false;
                                }
                                KeyCode::Enter => {
                                    if let Some(selected_index) = menu_state.selected() {
                                        if let Some((option, _)) = menu_items.get(selected_index) {
                                            match option {
                                                MenuOption::BasicInfo | MenuOption::MoreInfo => {
                                                    match option{
                                                        MenuOption::BasicInfo =>{
                                                            current_menu_item_chosen = MenuOption::BasicInfo;
                                                            current_visual_menu_option = 0;
                                                        }
                                                        MenuOption::MoreInfo =>{
                                                            current_menu_item_chosen = MenuOption::MoreInfo;
                                                            current_visual_menu_option = 1;
                                                        }
                                                        _ => {}
                                                    }
                                                    // Reset "cache"
                                                    map = HashMap::new();
                                                    // Load new entries - different attributes
                                                    //load_and_parse_dir(current_path.as_str(), current_visual_menu_option);
                                                    current_entries = get_dir_content_from_server(&stream, current_path.as_str(), current_visual_menu_option);

                                                    map.insert(current_path.clone(), current_entries.clone());

                                                },
                                                MenuOption::NewDir => {
                                                    current_menu_item_chosen = MenuOption::NewDir;
                                                    // Change the app state to make difference between controls
                                                    current_app_state = AppStates::Creating;
                                                    refresh_entries = false;
                                                },
                                                MenuOption::DeleteDir =>{
                                                    current_menu_item_chosen = MenuOption::DeleteDir;

                                                    current_app_state = AppStates::Creating;
                                                    refresh_entries = false;
                                                }
                                                MenuOption::NewFile =>{
                                                    current_menu_item_chosen = MenuOption::NewFile;

                                                    current_app_state = AppStates::Creating;
                                                    refresh_entries = false;
                                                }
                                                MenuOption::DeleteFile =>{
                                                    current_menu_item_chosen = MenuOption::DeleteFile;

                                                    current_app_state = AppStates::Creating;
                                                    refresh_entries = false;
                                                }
                                                MenuOption::ViewFile =>{
                                                    current_menu_item_chosen = MenuOption::ViewFile;

                                                    current_app_state = AppStates::Creating;
                                                    refresh_entries = false;
                                                }
                                                MenuOption::Exit => {
                                                    terminate_app = true;
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {refresh_entries = false;}
                            }
                        }
                        AppStates::Creating =>{
                            match key.code {
                                KeyCode::Char(c) => {
                                    input_field.push(c);
                                    wrong_input = false;
                                    refresh_entries = false;
                                }
                                KeyCode::Backspace => {
                                    input_field.pop();
                                    wrong_input = false;
                                    refresh_entries = false;
                                }
                                KeyCode::Esc => {
                                    input_field.clear();
                                    wrong_input = false;
                                    // Switch back to browsing
                                    current_app_state = AppStates::Browsing;
                                    refresh_entries = false;
                                }
                                KeyCode::Enter => {
                                    if let Some((option, _)) = menu_items.get(menu_state.selected().unwrap()) {
                                        match option {
                                            // New directory
                                            MenuOption::NewDir => {
                                                let create_dir = |path: &str| fs::create_dir(path);
                                                let res = handle_input_field_operations(create_dir,&current_path, &input_field, &mut current_entries,
                                                                                        &mut map, &mut current_app_state, &mut wrong_input, &current_visual_menu_option, &stream);
                                                if res == Ok(()){
                                                    input_field.clear();
                                                }

                                            }
                                            // Delete directory
                                            MenuOption::DeleteDir => {
                                                let remove_dir = |path: &str| fs::remove_dir(path);
                                                let res = handle_input_field_operations(remove_dir,&current_path, &input_field, &mut current_entries,
                                                                                        &mut map, &mut current_app_state, &mut wrong_input, &current_visual_menu_option, &stream);
                                                if res == Ok(()){
                                                    input_field.clear();
                                                }
                                            }
                                            MenuOption::NewFile => {
                                                let create_file = |path: &str| fs::File::create(path);
                                                let res = handle_input_field_operations(create_file,&current_path, &input_field, &mut current_entries,
                                                                                        &mut map, &mut current_app_state, &mut wrong_input, &current_visual_menu_option, &stream);
                                                if res == Ok(()){
                                                    input_field.clear();
                                                }
                                            }
                                            MenuOption::DeleteFile => {
                                                let remove_file = |path: &str| fs::remove_file(path);
                                                let res = handle_input_field_operations(remove_file,&current_path, &input_field, &mut current_entries,
                                                                                        &mut map, &mut current_app_state, &mut wrong_input, &current_visual_menu_option, &stream);
                                                if res == Ok(()){
                                                    input_field.clear();
                                                }
                                            }
                                            MenuOption::ViewFile =>{
                                                let new_path = format!("{}/{}", current_path, input_field);

                                                if let Some(str) = get_file_content_from_server(&stream, new_path.as_str()){ //read_file_content(new_path, &mut wrong_input){
                                                    file_content = str;
                                                    lines = file_content.split('\n').count();
                                                    current_app_state = AppStates::Viewing;
                                                    refresh_entries = false;
                                                }
                                                else{
                                                    // Handle some problem with opening/reading file -> highlight red
                                                    wrong_input = true;
                                                }

                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                _ => {refresh_entries = false;}
                            }
                        }
                        AppStates::Viewing => {
                            match key.code {
                                KeyCode::Up => {
                                    if scroll_offset > 0 {
                                        scroll_offset -= 1;
                                    }
                                }
                                KeyCode::Down => {

                                    // Limit the down scroll
                                    if lines >= scroll_offset as usize + view_height {
                                        scroll_offset += 1;
                                    }
                                }
                                KeyCode::Esc => {
                                    input_field.clear();
                                    wrong_input = false;
                                    lines = 0;
                                    scroll_offset = 0;
                                    // Switch back to browsing
                                    current_app_state = AppStates::Browsing;
                                }
                                _ => {}
                            }
                            refresh_entries = false;
                        }
                    }
                    break;
                }
            }
        }
        // Refresh the start time counter
        start = Instant::now();
    }

    Ok(())
}
