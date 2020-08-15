use gtk::prelude::*;
use gtk::{ToggleButton, Orientation, ButtonExt, ToggleButtonExt, IconSize, SearchEntry, Button};
use crate::{WorkbenchToolbarMsg};

pub struct FileViewToolbar {
    container: gtk::Box,
}

impl FileViewToolbar {
    pub fn new<T: 'static + Clone + Fn(WorkbenchToolbarMsg)>(tx: T) -> Self {
        let toolbar = gtk::Box::new(Orientation::Horizontal, 4);
        toolbar.set_property_margin(4);

        let search_txt = SearchEntry::new(); {
            search_txt.set_width_chars(40);
            let tx = tx.clone();
            search_txt.connect_changed(move |e| {
                let text = e.get_text().to_string();
                tx(WorkbenchToolbarMsg::TextChange(text));
            });
            search_txt.set_text(r".*\s((?i)error|fatal(?-i))\s.*");
            toolbar.add(&search_txt);
        }

        let search_btn = Button::with_label("Search"); {
            let tx = tx.clone();
            search_btn.connect_clicked(move |_| {
                tx(WorkbenchToolbarMsg::SearchPressed);
            });
            toolbar.add(&search_btn);
        }

        let clear_search_btn = Button::with_label("Clear"); {
            let tx = tx.clone();
            clear_search_btn.connect_clicked(move |_| {
                tx(WorkbenchToolbarMsg::ClearSearchPressed);
            });
            toolbar.add(&clear_search_btn);
        }

        let show_rules_btn = Button::with_label("Rules"); {
            let tx = tx.clone();
            show_rules_btn.connect_clicked(move |_| {
                tx(WorkbenchToolbarMsg::ShowRules);
            });
            toolbar.add(&show_rules_btn);
        }

        let toggle_auto_scroll_btn = ToggleButton::new(); {
            let tx = tx.clone();
            toggle_auto_scroll_btn.connect_clicked(move |b| {
                tx(WorkbenchToolbarMsg::ToggleAutoScroll(b.get_active()));
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
