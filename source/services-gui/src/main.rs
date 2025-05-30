// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

//! Table API example

use std::borrow::{BorrowMut, Cow};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::process::Command;
use std::sync::Arc;

use cosmic::action::cosmic;
use cosmic::app::{Core, Settings, Task};
use cosmic::cosmic_theme::ThemeBuilder;
use cosmic::iced::window::close;
use cosmic::iced::{time, Background, Border, Limits};
use cosmic::iced::Color;
use cosmic::iced::widget::{column, row};
use cosmic::iced_core::{Element, Size};
use cosmic::iced_widget::{Column, Row};
use cosmic::prelude::*;
use cosmic::widget::table::model::category;
use cosmic::widget::{table, table::Entity, Container, Text};
use cosmic::widget::{self, nav_bar};
use cosmic::{executor, iced};
use shared::{format_timestamp, format_uptime, get_response, CommandResponse, SMCommand, TOMLMessage};
use tracing_subscriber::registry::Data;

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Category {
    #[default]
    Name,
    Pid,
    Uptime,
    Msg,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Name => "Name",
            Self::Pid => "PID",
            Self::Uptime => "Uptime",
            Self::Msg => "Message",
        })
    }
}

impl table::ItemCategory for Category {
    fn width(&self) -> iced::Length {
        match self {
            Self::Name => iced::Length::Fill,
            Self::Pid => iced::Length::Fixed(100.0),
            Self::Uptime => iced::Length::Fixed(250.0),
            Self::Msg => iced::Length::Fixed(250.0),
        }
    }
}

struct Item {
    name: String,
    pid: usize,
    uptime: (i64, i64),
    msg: String,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            name: Default::default(),
            pid: Default::default(),
            uptime: Default::default(),
            msg: Default::default(),
        }
    }
}

impl table::ItemInterface<Category> for Item {
    fn get_icon(&self, category: Category) -> Option<cosmic::widget::Icon> {
        if category == Category::Name {
            Some(cosmic::widget::icon::from_name("application-x-executable-symbolic").icon())
        } else {
            None
        }
    }

    fn get_text(&self, category: Category) -> std::borrow::Cow<'static, str> {
        match category {
            Category::Name => self.name.clone().into(),
            Category::Pid => {
                if self.pid == 0 {
                    "".to_string().into()
                } else {
                    format!("{:^11}", self.pid.to_string()).into()
                }
            },
            Category::Uptime => {
                if self.uptime.0 == self.uptime.1 && self.uptime.0 == 0 {
                    "".to_string().into()
                } else {
                    let uptime_str = format_uptime(self.uptime.0, self.uptime.1);
                    format!("{:^25}", uptime_str).into()
                }
            },
            Category::Msg => {
                format!("{:^20}", self.msg.clone()).into()
            },
        }
    }

    fn compare(&self, other: &Self, category: Category) -> std::cmp::Ordering {
        match category {
            Category::Name => self.name.to_lowercase().cmp(&other.name.to_lowercase()),
            Category::Pid => self.pid.cmp(&other.pid),
            Category::Uptime => (self.uptime.1 - self.uptime.0).cmp(&(other.uptime.1 - other.uptime.0)),
            Category::Msg => self.msg.to_lowercase().cmp(&other.msg.to_lowercase()),
        }
    }
}

/// Runs application with these settings
#[rustfmt::skip]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let _ = tracing_log::LogTracer::init();

    let settings = Settings::default()
        .size(Size::new(1024., 768.))
        .size_limits(Limits::new(Size::new(640., 480.), Size::new(15360., 8640.)));

    cosmic::app::run::<App>(settings, ())?;

    Ok(())
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    ItemSelect(table::Entity),
    CategorySelect(Category),
    PrintMsg(String),
    Refresh,
    Detail,
    Start(String),
    Stop(String),
    ToPrimary,
    ToDoc,
    Log,
    NoOp,
    Tick,
}

#[derive(Clone, Debug)]
enum Screen {
    Primary,
    Doc,
}
/// The [`App`] stores application-specific state.
pub struct App {
    core: Core,
    table_model: table::SingleSelectModel<Item, Category>,
    selected: Option<String>,
    screen: Screen,
    info_table: bool,
    //info_table: Option<Container<'static, Message, Theme>>,
}

