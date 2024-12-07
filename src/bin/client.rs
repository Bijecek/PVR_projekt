use std::io::{Read, Write};
use std::net::TcpStream;
use std::{fs, io};
use std::clone::Clone;
use std::collections::HashMap;
use std::fmt::format;
use std::rc::Rc;
use std::time::{Duration, Instant};
use ratatui::{crossterm::event::{self, KeyCode}, DefaultTerminal, Frame};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::style::{Color, Style};
use serde_json::from_str;
use serde::Deserialize;
use std::process;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Direction::Vertical;

struct TwoSides{
    current_side : ClientLogic,
    both_sides: Vec<ClientLogic>,
    // For viewing
    current_side_index: usize
}
impl TwoSides {
    fn new(left_side : ClientLogic, right_side : ClientLogic) -> Self{
        TwoSides{
            current_side : left_side.clone(),
            both_sides : vec![left_side, right_side],
            current_side_index : 0,
        }
    }
    fn key_up(current_state: &mut ListState, length: usize){
        // Circular behavior
        if Some(current_state.selected().unwrap()) == Option::from(0) {
            // select_last was not working correctly
            current_state.select(Some(length - 1));
        } else {
            current_state.select_previous();
        }
    }
    fn key_down(current_state: &mut ListState, length: usize){
        // Circular behavior
        if Some(current_state.selected().unwrap()) == Option::from(length - 1) {
            current_state.select(Some(0));
        } else {
            current_state.select_next();
        }
    }
    fn key_up_down_logic(&mut self, key_pressed : KeyCode) {
        let (state, length) = if self.current_side.selected_content_row.selected().is_none() {
            let length = self.current_side.menu_items_len;
            (&mut self.current_side.selected_menu_row, length)
        } else {
            let length = self.current_side.current_entries.len();
            (&mut self.current_side.selected_content_row, length)
        };

        match key_pressed{
            KeyCode::Up => {
                Self::key_up(state, length);
            }
            KeyCode::Down =>{
                Self::key_down(state, length);
            }
            _ => {}
        }
        self.current_side.refresh_entries = false;
    }
    fn key_right_logic(&mut self, stream: &TcpStream){
        if !self.current_side.selected_content_row.selected().is_none() {
            let selected_index = self.current_side.selected_content_row.selected().unwrap();
            let selected_row = self.current_side.current_entries.get(selected_index).unwrap();

            // Parse the row
            let parsed_directory: String = selected_row[0].clone();

            let new_path = format!("{}{}", self.current_side.current_path, parsed_directory);

            if let Ok(metadata) = fs::metadata(&new_path) {
                if metadata.is_dir() {
                    let new_entries;
                    if let Some(entries) = self.current_side.cache.get(&new_path) {
                        // Cloning a reference to hashmap value
                        new_entries = entries.clone();
                        self.current_side.cached_flag = true;
                    } else {
                        new_entries = get_specific_content_from_server(RequestType::GET_DIR, stream, new_path.as_str(), Some(self.current_side.current_menu_option_index)).into();
                        self.current_side.cache.insert(new_path.clone(), new_entries.clone());
                        self.current_side.cached_flag = false;
                    }

                    self.current_side.current_entries = new_entries;
                    self.current_side.current_path = new_path;

                    self.current_side.selected_content_row.select(Some(0));
                }
            }
            self.current_side.refresh_entries = false;
        }
    }
    fn key_left_logic(&mut self, stream : &TcpStream){
        if !self.current_side.selected_content_row.selected().is_none() {
            if let Some(position) = self.current_side.current_path.rfind('/') {
                self.current_side.current_path.truncate(position);

                if let Some(entries) = self.current_side.cache.get(&self.current_side.current_path) {
                    // Cloning a reference to hashmap value
                    self.current_side.current_entries = entries.clone();
                    self.current_side.cached_flag = true;
                } else {
                    self.current_side.current_entries = get_specific_content_from_server(RequestType::GET_DIR, stream, self.current_side.current_path.as_str(), Some(self.current_side.current_menu_option_index)).into();
                    self.current_side.cache.insert(self.current_side.current_path.clone(), self.current_side.current_entries.clone());
                    self.current_side.cached_flag = false;
                }

                self.current_side.selected_content_row.select(Some(0));
            }
        }
        self.current_side.refresh_entries = false;
    }
    fn key_f_logic(&mut self){
        if self.current_side.selected_content_row.selected().is_none() {
            self.current_side.selected_menu_row.select(None);
            self.current_side.selected_content_row.select(Some(0));
        } else {
            self.current_side.selected_menu_row.select(Some(0));
            self.current_side.selected_content_row.select(None);
        }
        self.current_side.refresh_entries = false;
    }
    fn key_enter_browsing_logic(&mut self, stream: &TcpStream, option: &MenuOption){
        match option {
            MenuOption::BasicInfo | MenuOption::MoreInfo => {
                self.current_side.current_menu_item_chosen = *option;
                self.current_side.current_menu_option_index = match option {
                    MenuOption::BasicInfo => 0,
                    MenuOption::MoreInfo => 1,
                    _ => -1
                };
                // Reset "cache"
                self.current_side.cache = HashMap::new();
                // Load new entries - different attributes
                self.current_side.current_entries = get_specific_content_from_server(RequestType::GET_DIR, stream, self.current_side.current_path.as_str(), Some(self.current_side.current_menu_option_index)).into();
                self.current_side.refresh_entries = false;

                self.current_side.cache.insert(self.current_side.current_path.clone(), self.current_side.current_entries.clone());
            }
            MenuOption::NewDir | MenuOption::DeleteDir | MenuOption::NewFile | MenuOption::DeleteFile | MenuOption::ViewFile => {
                self.current_side.current_menu_item_chosen = *option;
                // Change the app state to make difference between controls
                self.current_side.current_app_state = AppStates::Creating;
                self.current_side.refresh_entries = false;
            }
            MenuOption::Exit => {
                self.current_side.terminate = true;
            }
        }
    }
    fn handle_input_field_edit_keys(&mut self, key_pressed : KeyCode){
        match key_pressed{
            KeyCode::Char(c) => {
                self.current_side.input_field.push(c);
            }
            KeyCode::Backspace =>{
                self.current_side.input_field.pop();
            }
            KeyCode::Esc =>{
                self.current_side.input_field.clear();
                // Switch back to browsing
                self.current_side.current_app_state = AppStates::Browsing;
            }
            _ => {}
        }
        self.current_side.wrong_input = false;
    }
    fn handle_input_field_operations<F, T>(&mut self, operation: F, stream: &TcpStream) -> Result<(), String>
    where
        F: FnOnce(&str) -> Result<T, io::Error>,
    {
        if !self.current_side.input_field.is_empty() {
            let new_dir_path = format!("{}/{}", self.current_side.current_path, self.current_side.input_field);

            if let Err(_err) = operation(&new_dir_path) {
                self.current_side.wrong_input = true;
                Err("Some error.".to_string())
            } else {
                self.current_side.current_entries = get_specific_content_from_server(RequestType::GET_DIR, stream, self.current_side.current_path.as_str(), Some(self.current_side.current_menu_option_index)).into();
                self.current_side.cache.insert(self.current_side.current_path.to_string(), self.current_side.current_entries.clone());

                self.current_side.current_app_state = AppStates::Browsing;
                self.current_side.wrong_input = false;
                Ok(())
            }
        } else {
            self.current_side.wrong_input = true;
            Err("Input field is empty.".to_string())
        }
    }
    fn draw_layouts(&mut self, f : &mut Frame, mut basic_layout: &mut Vec<Rc<[Rect]>>, mut view_layout: &mut Vec<Rc<[Rect]>>, mut input_layout: &mut Vec<Rc<[Rect]>>, mut path_layout: &mut Vec<Rc<[Rect]>>
                    , menu: List){
        // Save the state
        let saved_state = self.current_side.clone();
        // Loop through both sides
        for i in 0..2 {
            // Menu symbol logic
            let mut modified_menu = menu.clone();

            //Correct drawing to specific layout logic
            if i == 0 && self.current_side_index == 1{
                self.current_side = self.both_sides[0].clone();
            }
            else if i == 1 && self.current_side_index == 0 {
                self.current_side = self.both_sides[1].clone();
            }
            else{
                self.current_side = saved_state.clone();

                // Menu symbol logic
                modified_menu = modified_menu.highlight_symbol(">> ");
            }

            let title = if self.current_side.cached_flag {
                format!("{} (Cached)", self.current_side.current_path)
            } else {
                self.current_side.current_path.to_string()
            };

            match self.current_side.current_menu_item_chosen {
                MenuOption::BasicInfo => {
                    self.transform_and_render(f, basic_layout[i].clone(), path_layout[i].clone(), MenuOption::BasicInfo.get_visual_titles(title.clone()), i);
                }
                MenuOption::MoreInfo => {
                    self.transform_and_render(f, basic_layout[i].clone(), path_layout[i].clone(), MenuOption::MoreInfo.get_visual_titles(title.clone()), i);
                }
                // All other MenuOptions
                _ => {
                    let text_for_prompt = match self.current_side.current_menu_item_chosen {
                        MenuOption::NewDir => "Enter new directory name: ".to_string(),
                        MenuOption::DeleteDir => "Enter directory name to delete: ".to_string(),
                        MenuOption::NewFile => "Enter new file name: ".to_string(),
                        MenuOption::DeleteFile => "Enter file name to delete: ".to_string(),
                        MenuOption::ViewFile => "Enter file to view: ".to_string(),
                        _ => { "".to_string() }
                    };

                    match self.current_side.current_app_state {
                        AppStates::Browsing | AppStates::Creating => {
                            let menu_option = if self.current_side.current_menu_option_index == 0 {
                                MenuOption::BasicInfo
                            } else {
                                MenuOption::MoreInfo
                            };

                            self.transform_and_render(f, basic_layout[i].clone(), path_layout[i].clone(), menu_option.get_visual_titles(title.clone()), i);

                            // If we are creating add input_box to layout
                            if let AppStates::Creating = self.current_side.current_app_state {
                                let mut input_box = Paragraph::new(self.current_side.input_field.clone())
                                    .block(Block::default().borders(Borders::ALL).title(text_for_prompt.clone()))
                                    .style(Style::default().fg(Color::White));
                                // Notify user - change background if wrong input
                                if self.current_side.wrong_input {
                                    input_box = input_box.style(Style::default().fg(Color::White).bg(Color::Red));
                                }
                                f.render_widget(input_box, input_layout[i][0]);
                            }
                        }
                        AppStates::Viewing => {
                            let input_box = Paragraph::new(self.current_side.input_field.clone())
                                .block(Block::default().borders(Borders::ALL).title(text_for_prompt.clone()))
                                .style(Style::default().fg(Color::White));

                            f.render_widget(input_box, input_layout[i][0]);


                            let paragraph = Paragraph::new(self.current_side.file_content.clone())
                                .block(Block::default().borders(Borders::ALL).title("File Content"))
                                .style(Style::default().fg(Color::White))
                                .scroll((self.current_side.scroll_offset as u16, 0));

                            f.render_widget(paragraph, view_layout[i][1]);
                        }
                    }
                }
            }
            // Render the menu at the left (first 15% of the screen)
            f.render_stateful_widget(modified_menu.clone(), basic_layout[i][0], &mut self.current_side.selected_menu_row);
        }
        self.current_side = saved_state.clone();
    }
    fn switch_side(&mut self){
        if self.current_side_index == 0{
            //Save current state
            self.both_sides[0] = self.current_side.clone();

            //Switch to different side
            self.current_side_index = 1;
            self.current_side = self.both_sides[1].clone();

            self.current_side.cache.insert(self.current_side.current_path.clone(), self.current_side.current_entries.clone());

            if self.current_side.menu_items_len == 0 {
                //Initialize some item
                self.current_side.selected_content_row.select(Some(0));
                //Initalize menu items
                let menu_items: Vec<(MenuOption, ListItem)> = create_menu_items();

                let menu = List::new(menu_items.iter().map(|(_, item)| item.clone()).collect::<Vec<_>>())
                    .block(Block::default().title("Main Menu").borders(Borders::ALL))
                    .highlight_style(Style::default().fg(Color::Yellow))
                    .highlight_symbol(">> ");
                self.current_side.menu_items_len = menu.len();
            }
        }
        else{
            //Save current state
            self.both_sides[1] = self.current_side.clone();

            //Switch to different side
            self.current_side_index = 0;
            self.current_side = self.both_sides[0].clone();

            self.current_side.cache.insert(self.current_side.current_path.clone(), self.current_side.current_entries.clone());
        }
    }
    fn user_input_handling(&mut self, key : KeyEvent, stream : &TcpStream, menu_items : Vec<(MenuOption, ListItem)>, view_height : usize) {
        match self.current_side.current_app_state {
            AppStates::Browsing => {
                match key.code {
                    KeyCode::Up => {
                        self.key_up_down_logic(KeyCode::Up);
                    }
                    KeyCode::Down => {
                        self.key_up_down_logic(KeyCode::Down);
                    }
                    KeyCode::Right => {
                        self.key_right_logic(stream);
                    }
                    KeyCode::Left => {
                        self.key_left_logic(stream);
                    }
                    KeyCode::Esc => {
                        self.current_side.terminate = true;
                    }
                    KeyCode::Char('f') => {
                        self.key_f_logic();
                    }
                    KeyCode::Char('x') => {
                        self.switch_side();
                    }
                    KeyCode::Enter => {
                        if let Some(selected_index) = self.current_side.selected_menu_row.selected() {
                            if let Some((option, _)) = menu_items.get(selected_index) {
                                self.key_enter_browsing_logic(stream, option);
                            }
                        }
                    }
                    _ => { self.current_side.refresh_entries = false; }
                }
            }
            AppStates::Creating => {
                match key.code {
                    KeyCode::Enter => {
                        if let Some((option, _)) = menu_items.get(self.current_side.selected_menu_row.selected().unwrap()) {
                            let mut res = Result::Err("e".to_string());
                            match option {
                                MenuOption::NewDir => {
                                    let new_dir = |path: &str| fs::create_dir(path);
                                    res = self.handle_input_field_operations(new_dir, stream);
                                }
                                MenuOption::DeleteDir => {
                                    let del_dir = |path: &str| fs::remove_dir(path);
                                    res = self.handle_input_field_operations(del_dir, stream);
                                }
                                MenuOption::NewFile => {
                                    let new_file = |path: &str| fs::File::create(path);
                                    res = self.handle_input_field_operations(new_file, stream);
                                }
                                MenuOption::DeleteFile => {
                                    let del_file = |path: &str| fs::remove_file(path);
                                    res = self.handle_input_field_operations(del_file, stream);
                                }
                                MenuOption::ViewFile => {
                                    self.view_file(stream);
                                }
                                _ => {}
                            }
                            if res == Ok(()) {
                                self.current_side.input_field.clear();
                            }
                        }
                    }
                    _ => {
                        self.handle_input_field_edit_keys(key.code);
                        self.current_side.refresh_entries = false;
                    }
                }
            }
            AppStates::Viewing => {
                match key.code {
                    KeyCode::Up => {
                        if self.current_side.scroll_offset > 0 {
                            self.current_side.scroll_offset -= 1;
                        }
                    }
                    KeyCode::Down => {
                        // Limit the down scroll
                        if self.current_side.file_lines_count >= self.current_side.scroll_offset as usize + view_height {
                            self.current_side.scroll_offset += 1;
                        }
                    }
                    KeyCode::Esc => {
                        self.current_side.input_field.clear();
                        self.current_side.wrong_input = false;
                        self.current_side.file_lines_count = 0;
                        self.current_side.scroll_offset = 0;
                        // Switch back to browsing
                        self.current_side.current_app_state = AppStates::Browsing;
                    }
                    KeyCode::Char('x') => {
                        self.switch_side();
                    }
                    _ => {}
                }
                self.current_side.refresh_entries = false;
            }
        }
    }
    fn view_file(&mut self, stream : &TcpStream){
        let new_path = format!("{}/{}", self.current_side.current_path, self.current_side.input_field);

        let option_content: Option<String> = get_specific_content_from_server(RequestType::GET_FILE, stream, new_path.as_str(), None).into();
        if let Some(content) = option_content {
            self.current_side.file_content = content.clone();
            self.current_side.file_lines_count = content.split('\n').count();
            self.current_side.current_app_state = AppStates::Viewing;
            self.current_side.refresh_entries = false;
        } else {
            self.current_side.wrong_input = true;
        }
    }
    fn transform_and_render(&mut self, f: &mut Frame, layout: Rc<[Rect]>, path_layout: Rc<[Rect]>, title_names: Vec<String>, arrow_index : usize) {
        for i in 0..title_names.len() {
            let mut atribute = List::new(self.current_side.current_entries.iter()
                .map(|entry| entry[i].as_str())
                .collect::<Vec<_>>());
            let right_border_index = title_names.len() - 1;
            let borders = match i {
                0 => Borders::TOP | Borders::LEFT | Borders::BOTTOM,
                // Guard due to runtime check
                _ if i == right_border_index => Borders::TOP | Borders::BOTTOM | Borders::RIGHT,
                _ => Borders::TOP | Borders::BOTTOM,
            };
            atribute = atribute.block(Block::default().borders(borders).title(title_names[i].clone()))
                .highlight_style(Style::default().bg(Color::Yellow));

            // Add a >> for first
            if i == 0{
                let path_box = Paragraph::new(self.current_side.current_path.clone())
                    .block(Block::default().borders(Borders::ALL).title("Current directory path"))
                    .style(Style::default().fg(Color::White));
                f.render_widget(path_box, path_layout[0]);

                if arrow_index == self.current_side_index {
                    atribute = atribute.highlight_symbol(">> ");
                }
            }

            // Render
            f.render_stateful_widget(atribute, layout[i + 1], &mut self.current_side.selected_content_row);
        }
    }
    fn run(&mut self, mut terminal: DefaultTerminal, stream: &TcpStream) -> io::Result<()> {
        self.current_side.selected_content_row.select(Some(0));
        self.current_side.cache.insert(self.current_side.current_path.clone(), self.current_side.current_entries.clone());

        //let mut scroll_offset = 0;
        let mut view_height: Vec<usize> = Vec::default();

        //Menu
        let menu_items: Vec<(MenuOption, ListItem)> = create_menu_items();

        let menu = List::new(menu_items.iter().map(|(_, item)| item.clone()).collect::<Vec<_>>())
            .block(Block::default().title("Main Menu").borders(Borders::ALL))
            .highlight_style(Style::default().fg(Color::Yellow));
        self.current_side.menu_items_len = menu.len();


        loop {
            // Exit gracefully when ESC is pressed or "Quit" is selected
            if self.current_side.terminate {
                break;
            }
            // No input in the last 50 seconds -> refresh entries
            if self.current_side.refresh_entries {
                self.current_side.current_entries = get_specific_content_from_server(RequestType::GET_DIR, stream, self.current_side.current_path.as_str(), Some(self.current_side.current_menu_option_index)).into();

                // Update hashmap if different entries found
                if let Some(value) = self.current_side.cache.get(&self.current_side.current_path) {
                    if *value != self.current_side.current_entries {
                        self.current_side.cache.insert(self.current_side.current_path.clone(), self.current_side.current_entries.clone());
                    }
                }
            }
            // Main rendering logic
            terminal.draw(|f| {
                //Layouts
                let mut basic_layout : Vec<Rc<[Rect]>> = Vec::default();//Layout::default().split(Default::default());
                let mut path_layout: Vec<Rc<[Rect]>> = Vec::default();//Layout::default().split(Default::default());
                let mut view_layout: Vec<Rc<[Rect]>> = Vec::default();//Layout::default().split(Default::default());
                let mut input_layout: Vec<Rc<[Rect]>> = Vec::default();//Layout::default().split(Default::default());
                // Create layouts - layouts need to be recreated every loop cause of the possibility of window resize
                create_layouts(f, &mut basic_layout, &mut view_layout, &mut input_layout, &mut path_layout, &mut view_height);
                self.draw_layouts(f, &mut basic_layout,&mut view_layout, &mut input_layout, &mut path_layout, menu.clone());
            })?;

            // Listen for events for 20 seconds, if no event occurs then exit the loop -> which reloads entries
            self.current_side.refresh_entries = true;
            while self.current_side.start.elapsed().as_secs() < 60 {
                if event::poll(Duration::from_millis(100))? {
                    if let event::Event::Key(key) = event::read()? {
                        // User input handling
                        self.user_input_handling(key, stream, menu_items.clone(), view_height[self.current_side_index]);
                        break;
                    }
                }
            }
            // Refresh the start time counter (for refreshing entries)
            self.current_side.start = Instant::now();
        }
        Ok(())
    }
}
// Enum for different type of requests for server
enum RequestType {
    GET_DIR,
    GET_FILE,
}
// Enum for handling different datatypes from server response
enum StringVec {
    VecString(Option<String>),
    VecVecString(Vec<Vec<String>>),
}
impl From<StringVec> for Option<String> {
    fn from(sv: StringVec) -> Self {
        match sv {
            StringVec::VecString(s) => s,
            _ => None,
        }
    }
}
impl From<StringVec> for Vec<Vec<String>> {
    fn from(sv: StringVec) -> Self {
        match sv {
            StringVec::VecVecString(vec) => vec,
            StringVec::VecString(_) => Vec::new(),
        }
    }
}
// Enum for server response regarding reading file
#[derive(Deserialize)]
struct FileResponse {
    success: bool,
    message: String,
}
// Helper enum to track current app state == what is user doing now
#[derive(Clone)]
enum AppStates {
    Browsing,
    Creating,
    Viewing,
}
// Enum to track which Menu item is currently selected
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
    fn to_string(&self) -> String {
        match self {
            MenuOption::BasicInfo => "Basic information".to_string(),
            MenuOption::MoreInfo => "Extended information".to_string(),
            MenuOption::NewDir => "Create directory".to_string(),
            MenuOption::DeleteDir => "Delete directory".to_string(),
            MenuOption::NewFile => "Create file".to_string(),
            MenuOption::DeleteFile => "Delete file".to_string(),
            MenuOption::ViewFile => "View file".to_string(),
            MenuOption::Exit => "Exit".to_string(),
        }
    }
    fn get_visual_titles(&self, title: String) -> Vec<String> {
        match &self {
            MenuOption::BasicInfo => {
                let get_current_dir = format!{"/{}",title.split('/').last().unwrap().to_string()};
                let title_names_basic = [get_current_dir, "Last Modified".to_string()];
                Vec::from(title_names_basic)
            }
            MenuOption::MoreInfo => {
                let get_current_dir = format!{"/{}",title.split('/').last().unwrap().to_string()};
                let title_names_advanced = [get_current_dir, "Last Modified".to_string(), "Size".to_string(), "Owner".to_string(), "Group".to_string(), "Permissions".to_string()];
                Vec::from(title_names_advanced)
            }
            _ => { Vec::new() }
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
fn main() {
    match TcpStream::connect("127.0.0.1:7878") {
        Ok(stream) => {
            println!("Connected to server!");

            let terminal = ratatui::init();

            let initial_path = "/mnt/c/Users/sisin/OneDrive/Plocha/VSB-ING1";
            let second_path = "/mnt/c/Users/sisin/OneDrive/Plocha/VSB-ING1/PvR";

            let entries_info : Vec<Vec<String>> = get_specific_content_from_server(RequestType::GET_DIR, &stream, initial_path, Some(0)).into();

            let entries_info_2 : Vec<Vec<String>> = get_specific_content_from_server(RequestType::GET_DIR, &stream, second_path, Some(0)).into();

            let left_side = ClientLogic::new_left(entries_info.clone(), initial_path.to_string());
            let right_side = ClientLogic::new_right(entries_info_2.clone(), second_path.to_string());

            let mut client_logic = TwoSides::new(left_side, right_side);

            let _ = client_logic.run(terminal,&stream);
            //let _app_result = run(terminal, entries_info, initial_path.to_string(), &stream);

            ratatui::restore();
        }
        Err(e) => {
            eprintln!("Failed to connect to server: {}", e);
        }
    }
}
fn get_specific_content_from_server(request_type: RequestType, mut stream: &TcpStream, path: &str, index: Option<i32>) -> StringVec {
    let request = match request_type{
        RequestType::GET_DIR => {
            format!("GET_DIR {}-{}", path, index.unwrap())
        }
        RequestType::GET_FILE => {
            format!("GET_FILE {}", path)
        }
    };

    if let Err(e) = stream.write_all(request.as_bytes()) {
        eprintln!("Failed to send request: {}", e);
    }

    let mut buffer = Vec::new();
    let mut temp_buffer = [0u8; 1000];

    loop {
        match stream.read(&mut temp_buffer) {
            // Stream closed
            Ok(0) => {
                ratatui::restore();
                eprintln!("Server closed connection");
                process::exit(-1);
            }
            Ok(n) => {
                buffer.extend_from_slice(&temp_buffer[..n]);
                // Condition to read-all-until-empty
                if n != 1000 {
                    break;
                }
            }
            Err(e) => {
                eprintln!("Failed to read from server: {}", e);
                break;
            }
        }
    }
    let response = String::from_utf8_lossy(&buffer);
    match request_type {
        RequestType::GET_DIR => {
            StringVec::VecVecString(from_str::<Vec<Vec<String>>>(&response).unwrap())
        }
        RequestType::GET_FILE => {
            let response: FileResponse = from_str(&response).unwrap();

            //Handle if the server couldn't open/read file
            if response.success {
                StringVec::VecString(Some(response.message))
            } else {
                StringVec::VecString(None)
            }
        }
    }
}
fn create_menu_items<'a>() -> Vec<(MenuOption, ListItem<'a>)> {
    let mut output: Vec<(MenuOption, ListItem)> = Vec::new();
    for option in MenuOption::all() {
        output.push((*option, ListItem::new(option.to_string())))
    }
    output
}
fn create_layouts(f: &mut Frame, mut basic_layout: &mut Vec<Rc<[Rect]>>, mut view_layout: &mut Vec<Rc<[Rect]>>, mut input_layout: &mut Vec<Rc<[Rect]>>, mut path_layout: &mut Vec<Rc<[Rect]>>,
                  mut view_height: &mut Vec<usize>) {
    // Main split to two sides - left and right
    let left_right_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(f.area());
    // Loop left and right side
    for i in 0..2{
        // Split layout vertically - 95% for data, 5% for user input
        let grid_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(90),
                Constraint::Percentage(10),
            ])
            .split(left_right_layout[i]);
        // Split layout vertically - 10% for path, 90% for data
        path_layout.push(Layout::default()
                              .direction(Direction::Vertical)
                              .constraints([
                                  Constraint::Percentage(10),
                                  Constraint::Percentage(90),
                              ])
                              .split(grid_layout[0]));

        // Split layout horizontally - for each data attribute one column
        basic_layout.push(Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(35),
                Constraint::Percentage(15),
                Constraint::Percentage(7),
                Constraint::Percentage(7),
                Constraint::Percentage(7),
                Constraint::Fill(1),
                //Constraint::Percentage(7.5 as u16),
            ])
            .split(path_layout[i][1]));
        // Split layout horizontally - 15% for menu, 85% for file content
        view_layout.push(Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(15),
                Constraint::Percentage(85),
            ])
            .split(grid_layout[0]));
        // Save height for file scrolling
        view_height.push(view_layout[i][1].height as usize);

        // Layout for user input field
        input_layout.push(Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(100),
            ])
            .split(grid_layout[1]));
    }
}

