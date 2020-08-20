use gtk::prelude::*;
use gio::prelude::*;

use gtk::{Application, ApplicationWindow, Button, HeaderBar, Notebook, MenuButton, FileChooserDialog, FileChooserAction, ResponseType, Orientation, Label, IconSize, ReliefStyle, TreeStore, WindowPosition, TreeIter};
use std::rc::Rc;
use gio::{SimpleAction};

mod file;
pub mod rules;
pub mod util;

use crate::file::workbench::FileViewWorkbench;
use uuid::Uuid;
use crate::rules::Rule;
use crate::util::{get_pods, create_col, ColumnType};
use std::path::PathBuf;
use glib::Sender;

pub const SEARCH_TAG: &'static str = "SEARCH";

pub enum FileViewData {
    File(PathBuf),
    Kube(Vec<String>)
}

impl FileViewData {
    fn get_name(&self) -> String {
        match self {
            FileViewData::File(file_path) => file_path.file_name().unwrap().to_str().unwrap().to_string(),
            FileViewData::Kube(services) => services.join(",")
        }
    }
}

pub enum Msg {
    CloseTab(usize),
    CreateTab(FileViewData),
    WorkbenchMsg(usize, WorkbenchViewMsg)
}

pub enum WorkbenchViewMsg {
    ApplyRules,
    ToolbarMsg(WorkbenchToolbarMsg),
    RuleViewMsg(RuleListViewMsg),
    FileViewMsg(FileViewMsg)
}

pub enum WorkbenchToolbarMsg {
    TextChange(String),
    SearchPressed,
    ClearSearchPressed,
    ShowRules,
    ToggleAutoScroll(bool),
}

pub enum RuleListViewMsg {
    AddRule(Rule),
    RuleViewMsg(Uuid, RuleViewMsg)
}

pub enum RuleViewMsg {
    NameChanged(String),
    RegexChanged (String),
    ColorChanged(String),
    DeleteRule,
}