/// Implement [`cosmic::Application`] to integrate with COSMIC.
impl cosmic::Application for App {
    /// Default async executor to use with the app.
    type Executor = executor::Default;

    /// Argument received [`cosmic::Application::new`].
    type Flags = ();

    /// Message type specific to our [`App`].
    type Message = Message;

    /// The unique application ID to supply to the window manager.
    const APP_ID: &'static str = "org.cosmic.AppDemoTable";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Creates the application, and optionally emits task on initialize.
    fn init(core: Core, _: Self::Flags) -> (Self, Task<Self::Message>) {
        let mut nav_model = nav_bar::Model::default();

        nav_model.activate_position(0);

        let mut table_model = table::Model::new(vec![
            Category::Name,
            Category::Pid,
            Category::Uptime,
            Category::Msg,
        ]);

        get_services(&mut table_model);
        let screen: Screen = Screen::Primary;
        let app = App { core, table_model, selected: None, screen, info_table: false};

        let command = Task::none();

        (app, command)
    }

    /// Handle application events here.
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        //if let Some(window_id) = self.core.main_window_id() {
            let _ = self.set_header_title("services-gui".to_string());
        //}
        match message {
            Message::ItemSelect(entity) => {
                self.table_model.activate(entity);
            }
            Message::CategorySelect(category) => {
                let mut ascending = true;
                if let Some(old_sort) = self.table_model.get_sort() {
                    if old_sort.0 == category {
                        ascending = !old_sort.1;
                    }
                }
                self.table_model.sort(category, ascending)
            }
            Message::PrintMsg(string) => tracing_log::log::info!("{}", string),
            Message::Refresh => {}
            Message::Detail => {
                self.info_table = !self.info_table;
            }
            Message::Start(service_name) => {
                if let Ok(mut sm_fd) = OpenOptions::new()
                    .write(true)
                    .open("/scheme/service-monitor")
                {
                    let cmd = SMCommand::Start { service_name: service_name.clone() }.encode().unwrap();
                    let _ = sm_fd.write(&cmd);
                    tracing_log::log::info!("Started {}", service_name);
                    get_services(&mut self.table_model);
                }
                get_services(&mut self.table_model); //perform refresh automatically
            }
            Message::Stop(service_name) => {
                if let Ok(mut sm_fd) = OpenOptions::new()
                    .write(true)
                    .open("/scheme/service-monitor")
                {
                    let cmd = SMCommand::Stop { service_name: service_name.clone() }.encode().unwrap();
                    let _ = sm_fd.write(&cmd);
                    tracing_log::log::info!("Stopped {}", service_name);
                }
                get_services(&mut self.table_model); //perform refresh automatically
            }
            Message::ToPrimary => {
                self.screen = Screen::Primary;
            }
            Message::ToDoc => {
                self.screen = Screen::Doc;
            }
            Message::Log => {
                Command::new("cosmic-edit")
                    .arg("/scheme/sys/log")
                    .spawn()
                    .expect("failed to open log");
            }
            Message::Tick => {
                get_services(&mut self.table_model);
            }
            Message::NoOp => {}
        }
        Task::none()
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<Self::Message, Theme, Renderer> {
        match self.screen {
            Screen::Primary => {
                // by default start & stop buttons do nothing
                let mut start_msg = Message::NoOp;
                let mut stop_msg = Message::NoOp;
                let mut info_tbl: Option<Container<'_, Message, Theme>> = None;
                match self.table_model.item(self.table_model.active()) {
                    Some(selected) => {
                        // if some item is selected then start and stop should operate on that
                        start_msg = Message::Start(selected.name.clone());
                        stop_msg = Message::Stop(selected.name.clone());
                        if self.info_table {
                            info_tbl = get_info(selected.name.clone());
                        }
                    },
                    None => {}
                }

                let button_row = row![
                    cosmic::widget::button::text("Help").on_press(Message::ToDoc),
                    cosmic::widget::button::text("System Log").on_press(Message::Log),
                    cosmic::widget::button::text("Start").on_press(start_msg),
                    cosmic::widget::button::text("Stop").on_press(stop_msg),
                    cosmic::widget::button::text("Info").on_press(Message::Detail),
                ]
                .spacing(cosmic::theme::spacing().space_s)
                .align_y(iced::Alignment::Center);

                let centered = cosmic::widget::container(
                        cosmic::widget::responsive(|size| {
                            if size.width < 600.0 {
                                widget::compact_table(&self.table_model)
                                    .on_item_left_click(Message::ItemSelect)
                                    .item_context(|item| {
                                        Some(widget::menu::items(
                                            &HashMap::new(),
                                            vec![widget::menu::Item::Button(
                                                format!("Action on {}", item.name),
                                                None,
                                                Action::None,
                                            )],
                                        ))
                                    })
                                    .apply(Element::from)
                            } else {
                                widget::table(&self.table_model)
                                    .on_item_left_click(Message::ItemSelect)
                                    .on_category_left_click(Message::CategorySelect)
                                    .item_context(|item| {
                                        Some(widget::menu::items(
                                            &HashMap::new(),
                                            vec![widget::menu::Item::Button(
                                                format!("Action on {}", item.name),
                                                None,
                                                Action::None,
                                            )],
                                        ))
                                    })
                                    .category_context(|category| {
                                        Some(widget::menu::items(
                                            &HashMap::new(),
                                            vec![
                                                widget::menu::Item::Button(
                                                    format!("Action on {} category", category.to_string()),
                                                    None,
                                                    Action::None,
                                                ),
                                                widget::menu::Item::Button(
                                                    format!(
                                                        "Other action on {} category",
                                                        category.to_string()
                                                    ),
                                                    None,
                                                    Action::None,
                                                ),
                                            ],
                                        ))
                                    })
                                    .apply(Element::from)
                            }
                        })
                )
                .width(iced::Length::Fill)
                .height(iced::Length::Shrink)
                .align_x(iced::Alignment::Center)
                .align_y(iced::Alignment::Center);
                let body = match info_tbl {
                    Some(tbl) => {
                        cosmic::widget::container(
                            column![
                            button_row,
                                row![
                                    centered,
                                    tbl,
                                ]
                            ]
                            .spacing(cosmic::theme::spacing().space_s)
                            .width(iced::Length::Fill)
                            .height(iced::Length::Fill)
                            .align_x(iced::Alignment::Center)
                        )
                    }
                    _ => {
                        cosmic::widget::container(
                            column![
                                button_row,
                                centered,
                            ]
                            .spacing(cosmic::theme::spacing().space_s)
                            .width(iced::Length::Fill)
                            .height(iced::Length::Fill)
                            .align_x(iced::Alignment::Center)
                        )
                    }
                };
                Element::from(body)
            }

            Screen::Doc => {
                // by default start & stop buttons do nothing
                let button_row = row![
                    cosmic::widget::button::text("Back").on_press(Message::ToPrimary),
                ]
                .spacing(cosmic::theme::spacing().space_s)
                .align_y(iced::Alignment::Center);

                let centered = cosmic::widget::container(
                    column![
                        column![
                            button_row,
                        ]
                        .spacing(cosmic::theme::spacing().space_s)
                        .width(iced::Length::Fill)
                        .align_x(iced::Alignment::End),
                        column![
                            cosmic::widget::text("  1. Click on any service to select it.").size(20),
                            cosmic::widget::text("  2. With a service selected, click 'Start' or 'Stop' to start or stop the service.").size(20),
                            cosmic::widget::text("  3. With a service selected, click 'Info' to view detailed statistics on that service.").size(20),
                            cosmic::widget::text("  4. At any time click 'System Log' to view the system log.").size(20),
                            cosmic::widget::text("  5. Service statistics will be automatically updated every 3 seconds.").size(20),
                        ]
                        .spacing(cosmic::theme::spacing().space_s)
                        .width(iced::Length::Fill)
                        .align_x(iced::Alignment::Start),
                    ]
                )
                .width(iced::Length::Fill)
                .height(iced::Length::Shrink)
                .align_x(iced::Alignment::Center)
                .align_y(iced::Alignment::Center);
                Element::from(centered)
 
            }
        }        
    }
    
