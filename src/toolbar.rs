use gtk::prelude::*;
use gtk::{ToggleButton, Orientation, ButtonExt, ToggleButtonExt, IconSize, SearchEntry, Button, AccelFlags, AccelGroup};
use crate::model::{LogViewToolbarMsg};
use crate::util::add_css_with_name;

pub struct LogViewToolbar {
    container: gtk::Box,
}

impl LogViewToolbar {
    pub fn new<T>(tx: T, accelerators: &AccelGroup) -> Self
        where T: 'static + Clone + Fn(LogViewToolbarMsg)
    {
        let toolbar = gtk::Box::new(Orientation::Horizontal, 4);
        add_css_with_name(&toolbar, "toolbar", r"
            #toolbar {
                background-color: #f0f0f0;
                padding: 2px;
            }
        ");
        toolbar.set_property_margin(0);

        let search_txt = SearchEntry::new();
        let (key, modifier) = gtk::accelerator_parse("<Primary>F");
        search_txt.add_accelerator("grab-focus", accelerators, key, modifier, AccelFlags::VISIBLE);
        search_txt.set_width_chars(40);
        {
            let tx = tx.clone();
            search_txt.connect_changed(move |e| {
                let text = e.get_text().to_string();
                tx(LogViewToolbarMsg::TextChange(text));
            });
        }
        {
            let tx = tx.clone();
            search_txt.connect_icon_release(move |_, _, _| {
                tx(LogViewToolbarMsg::ClearSearchPressed);
            });
        }
        {
            let tx = tx.clone();
            search_txt.connect_activate(move |_| {
                tx(LogViewToolbarMsg::SearchPressed);
            });
            toolbar.add(&search_txt);
        }

        let search_btn = Button::with_label("Search"); {
            let tx = tx.clone();
            search_btn.connect_clicked(move |_| {
                tx(LogViewToolbarMsg::SearchPressed);
            });
            toolbar.add(&search_btn);
        }

        let prev_btn = gtk::Button::with_mnemonic("_Prev"); {
            let tx = tx.clone();
            let (key, modifier) = gtk::accelerator_parse("<Primary>P");
            prev_btn.add_accelerator("activate", accelerators, key, modifier, AccelFlags::VISIBLE);
            prev_btn.connect_clicked(move |_| {
                tx(LogViewToolbarMsg::SelectPrevMatch);
            });
            toolbar.add(&prev_btn);
        }

        let next_btn = gtk::Button::with_mnemonic("_Next"); {
            let (key, modifier) = gtk::accelerator_parse("<Primary>N");
            next_btn.add_accelerator("activate", accelerators, key, modifier, AccelFlags::VISIBLE);
            let tx = tx.clone();
            next_btn.connect_clicked(move |_| {
                tx(LogViewToolbarMsg::SelectNextMatch);
            });
            toolbar.add(&next_btn);
        }

        let show_rules_btn = Button::with_label("Rules"); {
            let tx = tx.clone();
            show_rules_btn.connect_clicked(move |_| {
                tx(LogViewToolbarMsg::ShowRules);
            });
            toolbar.add(&show_rules_btn);
        }

        let toggle_auto_scroll_btn = ToggleButton::new(); {
            let tx = tx.clone();
            toggle_auto_scroll_btn.connect_clicked(move |b| {
                tx(LogViewToolbarMsg::ToggleAutoScroll(b.get_active()));
            });

            toggle_auto_scroll_btn.set_image(Some(&gtk::Image::from_icon_name(Some("go-bottom-symbolic"), IconSize::Menu)));
            toolbar.add(&toggle_auto_scroll_btn);
        }

        let clear_btn = Button::with_label("Clear");
        {
            let tx = tx.clone();
            clear_btn.connect_clicked(move |_|{
                tx(LogViewToolbarMsg::Clear);
            });
            toolbar.add(&clear_btn);
        }

        Self {
            container: toolbar
        }
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }
}
