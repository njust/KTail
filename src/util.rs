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

pub struct IfElseWidget<A: IsA<Widget>, B: IsA<Widget>> {
    widget_a: A,
    widget_b: B,
    container: gtk::Box,
}

#[allow(dead_code)]
impl<A: IsA<Widget>, B: IsA<Widget>> IfElseWidget<A, B> {
    pub fn new(widget_a: A, widget_b: B) -> Self {
        let container = gtk::BoxBuilder::new()
            .orientation(Orientation::Vertical)
            .vexpand(true)
            .build();

        container.append(&widget_a);
        container.append(&widget_b);
        widget_a.set_visible(false);

        Self {
            widget_a,
            widget_b,
            container
        }
    }

    pub fn show_a(&self, show: bool) {
        self.widget_a.set_visible(show);
        self.widget_b.set_visible(!show);
    }

    pub fn show_b(&self, show: bool) {
        self.show_a(!show);
    }

    pub fn widget_a(&self) -> &A {
        &self.widget_a
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