    fn subscription(&self) -> iced::Subscription<Self::Message> {
        time::every(time::Duration::from_secs(3)).map(|_| Message::Tick)
    }
    
}

fn get_services(table_model: &mut table::SingleSelectModel<Item, Category>) {
    let mut saved_selected: String = String::new();
    match table_model.item(table_model.active()) {
        Some(selected) => {
            saved_selected = selected.name.clone(); //save selected item before refresh
        },
        None => {}
    }

    let save_sort = table_model.get_sort();

    *table_model = table::Model::new(vec![
        Category::Name,
        Category::Pid,
        Category::Uptime,
        Category::Msg,
    ]);

    let list_cmd = SMCommand::List.encode().unwrap();

    let Ok(sm_fd) = &mut OpenOptions::new()
        .write(true)
        .open("/scheme/service-monitor")
    else {
        panic!()
    };
    let _ = File::write(sm_fd, &list_cmd);

    let response_buffer = get_response(sm_fd);
    let response_string = std::str::from_utf8(&response_buffer)
        .expect("Error parsing response to UTF8")
        .to_string();
    let response: CommandResponse = toml::from_str(&response_string)
        .expect("Error parsing CommandResponse!");

    match &response.message {
        Some(TOMLMessage::ServiceStats(stats)) => {
            for s in stats {
                if s.running {
                    if !saved_selected.is_empty() && s.name.clone() == saved_selected {
                        let _ = table_model.insert(Item {
                            name: s.name.clone(),
                            pid: s.pid,
                            uptime: (s.time_init, s.time_now),
                            msg: s.message.clone(),
                        }).activate();
                    } else {
                        let _ = table_model.insert(Item {
                            name: s.name.clone(),
                            pid: s.pid,
                            uptime: (s.time_init, s.time_now),
                            msg: s.message.clone(),
                        });
                    }
                } else {
                    if !saved_selected.is_empty() && s.name.clone() == saved_selected {
                        let _ = table_model.insert(Item {
                            name: s.name.clone(),
                            pid: 0,
                            uptime: (0,0),
                            msg: "not running".to_string(),
                        }).activate();
                    } else {
                        let _ = table_model.insert(Item {
                            name: s.name.clone(),
                            pid: 0,
                            uptime: (0,0),
                            msg: "not running".to_string(),
                        });
                    }
                }
            }
        }
        _ => {}
    }
    if save_sort != None {
        let (category, ascend) = save_sort.unwrap();
        table_model.sort(category, ascend);
    }
}

