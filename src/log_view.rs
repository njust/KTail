use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use uuid::Uuid;
use gtk4_helper::{
    prelude::*,
    gtk,
    glib
};
use crate::util;
use gtk4_helper::gtk::{ComboBoxText, TextTag, TextTagTable, WrapMode};

use gtk4_helper::prelude::{Command, MsgHandler};
use gtk4_helper::component::Component;
use gtk4_helper::glib::SourceId;
use regex::Regex;
use sourceview5::Buffer;
use stream_cancel::Trigger;
use crate::cluster_list_view::NamespaceViewData;
use crate::config::{CONFIG};
use crate::gtk::{TextIter, ToggleButton};
use crate::log_overview::{LogOverview, LogOverviewMsg};
use crate::log_stream::LogData;
use crate::log_text_contrast::matching_foreground_color_for_background;
use crate::pod_list_view::PodViewData;

pub const SEARCH_TAG: &'static str = "SEARCH";
pub const SEARCH_COLOR: &'static str = "rgba(188,150,0,1)";

#[derive(Clone)]
pub struct SearchData {
    pub name: String,
    pub search: Regex,
}

#[derive(Clone)]
pub struct SearchResult {
    pub lines: Vec<usize>,
    pub timestamp: Option<DateTime<Utc>>,
}

impl SearchResult {
    pub fn new(timestamp: Option<DateTime<Utc>>) -> Self {
        Self {
            lines: vec![],
            timestamp,
        }
    }
}

pub struct LogView {
    container: gtk::Box,
    exit_trigger: Option<Arc<Trigger>>,
    sender: Arc<dyn MsgHandler<LogViewMsg>>,
    text_buffer: Buffer,
    text_view: sourceview5::View,
    selected_context: Option<NamespaceViewData>,
    selected_pods: Option<Vec<PodViewData>>,
    since_seconds: u32,
    active_search: Option<Regex>,
    highlighters: Vec<SearchData>,
    scroll_handler: Option<SourceId>,
    overview: ComponentContainer<LogOverview>,
    log_entry_times: Vec<DateTime<Utc>>,
}

#[derive(Clone)]
pub enum LogViewMsg {
    PodSelected(Vec<PodViewData>),
    ContextSelected(NamespaceViewData),
    Loaded(Arc<Trigger>),
    LogDataLoaded(LogData),
    EnableScroll(bool),
    WrapText(bool),
    ShowOverview(bool),
    SinceTimespanChanged(String),
    Search(String),
    SearchResult((Option<String>, HashMap<String, SearchResult>)),
    LogOverview(LogOverviewMsg),
}

impl LogView {
    fn clear(&mut self) {
        self.overview.update(LogOverviewMsg::Clear);
        self.log_entry_times.clear();

        //TODO: check if it is necessary to remove tags if we clear the buffer anyway
        let (start, end) = self.text_buffer.bounds();
        for highlighter in &self.highlighters {
            self.text_buffer.remove_tag_by_name(&highlighter.name, &start, &end);
        }

        self.text_buffer.set_text("");
        if let Some(exit) = self.exit_trigger.take() {
            drop(exit);
        }
    }

    fn scroll_to_bottom(&mut self, scroll: bool) {
        let text_view = self.text_view.clone();
        if scroll && self.scroll_handler.is_none() {
            let handle = glib::timeout_add_local(std::time::Duration::from_millis(500),move || {
                let buffer = text_view.buffer();
                let (_, mut end) = buffer.bounds();
                text_view.scroll_to_iter(&mut end, 0.0, true, 0.5, 0.5);
                glib::Continue(true)
            });
            self.scroll_handler = Some(handle);
        } else {
            if let Some(sh)  = self.scroll_handler.take() {
                glib::source::source_remove(sh);
            }
        }
    }

    #[allow(dead_code)]
    fn scroll_to_line(&mut self, line: i32) {
        let text_view = self.text_view.clone();
        glib::idle_add_local(move || {
            let buffer = text_view.buffer();
            if let Some(mut iter) = buffer.iter_at_line(line) {
                let iter_loc = text_view.iter_location(&iter);
                let visible_rect = text_view.visible_rect();

                text_view.scroll_to_iter(&mut iter, 0.0, true, 0.5, 0.5);
                if visible_rect.intersect(&iter_loc).is_none() {
                    glib::Continue(true)
                } else {
                    glib::Continue(false)
                }
            } else {
                glib::Continue(false)
            }
        });
    }

