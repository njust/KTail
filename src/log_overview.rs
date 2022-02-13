use std::collections::{HashMap};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use chrono::{Datelike, DateTime, NaiveDateTime, Timelike, TimeZone, Utc};
use gtk4_helper::component::{Command, MsgHandler};
use gtk4_helper::prelude::Component;
use gtk4_helper::{gtk, gtk::prelude::*};
use itertools::Itertools;
use plotters::coord::ReverseCoordTranslate;

enum WorkerData {
    Timestamp(Vec<DateTime<Utc>>),
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
    click_pos: Option<(f64, f64)>,
    mouse_pos: Option<(f64, f64)>,
}

#[derive(Clone)]
pub enum LogOverviewMsg {
    Redraw,
    Clear,
    HighlightResults(HighlightResultData),
    LogData(Vec<DateTime<Utc>>),
    MouseClick((i64, u32)),
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
            data: HashMap::new(),
            click_pos: None,
            mouse_pos: None,
        }));

        let tx = sender.clone();
        drawing_area.connect_resize(move |_,_,_|{
            tx(LogOverviewMsg::Redraw);
        });

        let mouse_move_events = gtk::EventControllerMotion::new();
        let cd = chart_data.clone();
        let tx = sender.clone();
        mouse_move_events.connect_motion(move |_, x, y| {
            if let Ok(mut cd) = cd.lock() {
                cd.mouse_pos = Some((x, y));
                tx(LogOverviewMsg::Redraw);
            }
        });

        let cd = chart_data.clone();
        let tx = sender.clone();
        mouse_move_events.connect_leave(move |_| {
            if let Ok(mut cd) = cd.lock() {
                cd.mouse_pos.take();
                tx(LogOverviewMsg::Redraw);
            }
        });
        drawing_area.add_controller(&mouse_move_events);

        let click = gtk::GestureClick::new();
        let cd = chart_data.clone();
        let tx = sender.clone();
        click.connect_released(move |_gesture, _p,x,y| {
            if let Ok(mut cd) = cd.lock() {
                cd.click_pos = Some((x, y));
                tx(LogOverviewMsg::Redraw);
            }
        });

        drawing_area.add_controller(&click);
        let cd = chart_data.clone();
        let tx = sender.clone();
        drawing_area.set_draw_func(move |_, ctx, width, height| {
            if let Ok(mut cd) = cd.lock() {
                let cdc = cd.clone();
                cd.click_pos.take();
                if let Some((dt, val)) = draw(cdc, &ctx, width, height) {
                    tx(LogOverviewMsg::MouseClick((dt, val)));
                }
            }
        });

        let (s, r) = std::sync::mpsc::channel::<WorkerData>();
        let cd = chart_data.clone();
        let tx = sender.clone();
        std::thread::spawn(move|| {
            while let Ok(data) = r.recv() {
                match data {
                    WorkerData::Timestamp(data) => {
                        for timestamp in data {
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
                        tx(LogOverviewMsg::Redraw);
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
                            tx(LogOverviewMsg::Redraw);
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
                    self.drawing_area.queue_draw();
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
            LogOverviewMsg::MouseClick(_) => {}
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


const Y_LABEL_AREA_SIZE: i32 = 25;
const X_LABEL_AREA_SIZE: i32 = 25;
const MARGIN_TOP: i32 = 15;
const MARGIN_LEFT: i32 = 10;
const LINE_HEIGHT: i32 = 2;
const X_START: f64 = (Y_LABEL_AREA_SIZE + MARGIN_LEFT) as f64;

fn draw(
    chart_data: ChartData,
    ctx: &gtk::cairo::Context, width: i32, height: i32) -> Option<(i64, u32)>
{
    let root = CairoBackend::new(ctx, (width as u32, height as u32)).unwrap().into_drawing_area();
    let mut resolved = None;
    if let (Some(start), Some(end)) = (chart_data.start_date, chart_data.end_date) {
        let max = CONFIG.lock().ok()
            .and_then(|cfg| chart_data.data.iter()
                .filter(|i| cfg.highlighters.contains_key(i.0))
                .flat_map(| l| l.1)
                .map(|i|*i.1).max()).unwrap_or(0);
        let mut chart = match ChartBuilder::on(&root)
            .x_label_area_size(X_LABEL_AREA_SIZE)
            .y_label_area_size(Y_LABEL_AREA_SIZE)
            .margin_top(MARGIN_TOP)
            .margin_bottom(2)
            .margin_left(MARGIN_LEFT)
            .margin_right(10)
            .build_cartesian_2d(start.timestamp()..end.timestamp(), 0u32..max + 1)
        {
            Ok(chart) => chart,
            Err(e) => {
                eprintln!("Could not build chart: {}", e);
                return None;
            }
        };

        if let Some((x,y )) = chart_data.mouse_pos {
            if x > X_START as f64 {
                if let Some((dt, _)) = chart.as_coord_spec().reverse_translate((x as i32, y as i32)) {
                    for y in (MARGIN_TOP - LINE_HEIGHT)..(height - (Y_LABEL_AREA_SIZE + LINE_HEIGHT)) {
                        if let Err(e) = root.draw_pixel((x as i32, y), &BLACK.mix(0.3)) {
                            eprintln!("Could not draw pixel: {}", e);
                        }
                    }

                    let ts = TextStyle::from("13 px Monospace");
                    let time = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(dt, 0), Utc);
                    if let Err(e) = root.draw_text(&format!("{:02}:{:02}:{:02}", time.hour(), time.minute(), time.second()), &ts, (x as i32 - 25, 1)) {
                        eprintln!("Could not draw text: {}", e);
                    }
                }
            }
        }

        if let Some((x, y)) = chart_data.click_pos {
            if let Some((dt, val)) = chart.as_coord_spec().reverse_translate((x as i32, y as i32)) {
                resolved = Some((dt, val));
            }
        }

        if let Err(e) = chart
            .configure_mesh()
            .disable_x_mesh()
            .bold_line_style(&WHITE.mix(0.3))
            .x_label_formatter(&|dt| {
                let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(*dt, 0), Utc);
                let time = dt.time();
                format!("{:02}:{:02}:{:02}", time.hour(), time.minute(), time.second())
            })
            .draw() {
            eprintln!("Could not draw chart: {}", e);
            return None;
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
                            LineSeries::new(data.iter().sorted_by_key(|(i, _)| **i).map(|(k, v)| (k.timestamp(), *v)),
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
    resolved
}