// TODO maybe this should build the whole component for the view function instead of just getting the string
// Either way needs TOML updates
fn get_info(service: String) -> Option<Container<'static, Message, Theme>> {
    let info_cmd = SMCommand::Info { service_name: service }.encode().unwrap();

    let Ok(sm_fd) = &mut OpenOptions::new()
        .write(true)
        .open("/scheme/service-monitor")
    else {
        panic!()
    };
    let _ = File::write(sm_fd, &info_cmd);

    let response_buffer = get_response(sm_fd);
    let response_string = std::str::from_utf8(&response_buffer)
        .expect("Error parsing response to UTF8")
        .to_string();
    let response: CommandResponse = toml::from_str(&response_string)
        .expect("Error parsing CommandResponse!");
    //let msg: TOMLMessage = toml::from_str(&response_string).expect("Error parsing UTF8 to TOMLMessage");

    match response.message {
        Some(TOMLMessage::ServiceDetail(service)) => {
            let mut column: Column<'static, Message, Theme, Renderer> = Column::new();
            if service.running {
                let uptime_string = format_uptime(service.time_init, service.time_now);
                let time_init_string = format_uptime(service.time_started, service.time_init);

                let name_text: Vec<String> = ["Name:".to_string(), service.name.clone()].to_vec();
                column = column.push(get_detail_row(name_text));
                let uptime_text: Vec<String> = ["Uptime:".to_string(), uptime_string.clone()].to_vec();
                column = column.push(get_detail_row(uptime_text));
                let time_init_text: Vec<String> = ["Time to init:".to_string(), time_init_string.clone()].to_vec();
                column = column.push(get_detail_row(time_init_text));
                let message_text: Vec<String> = ["Message:".to_string(), service.message.clone()].to_vec();
                column = column.push(get_detail_row(message_text));
                let message_time_text: Vec<String> = ["Message time:".to_string(), format_timestamp(service.message_time)].to_vec();
                column = column.push(get_detail_row(message_time_text));
                let read_text: Vec<String> = ["Live READ count:".to_string(), format!("{}", service.read_count), "total:".to_string(), format!("{}", service.total_reads)].to_vec();
                column = column.push(get_detail_row(read_text));
                let write_text: Vec<String> = ["Live WRITE count:".to_string(), format!("{}", service.write_count), "total:".to_string(), format!("{}", service.total_writes)].to_vec();
                column = column.push(get_detail_row(write_text));
                let open_text: Vec<String> = ["Live OPEN count:".to_string(), format!("{}", service.open_count), "total:".to_string(), format!("{}", service.total_opens)].to_vec();
                column = column.push(get_detail_row(open_text));
                let close_text: Vec<String> = ["Live CLOSE count:".to_string(), format!("{}", service.close_count), "total:".to_string(), format!("{}", service.total_closes)].to_vec();
                column = column.push(get_detail_row(close_text));
                let dup_text: Vec<String> = ["Live DUP count:".to_string(), format!("{}", service.dup_count), "total:".to_string(), format!("{}", service.total_dups)].to_vec(); 
                column = column.push(get_detail_row(dup_text));
                let error_text: Vec<String> = ["Live ERROR count:".to_string(), format!("{}", service.error_count), "total:".to_string(), format!("{}", service.total_errors)].to_vec(); 
                column = column.push(get_detail_row(error_text));
            } else {
                let name_text: Vec<String> = ["Name:".to_string(), service.name.clone()].to_vec();
                column = column.push(get_detail_row(name_text));

                let message_text: Vec<String> = ["Message:".to_string(), service.message.clone()].to_vec();
                column = column.push(get_detail_row(message_text));
                let message_time_text: Vec<String> = ["Message time:".to_string(), format_timestamp(service.message_time)].to_vec();
                column = column.push(get_detail_row(message_time_text));
                let read_text: Vec<String> = ["Total READ count:".to_string(), format!("{}", service.total_reads)].to_vec();
                column = column.push(get_detail_row(read_text));
                let write_text: Vec<String> = ["Total WRITE count:".to_string(), format!("{}", service.total_writes)].to_vec();
                column = column.push(get_detail_row(write_text));
                let open_text: Vec<String> = ["Total OPEN count:".to_string(), format!("{}", service.total_opens)].to_vec();
                column = column.push(get_detail_row(open_text));
                let close_text: Vec<String> = ["Total CLOSE count:".to_string(), format!("{}", service.total_closes)].to_vec();
                column = column.push(get_detail_row(close_text));
                let dup_text: Vec<String> = ["Total DUP count:".to_string(), format!("{}", service.total_dups)].to_vec(); 
                column = column.push(get_detail_row(dup_text));
                let error_text: Vec<String> = ["Total ERROR count:".to_string(), format!("{}", service.total_errors)].to_vec(); 
                column = column.push(get_detail_row(error_text));
            }
            Some(
                cosmic::widget::container(
                    column,
                )                
                .style(|_theme| {
                    //TODO: theme this color
                    widget::container::Style {
                        background: Some(Background::Color(Color::from_rgba8(
                            0x40, 0x40, 0x40, 0.5
                        ))),
                        ..Default::default()
                    }
                })
                .width(iced::Length::Fill)
                .height(iced::Length::Shrink)

                .into()
            )
        }
        _ => {None}
    }
}

