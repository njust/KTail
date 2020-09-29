use gtk::prelude::*;
use gtk::{ToggleButton, Orientation, ButtonExt, ToggleButtonExt, IconSize, SearchEntry, Button, AccelFlags, AccelGroup, TreeIter};
use crate::{WorkbenchToolbarMsg};
use crate::rules::Rule;

pub struct FileViewToolbar {
    container: gtk::Box,
    rules_selector_data: gtk::ListStore,
}

impl FileViewToolbar {
    pub fn new<T>(tx: T, accelerators: &AccelGroup, init_rules: &Vec<Rule>) -> Self
        where T: 'static + Clone + Fn(WorkbenchToolbarMsg)
    {
        let toolbar = gtk::Box::new(Orientation::Horizontal, 4);
        toolbar.set_property_margin(4);

        let search_txt = SearchEntry::new();
        let (key, modifier) = gtk::accelerator_parse("<Primary>F");
        search_txt.add_accelerator("grab-focus", accelerators, key, modifier, AccelFlags::VISIBLE);
        search_txt.set_width_chars(40);
        {
            let tx = tx.clone();
            search_txt.connect_changed(move |e| {
                let text = e.get_text().to_string();
                tx(WorkbenchToolbarMsg::TextChange(text));
            });
        }
        {
            let tx = tx.clone();
            search_txt.connect_icon_release(move |_, _, _| {
                tx(WorkbenchToolbarMsg::ClearSearchPressed);
            });
        }
        {
            let tx = tx.clone();
            search_txt.connect_activate(move |_| {
                tx(WorkbenchToolbarMsg::SearchPressed);
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

        let prev_btn = gtk::Button::with_mnemonic("_Prev"); {
            let tx = tx.clone();
            let (key, modifier) = gtk::accelerator_parse("<Primary>P");
            prev_btn.add_accelerator("activate", accelerators, key, modifier, AccelFlags::VISIBLE);
            prev_btn.connect_clicked(move |_| {
                tx(WorkbenchToolbarMsg::SelectPrevMatch);
            });
            toolbar.add(&prev_btn);
        }

        let rules_data = gtk::ListStore::new(&[glib::Type::String, glib::Type::String]);
        let default_name = String::from("Unamed rule");
        for rule in init_rules {
            let name = rule.name.as_ref().unwrap_or(&default_name);
            let id = rule.id.to_string();
            rules_data.insert_with_values(None, &[0, 1], &[&id, &name]);
        }

        let rule_selector = gtk::ComboBox::with_model(&rules_data);
        let renderer =  gtk::CellRendererText::new();
        rule_selector.pack_start(&renderer, true);
        rule_selector.add_attribute(&renderer, "text", 1);
        rule_selector.set_property_width_request(50);
        rule_selector.set_id_column(0);
        rule_selector.set_active(Some(0));
        toolbar.add(&rule_selector);
        {
            let tx = tx.clone();
            rule_selector.connect_changed(move |cb| {
                if let Some(selected) = cb.get_active_id() {
                    tx(WorkbenchToolbarMsg::SelectRule(selected.as_str().into()))
                }
            });
        }

        let next_btn = gtk::Button::with_mnemonic("_Next"); {
            let (key, modifier) = gtk::accelerator_parse("<Primary>N");
            next_btn.add_accelerator("activate", accelerators, key, modifier, AccelFlags::VISIBLE);
            let tx = tx.clone();
            next_btn.connect_clicked(move |_| {
                tx(WorkbenchToolbarMsg::SelectNextMatch);
            });
            toolbar.add(&next_btn);
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

        Self {
            container: toolbar,
            rules_selector_data: rules_data,
        }
    }

    fn get_rule_iter(&mut self, id: &str) -> Option<TreeIter> {
        if let Some(mut current) = self.rules_selector_data.get_iter_first() {
            loop {
                if let Some(current_id) = self.rules_selector_data.get_value(&current, 0).get::<String>().ok().and_then(|v|v) {
                    if id == current_id {
                        return Some(current);
                    }
                }
                if !self.rules_selector_data.iter_next(&current) {
                    break;
                }
            }
        }
        None
    }

    pub fn update_rule(&mut self, id: &str, name: &str) {
        if let Some(iter) = self.get_rule_iter(id) {
            self.rules_selector_data.set(&iter, &[1], &[&name]);
        }
    }

    pub fn delete_rule(&mut self, id: &str) {
        if let Some(iter) = self.get_rule_iter(id) {
            self.rules_selector_data.remove(&iter);
        }
    }

    pub fn add_rule(&mut self, rule: &Rule) {
        let default_name = String::from("Unamed rule");
        let name = rule.name.as_ref().unwrap_or(&default_name);
        let id = rule.id.to_string();
        self.rules_selector_data.insert_with_values(None, &[0, 1], &[&id, &name]);
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }
}