    fn get_line_offset_for_data(&mut self, data: &LogData) -> Option<TextIter> {
        data.timestamp.map(|ts| {
            if self.log_entry_times.is_empty() {
                self.log_entry_times.push(ts.clone());
                self.text_buffer.end_iter()
            } else {
                let mut idx = self.log_entry_times.len();
                for log_entry_time in self.log_entry_times.iter().rev() {
                    if ts > *log_entry_time {
                        if let Some(iter) = self.text_buffer.iter_at_line(idx as i32) {
                            self.log_entry_times.insert(idx, ts.clone());
                            return iter;
                        } else {
                            eprintln!("No iter at line: {}", idx);
                        }
                    }
                    idx = idx -1;
                }

                self.log_entry_times.insert(0, ts.clone());
                self.text_buffer.start_iter()
            }
        })
    }
}

impl Component for LogView {
    type Msg = LogViewMsg;
    type View = gtk::Box;
    type Input = ();

    fn create<T: MsgHandler<Self::Msg> + Clone>(sender: T, _: Option<Self::Input>) -> Self {
        let toolbar = gtk::BoxBuilder::new()
            .margin_start(4)
            .margin_end(4)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        let search_entry = gtk::SearchEntryBuilder::new()
            .placeholder_text("Search")
            .margin_end(4)
            .build();
        toolbar.append(&search_entry);

        let tx = sender.clone();
        search_entry.connect_activate(move |se|{
            let text = se.text().to_string();
            tx(LogViewMsg::Search(text));
        });

        let tx = sender.clone();
        search_entry.connect_changed(move |se|{
            let text = se.text().to_string();
            if text.len() <= 0 {
                tx(LogViewMsg::Search(String::new()));
            }
        });

        let auto_scroll_btn = toggle_btn(sender.clone(), "Scroll", |active| LogViewMsg::EnableScroll(active));
        toolbar.append(&auto_scroll_btn);

        let wrap_text_btn = toggle_btn(sender.clone(), "Wrap text", |active| LogViewMsg::WrapText(active));
        toolbar.append(&wrap_text_btn);

        let show_overview_btn = toggle_btn(sender.clone(), "Show overview", |active| LogViewMsg::ShowOverview(active));
        toolbar.append(&show_overview_btn);

        let since_selector = since_duration_selection(sender.clone());
        toolbar.append(&since_selector);

        let search_tag = TextTag::new(Some(SEARCH_TAG));
        search_tag.set_background(Some(SEARCH_COLOR));
        let background = search_tag.background_rgba();
        search_tag.set_foreground_rgba(matching_foreground_color_for_background(&background).as_ref());

        let tag_table = TextTagTable::new();
        tag_table.add(&search_tag);

        let buffer = sourceview5::Buffer::new(Some(&tag_table));
        let log_data_view = sourceview5::View::builder()
            .buffer(&buffer)
            .monospace(true)
            .editable(false)
            .show_line_numbers(true)
            .highlight_current_line(true)
            .hexpand(true)
            .vexpand(true)
            .build();

        let search: Vec<SearchData> = if let Ok(cfg) = CONFIG.lock() {
            for (name, highlighter)  in &cfg.highlighters {
                let tag = TextTag::new(Some(name));
                tag.set_background(Some(&highlighter.color));
                let background = tag.background_rgba();
                tag.set_foreground_rgba(matching_foreground_color_for_background(&background).as_ref());
                tag_table.add(&tag);
            }

            util::add_css_with_name(&log_data_view,
            "textview",
            &format!("#textview {{ font: {}; }}", cfg.log_view_font)
            );

            let mut search_data = vec![];
            for (name, highlighter) in &cfg.highlighters {
                if let Ok(regex) = Regex::new(&highlighter.search) {
                    search_data.push(SearchData {
                        search: regex,
                        name: name.clone(),
                    });
                }
            }
            search_data
        } else {
            vec![]
        };

        let scroll_wnd = gtk::ScrolledWindow::new();
        scroll_wnd.set_child(Some(&log_data_view));

        let tx = sender.clone();
        let overview = LogOverview::new(move |msg| {
            tx(LogViewMsg::LogOverview(msg));
        });

        overview.view().set_visible(false);

        let pane = gtk::PanedBuilder::new()
            .orientation(gtk::Orientation::Vertical)
            .start_child(overview.view())
            .end_child(&scroll_wnd)
            .position(110)
            .build();

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.append(&toolbar);
        container.append(&pane);

        Self {
            container,
            exit_trigger: None,
            sender: Arc::new(sender.clone()),
            text_buffer: buffer,
            text_view: log_data_view,
            selected_context: None,
            selected_pods: None,
            since_seconds: 60*10,
            active_search: None,
            highlighters: search,
            scroll_handler: None,
            overview,
            log_entry_times: vec![],
        }
    }

