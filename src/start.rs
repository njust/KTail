
use gtk::prelude::*;
use crate::model::{StartViewMsg, StartViewOutMsg};
use crate::widget::{CustomWidget, WidgetMsg};
use glib::Sender;

pub struct StartView {
    counter: i32,
    lbl: gtk::Label,
}

impl CustomWidget for StartView {
    type Msg = StartViewMsg;
    type Output = StartViewOutMsg;
    type Input = i32;

    fn init(data: Self::Input) -> Self {
        let lbl = gtk::Label::new(Some(&format!("Count: {}", data)));
        Self {
            counter: data,
            lbl
        }
    }

    fn create(&self, container: &gtk::Box, action_sender: Sender<Self::Msg>) {
        let btn = gtk::ButtonBuilder::new()
            .label("Dec")
            .build();

        let tx = action_sender.clone();
        btn.connect_clicked(move |_| {
            tx.send(StartViewMsg::Dec).expect("Could not send dec");
        });

        container.add(&btn);
        container.add(&self.lbl);

        let btn = gtk::ButtonBuilder::new()
            .label("Inc")
            .build();

        let tx = action_sender.clone();
        btn.connect_clicked(move |_| {
            tx.send(StartViewMsg::Inc).expect("Could not send inc");
        });
        container.add(&btn);
    }

    fn update(&mut self, msg: StartViewMsg) -> WidgetMsg<Self> {
        match msg {
            StartViewMsg::Inc => {
                self.run_async(inc_async(2))
            }
            StartViewMsg::Dec => {
                self.update_counter(false)
            }
            StartViewMsg::AsyncInc => {
                self.update_counter(true)
            }
        }
    }
}

impl StartView {
    fn update_counter(&mut self, inc: bool) -> WidgetMsg<Self> {
        self.counter = if inc { self.counter + 1 } else { self.counter -1 };
        self.lbl.set_text(&format!("Count: {}", self.counter));
        self.msg_out(StartViewOutMsg::Changed(self.counter))
    }
}

async fn inc_async(seconds: u64) -> StartViewMsg {
    tokio::time::delay_for(std::time::Duration::from_secs(seconds)).await;
    StartViewMsg::AsyncInc
}