#[derive(Clone)]
struct ClientLogic{
    selected_content_row: ListState,
    selected_menu_row: ListState,
    current_entries : Vec<Vec<String>>,
    current_path : String,
    cache : HashMap<String, Vec<Vec<String>>>,
    cached_flag : bool,
    start : Instant,
    refresh_entries : bool,
    terminate : bool,
    input_field : String,
    current_app_state : AppStates,
    wrong_input : bool,
    menu_items_len: usize,
    current_menu_option_index : i32,
    current_menu_item_chosen: MenuOption,
    file_content : String,
    file_lines_count : usize,
    scroll_offset : i32,
}
impl ClientLogic {
    fn new_left(current_entries : Vec<Vec<String>>, current_path : String) -> Self{
        ClientLogic {
            selected_content_row: ListState::default(),
            selected_menu_row: ListState::default(),
            current_entries,
            current_path,
            cache: Default::default(),
            cached_flag : false,
            start : Instant::now(),
            refresh_entries : false,
            terminate : false,
            input_field : String::new(),
            current_app_state : AppStates::Browsing,
            wrong_input : false,
            menu_items_len: 0,
            current_menu_option_index : 0,
            current_menu_item_chosen : MenuOption::BasicInfo,
            file_content : String::new(),
            file_lines_count : 0,
            scroll_offset : 0,
        }
    }
    fn new_right(current_entries : Vec<Vec<String>>, current_path : String) -> Self{
        ClientLogic {
            selected_content_row: ListState::default(),
            selected_menu_row: ListState::default(),
            current_entries,
            current_path,
            cache: Default::default(),
            cached_flag : false,
            start : Instant::now(),
            refresh_entries : false,
            terminate : false,
            input_field : String::new(),
            current_app_state : AppStates::Browsing,
            wrong_input : false,
            menu_items_len: 0,
            current_menu_option_index : 0,
            current_menu_item_chosen : MenuOption::BasicInfo,
            file_content : String::new(),
            file_lines_count : 0,
            scroll_offset : 0,
        }
    }

}