fn get_detail_row(strings: Vec<String>) -> Row<'static, Message, Theme, Renderer>{
    let mut row: Row<'static, Message, Theme, Renderer> = Row::new();
    // let static_strings: Vec<String> = strings.clone();
    for (i, string) in strings.iter().enumerate() {
        if i % 2 == 0 {
            let title: Container<'static, Message, Theme> = cosmic::widget::container(
        cosmic::widget::text(format!(" {}", string.clone())).size(20),
            )
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .style(|_theme| {
                //TODO: theme this color
                widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba8(
                        0x00, 0x00, 0x00, 0.5
                    ))),
                    border: Border::default().color(Color::WHITE).width(1),
                    text_color: Some(Color::WHITE),
                    ..Default::default()
                }
            });
            row = row.push(title);
        } else {
            let data: Container<'static, Message, Theme> = cosmic::widget::container(
                cosmic::widget::text(format!(" {}", string.clone())).size(20),
            )
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .style(|_theme| {
                //TODO: theme this color
                widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba8(
                        0x40, 0x40, 0x44, 0.5
                    ))),
                    border: Border::default().color(Color::WHITE).width(1),
                    ..Default::default()
                }
            });
            row = row.push(data);
        }
    }

    //row = row.height(iced::Length::Shrink);
    return row;
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    None,
}

impl widget::menu::Action for Action {
    type Message = Message;

    fn message(&self) -> Self::Message {
        Message::NoOp
    }
}
