use std::rc::Rc;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use uuid::Uuid;
use gtk4_helper::{
    prelude::*,
    gtk,
    glib,
    gio
};
use crate::util;
use gtk4_helper::gtk::{ComboBoxText, TextTag, TextTagTable, WrapMode};

use gtk4_helper::prelude::{Command, MsgHandler};
use gtk4_helper::component::Component;
use gtk4_helper::gio::SimpleActionGroup;
use gtk4_helper::glib::SourceId;
use regex::Regex;
use sourceview5::Buffer;
use stream_cancel::Trigger;
use tokio_stream::wrappers::IntervalStream;
use crate::cluster_list_view::NamespaceViewData;
use crate::config::{CONFIG};
use crate::gtk::{TextIter, ToggleButton};
use crate::log_overview::{LogOverview, LogOverviewMsg};
use crate::log_stream::LogData;
use crate::log_text_contrast::matching_foreground_color_for_background;
use crate::pod_list_view::PodViewData;
use crate::util::search_offset;

pub const SEARCH_TAG: &'static str = "SEARCH";
pub const SEARCH_COLOR: &'static str = "rgba(188,150,0,0.7)";

pub const SELECTED_SEARCH_TAG: &'static str = "SELECTED_SEARCH";
pub const SELECTED_SEARCH_COLOR: &'static str = "rgba(188,150,0,1)";
const SCROLL_TO_LINE_MARKER: &'static str = "SCROLL_TO_LINE_MARKER";

pub const DEFAULT_MARGIN: i32 = 4;

#[derive(Clone)]
pub struct SearchData {
    pub name: String,
    pub search: Regex,
}

#[derive(Clone)]
pub struct SearchResultData {
    pub lines: Vec<usize>,
}

impl SearchResultData {
    pub fn new() -> Self {
        Self {
            lines: vec![],
        }
    }
}

#[derive(Clone)]
pub struct HighlightResultData {
    pub text_marker_id: String,
    pub timestamp: DateTime<Utc>,
    pub matching_highlighters: Vec<String>,
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
    settings: Settings,
    search_match_markers: Vec<String>,
    search_results_lbl: gtk::Label,
    current_search_match_pos: Option<usize>,
    worker_action: std::sync::mpsc::Sender<WorkerData>,
    settings_obj: glib::Object
}

#[derive(Clone)]
pub enum LogViewMsg {
    PodSelected(Vec<PodViewData>),
    ContextSelected(NamespaceViewData),
    Loaded(Arc<Trigger>),
    LogDataLoaded(Vec<LogData>),
    LogDataProcessed(Vec<(i64, LogData)>),
    EnableScroll(bool),
    ToggleWrapText,
    ToggleShowContainerNames,
    ToggleShowPodNames,
    ToggleShowTimestamps,
    SinceTimespanChanged(String),
    Search(String),
    SearchResult(SearchResultData),
    HighlightResult(HighlightResultData),
    LogOverview(LogOverviewMsg),
    SelectNextSearchMatch,
    SelectPrevSearchMatch,
    ScrollToLine(i64),
}

impl LogView {
    fn clear(&mut self) {
        self.overview.update(LogOverviewMsg::Clear);
        self.clear_search_markers();
        if let Err(e) = self.worker_action.send(WorkerData::Clear) {
            log::error!("Could not send msg to worker: {}", e);
        }

        let (start, end) = self.text_buffer.bounds();
        for highlighter in &self.highlighters {
            self.text_buffer.remove_tag_by_name(&highlighter.name, &start, &end);
        }

        self.text_buffer.set_text("");
        if let Some(exit) = self.exit_trigger.take() {
            drop(exit);
        }
    }

    fn update_search_label(&self) {
        if self.search_match_markers.len() <= 0 {
            self.search_results_lbl.set_label("");
        } else {
            self.search_results_lbl.set_label(&format!("{} matches", self.search_match_markers.len()));
        }
    }

    fn add_search_marker(&mut self, pos: &TextIter) {
        let marker_id = Uuid::new_v4().to_string();
        self.text_buffer.add_mark(&gtk::TextMark::new(Some(&marker_id), false), &pos);
        self.search_match_markers.push(marker_id);
    }

    fn clear_search_markers(&mut self) {
        for search_match_marker in &self.search_match_markers {
            self.text_buffer.delete_mark_by_name(&search_match_marker);
        }
        self.search_match_markers.clear();
        self.search_results_lbl.set_label("");
        self.current_search_match_pos.take();
        self.clear_active_search_highlight();
    }

