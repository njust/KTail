use std::collections::HashMap;
use std::sync::Arc;
use futures::StreamExt;
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
use crate::gtk::{ToggleButton};
use crate::log_text_contrast::matching_foreground_color_for_background;
use crate::pod_list_view::PodViewData;

pub const SEARCH_TAG: &'static str = "SEARCH";

#[derive(Clone)]
pub struct SearchData {
    pub name: String,
    pub search: Regex,
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
}

#[derive(Clone)]
pub enum LogViewMsg {
    PodSelected(Vec<PodViewData>),
    ContextSelected(NamespaceViewData),
    Loaded(Arc<Trigger>),
    LogDataLoaded(String),
    EnableScroll(bool),
    WrapText(bool),
    SinceTimespanChanged(String),
    Search(String),
    SearchResult(HashMap<String, Vec<usize>>)
}

impl LogView {
    fn clear(&mut self) {
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

        let auto_scroll_btn = auto_scroll_btn(sender.clone());
        toolbar.append(&auto_scroll_btn);

        let wrap_text_btn = wrap_text_btn(sender.clone());
        toolbar.append(&wrap_text_btn);

        let since_selector = since_duration_selection(sender.clone());
        toolbar.append(&since_selector);

        let search_tag = TextTag::new(Some(SEARCH_TAG));
        search_tag.set_background(Some("rgba(188,150,0,1)"));
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
            for highlighter  in &cfg.highlighters {
                let tag = TextTag::new(Some(&highlighter.name));
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
            for highlighter in &cfg.highlighters {
                if let Ok(regex) = Regex::new(&highlighter.search) {
                    search_data.push(SearchData {
                        search: regex,
                        name: highlighter.name.clone()
                    });
                }
            }
            search_data
        } else {
            vec![]
        };

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.append(&toolbar);
        let scroll_wnd = gtk::ScrolledWindow::new();
        scroll_wnd.set_child(Some(&log_data_view));
        container.append(&scroll_wnd);

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
            scroll_handler: None
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
                let mut end = self.text_buffer.end_iter();
                self.text_buffer.insert(&mut end, &data);
                let mut highlighters = self.highlighters.clone();
                if let Some(query) = self.active_search.as_ref() {
                    highlighters.push(SearchData {
                        search: query.clone(),
                        name: SEARCH_TAG.to_string()
                    });
                }
                let offset = self.text_buffer.line_count() - 2;
                return self.run_async(search(highlighters, data, offset as usize));
            }
            LogViewMsg::ContextSelected(ctx) => {
                self.selected_context = Some(ctx);
            }
            LogViewMsg::EnableScroll(enable) => {
                self.scroll_to_bottom(enable);
            }
            LogViewMsg::WrapText(wrap) => {
                if wrap {
                    self.text_view.set_wrap_mode(WrapMode::WordChar)
                } else {
                    self.text_view.set_wrap_mode(WrapMode::None)
                }
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
                        return self.run_async(search(vec![ SearchData { name: SEARCH_TAG.to_string(), search: query.clone() }], text, 0));
                    }
                }
            }
            LogViewMsg::SearchResult(matching_lines) => {
                for (search, matching_lines) in matching_lines {
                    for idx in matching_lines {
                        if let Some(start) = self.text_buffer.iter_at_line(idx as i32) {
                            let mut end = start.clone();
                            end.forward_to_line_end();
                            self.text_buffer.apply_tag_by_name(&search, &start, &end);
                        }
                    }
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
        }
        Command::None
    }

    fn view(&self) -> &Self::View {
        &self.container
    }
}

fn find_matching_lines(highlighters: Vec<SearchData>, text: String, line_offset: usize) -> HashMap<String, Vec<usize>> {
    let mut lines = text.lines().enumerate();
    let mut matching_lines = HashMap::<String, Vec<usize>>::new();
    while let Some((idx, line)) = lines.next() {
        for highlighter in &highlighters {
            if highlighter.search.is_match(line) {
                if !matching_lines.contains_key(&highlighter.name) {
                    matching_lines.insert(highlighter.name.clone(), vec![]);
                }
                let lines = matching_lines.get_mut(&highlighter.name).unwrap();
                lines.push(line_offset + idx);
                break;
            }
        }
    }

    matching_lines
}

async fn search(highlighters: Vec<SearchData>, text: String, line_offset: usize) -> LogViewMsg {
    let matching_lines = find_matching_lines(highlighters, text, line_offset);
    LogViewMsg::SearchResult(matching_lines)
}

async fn load_log_stream(ctx: NamespaceViewData, pod_data: Vec<PodViewData>, tx: Arc<dyn MsgHandler<LogViewMsg>>, since_seconds: u32) -> LogViewMsg {
    let pods = pod_data.iter().map(|pd| pd.name.clone()).collect();
    let client = crate::log_stream::k8s_client(&ctx.config_path, &ctx.context);
    let (mut log_stream, exit) = crate::log_stream::log_stream(&client, &ctx.name, &pods, since_seconds).await;
    let tx = tx.clone();
    tokio::task::spawn(async move {
        while let Some(data) = log_stream.next().await {
            let data = data.to_vec();
            let data = String::from_utf8_lossy(&data);
            tx(LogViewMsg::LogDataLoaded(data.to_string()));
        }
    });
    LogViewMsg::Loaded(Arc::new(exit))
}

const SINCE_10_MIN: u32 = 60*10;
const SINCE_30_MIN: u32 = 60*30;
const SINCE_60_MIN: u32 = 60*60;
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

    since_selector.append(Some(&SINCE_10_MIN.to_string()), "10 min");
    since_selector.append(Some(&SINCE_30_MIN.to_string()), "30 min");
    since_selector.append(Some(&SINCE_60_MIN.to_string()), "1 hour");
    since_selector.append(Some(&SINCE_2H.to_string()), "2 hours");
    since_selector.append(Some(&SINCE_4H.to_string()), "4 hours");
    since_selector.append(Some(&SINCE_6H.to_string()), "6 hours");
    since_selector.append(Some(&SINCE_8H.to_string()), "8 hours");
    since_selector.append(Some(&SINCE_10H.to_string()), "10 hours");
    since_selector.append(Some(&SINCE_12H.to_string()), "12 hours");
    since_selector.append(Some(&SINCE_24H.to_string()), "24 hours");
    since_selector.set_active_id(Some(&SINCE_10_MIN.to_string()));

    since_selector.connect_changed(move |a| {
        if let Some(active) =  a.active_id() {
            let active = active.to_string();
            tx(LogViewMsg::SinceTimespanChanged(active));
        }
    });

    since_selector
}

fn auto_scroll_btn<T: MsgHandler<LogViewMsg>>(tx: T) -> ToggleButton {
    let auto_scroll_btn = gtk::ToggleButtonBuilder::new()
        .label("Scroll")
        .margin_end(4)
        .build();

    auto_scroll_btn.connect_toggled(move |btn| {
        tx(LogViewMsg::EnableScroll(btn.is_active()));
    });

    auto_scroll_btn
}

fn wrap_text_btn<T: MsgHandler<LogViewMsg>>(tx: T) -> ToggleButton {
    let wrap_text_btn = gtk::ToggleButtonBuilder::new()
        .label("Wrap text")
        .margin_end(4)
        .build();

    wrap_text_btn.connect_toggled(move |btn| {
        tx(LogViewMsg::WrapText(btn.is_active()));
    });

    wrap_text_btn
}