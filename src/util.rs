use gtk4_helper::glib::IsA;
use gtk4_helper::{
    gtk,
    gtk::Widget,
    gtk::prelude::*,
};
use gtk4_helper::gtk::Orientation;

pub struct WidgetLoadingWrapper<T: IsA<Widget>> {
    widget: T,
    spinner: gtk::Spinner,
    spinner_wrapper: gtk::Box,
    container: gtk::Box,
}

impl<T: IsA<Widget>> WidgetLoadingWrapper<T> {
    pub fn new(widget: T) -> Self {
        let spinner = gtk::Spinner::builder()
            .visible(true)
            .height_request(32)
            .width_request(32)
            .build();

        let spinner_wrapper = gtk::BoxBuilder::new()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::Center)
            .visible(false)
            .vexpand(true)
            .build();

        spinner_wrapper.append(&spinner);

        let container = gtk::BoxBuilder::new()
            .orientation(Orientation::Vertical)
            .vexpand(true)
            .build();

        container.append(&spinner_wrapper);
        container.append(&widget);

        WidgetLoadingWrapper {
            widget,
            container,
            spinner,
            spinner_wrapper
        }
    }

    pub fn set_is_loading(&self, loading: bool) {
        self.widget.set_visible(!loading);
        self.spinner_wrapper.set_visible(loading);
        self.spinner.set_spinning(loading);
    }

    pub fn container(&self) -> &gtk::Box {
        &self.container
    }
}

pub fn add_css<T: IsA<Widget>>(w: &T, css: &str) {
    let sc = w.style_context();
    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_data(css.as_bytes());
    sc.add_provider(&css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

pub fn add_css_with_name<T: IsA<Widget>>(w: &T, widget_name: &str, css: &str) {
    w.set_widget_name(widget_name);
    add_css(w, css);
}
