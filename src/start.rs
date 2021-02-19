use gtk::{Align};
use gtk::prelude::*;
use crate::model::{StartViewMsg};
use crate::widget::CustomWidget;

pub struct StartView {
    container: gtk::Box,
    counter: i32,
    lbl: gtk::Label,
}

impl CustomWidget for StartView {
    type Msg = StartViewMsg;
    fn init() -> Self {
        let container = gtk::BoxBuilder::new()
            .valign(Align::Center)
            .halign(Align::Center)
            .spacing(12)
            .build();

        let lbl = gtk::Label::new(Some("0"));

        Self {
            container,
            counter: 0,
            lbl
        }
    }

    fn create<T: 'static + Clone + Fn(StartViewMsg)>(&self, action_sender: T) {
        let btn = gtk::ButtonBuilder::new()
            .label("Dec")
            .build();

        let tx = action_sender.clone();
        btn.connect_clicked(move |_| {
            tx(StartViewMsg::Dec);
        });

        self.container.add(&btn);
        self.container.add(&self.lbl);

        let btn = gtk::ButtonBuilder::new()
            .label("Inc")
            .build();

        let tx = action_sender.clone();
        btn.connect_clicked(move |_| {
            tx(StartViewMsg::Inc);
        });
        self.container.add(&btn);
    }

    fn view(&self) -> &gtk::Box {
        &self.container
    }

    fn update(&mut self, msg: StartViewMsg) {
        match msg {
            StartViewMsg::Inc => {
                self.counter += 1;
            }
            StartViewMsg::Dec => {
                self.counter -= 1;
            }
        }
        self.lbl.set_text(&format!("{}", self.counter));
    }
}