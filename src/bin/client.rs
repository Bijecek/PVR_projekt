use std::io::{Read, Write};
use std::net::{TcpStream};
use std::{io};
use std::clone::Clone;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};
use ratatui::{crossterm::event::{self, KeyCode}, DefaultTerminal, Frame};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::style::{Color, Style};
use serde_json::from_str;
use serde::Deserialize;
use std::process;
use ratatui::crossterm::event::{KeyEvent, KeyModifiers};
use ssh2::{Channel, Session};
use tui_textarea::{CursorMove, TextArea};
use std::env;


#[derive(Clone)]
struct ClientLogic<'a>{
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
    input_field_rename : String,
    filter_active: bool,
    current_input_field : i32,
    current_app_state : AppStates,
    wrong_input : bool,
    menu_items_len: usize,
    current_menu_option_index : i32,
    current_menu_item_chosen: MenuOption,
    file_content : String,
    file_lines_count : usize,
    scroll_offset : usize,
    scroll_right_offset : usize,
    text_area: TextArea<'a>,
}
impl <'a>ClientLogic<'a> {
    fn new(current_entries : Vec<Vec<String>>, current_path : String) -> Self{
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
            input_field_rename : String::new(),
            filter_active : false,
            current_input_field : 0,
            current_app_state : AppStates::Browsing,
            wrong_input : false,
            menu_items_len: 0,
            current_menu_option_index : 0,
            current_menu_item_chosen : MenuOption::BasicInfo,
            file_content : String::new(),
            file_lines_count : 0,
            scroll_offset : 0,
            scroll_right_offset : 0,
            text_area : TextArea::default(),
        }
    }
}
struct TwoSides<'a>{
    current_side : ClientLogic<'a>,
    both_sides: Vec<ClientLogic<'a>>,
    // For viewing
    current_side_index: usize
}
impl <'a>TwoSides<'a> {
    fn new(left_side : ClientLogic<'a>, right_side : ClientLogic<'a>) -> Self{
        TwoSides{
            current_side : left_side.clone(),
            both_sides : vec![left_side, right_side],
            current_side_index : 0,
        }
    }
    fn key_up(current_state: &mut ListState, length: usize){
        // Prevent going up on empty entries
        if length != 0 {
            // Circular behavior
            if Some(current_state.selected().unwrap()) == Option::from(0) {
                // select_last was not working correctly
                current_state.select(Some(length - 1));
            } else {
                current_state.select_previous();
            }
        }
    }
    fn key_down(current_state: &mut ListState, length: usize){
        // Prevent going down on empty entries
        if length != 0 {
            // Circular behavior
            if Some(current_state.selected().unwrap()) == Option::from(length - 1) {
                current_state.select(Some(0));
            } else {
                current_state.select_next();
            }
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

        match key_pressed {
            KeyCode::Up => {
                Self::key_up(state, length);
            }
            KeyCode::Down => {
                Self::key_down(state, length);
            }
            _ => {}
        }

        self.current_side.refresh_entries = false;
    }
    fn key_right_logic(&mut self, stream: &mut Channel){
        if self.current_side.selected_menu_row.selected().is_none() && !self.current_side.current_entries.is_empty() {
            let selected_index = self.current_side.selected_content_row.selected().unwrap();
            let selected_row = self.current_side.current_entries.get(selected_index).unwrap();

            // Parse the row
            let parsed_directory: String = selected_row[0].clone();

            let new_path = format!("{}{}", self.current_side.current_path, parsed_directory);
            if parsed_directory.starts_with("/"){
                let new_entries;
                if let Some(entries) = self.current_side.cache.get(&new_path) {
                    // Cloning a reference to hashmap value
                    new_entries = entries.clone();
                    self.current_side.cached_flag = true;
                } else {
                    new_entries = get_specific_content_from_server(RequestType::GetDir, stream, new_path.as_str(), Some(self.current_side.current_menu_option_index), None).into();
                    self.current_side.cache.insert(new_path.clone(), new_entries.clone());
                    self.current_side.cached_flag = false;
                }

                self.current_side.current_entries = new_entries;
                self.current_side.current_path = new_path;

                self.current_side.selected_content_row.select(Some(0));
            }
            self.current_side.filter_active = false;
            self.current_side.input_field.clear();
        }
        self.current_side.refresh_entries = false;
    }
    fn key_left_logic(&mut self, stream : &mut Channel){
        if self.current_side.selected_menu_row.selected().is_none() {
            if let Some(position) = self.current_side.current_path.rfind('/') {
                self.current_side.current_path.truncate(position);

                if let Some(entries) = self.current_side.cache.get(&self.current_side.current_path) {
                    // Cloning a reference to hashmap value
                    self.current_side.current_entries = entries.clone();
                    self.current_side.cached_flag = true;
                } else {
                    self.current_side.current_entries = get_specific_content_from_server(RequestType::GetDir, stream, self.current_side.current_path.as_str(),
                                                                                         Some(self.current_side.current_menu_option_index), None).into();
                    self.current_side.cache.insert(self.current_side.current_path.clone(), self.current_side.current_entries.clone());
                    self.current_side.cached_flag = false;
                }

                self.current_side.selected_content_row.select(Some(0));
            }
            self.current_side.filter_active = false;
            self.current_side.input_field.clear();
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
    fn key_enter_browsing_logic(&mut self, stream: &mut Channel, option: &MenuOption){
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
                self.current_side.current_entries = get_specific_content_from_server(RequestType::GetDir, stream, self.current_side.current_path.as_str(),
                                                                                     Some(self.current_side.current_menu_option_index), None).into();
                self.current_side.refresh_entries = false;

                self.current_side.cache.insert(self.current_side.current_path.clone(), self.current_side.current_entries.clone());
            }
            MenuOption::NewDir | MenuOption::DeleteDir | MenuOption::NewFile | MenuOption::DeleteFile | MenuOption::ViewFile | MenuOption::MoveFile | MenuOption::RenameFile | MenuOption::FilterDir => {
                self.current_side.current_menu_item_chosen = *option;
                // Change the app state to make difference between controls
                self.current_side.current_app_state = AppStates::Creating;
                self.current_side.refresh_entries = false;
                self.current_side.input_field.clear();
            }
            MenuOption::Exit => {
                self.current_side.terminate = true;
            }
        }
    }
    fn handle_input_field_edit_keys(&mut self, key_pressed : KeyCode){
        // Handle two input fields logic
        let current_input_field = if self.current_side.current_input_field == 0{
           &mut self.current_side.input_field
        }
        else{
            &mut self.current_side.input_field_rename
        };
        match key_pressed{
            KeyCode::Char(c) => {
                current_input_field.push(c);
            }
            KeyCode::Backspace =>{
                current_input_field.pop();
            }
            KeyCode::Esc =>{
                self.current_side.input_field.clear();
                self.current_side.input_field_rename.clear();
                self.current_side.current_input_field = 0;
                // Switch back to browsing
                self.current_side.current_app_state = AppStates::Browsing;
            }
            _ => {}
        }
        self.current_side.wrong_input = false;
    }
    fn handle_input_field_operations2(&mut self, request_type: RequestType, stream: &mut Channel) -> Result<(), String> {
        if !self.current_side.input_field.is_empty() {
            let new_dir_path;
            match request_type{
                RequestType::MoveFile =>{
                    // Format current path and path from second "window"
                    new_dir_path = format!("{}/{}|{}", self.current_side.current_path, self.current_side.input_field, self.both_sides[(self.current_side_index+1)%2].current_path);
                }
                RequestType::RenameFile =>{
                    if self.current_side.input_field_rename.is_empty(){
                        self.current_side.wrong_input = true;
                        return Err("Rename field is missing.".to_string())
                    }
                    new_dir_path = format!("{}/{}|{}", self.current_side.current_path, self.current_side.input_field, self.current_side.input_field_rename);
                }
                _ => {
                    new_dir_path = format!("{}/{}", self.current_side.current_path, self.current_side.input_field);
                }
            }
            send_request_to_server(stream, request_type, new_dir_path.as_str(), String::new());

            if let Ok(()) = wait_for_server_response(stream){
                self.current_side.current_entries = get_specific_content_from_server(RequestType::GetDir, stream, self.current_side.current_path.as_str(),
                                                                                     Some(self.current_side.current_menu_option_index), None).into();
                self.current_side.cache.insert(self.current_side.current_path.to_string(), self.current_side.current_entries.clone());

                self.current_side.current_app_state = AppStates::Browsing;
                self.current_side.wrong_input = false;
                Ok(())
            }
            else{
                self.current_side.wrong_input = true;
                Err("Input field is wrong.".to_string())
            }
        } else {
            self.current_side.wrong_input = true;
            Err("Input field is empty.".to_string())
        }
    }
    fn draw_layouts(&mut self, f : &mut Frame, basic_layout: &mut [Rc<[Rect]>], view_layout: &mut [Rc<[Rect]>], input_layout: &mut [Rc<[Rect]>], path_layout: &mut [Rc<[Rect]>], menu: List){
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
                        MenuOption::MoveFile => "Enter file to move: ".to_string(),
                        MenuOption::RenameFile => "Enter file to rename: ".to_string(),
                        MenuOption::FilterDir => "Enter keyword to filter: ".to_string(),
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

                            // Filtering logic
                            if let AppStates::Browsing = self.current_side.current_app_state {
                                if self.current_side.filter_active {
                                    let mut input_box = Paragraph::new(self.current_side.input_field.clone())
                                        .block(Block::default().borders(Borders::ALL).title(text_for_prompt.clone()))
                                        .style(Style::default().fg(Color::White));
                                    input_box = input_box.style(Style::default().fg(Color::White).bg(Color::Green));
                                    f.render_widget(input_box, input_layout[i][0]);
                                }
                            }

                            // If we are creating add input_box to layout
                            if let AppStates::Creating = self.current_side.current_app_state {
                                let mut input_box = Paragraph::new(self.current_side.input_field.clone())
                                    .block(Block::default().borders(Borders::ALL).title(text_for_prompt.clone()))
                                    .style(Style::default().fg(Color::White));

                                // If we are renaming, add another input_box
                                if let MenuOption::RenameFile = self.current_side.current_menu_item_chosen{
                                    let input_box = Paragraph::new(self.current_side.input_field_rename.clone())
                                        .block(Block::default().borders(Borders::ALL).title("Enter new filename: "))
                                        .style(Style::default().fg(Color::White));
                                    f.render_widget(input_box, input_layout[i][1]);
                                }

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
                                .scroll((self.current_side.scroll_offset as u16, self.current_side.scroll_right_offset as u16));

                            f.render_widget(paragraph, view_layout[i][1]);
                        }
                        AppStates::Editing => {
                            self.current_side.text_area.set_block(
                                Block::default()
                                    .borders(Borders::ALL).title("File Content"));
                            f.render_widget(&self.current_side.text_area, view_layout[i][1]);
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
        }
    }
    fn key_esc_logic(&mut self){
        self.current_side.input_field.clear();
        self.current_side.input_field_rename.clear();
        self.current_side.wrong_input = false;
        self.current_side.file_lines_count = 0;
        self.current_side.scroll_offset = 0;
        // Switch back to browsing
        self.current_side.current_app_state = AppStates::Browsing;

    }
    fn browsing_key_logic(&mut self, key : KeyEvent, stream : &mut Channel, menu_items : Vec<(MenuOption, ListItem)>){
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
                if self.current_side.filter_active{
                    // Reset input fields
                    self.current_side.input_field.clear();
                    self.current_side.input_field_rename.clear();
                    self.current_side.filter_active = false;
                    self.current_side.current_input_field = 0;
                }
                else {
                    self.current_side.terminate = true;
                }
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
    fn creating_key_logic(&mut self, key : KeyEvent, stream : &mut Channel, menu_items : Vec<(MenuOption, ListItem)>){
        match key.code {
            KeyCode::Enter => {
                //TODO REFACTOR THIS - can use self.current_side.current_menu_item_chosen
                if let Some((option, _)) = menu_items.get(self.current_side.selected_menu_row.selected().unwrap()) {
                    let mut res = Result::Err("e".to_string());
                    match option {
                        MenuOption::NewDir => {
                            res = self.handle_input_field_operations2(RequestType::CreateDir, stream);
                        }
                        MenuOption::DeleteDir => {
                            res = self.handle_input_field_operations2(RequestType::RemoveDir, stream);
                        }
                        MenuOption::FilterDir =>{
                            // Load new entries - different attributes
                            self.current_side.current_entries = get_specific_content_from_server(RequestType::FilterDir, stream, self.current_side.current_path.as_str(),
                                                                                                 Some(self.current_side.current_menu_option_index), Some(self.current_side.input_field.clone())).into();
                            self.current_side.refresh_entries = false;

                            self.current_side.current_app_state = AppStates::Browsing;
                            // Save the state - will have green background
                            self.current_side.filter_active = true;
                            // Do not erase the input field
                            return;
                        }
                        MenuOption::NewFile => {
                            res = self.handle_input_field_operations2(RequestType::CreateFile, stream);
                        }
                        MenuOption::DeleteFile => {
                            res = self.handle_input_field_operations2(RequestType::RemoveFile, stream);
                        }
                        MenuOption::MoveFile => {
                            res = self.handle_input_field_operations2(RequestType::MoveFile, stream);
                        }
                        MenuOption::RenameFile => {
                            res = self.handle_input_field_operations2(RequestType::RenameFile, stream);
                        }
                        MenuOption::ViewFile => {
                            self.view_file(stream);
                        }

                        _ => {}
                    }
                    if res == Ok(()) {
                        self.current_side.input_field.clear();
                        self.current_side.input_field_rename.clear();
                        self.current_side.filter_active = false;
                        self.current_side.current_input_field = 0;
                    }
                }
            }
            // Tab is used to switch between text_fields when renaming file
            KeyCode::Tab => {
                if let MenuOption::RenameFile = self.current_side.current_menu_item_chosen{
                    if self.current_side.current_input_field == 0 {
                        self.current_side.current_input_field = 1;
                    }
                    else{
                        self.current_side.current_input_field = 0;
                    }
                }
            }
            _ => {
                self.handle_input_field_edit_keys(key.code);
                self.current_side.refresh_entries = false;
            }
        }
    }
    fn viewing_key_logic(&mut self, key : KeyEvent, view_height : usize){
        match key.code {
            KeyCode::Up => {
                if self.current_side.scroll_offset > 0 {
                    self.current_side.scroll_offset -= 1;
                }
            }
            KeyCode::Down => {
                // Limit the down scroll
                if self.current_side.file_lines_count >= self.current_side.scroll_offset + view_height {
                    self.current_side.scroll_offset += 1;
                }
            }
            KeyCode::Left =>{
                if self.current_side.scroll_right_offset > 0 {
                    self.current_side.scroll_right_offset -= 1;
                }
            }
            KeyCode::Right =>{
                self.current_side.scroll_right_offset += 1;
            }
            KeyCode::Esc => {
                self.key_esc_logic();
            }
            KeyCode::Char('i') => {
                self.current_side.current_app_state = AppStates::Editing;
                self.current_side.scroll_offset = 1;
                self.current_side.scroll_right_offset = 0;

                let splitted_vec: Vec<String> = self.current_side.file_content.split('\n')
                    .map(|s| s.to_string())
                    .collect();
                self.current_side.text_area = TextArea::new(splitted_vec);
            }
            KeyCode::Char('x') => {
                self.switch_side();
            }
            _ => {}
        }
        self.current_side.refresh_entries = false;
    }
    fn editing_key_logic(&mut self, key : KeyEvent, stream : &mut Channel){
        match key.code {
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 's' => {
                send_request_to_server(stream, RequestType::SaveFile, format!{"{}/{}", self.current_side.current_path.as_str(), self.current_side.input_field}.as_str(), self.current_side.text_area.lines().join("\n"));

                self.key_esc_logic();
                self.current_side.text_area = TextArea::default();
            }
            KeyCode::Esc => {
                self.key_esc_logic();
                self.current_side.text_area = TextArea::default();
            }
            KeyCode::Char(c) => {
                self.current_side.text_area.insert_char(c);
            }
            KeyCode::Backspace => {
                self.current_side.text_area.delete_char();
            }
            KeyCode::Enter => {
                self.current_side.text_area.insert_char('\n');
            }
            KeyCode::Right =>{
                self.current_side.text_area.move_cursor(CursorMove::Forward)
            }
            KeyCode::Left =>{
                self.current_side.text_area.move_cursor(CursorMove::Back)
            }
            KeyCode::Up =>{
                self.current_side.text_area.move_cursor(CursorMove::Up)
            }
            KeyCode::Down =>{
                self.current_side.text_area.move_cursor(CursorMove::Down)
            }
            _ => {}
        }
        self.current_side.refresh_entries = false;
    }
    fn user_input_handling(&mut self, key : KeyEvent, stream : &mut Channel, menu_items : Vec<(MenuOption, ListItem)>, view_height : usize) {
        match self.current_side.current_app_state {
            AppStates::Browsing => {
                self.browsing_key_logic(key, stream, menu_items);
            }
            AppStates::Creating => {
                self.creating_key_logic(key, stream, menu_items);
            }
            AppStates::Viewing => {
                self.viewing_key_logic(key,view_height);
            }
            AppStates::Editing => {
                self.editing_key_logic(key,stream);
            }
        }
    }
    fn view_file(&mut self, stream : &mut Channel){
        let new_path = format!("{}/{}", self.current_side.current_path, self.current_side.input_field);

        let option_content: Option<String> = get_specific_content_from_server(RequestType::GetName, stream, new_path.as_str(), None, None).into();
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
    fn run(&mut self, mut terminal: DefaultTerminal, stream: &mut Channel) -> io::Result<()> {
        self.current_side.selected_content_row.select(Some(0));
        self.current_side.cache.insert(self.current_side.current_path.clone(), self.current_side.current_entries.clone());

        //let mut scroll_offset = 0;
        let mut view_height: Vec<usize> = Vec::default();
        let mut view_width: Vec<usize> = Vec::default();

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
            if self.current_side.refresh_entries && !self.current_side.filter_active {
                self.current_side.current_entries = get_specific_content_from_server(RequestType::GetDir, stream, self.current_side.current_path.as_str(),
                                                                                     Some(self.current_side.current_menu_option_index), None).into();

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
                let mut basic_layout : Vec<Rc<[Rect]>> = Vec::default();
                let mut path_layout: Vec<Rc<[Rect]>> = Vec::default();
                let mut view_layout: Vec<Rc<[Rect]>> = Vec::default();
                let mut input_layout: Vec<Rc<[Rect]>> = Vec::default();
                // Create layouts - layouts need to be recreated every loop cause of the possibility of window resize
                create_layouts(f, &mut basic_layout, &mut view_layout, &mut input_layout, &mut path_layout, &mut view_height, &mut view_width);
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
#[derive(Debug)]
enum RequestType {
    GetDir,
    GetName,
    FilterDir,
    RemoveDir,
    CreateDir,
    RemoveFile,
    CreateFile,
    SaveFile,
    MoveFile,
    RenameFile
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
    Editing
}
// Enum to track which Menu item is currently selected
#[derive(Clone, Copy)]
enum MenuOption {
    BasicInfo,
    MoreInfo,
    FilterDir,
    NewDir,
    DeleteDir,
    NewFile,
    DeleteFile,
    ViewFile,
    MoveFile,
    RenameFile,
    Exit,
}
impl MenuOption {
    fn get_string_equivalent(&self) -> String {
        match self {
            MenuOption::BasicInfo => "Basic information".to_string(),
            MenuOption::MoreInfo => "Extended information".to_string(),
            MenuOption::FilterDir => "Filter entries".to_string(),
            MenuOption::NewDir => "Create directory".to_string(),
            MenuOption::DeleteDir => "Delete directory".to_string(),
            MenuOption::NewFile => "Create file".to_string(),
            MenuOption::DeleteFile => "Delete file".to_string(),
            MenuOption::ViewFile => "View file".to_string(),
            MenuOption::MoveFile => "Move file".to_string(),
            MenuOption::RenameFile => "Rename file".to_string(),
            MenuOption::Exit => "Exit".to_string(),
        }
    }
    fn get_visual_titles(&self, title: String) -> Vec<String> {
        match &self {
            MenuOption::BasicInfo => {
                let get_current_dir = format!{"/{}",title.split('/').last().unwrap()};
                let title_names_basic = [get_current_dir, "Last Modified".to_string()];
                Vec::from(title_names_basic)
            }
            MenuOption::MoreInfo => {
                let get_current_dir = format!{"/{}",title.split('/').last().unwrap()};
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
            MenuOption::FilterDir,
            MenuOption::NewDir,
            MenuOption::DeleteDir,
            MenuOption::NewFile,
            MenuOption::DeleteFile,
            MenuOption::ViewFile,
            MenuOption::MoveFile,
            MenuOption::RenameFile,
            MenuOption::Exit,
        ]
    }
}

fn get_specific_content_from_server(request_type: RequestType, stream: &mut Channel, path: &str, index: Option<i32>, filter_keyword : Option<String>) -> StringVec {
    let request = match request_type{
        RequestType::GetDir => {
            format!("GetDir {}|{}", path, index.unwrap())
        }
        RequestType::GetName => {
            format!("GetFile {}", path)
        }
        RequestType::FilterDir =>{
            if filter_keyword.is_none() {
                format!("GetDir {}|{}", path, index.unwrap())
            }
            else{
                format!("FilterDir {}|{}|{}", path, index.unwrap(), filter_keyword.unwrap())
            }
        }
        _ => {String::new()}
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
        RequestType::GetDir | RequestType::FilterDir => {
            StringVec::VecVecString(from_str::<Vec<Vec<String>>>(&response).unwrap())
        }
        RequestType::GetName => {
            let response: Result<FileResponse, _> = from_str(&response);

            match response {
                //Handle if the server couldn't open/read file
                Ok(response) if response.success => StringVec::VecString(Some(response.message)),
                _ => StringVec::VecString(None),
            }
        }
        _ => {StringVec::VecString(None)}
    }
}
fn send_request_to_server(stream: &mut Channel, request_type: RequestType, path: &str, file_content: String){
    match request_type {
        RequestType::SaveFile =>{
            let request = format!("SaveFile {};{}", path, file_content);
            if let Err(e) = stream.write_all(request.as_bytes()) {
                eprintln!("Failed to send request: {}", e);
            }
        }
        _ =>{
            // Handle other operations that need to be performed on the server
            let request = format!("{:?} {}", request_type, path);
            if let Err(e) = stream.write_all(request.as_bytes()) {
                eprintln!("Failed to send request: {}", e);
            }
        }
    }
}
fn wait_for_server_response(stream: &mut Channel) -> Result<(),()>{
    let mut buffer = Vec::new();
    let mut temp_buffer = [0u8; 1000];
    match stream.read(&mut temp_buffer) {
        // Stream closed
        Ok(0) => {
            ratatui::restore();
            eprintln!("Server closed connection");
            process::exit(-1);
        }
        Ok(n) => {
            buffer.extend_from_slice(&temp_buffer[..n]);
        }
        Err(e) => {
            eprintln!("Failed to read from server: {}", e);
        }
    }
    let response = String::from_utf8_lossy(&buffer);
    if response.starts_with("Error"){
        Err(())
    }
    else{
        Ok(())
    }

}
fn create_menu_items<'a>() -> Vec<(MenuOption, ListItem<'a>)> {
    let mut output: Vec<(MenuOption, ListItem)> = Vec::new();
    for option in MenuOption::all() {
        output.push((*option, ListItem::new(option.get_string_equivalent())))
    }
    output
}
fn create_layouts(f: &mut Frame, basic_layout: &mut Vec<Rc<[Rect]>>, view_layout: &mut Vec<Rc<[Rect]>>, input_layout: &mut Vec<Rc<[Rect]>>, path_layout: &mut Vec<Rc<[Rect]>>,
                  view_height: &mut Vec<usize>, view_width: &mut Vec<usize>) {
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
            ])
            .split(path_layout[i][1]));
        // Split layout horizontally - 20% for menu, 80% for file content
        view_layout.push(Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(80),
            ])
            .split(grid_layout[0]));
        // Save height for file scrolling
        view_height.push(view_layout[i][1].height as usize);
        view_width.push(view_layout[i][1].width as usize);

        // Layout for user input field
        input_layout.push(Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(grid_layout[1]));
    }
}
fn main() -> Result<(), String> {
    // Argument logic
    let args: Vec<String> = env::args().collect();
    if args.len() < 4{
        eprintln!("No valid option specified, choose LOCAL or REMOTE followed by server IP followed by default directory");
        return Ok(());
    }
    let program_type = args[1].clone();
    match program_type.as_str() {
        "LOCAL" =>{
            Ok(())
            /*
            match TcpStream::connect("127.0.0.1:8080") {
                Ok(mut stream) => {
                    println!("Connected to server!");

                    let mut session = Session::new().unwrap();

                    // Step 3: Handshake with the server over the TcpStream
                    session.set_tcp_stream(stream);

                    let mut channel = session.channel_session().unwrap();

                    let terminal = ratatui::init();

                    let initial_path = "/mnt/c/Users/sisin/OneDrive/Plocha/VSB-ING1";
                    let second_path = "/mnt/c/Users/sisin/OneDrive/Plocha/VSB-ING1/PvR";

                    let entries_info : Vec<Vec<String>> = get_specific_content_from_server(RequestType::GetDir, &mut channel, initial_path, Some(0), None).into();
                    let entries_info_2 : Vec<Vec<String>> = get_specific_content_from_server(RequestType::GetDir, &mut channel, second_path, Some(0), None).into();

                    let left_side = ClientLogic::new(entries_info.clone(), initial_path.to_string());
                    let right_side = ClientLogic::new(entries_info_2.clone(), second_path.to_string());

                    let mut client_logic = TwoSides::new(left_side, right_side);

                    let _ = client_logic.run(terminal, &mut channel);

                    ratatui::restore();
                    Ok(())
                }
                Err(e) => {
                    eprintln!("Failed to connect to server: {}", e);
                    Ok(())
                }
            }

             */
        }
        "REMOTE" =>{
            // Win server IP
            let server_address = format!("{}:22", args[2].clone());
            let local_port = 8080;
            let remote_ip = "localhost";
            let remote_port = 8080;

            let username = "bije";
            let password = "bije";

            // Connect to the SSH server
            let tcp = TcpStream::connect(server_address).map_err(|e| e.to_string())?;
            let mut session = Session::new().map_err(|e| e.to_string())?;
            session.set_tcp_stream(tcp);
            session.handshake().map_err(|e| e.to_string())?;

            // Authenticate with the SSH server
            session.userauth_password(username, password).map_err(|e| e.to_string())?;
            if !session.authenticated() {
                eprintln!("SSH Authentication failed");
                return Ok(());
            }
            // Set up SSH tunnel to forward data from the client through SSH to the server
            // Due to Firewall + needed port openings
            let local_address = ("127.0.0.1", local_port);
            let tunnel_connection = session.channel_direct_tcpip(remote_ip, remote_port, Some(local_address));
            match tunnel_connection {
                Ok(mut tunnel_connection) => {
                    let terminal = ratatui::init();

                    let initial_path = args[3].clone();
                    let second_path = args[3].clone();

                    let entries_info: Vec<Vec<String>> = get_specific_content_from_server(RequestType::GetDir, &mut tunnel_connection, initial_path.as_str(), Some(0), None).into();
                    let entries_info_2: Vec<Vec<String>> = get_specific_content_from_server(RequestType::GetDir, &mut tunnel_connection, second_path.as_str(), Some(0), None).into();

                    let left_side = ClientLogic::new(entries_info.clone(), initial_path.to_string());
                    let right_side = ClientLogic::new(entries_info_2.clone(), second_path.to_string());

                    let mut client_logic = TwoSides::new(left_side, right_side);

                    let _ = client_logic.run(terminal, &mut tunnel_connection);
                    ratatui::restore();
                    Ok(())
                }
                Err(e) =>{
                    eprintln!("Failed to establish SSH tunnel: {}", e);
                    Ok(())
                }
            }
        }
        _ => {
            eprintln!("No valid option specified, choose LOCAL or REMOTE");
            Ok(())
        }
    }
}