    fn clear_active_search_highlight(&mut self) {
        let (start, end) = self.text_buffer.bounds();
        self.text_buffer.remove_tag_by_name(SELECTED_SEARCH_TAG, &start, &end);
    }

    fn highlight_search_at_pos(&mut self, pos: usize) {
        if let Some(next_marker) = self.search_match_markers.get(pos).map(|m| m.to_string()) {
            if let Some(marker) = self.text_buffer.mark(&next_marker) {
                self.scroll_to_mark(&next_marker);
                self.clear_active_search_highlight();
                let line_start = self.text_buffer.iter_at_mark(&marker);
                let mut line_end = line_start.clone();
                line_end.forward_to_line_end();
                self.text_buffer.apply_tag_by_name(SELECTED_SEARCH_TAG, &line_start, &line_end);
                self.current_search_match_pos = Some(pos);
            }
        }
    }

    fn reload(&mut self) -> Command<LogViewMsg> {
        if let Some(pods) = self.selected_pods.as_ref()
            .map(|pods|pods.clone())
        {
            self.clear();
            let tx = self.sender.clone();
            let ctx = self.selected_context.clone().unwrap();
            return self.run_async(load_log_stream(ctx, pods, tx, self.since_seconds));
        }
        Command::None
    }

    fn scroll_to_bottom(&mut self, scroll: bool) {
        let text_view = self.text_view.clone();
        if scroll && self.scroll_handler.is_none() {
            let handle = glib::timeout_add_local(std::time::Duration::from_millis(500),move || {
                let buffer = text_view.buffer();
                let (_, mut end) = buffer.bounds();
                text_view.scroll_to_iter(&mut end, 0.0, true, 0.5, 0.5);
                Continue(true)
            });
            self.scroll_handler = Some(handle);
        } else {
            if let Some(_sh)  = self.scroll_handler.take() {
                // TODO: glib::source::source_remove was removed..
                // glib::source::source_remove(sh);
            }
        }
    }

    fn scroll_to_mark(&mut self, mark: &str) {
        let text_view = self.text_view.clone();
        let mark = mark.to_string();
        glib::idle_add_local(move || {
            let buffer = text_view.buffer();
            if let Some(mut iter) = buffer.mark(&mark).map(|mark|buffer.iter_at_mark(&mark)) {
                let iter_loc = text_view.iter_location(&iter);
                let visible_rect = text_view.visible_rect();

                text_view.scroll_to_iter(&mut iter, 0.0, true, 0.5, 0.5);
                if visible_rect.intersect(&iter_loc).is_none() {
                    Continue(true)
                } else {
                    Continue(false)
                }
            } else {
                Continue(false)
            }
        });
    }
}

enum WorkerData {
    ProcessLogData(Vec<LogData>),
    ProcessHighlighters(Vec<SearchData>, LogData, String),
    Clear,
    GetOffsetForTimestamp(i64),
}

use gtk4_helper::model::prelude::*;

// macro attributes in `#[derive]` output are unstable
// https://github.com/rust-lang/rust/issues/81119
#[model]
struct Settings {
    #[field]
    wrap_text: bool,
    #[field]
    show_pod_names: bool,
    #[field]
    show_container_names: bool,
    #[field]
    show_timestamps: bool,
}

impl Component for LogView {
    type Msg = LogViewMsg;
    type View = gtk::Box;
    type Input = Rc<SimpleActionGroup>;

