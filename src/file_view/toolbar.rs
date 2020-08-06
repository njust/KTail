use gtk::prelude::*;
use gtk::{ToggleButton, Orientation, ButtonExt, ToggleButtonExt, IconSize, SearchEntry, Button};
use glib::{Sender};
use crate::file_view::workbench::Msg;

pub struct FileViewToolbar {
    container: gtk::Box,
}

impl FileViewToolbar {
    pub fn new(tx: Sender<Msg>) -> Self {
        let toolbar = gtk::Box::new(Orientation::Horizontal, 4);
        toolbar.set_property_margin(4);

        let search_txt = SearchEntry::new(); {
            let tx = tx.clone();
            search_txt.connect_changed(move |e| {
                let text = e.get_text().to_string();
                tx.send(Msg::TextChange(text));
            });
            toolbar.add(&search_txt);
        }

        let search_btn = Button::with_label("Search"); {
            let tx = tx.clone();
            search_btn.connect_clicked(move |_| {
                tx.send(Msg::SearchPressed);
            });
            toolbar.add(&search_btn);
        }

        let toggle_auto_scroll_btn = ToggleButton::new(); {
            let tx = tx.clone();
            toggle_auto_scroll_btn.set_active(true);
            toggle_auto_scroll_btn.connect_clicked(move |b| {
                tx.send(Msg::ToggleAutoScroll(b.get_active()));
            });

            toggle_auto_scroll_btn.set_image(Some(&gtk::Image::from_icon_name(Some("go-bottom-symbolic"), IconSize::Menu)));
            toolbar.add(&toggle_auto_scroll_btn);
        }

        Self {
            container: toolbar,
        }
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }
}
