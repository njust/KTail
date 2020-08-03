use gtk::prelude::*;
use gio::prelude::*;

use gtk::{Application, ScrolledWindow, TextView, ApplicationWindow, Button, Adjustment, HeaderBar, Notebook, MenuButton, FileChooserDialog, FileChooserAction, ResponseType, Orientation, Label, ArrowType, IconSize, ReliefStyle};
use std::time::Duration;
use std::rc::Rc;
use std::error::Error;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use gio::{SimpleAction};

use encoding::all::{UTF_8, WINDOWS_1252, UTF_16BE, UTF_16LE};
use encoding::{Encoding, DecoderTrap};
use std::path::PathBuf;

mod file_view;
use file_view::FileView;

fn main() {
    let application = Application::new(
        Some("com.github.gtk-rs.examples.basic"),
        Default::default(),
    ).expect("failed to initialize GTK application");

    application.connect_activate(move |app| {
        let window = ApplicationWindow::new(app);
        let exit_action = SimpleAction::new("quit", None);
        exit_action.connect_activate(|a, b| {
            gio::Application::get_default()
                .expect("no default Application!")
                .quit();
        });

        let container = Rc::new(Notebook::new());
        container.set_hexpand(true);
        container.set_vexpand(true);
        window.add(&*container);

        let open_action = SimpleAction::new("open", None);
        open_action.connect_activate(move |a, b| {
            let dialog = FileChooserDialog::new::<ApplicationWindow>(Some("Open File"), None, FileChooserAction::Open);
            dialog.add_button("_Cancel", ResponseType::Cancel);
            dialog.add_button("_Open", ResponseType::Accept);
            let res = dialog.run();
            dialog.close();
            if res == ResponseType::Accept {
                if let Some(file_path) = dialog.get_filename() {
                    let file_name = file_path.file_name().unwrap().to_str().unwrap().to_string();
                    let file_view = FileView::new(file_path);

                    let close_btn = Button::from_icon_name(Some("window-close-symbolic"), IconSize::Menu);
                    close_btn.set_relief(ReliefStyle::None);
                    let tab_header = gtk::Box::new(Orientation::Horizontal, 0);
                    tab_header.add(&Label::new(Some(&file_name)));
                    tab_header.add(&close_btn);

                    let idx = container.append_page(file_view.get_view(), Some(&tab_header));
                    let c = container.clone();
                    close_btn.connect_clicked(move |_| {
                        if let Some(page) = c.get_nth_page(Some(idx)) {
                            c.detach_tab(&page);
                        }
                    });

                    container.show_all();
                    tab_header.show_all();
                }
            }
        });
        app.add_action(&open_action);
        app.add_action(&exit_action);

        window.set_title("Log Viewer");
        window.set_default_size(800, 600);

        let menu_model = gio::Menu::new();
        menu_model.append_item(&gio::MenuItem::new(Some("Open"), Some("app.open")));
        menu_model.append_item(&gio::MenuItem::new(Some("Quit"), Some("app.quit")));

        let menu_button = MenuButton::new();
        menu_button.set_relief(ReliefStyle::None);
        menu_button.set_popup(Some(&gtk::Menu::from_model(&menu_model)));
        menu_button.set_image(Some(&gtk::Image::from_icon_name(Some("open-menu-symbolic"), IconSize::Menu)));

        let header_bar = HeaderBar::new();
        header_bar.pack_end(&menu_button);
        header_bar.set_show_close_button(true);
        header_bar.set_title(Some("Log viewer"));

        window.set_titlebar(Some(&header_bar));
        window.show_all();
    });

    application.run(&[]);
}