pub struct SearchResultMatch {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

pub struct SearchResult {
    tag: String,
    with_offset: bool,
    matches: Vec<SearchResultMatch>,
}

pub enum FileViewMsg {
    Data(u64, String, Vec<SearchResult>),
    Clear,
}

fn map_model(model: &TreeStore, iter: &TreeIter) -> Option<(String, bool)> {
    let name = model.get_value(iter, 0).get::<String>().unwrap_or(None)?;
    let active = model.get_value(iter, 1).get::<bool>().unwrap_or(None)?;
    Some((name, active))
}

fn create_tab(data: FileViewData, tx: Sender<Msg>, idx: usize) -> (gtk::Box, FileViewWorkbench) {
    let tx2 = tx.clone();
    let tab_name = data.get_name();
    let file_view = FileViewWorkbench::new(data, move |msg| {
        tx2.send(Msg::WorkbenchMsg(idx, msg)).expect("Could not send msg");
    });

    let close_btn = Button::from_icon_name(Some("window-close-symbolic"), IconSize::Menu);
    close_btn.set_relief(ReliefStyle::None);


    let tab_header = gtk::Box::new(Orientation::Horizontal, 0);
    tab_header.add(&Label::new(Some(&tab_name)));
    tab_header.add(&close_btn);

    let tx = tx.clone();
    close_btn.connect_clicked(move |_| {
        tx.send(Msg::CloseTab(idx as usize));
    });

    tab_header.show_all();
    (tab_header, file_view)
}

fn create_open_kube_action(tx: Sender<Msg>) -> SimpleAction {
    let kube_action = SimpleAction::new("kube", None);
    kube_action.connect_activate(move |a,b| {
        let m = Rc::new(TreeStore::new(&[glib::Type::String, glib::Type::Bool]));
        let list = gtk::TreeView::with_model(&*m);

        list.append_column(&create_col(None, 1, ColumnType::Bool, m.clone()));
        list.append_column(&create_col(Some("Service"), 0, ColumnType::String, m.clone()));

        let dlg = gtk::Dialog::new();
        dlg.set_position(WindowPosition::Mouse);
        dlg.set_default_size(400, 200);
        let header_bar = HeaderBar::new();
        header_bar.set_show_close_button(true);
        header_bar.set_title(Some("Services"));
        dlg.set_titlebar(Some(&header_bar));
        dlg.set_modal(true);

        let content = dlg.get_content_area();
        content.add(&list);

        dlg.connect_delete_event(move |dlg, _| {
            dlg.hide();
            gtk::Inhibit(true)
        });

        dlg.add_button("_Cancel", ResponseType::Cancel);
        dlg.add_button("_Open", ResponseType::Accept);
        dlg.show_all();
        if let Ok(pods) = get_pods() {
            for pod in pods {
                m.insert_with_values(None, None, &[0, 1], &[&pod, &false]);
            }
        }

        let res = dlg.run();
        dlg.close();
        if res == ResponseType::Accept {
            let mut models = vec![];
            if let Some(iter)  = m.get_iter_first() {
                if let Some((service, active)) = map_model(&m, &iter) {
                    if active {
                        models.push(service);
                    }
                }
                while m.iter_next(&iter) {
                    if let Some((service, active)) = map_model(&m, &iter) {
                        if active {
                            models.push(service);
                        }
                    }
                }
            }
            tx.send(Msg::CreateTab(FileViewData::Kube(models)));
        }

    });
    kube_action
}

fn create_open_dlg_action(tx: Sender<Msg>) -> SimpleAction {
    let open_action = SimpleAction::new("open", None);
    open_action.connect_activate(move |_a, _b| {
        let dialog = FileChooserDialog::new::<ApplicationWindow>(Some("Open File"), None, FileChooserAction::Open);
        dialog.add_button("_Cancel", ResponseType::Cancel);
        dialog.add_button("_Open", ResponseType::Accept);
        let res = dialog.run();
        dialog.close();
        if res == ResponseType::Accept {
            if let Some(file_path) = dialog.get_filename() {
                tx.send(Msg::CreateTab(FileViewData::File(file_path)));
            }
        }
    });
    open_action
}

fn main() {
    let application = Application::new(
        Some("de.njust.ktail"),
        Default::default(),
    ).expect("failed to initialize GTK application");

    application.connect_activate(move |app| {
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let open_action = create_open_dlg_action(tx.clone());
        app.add_action(&open_action);
        let kube_action = create_open_kube_action(tx.clone());
        app.add_action(&kube_action);

        let exit_action = SimpleAction::new("quit", None);
        exit_action.connect_activate(|_a, _b| {
            gio::Application::get_default()
                .expect("no default Application!")
                .quit();
        });
        app.add_action(&exit_action);

        let mut file_views = Vec::<FileViewWorkbench>::new();
        let window = ApplicationWindow::new(app);

        let notebook = Notebook::new();
        notebook.set_hexpand(true);
        notebook.set_vexpand(true);
        window.add(&notebook);

        let tx = tx.clone();
        rx.attach(None, move |msg| {
            match msg {
                Msg::WorkbenchMsg(idx, msg) => {
                    if let Some(tab) = file_views.get_mut(idx) {
                        tab.update(msg);
                    }
                }
                Msg::CloseTab(idx) => {
                    if let Some(page) = notebook.get_nth_page(Some(idx as u32)) {
                        file_views.remove(idx as usize);
                        notebook.detach_tab(&page);
                    }
                }
                Msg::CreateTab(tab) => {
                    let idx = file_views.len();
                    let (tab_header, file_view) = create_tab(tab, tx.clone(), idx);
                    notebook.append_page(file_view.view(), Some(&tab_header));
                    file_views.push(file_view);
                    notebook.show_all();
                }
            }
            glib::Continue(true)
        });


        window.set_title("Log Viewer");
        window.set_default_size(800, 600);

        let menu_model = gio::Menu::new();
        menu_model.append_item(&gio::MenuItem::new(Some("Open"), Some("app.open")));
        menu_model.append_item(&gio::MenuItem::new(Some("Quit"), Some("app.quit")));
        menu_model.append_item(&gio::MenuItem::new(Some("Kube"), Some("app.kube")));

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