    fn create<T: MsgHandler<Self::Msg> + Clone>(sender: T, input: Option<Self::Input>) -> Self {
        let global_actions = input.expect("Should input global actions");
        let settings = CONFIG.lock().map(|cfg| Settings {
            show_timestamps: cfg.log_view_settings.show_timestamps,
            show_container_names: cfg.log_view_settings.show_container_names,
            show_pod_names: cfg.log_view_settings.show_pod_names,
            wrap_text: cfg.log_view_settings.wrap_text
        }).unwrap_or(Settings::default());

        let toolbar = gtk::builders::BoxBuilder::new()
            .margin_start(4)
            .margin_end(4)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        let settings_obj = settings.to_object();
        add_log_view_settings_menu(global_actions.clone(), &toolbar, &settings_obj, sender.clone());

        let auto_scroll_btn = toggle_btn(sender.clone(), "Scroll", |active| LogViewMsg::EnableScroll(active));
        global_actions.add_action(&gio::PropertyAction::new("scroll", &auto_scroll_btn, "active"));
        toolbar.append(&auto_scroll_btn);

        let since_selector = since_duration_selection(sender.clone());
        toolbar.append(&since_selector);

        let search_results_lbl = add_search_toolbar(global_actions.clone(), &toolbar, sender.clone());

        let search_tag = TextTag::new(Some(SEARCH_TAG));
        search_tag.set_background(Some(SEARCH_COLOR));
        let background = search_tag.background_rgba();
        search_tag.set_foreground_rgba(matching_foreground_color_for_background(&background).as_ref());

        let selected_search_tag = TextTag::new(Some(SELECTED_SEARCH_TAG));
        selected_search_tag.set_background(Some(SELECTED_SEARCH_COLOR));
        let background = selected_search_tag.background_rgba();
        selected_search_tag.set_foreground_rgba(matching_foreground_color_for_background(&background).as_ref());

        let tag_table = TextTagTable::new();
        tag_table.add(&search_tag);
        tag_table.add(&selected_search_tag);

        let buffer = sourceview5::Buffer::new(Some(&tag_table));
        let log_data_view = sourceview5::View::builder()
            .buffer(&buffer)
            .monospace(true)
            .editable(false)
            .show_line_numbers(true)
            .highlight_current_line(true)
            .wrap_mode(
                if settings.wrap_text {
                    WrapMode::WordChar
                } else {
                    WrapMode::None
                }
            )
            .hexpand(true)
            .vexpand(true)
            .build();

        let search: Vec<SearchData> = if let Ok(cfg) = CONFIG.lock() {
            for highlighter  in &cfg.highlighters {
                let tag = TextTag::new(Some(&highlighter.name));
                tag.set_background(Some(&highlighter.color));
                let background = tag.background_rgba();
                tag.set_foreground_rgba(matching_foreground_color_for_background(&background).as_ref());
                tag_table.add(&tag);
            }

            util::add_css_with_name(&log_data_view,
            "textview",
            &format!("#textview {{ font: {}; }}", cfg.log_view_settings.font)
            );

            let mut search_data = vec![];
            for highlighter in &cfg.highlighters {
                if let Ok(regex) = Regex::new(&highlighter.search) {
                    search_data.push(SearchData {
                        search: regex,
                        name: highlighter.name.clone(),
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

        let pane = gtk::builders::PanedBuilder::new()
            .orientation(gtk::Orientation::Vertical)
            .start_child(overview.view())
            .end_child(&scroll_wnd)
            .position(110)
            .build();

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.append(&toolbar);
        container.append(&pane);

        let (w_tx, w_rx) = std::sync::mpsc::channel::<WorkerData>();
        let tx = sender.clone();
        std::thread::spawn(move || {
            let mut log_entry_times: Vec<i64> = vec![];
            while let Ok(data) = w_rx.recv() {
                match data {
                    WorkerData::ProcessLogData(data) => {
                        let mut res = vec![];
                        for datum in data {
                            let timestamp = datum.timestamp.timestamp_nanos();
                            let mut offset = search_offset(&log_entry_times, timestamp);
                            let len = log_entry_times.len();
                            while offset < len && log_entry_times[offset] == timestamp {
                                offset += 1;
                            }
                            log_entry_times.insert(offset, timestamp);
                            // We need to insert a extra entry for lines starting with a linefeed or a new line
                            if datum.text.starts_with("\r") || datum.text.starts_with("\n") {
                                // Sourceview seems to ignore those
                                if datum.text != "\r\n" && datum.text != "\n" {
                                    log_entry_times.insert(offset, timestamp);
                                }
                            }
                            res.push((offset as i64, datum));
                        }

                        tx(LogViewMsg::LogDataProcessed(res))
                    }
                    WorkerData::Clear => {
                        log_entry_times.clear();
                    }
                    WorkerData::ProcessHighlighters(highlighters, data, text_marker_id) => {
                        let mut res = HighlightResultData {
                            text_marker_id,
                            timestamp: data.timestamp,
                            matching_highlighters: vec![]
                        };

                        for highlighter in highlighters {
                            if highlighter.search.is_match(&data.text) {
                                res.matching_highlighters.push(highlighter.name)
                            }
                        }
                        tx(LogViewMsg::HighlightResult(res));
                    }
                    WorkerData::GetOffsetForTimestamp(timestamp) => {
                        let ts = timestamp * 1000 * 1000 * 1000; // Seconds to nanoseconds
                        let offset = search_offset(&log_entry_times, ts);
                        tx(LogViewMsg::ScrollToLine(offset as i64));
                    }
                }
            }
        });

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
            search_match_markers: vec![],
            search_results_lbl,
            current_search_match_pos: None,
            worker_action: w_tx,
            settings,
            settings_obj
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
                let timestamps: Vec<DateTime<Utc>> = data.iter().map(|d| d.timestamp.clone()).collect();
                self.overview.update(LogOverviewMsg::LogData(timestamps));
                if let Err(e) = self.worker_action.send(WorkerData::ProcessLogData(data)) {
                    eprint!("Could not send msg to worker: {}", e);
                }
            }
            LogViewMsg::LogDataProcessed(res) => {
                for (idx, data) in res {
                    if let Some(mut insert_at) = self.text_buffer.iter_at_line(idx as i32) {
                        let mut log_line = String::new();
                        if self.settings.show_pod_names {
                            log_line.push_str(&data.pod)
                        }
                        if self.settings.show_container_names {
                            log_line.push_str(&format!(" {}", data.container))
                        }
                        if self.settings.show_timestamps {
                            log_line.push_str(&format!(" {}", data.timestamp))
                        }

                        log_line.push_str(&format!(" {}", data.text));
                        self.text_buffer.insert(&mut insert_at, &log_line);

                        let mut highlighters = self.highlighters.clone();
                        if let Some(query) = self.active_search.as_ref() {
                            highlighters.push(SearchData {
                                search: query.clone(),
                                name: SEARCH_TAG.to_string(),
                            });
                        }

                        let text_marker_id = Uuid::new_v4().to_string();
                        if let Some(iter) = self.text_buffer.iter_at_line(insert_at.line() - 1) {
                            self.text_buffer.add_mark(&gtk::TextMark::new(Some(&text_marker_id), false), &iter);
                        }

                        if let Err(e) = self.worker_action.send(WorkerData::ProcessHighlighters(highlighters, data, text_marker_id)) {
                            log::error!("Could not send msg to worker: {}", e);
                        }
                    } else {
                        log::error!("No iter at line: {}", idx);
                    }
                }
            }
            LogViewMsg::HighlightResult(res) => {
                self.overview.update(LogOverviewMsg::HighlightResults(res.clone()));
                for highlighter_name in res.matching_highlighters {
                    if let Some(start) = self.text_buffer.mark(&res.text_marker_id).map(|m| self.text_buffer.iter_at_mark(&m)) {
                        let mut end = start.clone();
                        end.forward_to_line_end();
                        self.text_buffer.apply_tag_by_name(&highlighter_name, &start, &end);

                        if &highlighter_name == SEARCH_TAG {
                            self.add_search_marker(&start);
                        }
                    }
                }
                self.update_search_label();
                self.text_buffer.delete_mark_by_name(&res.text_marker_id);
            }
            LogViewMsg::Search(query) => {
                let (start, end) = self.text_buffer.bounds();
                self.text_buffer.remove_tag_by_name(SEARCH_TAG, &start, &end);
                self.clear_search_markers();

                if query.len() <= 0 {
                    self.active_search.take();
                } else {
                    self.active_search = Regex::new(&format!("(?i){}", query)).ok();
                    let text = self.text_buffer.text(&start, &end, false).to_string();
                    if let Some(query) = &self.active_search {
                        return self.run_async(search( query.clone(), text));
                    }
                }
            }
            LogViewMsg::SearchResult(res) => {
                for idx in res.lines {
                    if let Some(start) = self.text_buffer.iter_at_line(idx as i32) {
                        self.add_search_marker(&start);
                        let mut end = start.clone();
                        end.forward_to_line_end();
                        self.text_buffer.apply_tag_by_name(SEARCH_TAG, &start, &end);
                    }
                }
                self.update_search_label();

            }
            LogViewMsg::ContextSelected(ctx) => {
                self.selected_context = Some(ctx);
            }
            LogViewMsg::EnableScroll(enable) => {
                self.scroll_to_bottom(enable);
            }
            LogViewMsg::ToggleShowContainerNames => {
                let settings: Settings = Settings::from_object(&self.settings_obj);
                self.settings.show_container_names = settings.show_container_names;
                if let Ok(mut cfg) = CONFIG.lock() {
                    cfg.log_view_settings.show_container_names = settings.show_container_names;
                }
                return self.reload();
            }
            LogViewMsg::ToggleShowPodNames => {
                let settings: Settings = Settings::from_object(&self.settings_obj);
                self.settings.show_pod_names = settings.show_pod_names;
                if let Ok(mut cfg) = CONFIG.lock() {
                    cfg.log_view_settings.show_pod_names = settings.show_pod_names;
                }
                return self.reload();
            }
            LogViewMsg::ToggleShowTimestamps => {
                let settings: Settings = Settings::from_object(&self.settings_obj);
                self.settings.show_timestamps = settings.show_timestamps;
                if let Ok(mut cfg) = CONFIG.lock() {
                    cfg.log_view_settings.show_timestamps = settings.show_timestamps;
                }
                return self.reload();
            }
            LogViewMsg::ToggleWrapText => {
                let settings: Settings = Settings::from_object(&self.settings_obj);
                self.settings.wrap_text = settings.wrap_text;
                if let Ok(mut cfg) = CONFIG.lock() {
                    cfg.log_view_settings.wrap_text = settings.wrap_text;
                }
                self.text_view.set_wrap_mode(
                    if settings.wrap_text {
                        WrapMode::WordChar
                    } else {
                        WrapMode::None
                    }
                );
            }
            LogViewMsg::SelectNextSearchMatch => {
                let next_pos = self.current_search_match_pos.map(|current|{
                    if current == self.search_match_markers.len() -1 {
                        0
                    } else {
                        current + 1
                    }
                }).unwrap_or(0);
                self.highlight_search_at_pos(next_pos);
            }
            LogViewMsg::SelectPrevSearchMatch => {
                let next_pos = self.current_search_match_pos.map(|current|{
                    if current == 0 {
                        self.search_match_markers.len() -1
                    } else {
                        current -1
                    }
                }).unwrap_or_else(||{
                    let len = self.search_match_markers.len();
                    if len > 0 {
                        self.search_match_markers.len() -1
                    } else {
                        0
                    }
                });
                self.highlight_search_at_pos(next_pos);
            }
            LogViewMsg::SinceTimespanChanged(id) => {
                self.since_seconds = id.parse::<u32>().expect("Since seconds should be an u32");
                return self.reload();
            }
            LogViewMsg::LogOverview(msg) => {
                if let LogOverviewMsg::MouseClick((timestamp, _)) = &msg {
                    if let Err(e) = self.worker_action.send(WorkerData::GetOffsetForTimestamp(*timestamp)) {
                        log::error!("Could not send msg: {}", e);
                    }
                }
                self.overview.update(msg);
            }
            LogViewMsg::ScrollToLine(idx) => {
                if let Some(iter) = self.text_buffer.iter_at_line(idx as i32) {
                    if let Some(m) = self.text_buffer.mark(SCROLL_TO_LINE_MARKER) {
                        self.text_buffer.delete_mark(&m);
                    }
                    self.text_buffer.add_mark(&gtk::TextMark::new(Some(SCROLL_TO_LINE_MARKER), false), &iter);
                    self.scroll_to_mark(SCROLL_TO_LINE_MARKER);
                }
            }
        }
        Command::None
    }

    fn view(&self) -> &Self::View {
        &self.container
    }
}

async fn search(query: Regex, text: String) -> LogViewMsg {
    let mut lines = text.lines();
    let mut search_results = SearchResultData::new();
    let mut idx = 0;
    while let Some(line) = lines.next() {
        // Some log data contained \r without \n as new line
        // Sourceview handles it as a new line anyway
        let sub_lines = line.split("\r");
        for sub_line in sub_lines {
            if query.is_match(sub_line) {
                search_results.lines.push(idx);
            }
            idx += 1;
        }
    }
    LogViewMsg::SearchResult(search_results)
}

async fn load_log_stream(ctx: NamespaceViewData, pods: Vec<PodViewData>, tx: Arc<dyn MsgHandler<LogViewMsg>>, since_seconds: u32) -> LogViewMsg {
    let client = crate::log_stream::k8s_client(&ctx.config_path, &ctx.context);
    let (log_stream, exit) = crate::log_stream::log_stream(&client, &ctx.name, pods, since_seconds).await;
    let tx = tx.clone();
    tokio::task::spawn(async move {
        // Throttle the stream to keep the ui responsive.
        let mut throttled_stream = StreamExt::zip(
            StreamExt::ready_chunks(log_stream, 1000),
            IntervalStream::new(tokio::time::interval(std::time::Duration::from_millis(50)))
        );
        while let Some((data, _)) = throttled_stream.next().await {
            tx(LogViewMsg::LogDataLoaded(data));
        }
    });
    LogViewMsg::Loaded(Arc::new(exit))
}

fn add_search_toolbar<T: MsgHandler<LogViewMsg> + Clone>(global_actions: Rc<SimpleActionGroup>,toolbar: &gtk::Box, sender: T) -> gtk::Label {
    let search_entry = gtk::builders::SearchEntryBuilder::new()
        .placeholder_text("Search")
        .margin_end(DEFAULT_MARGIN)
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

    let action = gio::SimpleAction::new("search", None);
    action.connect_activate(move |_,_|{
        search_entry.grab_focus();
    });
    global_actions.add_action(&action);

    let prev_match_btn = gtk::builders::ButtonBuilder::new()
        .label("Previous")
        .margin_end(DEFAULT_MARGIN)
        .build();

    let tx = sender.clone();
    prev_match_btn.connect_clicked(move |_| {
        tx(LogViewMsg::SelectPrevSearchMatch);
    });
    toolbar.append(&prev_match_btn);
    let action = gio::SimpleAction::new("prevMatch", None);
    action.connect_activate(move |_,_|{
        prev_match_btn.activate();
    });
    global_actions.add_action(&action);

    let next_match_btn = gtk::builders::ButtonBuilder::new()
        .label("Next")
        .margin_end(DEFAULT_MARGIN)
        .build();

    let tx = sender.clone();
    next_match_btn.connect_clicked(move |_| {
        tx(LogViewMsg::SelectNextSearchMatch);
    });
    toolbar.append(&next_match_btn);
    let action = gio::SimpleAction::new("nextMatch", None);
    action.connect_activate(move |_,_|{
        next_match_btn.activate();
    });
    global_actions.add_action(&action);

    let search_results_lbl = gtk::Label::new(None);
    toolbar.append(&search_results_lbl);
    search_results_lbl
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
    let since_selector = gtk::builders::ComboBoxTextBuilder::new()
        .margin_end(DEFAULT_MARGIN)
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

fn add_log_view_settings_menu<T: MsgHandler<LogViewMsg> + Clone>(action_group: Rc<SimpleActionGroup>, toolbar: &gtk::Box, settings_obj: &glib::Object, sender: T) {
    let menu = gio::Menu::new();
    menu.append(Some("Wrap lines"), Some("app.toggleWrapText"));
    menu.append(Some("Show pod names"), Some("app.showPodNames"));
    menu.append(Some("Show container names "), Some("app.showContainerNames"));
    menu.append(Some("Show timestamps"), Some("app.showTimestamps"));

    let menu_btn =gtk::builders::MenuButtonBuilder::new()
        .icon_name("emblem-system-symbolic")
        .menu_model(&menu)
        .margin_end(DEFAULT_MARGIN)
        .build();

    add_property_action(&action_group, "toggleWrapText", settings_obj, Settings::wrap_text, || LogViewMsg::ToggleWrapText, sender.clone());
    add_property_action(&action_group, "showContainerNames", settings_obj, Settings::show_container_names, || LogViewMsg::ToggleShowContainerNames, sender.clone());
    add_property_action(&action_group, "showTimestamps", settings_obj, Settings::show_timestamps, || LogViewMsg::ToggleShowTimestamps, sender.clone());
    add_property_action(&action_group, "showPodNames", settings_obj, Settings::show_pod_names, || LogViewMsg::ToggleShowPodNames, sender.clone());
    toolbar.append(&menu_btn);
}

fn add_property_action<T: MsgHandler<LogViewMsg> + Clone, M: 'static + Fn() -> LogViewMsg>(
    action_group: &gio::SimpleActionGroup,
    name: &str,
    settings_obj: &glib::Object,
    property_name: &str,
    msg: M,
    tx: T
) {
    let action = gio::PropertyAction::new(name, settings_obj, property_name);
    action.connect_state_notify(move |_| {
        tx(msg());
    });
    action_group.add_action(&action);
}

fn toggle_btn<T: MsgHandler<LogViewMsg>, M: 'static + Fn(bool) -> LogViewMsg>(tx: T, label: &str, msg: M) -> ToggleButton {
    let toggle_btn = gtk::builders::ToggleButtonBuilder::new()
        .label(label)
        .margin_end(DEFAULT_MARGIN)
        .build();

    toggle_btn.connect_toggled(move |btn| {
        tx(msg(btn.is_active()));
    });

    toggle_btn
}