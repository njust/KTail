use gtk::prelude::*;
use gio::prelude::*;
use gtk::{Application, ApplicationWindow, FileChooserDialogBuilder, FileChooserAction, ResponseType};
use gio::{SimpleAction};
use glib::Sender;
use crate::model::{Msg, PodSelectorMsg, CreateLogView, LogViewData};
use crate::k8s_client::KubeConfig;
use crate::util::{get_app_icon, send_msg};

pub fn create_open_file_dlg_action(tx: Sender<Msg>) -> SimpleAction {
    let open_action = SimpleAction::new("open", None);
    open_action.connect_activate(move |_a, _b| {
        let dialog = FileChooserDialogBuilder::new()
            .title("Open File")
            .action(FileChooserAction::Open)
            .icon(&get_app_icon())
            .build();

        dialog.add_button("_Cancel", ResponseType::Cancel);
        dialog.add_button("_Open", ResponseType::Accept);
        let res = dialog.run();
        dialog.close();
        if res == ResponseType::Accept {
            if let Some(file_path) = dialog.get_filename() {
                send_msg(&tx, CreateLogView::new(LogViewData::File(file_path)));
            }
        }
    });
    open_action
}

#[cfg(target_os = "macos")]
pub fn configure_menu(tx: Sender<Msg>, app: &Application, _window: &ApplicationWindow, main: &gtk::Box) {
    let menu_bar = gtk::MenuBar::new();
    let file_menu_item = gtk::MenuItem::with_label("File");
    menu_bar.append(&file_menu_item);

    let file_menu = gtk::Menu::new();
    file_menu_item.set_submenu(Some(&file_menu));

    let open_action = create_open_file_dlg_action(tx.clone());
    app.add_action(&open_action);

    if std::path::Path::new(&KubeConfig::default_path()).exists() {
        let kube_action = SimpleAction::new("kube", None);
        app.add_action(&kube_action);
        let pod_selector_tx = tx.clone();
        kube_action.connect_activate(move |_,_| {
            pod_selector_tx.send(Msg::PodSelectorMsg(PodSelectorMsg::Show)).expect("Could not send pod selector msg");
        });

        app.set_accels_for_action("app.kube", &["<Primary>K"]);
        let kube_menu_item = gtk::MenuItem::with_label("Kubernetes");
        kube_menu_item.set_action_name(Some("app.kube"));
        file_menu.append(&kube_menu_item);
    }

    app.set_accels_for_action("app.open", &["<Primary>O"]);
    let open_menu_item = gtk::MenuItem::with_label("Open");
    open_menu_item.set_action_name(Some("app.open"));
    file_menu.append(&open_menu_item);

    let quit_menu_item = gtk::MenuItem::with_label("Quit");
    quit_menu_item.set_action_name(Some("app.quit"));
    file_menu.append(&quit_menu_item);

    main.add(&menu_bar);
}

#[cfg(not(target_os = "macos"))]
pub fn configure_menu(tx: Sender<Msg>, app: &Application, window: &ApplicationWindow, _main: &gtk::Box) {
    use gtk::{MenuButtonBuilder, HeaderBarBuilder, ReliefStyle, IconSize};
    let menu_model = gio::Menu::new();

    if std::path::Path::new(&KubeConfig::default_path()).exists() {
        let kube_action = SimpleAction::new("kube", None);
        app.add_action(&kube_action);
        let pod_selector_tx = tx.clone();
        kube_action.connect_activate(move |_,_| {
            pod_selector_tx.send(Msg::PodSelectorMsg(PodSelectorMsg::Show)).expect("Could not send pod selector msg");
        });

        app.set_accels_for_action("app.kube", &["<Primary>K"]);
        menu_model.append_item(&gio::MenuItem::new(Some("Kube"), Some("app.kube")));
    }

    app.set_accels_for_action("app.open", &["<Primary>O"]);
    menu_model.append_item(&gio::MenuItem::new(Some("Open"), Some("app.open")));
    menu_model.append_item(&gio::MenuItem::new(Some("Quit"), Some("app.quit")));

    let menu_button = MenuButtonBuilder::new()
        .relief(ReliefStyle::None)
        .popup(&gtk::Menu::from_model(&menu_model))
        .image(&gtk::Image::from_icon_name(Some("open-menu-symbolic"), IconSize::Menu))
        .build();

    let header_bar = HeaderBarBuilder::new()
        .show_close_button(true)
        .title("Log viewer")
        .build();
    header_bar.pack_end(&menu_button);

    window.set_titlebar(Some(&header_bar));
}