use gtk::prelude::*;
use gio::prelude::*;

use gtk::{Application, ApplicationWindow, Button, HeaderBar, Notebook, MenuButton, FileChooserDialog, FileChooserAction, ResponseType, Orientation, Label, IconSize, ReliefStyle};
use std::rc::Rc;
use gio::{SimpleAction};

mod file;
pub mod rules;
pub mod util;

use glib::bitflags::_core::cell::RefCell;
use crate::file::workbench::FileViewWorkbench;
use uuid::Uuid;
use crate::rules::CustomRule;

pub const SEARCH_TAG: &'static str = "SEARCH";

pub enum Msg {
    WorkbenchMsg(usize, WorkbenchMsg)
}

pub enum WorkbenchMsg {
    RuleMsg(RuleMsg),
    ApplyRules,
    ToolbarMsg(WorkbenchToolbarMsg),
}

pub enum WorkbenchToolbarMsg {
    TextChange(String),
    SearchPressed,
    ClearSearchPressed,
    ShowRules,
    ToggleAutoScroll(bool),
}

pub enum RuleMsg {
    AddRule(CustomRule),
    DeleteRule(Uuid),
    NameChanged(Uuid, String),
    RegexChanged (Uuid, String),
    ColorChanged(Uuid, String),
}


fn main() {
    let application = Application::new(
        Some("de.njust.ktail"),
        Default::default(),
    ).expect("failed to initialize GTK application");
    let fv = Rc::new(RefCell::new(Vec::<FileViewWorkbench>::new()));

    let file_views = fv.clone();
    application.connect_activate(move |app| {
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let window = ApplicationWindow::new(app);
        let exit_action = SimpleAction::new("quit", None);
        exit_action.connect_activate(|_a, _b| {
            gio::Application::get_default()
                .expect("no default Application!")
                .quit();
        });

        let container = Rc::new(Notebook::new());

        let file_views = file_views.clone(); {
            let file_views = file_views.clone();
            rx.attach(None, move |msg| {
                match msg {
                    Msg::WorkbenchMsg(idx, msg) => {
                        if let Some(tab) = file_views.borrow_mut().get_mut(idx) {
                            tab.update(msg);
                        }
                    }
                }
                glib::Continue(true)
            });
        }


        container.set_hexpand(true);
        container.set_vexpand(true);
        window.add(&*container);

        let open_action = SimpleAction::new("open", None);
        let file_views = file_views.clone();

        open_action.connect_activate(move |_a, _b| {
            let dialog = FileChooserDialog::new::<ApplicationWindow>(Some("Open File"), None, FileChooserAction::Open);
            dialog.add_button("_Cancel", ResponseType::Cancel);
            dialog.add_button("_Open", ResponseType::Accept);
            let res = dialog.run();
            dialog.close();
            if res == ResponseType::Accept {
                if let Some(file_path) = dialog.get_filename() {
                    let file_name = file_path.file_name().unwrap().to_str().unwrap().to_string();
                    let tx = tx.clone();
                    let idx = file_views.borrow().len();
                    let file_view = FileViewWorkbench::new(file_path, move |msg| {
                        tx.send(Msg::WorkbenchMsg(idx, msg)).expect("Could not send msg");
                    });

                    let close_btn = Button::from_icon_name(Some("window-close-symbolic"), IconSize::Menu);
                    close_btn.set_relief(ReliefStyle::None);
                    let tab_header = gtk::Box::new(Orientation::Horizontal, 0);
                    tab_header.add(&Label::new(Some(&file_name)));
                    tab_header.add(&close_btn);


                    let idx= container.append_page(file_view.view(), Some(&tab_header));
                    let notebook = container.clone();
                    {
                        let file_views = file_views.clone();
                        close_btn.connect_clicked(move |_| {
                            if let Some(page) = notebook.get_nth_page(Some(idx)) {
                                notebook.detach_tab(&page);
                                file_views.borrow_mut().remove(idx as usize);
                            }
                        });
                    }

                    container.show_all();
                    tab_header.show_all();
                    file_views.borrow_mut().push(file_view);
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
