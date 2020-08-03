use gtk::prelude::*;
use gio::prelude::*;

use gtk::{Application, ScrolledWindow, TextView, ApplicationWindow, Button, Adjustment, HeaderBar, Notebook, MenuButton, FileChooserDialog, FileChooserAction, ResponseType, Orientation, Label};
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

        let container = Notebook::new();
        container.set_hexpand(true);
        container.set_vexpand(true);
        window.add(&container);

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
                    container.append_page(file_view.get_view(), Some(&Label::new(Some(&file_name))));
                    container.show_all();
                }
            }
        });
        app.add_action(&open_action);
        app.add_action(&exit_action);

        window.set_title("Log Viewer");
        window.set_default_size(800, 600);

        let menu = gio::Menu::new();
        menu.append_item(&gio::MenuItem::new(Some("Open"), Some("app.open")));
        menu.append_item(&gio::MenuItem::new(Some("Quit"), Some("app.quit")));

        let menu_button = MenuButton::new();
        menu_button.set_menu_model(Some(&menu));

        let header_bar = HeaderBar::new();
        header_bar.add(&menu_button);
        header_bar.set_show_close_button(true);
        header_bar.set_title(Some("Log viewer"));

        window.set_titlebar(Some(&header_bar));
        window.show_all();
    });

    application.run(&[]);
}