// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

//! Table API example

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

use bstr::ByteSlice;
use cosmic::app::{Core, Settings, Task};
use cosmic::iced::widget::column;
use cosmic::iced_core::{Element, Size};
use cosmic::prelude::*;
use cosmic::widget::table;
use cosmic::widget::{self, nav_bar};
use cosmic::{executor, iced};
use shared::SMCommand;

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
    pid: String,
    uptime: String,
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
            Category::Pid => self.pid.to_string().into(),
            Category::Uptime => self.uptime.clone().into(),
            Category::Msg => self.msg.clone().into(),
        }
    }

    fn compare(&self, other: &Self, category: Category) -> std::cmp::Ordering {
        match category {
            Category::Name => self.name.to_lowercase().cmp(&other.name.to_lowercase()),
            Category::Pid => self.pid.cmp(&other.pid),
            Category::Uptime => self.uptime.cmp(&other.uptime),
            Category::Msg => self.msg.cmp(&other.msg),
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
    NoOp,
}

/// The [`App`] stores application-specific state.
pub struct App {
    core: Core,
    table_model: table::SingleSelectModel<Item, Category>,
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

        let list_cmd = SMCommand::List.encode().unwrap();

        let Ok(sm_fd) = &mut OpenOptions::new()
            .write(true)
            .open("/scheme/service-monitor")
        else {
            panic!()
        };
        let _ = File::write(sm_fd, &list_cmd);

        let mut response_buffer = vec![0u8; 1024]; // 1024 is kinda arbitrary here, may cause issues later
        let size = File::read(sm_fd, &mut response_buffer)
            .expect("Failed to read PIDs from service monitor");
        response_buffer.truncate(size);

        let mut lines: Vec<&[u8]> = response_buffer.lines().collect();
        lines.swap_remove(0);
        for line in lines {
            let name_idx = line.iter().position(|c| *c == b'|').unwrap();
            let pid_idx = name_idx
                + 1
                + line[name_idx + 1..]
                    .iter()
                    .position(|c| *c == b'|')
                    .unwrap();
            let uptime_idx =
                pid_idx + 1 + line[pid_idx + 1..].iter().position(|c| *c == b'|').unwrap();
            let msg_idx = uptime_idx
                + 1
                + line[uptime_idx + 1..]
                    .iter()
                    .position(|c| *c == b'|')
                    .unwrap();

            let _ = table_model.insert(Item {
                name: line[0..name_idx].to_str().unwrap().to_string(),
                pid: line[name_idx + 1..pid_idx - 1]
                    .to_str()
                    .unwrap()
                    .to_string(),
                uptime: line[pid_idx + 1..uptime_idx - 1]
                    .to_str()
                    .unwrap()
                    .to_string(),
                msg: line[uptime_idx + 1..msg_idx - 1]
                    .to_str()
                    .unwrap()
                    .to_string(),
            });
        }

        let app = App { core, table_model };

        let command = Task::none();

        (app, command)
    }

    /// Handle application events here.
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::ItemSelect(entity) => self.table_model.activate(entity),
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
            }
            Message::NoOp => {}
        }
        Task::none()
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<Self::Message, Theme, Renderer> {
        let centered = cosmic::widget::container(
            column![
                cosmic::widget::button::text("Refresh").on_press(Message::Refresh),
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

    let mut response_buffer = vec![0u8; 1024]; // 1024 is kinda arbitrary here, may cause issues later
    let size =
        File::read(sm_fd, &mut response_buffer).expect("Failed to read PIDs from service monitor");
    response_buffer.truncate(size);

    let mut lines: Vec<&[u8]> = response_buffer.lines().collect();
    lines.swap_remove(0);
    for line in lines {
        let name_idx = line.iter().position(|c| *c == b'|').unwrap();
        let pid_idx = name_idx + 1
            + line[name_idx + 1..]
                .iter()
                .position(|c| *c == b'|')
                .unwrap();
        let uptime_idx = pid_idx + 1 + line[pid_idx + 1..].iter().position(|c| *c == b'|').unwrap();
        let msg_idx = uptime_idx + 1
            + line[uptime_idx + 1..]
                .iter()
                .position(|c| *c == b'|')
                .unwrap();

        let _ = table_model.insert(Item {
            name: line[0..name_idx].to_str().unwrap().to_string(),
            pid: line[name_idx + 1..pid_idx - 1]
                .to_str()
                .unwrap()
                .to_string(),
            uptime: line[pid_idx + 1..uptime_idx - 1]
                .to_str()
                .unwrap()
                .to_string(),
            msg: line[uptime_idx + 1..msg_idx - 1]
                .to_str()
                .unwrap()
                .to_string(),
        });
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
