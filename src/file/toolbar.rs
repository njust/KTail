use gtk::prelude::*;
use gtk::{ToggleButton, Orientation, ButtonExt, ToggleButtonExt, IconSize, SearchEntry, Button};
use crate::{WorkbenchToolbarMsg, SearchResultMatch};
use std::collections::HashMap;
use crate::rules::SEARCH_ID;

pub struct FileViewToolbar {
    container: gtk::Box,
    label: gtk::Label,
}

impl FileViewToolbar {
    pub fn new<T>(tx: T) -> Self
        where T: 'static + Clone + Fn(WorkbenchToolbarMsg)
    {
        let toolbar = gtk::Box::new(Orientation::Horizontal, 4);
        toolbar.set_property_margin(4);

        let search_txt = SearchEntry::new(); {
            search_txt.set_width_chars(40);
            let tx2 = tx.clone();
            search_txt.connect_changed(move |e| {
                let text = e.get_text().to_string();
                tx2(WorkbenchToolbarMsg::TextChange(text));
            });
            let tx = tx.clone();
            search_txt.connect_icon_release(move |_,_,_| {
                tx(WorkbenchToolbarMsg::ClearSearchPressed);
            });
            toolbar.add(&search_txt);
        }

        let search_btn = Button::with_label("Search"); {
            let tx = tx.clone();
            search_btn.connect_clicked(move |_| {
                tx(WorkbenchToolbarMsg::SearchPressed);
            });
            toolbar.add(&search_btn);
        }

        let show_rules_btn = Button::with_label("Highlighters"); {
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


        let prev_btn = gtk::Button::with_label("Prev"); {
            let tx = tx.clone();
            prev_btn.connect_clicked(move |_| {
                tx(WorkbenchToolbarMsg::SelectPrevMatch);
            });
            toolbar.add(&prev_btn);
        }

        let results = gtk::Label::new(None);
        toolbar.add(&results);

        let next_btn = gtk::Button::with_label("Next"); {
            let tx = tx.clone();
            next_btn.connect_clicked(move |_| {
                tx(WorkbenchToolbarMsg::SelectNextMatch);
            });
            toolbar.add(&next_btn);
        }

        Self {
            container: toolbar,
            label: results
        }
    }

    pub fn update(&mut self, data: &HashMap<String, Vec<SearchResultMatch>>) {
        if let Some(search_results) = data.get(SEARCH_ID) {
            let text = format!("Matches: {}", search_results.len());
            self.label.set_text(&text);
        }
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }
}