    fn update(&mut self, msg: Self::Msg) -> Command<Self::Msg> {
        match msg {
            LogViewMsg::PodSelected(pod_data) => {
                self.selected_pods = Some(pod_data.clone());
                self.clear();

                let tx = self.sender.clone();
                let ctx = self.selected_context.clone().unwrap();
                return self.run_async(load_log_stream(ctx, pod_data, tx, self.since_seconds));
            }
            LogViewMsg::Loaded(exit_tx) => {
                self.exit_trigger = Some(exit_tx);
            }
            LogViewMsg::LogDataLoaded(data) => {
                if let Some(timestamp) = &data.timestamp {
                    self.overview.update(LogOverviewMsg::LogData(timestamp.clone()));
                }


                let mut insert_at = self.get_line_offset_for_data(&data).unwrap_or_else(|| self.text_buffer.end_iter());

                self.text_buffer.insert(&mut insert_at, &format!("{} {}", data.pod, data.text));
                let mut highlighters = self.highlighters.clone();
                if let Some(query) = self.active_search.as_ref() {
                    highlighters.push(SearchData {
                        search: query.clone(),
                        name: SEARCH_TAG.to_string(),
                    });
                }

                let id = Uuid::new_v4().to_string();
                if let Some(iter) = self.text_buffer.iter_at_line(insert_at.line() - 1) {
                    self.text_buffer.add_mark(&gtk::TextMark::new(Some(&id), false), &iter);
                }

                return self.run_async(search(highlighters, data.text, data.timestamp, Some(id)));
            }
            LogViewMsg::ContextSelected(ctx) => {
                self.selected_context = Some(ctx);
            }
            LogViewMsg::EnableScroll(enable) => {
                self.scroll_to_bottom(enable);
            }
            LogViewMsg::ShowOverview(show) => {
                self.overview.view().set_visible(show);
            }
            LogViewMsg::WrapText(wrap) => {
                self.text_view.set_wrap_mode(
                    if wrap {
                        WrapMode::WordChar
                    } else {
                        WrapMode::None
                    }
                );
            }
            LogViewMsg::Search(query) => {
                let (start, end) = self.text_buffer.bounds();
                self.text_buffer.remove_tag_by_name(SEARCH_TAG, &start, &end);

                if query.len() <= 0 {
                    self.active_search.take();
                } else {
                    self.active_search = Regex::new(&format!("(?i){}", query)).ok();
                    let text = self.text_buffer.text(&start, &end, false).to_string();
                    if let Some(query) = &self.active_search {
                        return self.run_async(search(vec![ SearchData { name: SEARCH_TAG.to_string(), search: query.clone() }], text, None,None));
                    }
                }
            }
            LogViewMsg::SearchResult((marker, res)) => {
                self.overview.update(LogOverviewMsg::SearchResults(res.clone()));
                for (search, search_result) in res {
                    if let Some(mark) = &marker {
                        if let Some(start) = self.text_buffer.mark(mark).map(|m| self.text_buffer.iter_at_mark(&m)) {
                            let mut end = start.clone();
                            end.forward_to_line_end();
                            self.text_buffer.apply_tag_by_name(&search, &start, &end);
                        }
                    } else {
                        for idx in search_result.lines {
                            if let Some(start) = self.text_buffer.iter_at_line(idx as i32) {
                                let mut end = start.clone();
                                end.forward_to_line_end();
                                self.text_buffer.apply_tag_by_name(&search, &start, &end);
                            }
                        }
                    }
                }
                if let Some(marker) = marker {
                    self.text_buffer.delete_mark_by_name(&marker);
                }
            }
            LogViewMsg::SinceTimespanChanged(id) => {
                self.since_seconds = id.parse::<u32>().expect("Since seconds should be an u32");
                if let Some(pods) = self.selected_pods.as_ref()
                    .map(|pods|pods.clone())
                {
                    self.clear();
                    let tx = self.sender.clone();
                    let ctx = self.selected_context.clone().unwrap();
                    return self.run_async(load_log_stream(ctx, pods, tx, self.since_seconds));
                }
            }
            LogViewMsg::LogOverview(msg) => {
                self.overview.update(msg);
            }
        }
        Command::None
    }

