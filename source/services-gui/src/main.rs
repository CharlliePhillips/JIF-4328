// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

//! Table API example

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

use bstr::ByteSlice;
use cosmic::app::{Core, Settings, Task};
use cosmic::iced::widget::{column, row};
use cosmic::iced_core::{Element, Size};
use cosmic::prelude::*;
use cosmic::widget::table;
use cosmic::widget::{self, nav_bar};
use cosmic::{executor, iced};
use shared::{format_uptime, get_response, SMCommand, TOMLMessage};

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
                    format!("   {}", self.pid.to_string()).into()
                }
            },
            Category::Uptime => {
                if self.uptime.0 == self.uptime.1 && self.uptime.0 == 0 {
                    "".to_string().into()
                } else {
                    format_uptime(self.uptime.0, self.uptime.1).into()
                }
            },
            Category::Msg => self.msg.clone().into(),
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
        .size(Size::new(1024., 768.));

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
    Start(String),
    Stop(String),
    NoOp,
}

/// The [`App`] stores application-specific state.
pub struct App {
    core: Core,
    table_model: table::SingleSelectModel<Item, Category>,
    selected: Option<String>,
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

        let app = App { core, table_model, selected: None, };

        let command = Task::none();

        (app, command)
    }

    /// Handle application events here.
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::ItemSelect(entity) => {
                self.table_model.activate(entity);
                /* if let Some(item) = self.table_model.get(entity) { //get not a method, need another way
                    self.selected = Some(item.name.clone());
                } else {
                    self.selected = None;
                } */
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
            Message::Refresh => {
                get_services(&mut self.table_model);
                self.selected = None;
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
                //perform refresh automatically
                get_services(&mut self.table_model);
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
                //perform refresh automatically
                get_services(&mut self.table_model);
            }
            Message::NoOp => {}
        }
        Task::none()
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<Self::Message, Theme, Renderer> {
        // by default start & stop buttons do nothing
        let mut start_msg = Message::NoOp;
        let mut stop_msg = Message::NoOp;
        match self.table_model.item(self.table_model.active()) {
            Some(selected) => {
                // if some item is selected then start and stop should operate on that
                start_msg = Message::Start(selected.name.clone());
                stop_msg = Message::Stop(selected.name.clone());
            },
            None => {}
        }
                
        let button_row = row![
            cosmic::widget::button::text("Start").on_press(start_msg),
            cosmic::widget::button::text("Stop").on_press(stop_msg),
            cosmic::widget::button::text("Refresh").on_press(Message::Refresh),
        ]
        .spacing(cosmic::theme::spacing().space_s)
        .align_y(iced::Alignment::Center);
    
        let centered = cosmic::widget::container(
            column![
                button_row,
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
            ]
            .spacing(cosmic::theme::spacing().space_s)
            .width(iced::Length::Fill)
            .align_x(iced::Alignment::Center),
        )
        .width(iced::Length::Fill)
        .height(iced::Length::Shrink)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center);
        Element::from(centered)
    }
}

fn get_services(table_model: &mut table::SingleSelectModel<Item, Category>) {
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
    let msg: TOMLMessage = toml::from_str(&response_string).expect("Error parsing UTF8 to TOMLMessage");

    match &msg {
        TOMLMessage::String(_str) => {}
        TOMLMessage::ServiceStats(stats) => {
            for s in stats {
                if s.running {
                    let _ = table_model.insert(Item {
                        name: s.name.clone(),
                        pid: s.pid,
                        uptime: (s.time_init, s.time_now),
                        msg: s.message.clone(),
                    });
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
