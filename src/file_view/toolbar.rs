use gtk::prelude::*;
use gtk::{ToggleButton, Orientation, ButtonExt, ToggleButtonExt, IconSize};

pub struct FileViewToolbar {
    container: gtk::Box,
    toggle_auto_scroll_btn: ToggleButton,
}

impl FileViewToolbar {
    pub fn new() -> Self {
        let toolbar = gtk::Box::new(Orientation::Horizontal, 4);
        toolbar.set_property_margin(4);
        let toggle_auto_scroll_btn = ToggleButton::new();
        toggle_auto_scroll_btn.set_image(Some(&gtk::Image::from_icon_name(Some("go-bottom-symbolic"), IconSize::Menu)));
        toggle_auto_scroll_btn.set_active(true);

        toolbar.add(&toggle_auto_scroll_btn);
        Self{
            container: toolbar,
            toggle_auto_scroll_btn,
        }
    }

    pub fn on_toggle_autoscroll<F: Fn() + 'static>(&self, handler: F) {
        self.toggle_auto_scroll_btn.connect_clicked(move |_| {
            handler();
        });
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }
}