    fn view(&self) -> &Self::View {
        &self.container
    }
}

async fn search(highlighters: Vec<SearchData>, text: String, timestamp: Option<DateTime<Utc>>, marker: Option<String>) -> LogViewMsg {
    let mut lines = text.lines().enumerate();
    let mut search_results = HashMap::<String, SearchResult>::new();
    while let Some((idx, line)) = lines.next() {
        for highlighter in &highlighters {
            if highlighter.search.is_match(line) {
                if !search_results.contains_key(&highlighter.name) {
                    search_results.insert(highlighter.name.clone(), SearchResult::new(timestamp.clone()));
                }

                let res = search_results.get_mut(&highlighter.name).unwrap();
                res.lines.push(idx);
                break;
            }
        }
    }
    LogViewMsg::SearchResult((marker, search_results))
}


async fn load_log_stream(ctx: NamespaceViewData, pod_data: Vec<PodViewData>, tx: Arc<dyn MsgHandler<LogViewMsg>>, since_seconds: u32) -> LogViewMsg {
    let pods = pod_data.iter().map(|pd| pd.name.clone()).collect();
    let client = crate::log_stream::k8s_client(&ctx.config_path, &ctx.context);
    let (mut log_stream, exit) = crate::log_stream::log_stream(&client, &ctx.name, &pods, since_seconds).await;
    let tx = tx.clone();
    tokio::task::spawn(async move {
        while let Some(data) = log_stream.next().await {
            tx(LogViewMsg::LogDataLoaded(data));
        }
    });
    LogViewMsg::Loaded(Arc::new(exit))
}

const SINCE_5M: u32 = 60*5;
const SINCE_10M: u32 = 60*10;
const SINCE_30M: u32 = 60*30;
const SINCE_1H: u32 = 60*60;
const SINCE_2H: u32 = 60*60*2;
const SINCE_4H: u32 = 60*60*4;
const SINCE_6H: u32 = 60*60*6;
const SINCE_8H: u32 = 60*60*8;
const SINCE_10H: u32 = 60*60*10;
const SINCE_12H: u32 = 60*60*12;
const SINCE_24H: u32 = 60*60*24;

fn since_duration_selection<T: MsgHandler<LogViewMsg>>(tx: T) -> ComboBoxText {
    let since_selector = gtk::ComboBoxTextBuilder::new()
        .build();

    since_selector.append(Some(&SINCE_5M.to_string()), "5 min");
    since_selector.append(Some(&SINCE_10M.to_string()), "10 min");
    since_selector.append(Some(&SINCE_30M.to_string()), "30 min");
    since_selector.append(Some(&SINCE_1H.to_string()), "1 hour");
    since_selector.append(Some(&SINCE_2H.to_string()), "2 hours");
    since_selector.append(Some(&SINCE_4H.to_string()), "4 hours");
    since_selector.append(Some(&SINCE_6H.to_string()), "6 hours");
    since_selector.append(Some(&SINCE_8H.to_string()), "8 hours");
    since_selector.append(Some(&SINCE_10H.to_string()), "10 hours");
    since_selector.append(Some(&SINCE_12H.to_string()), "12 hours");
    since_selector.append(Some(&SINCE_24H.to_string()), "24 hours");
    since_selector.set_active_id(Some(&SINCE_10M.to_string()));

    since_selector.connect_changed(move |a| {
        if let Some(active) =  a.active_id() {
            let active = active.to_string();
            tx(LogViewMsg::SinceTimespanChanged(active));
        }
    });

    since_selector
}

fn toggle_btn<T: MsgHandler<LogViewMsg>, M: 'static + Fn(bool) -> LogViewMsg>(tx: T, label: &str, msg: M) -> ToggleButton {
    let toggle_btn = gtk::ToggleButtonBuilder::new()
        .label(label)
        .margin_end(4)
        .build();

    toggle_btn.connect_toggled(move |btn| {
        tx(msg(btn.is_active()));
    });

    toggle_btn
}