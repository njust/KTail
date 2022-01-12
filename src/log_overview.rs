use std::collections::{HashMap};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use chrono::{Datelike, DateTime, Timelike, TimeZone, Utc};
use gtk4_helper::component::{Command, MsgHandler};
use gtk4_helper::prelude::Component;
use gtk4_helper::{
    gtk,
    gtk::prelude::*,
};
use itertools::Itertools;

enum WorkerData {
    Timestamp(DateTime<Utc>),
    Highlight(HighlightResultData)
}

pub struct LogOverview {
    drawing_area: gtk::DrawingArea,
    chart_data: Arc<Mutex<ChartData>>,
    worker: Sender<WorkerData>,
}

#[derive(Clone)]
pub struct ChartData {
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    data: HashMap<String, HashMap<DateTime<Utc>, u32>>,
}

#[derive(Clone)]
pub enum LogOverviewMsg {
    Redraw,
    Clear,
    HighlightResults(HighlightResultData),
    LogData(DateTime<Utc>),
}

impl Component for LogOverview {
    type Msg = LogOverviewMsg;
    type View = gtk::DrawingArea;
    type Input = ();

    fn create<T: MsgHandler<Self::Msg> + Clone>(sender: T, _input: Option<Self::Input>) -> Self {
        let drawing_area = gtk::DrawingArea::new();
        let chart_data = Arc::new(Mutex::new(ChartData {
            start_date: None,
            end_date: None,
            data: HashMap::new()
        }));

        let cd = chart_data.clone();
        drawing_area.set_draw_func(move |_, ctx, width, height| {
            if let Ok(cd) = cd.lock().map(|cd| cd.clone()) {
                draw(cd, &ctx, width, height);
            }
        });

        let tx = sender.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            tx(LogOverviewMsg::Redraw);
            gtk::glib::Continue(true)
        });

        let (s, r) = std::sync::mpsc::channel::<WorkerData>();
        let cd = chart_data.clone();
        std::thread::spawn(move|| {
            while let Ok(data) = r.recv() {
                match data {
                    WorkerData::Timestamp(timestamp) => {
                        if let Ok(mut chart_data) = cd.lock() {
                            if let Some(ts) = chart_data.start_date {
                                if timestamp < ts {
                                    chart_data.start_date.replace(timestamp);
                                }
                            } else {
                                chart_data.start_date.replace(timestamp.clone());
                            }

                            if let Some(ts) = chart_data.end_date {
                                if timestamp > ts {
                                    chart_data.end_date.replace(timestamp.clone());
                                }
                            } else {
                                chart_data.end_date.replace(timestamp);
                            }

                            let time = timestamp.time();
                            let ts = Utc.ymd(timestamp.year(),timestamp.month(),timestamp.day()).and_hms(time.hour(), time.minute(), 0);
                            for (_, data) in chart_data.data.iter_mut() {
                                if data.len() > 0 && !data.contains_key(&ts) {
                                    data.insert(ts.clone(), 0);
                                }
                            }
                        }
                    }
                    WorkerData::Highlight(results) => {
                        let ts = results.timestamp;
                        if let Ok(mut chart_data) = cd.lock() {
                            for tag in results.tags {
                                let series_data = chart_data.data.entry(tag).or_insert(HashMap::new());
                                let time = ts.time();

                                let timestamp = Utc.ymd(ts.year(), ts.month(), ts.day()).and_hms(time.hour(), time.minute(), 0);
                                if let Some(ts_count) = series_data.get_mut(&timestamp) {
                                    *ts_count = *ts_count +1;
                                } else {
                                    series_data.insert(timestamp, 1);
                                }
                            }
                        }
                    }
                }
            }
        });

        Self {
            drawing_area,
            chart_data,
            worker: s,
        }
    }

    fn update(&mut self, msg: Self::Msg) -> Command<Self::Msg> {
        match msg {
            LogOverviewMsg::Redraw => {
                self.drawing_area.queue_draw();
            }
            LogOverviewMsg::Clear => {
                if let Ok(mut cd) = self.chart_data.lock() {
                    cd.start_date.take();
                    cd.end_date.take();
                    cd.data.clear();
                }
            }
            LogOverviewMsg::LogData(timestamp) => {
                if let Err(e) = self.worker.send(WorkerData::Timestamp(timestamp)) {
                    eprintln!("Failed to send worker data: {}", e);
                }
            }
            LogOverviewMsg::HighlightResults(results) => {
                if let Err(e) = self.worker.send(WorkerData::Highlight(results)) {
                    eprintln!("Failed to send worker data: {}", e);
                }
            }
        }

        Command::None
    }

    fn view(&self) -> &Self::View {
        &self.drawing_area
    }
}

use plotters::prelude::*;
use plotters_cairo::CairoBackend;
use crate::config::CONFIG;
use crate::log_view::{HighlightResultData};

fn draw(
    chart_data: ChartData,
    ctx: &gtk::cairo::Context, width: i32, height: i32) {
    let root = CairoBackend::new(ctx, (width as u32, height as u32)).unwrap().into_drawing_area();

    if let (Some(start), Some(end)) = (chart_data.start_date, chart_data.end_date) {
        let max = CONFIG.lock().ok()
            .and_then(|cfg| chart_data.data.iter()
            .filter(|i| cfg.highlighters.contains_key(i.0))
            .flat_map(| l| l.1)
            .map(|i|*i.1).max()).unwrap_or(0);
        let mut chart = match ChartBuilder::on(&root)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .margin(8)
            .build_cartesian_2d(start..end, 0u32..max + 1)
        {
            Ok(chart) => chart,
            Err(e) => {
                eprintln!("Could not build chart: {}", e);
                return;
            }
        };

        if let Err(e) = chart
            .configure_mesh()
            .disable_x_mesh()
            .bold_line_style(&WHITE.mix(0.3))
            .x_label_formatter(&|dt| {
                let time = dt.time();
                format!("{:02}:{:02}", time.hour(), time.minute())
            })
            .draw() {
            eprintln!("Could not draw chart: {}", e);
            return;
        }

        if let Ok(cfg) = CONFIG.lock() {
            for (name, data) in &chart_data.data {
                if let Some(highlighter) = cfg.highlighters.get(name) {
                    let parts = &mut highlighter.color[4..highlighter.color.len() -1].split(",");
                    let r = parts.next().and_then(|p| p.parse::<u8>().ok());
                    let g = parts.next().and_then(|p| p.parse::<u8>().ok());
                    let b = parts.next().and_then(|p| p.parse::<u8>().ok());
                    if let (Some(r), Some(g), Some(b)) = (r,g,b)  {
                        let color = plotters::style::RGBColor(r,g,b);
                        if let Err(e) = chart.draw_series(
                        LineSeries::new(data.iter().sorted_by_key(|(i, _)| **i).map(|(k, v)| (k.clone(), *v)),
                        color.stroke_width(2))
                        ) {
                            eprintln!("Could not draw line series: {}", e);
                        }
                    } else {
                      eprintln!("Could not parse highlighter color: {}", highlighter.color);
                    }
                }
            }
        }
    }
}