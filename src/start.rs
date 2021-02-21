use gtk::{Align};
use gtk::prelude::*;
use crate::model::{StartViewMsg, StartViewOutputMsg};
use crate::widget::CustomWidget;

pub struct StartView {
    container: gtk::Box,
    counter: i32,
    lbl: gtk::Label,
}

impl CustomWidget for StartView {
    type Msg = StartViewMsg;
    type Output = StartViewOutputMsg;
    type Input = i32;

    fn init(data: Self::Input) -> Self {
        let container = gtk::BoxBuilder::new()
            .valign(Align::Center)
            .halign(Align::Center)
            .spacing(12)
            .build();

        let lbl = gtk::Label::new(Some(&format!("Count: {}", data)));

        Self {
            container,
            counter: data,
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

    fn update(&mut self, msg: StartViewMsg) -> Self::Output {
        match msg {
            StartViewMsg::Inc => {
                self.counter += 1;
            }
            StartViewMsg::Dec => {
                self.counter -= 1;
            }
        }
        self.lbl.set_text(&format!("Count: {}", self.counter));
        StartViewOutputMsg::CounterChanged(self.counter)
    }
}