use gtk4_helper::component::{Command, MsgHandler};
use gtk4_helper::{gtk::{self, prelude::*}, model::prelude::* };
use gtk4_helper::gio::ListStore;
use gtk4_helper::prelude::Component;
use crate::column_view_helper;
use crate::log_view::SearchResultData;

pub struct SearchResultView {
    container: gtk::Box,
    list_store: ListStore
}

#[derive(Clone)]
pub enum SearchResultViewMsg {
    SearchResults(SearchResultData)
}


#[model]
struct SearchResultItem {
    #[field]
    text: String,
    #[field]
    date: String,
}

impl Component for SearchResultView {
    type Msg = SearchResultViewMsg;
    type View = gtk::Box;
    type Input = ();

    fn create<T: MsgHandler<Self::Msg> + Clone>(sender: T, input: Option<Self::Input>) -> Self {
        let (column_view, list_store) =
            column_view_helper::create_column_view(SearchResultItem::static_type(), column_view_helper::single_selection_model);

        column_view_helper::create_label_column(&column_view, SearchResultItem::static_type(), SearchResultItem::date, "Timestamp", column_view_helper::string_sorter);
        column_view_helper::create_label_column(&column_view, SearchResultItem::static_type(), SearchResultItem::text, "Text", column_view_helper::string_sorter);

        let scroll_wnd = gtk::ScrolledWindow::new();
        scroll_wnd.set_vexpand(true);
        scroll_wnd.set_child(Some(&column_view));

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.append(&scroll_wnd);

        Self {
            container,
            list_store
        }

    }

    fn update(&mut self, msg: Self::Msg) -> Command<Self::Msg> {
        match msg {
            SearchResultViewMsg::SearchResults(data) => {
                for (_, datum) in data.lines {
                    let sr = SearchResultItem {
                        text: datum.text.lines().next().unwrap_or("").to_string(),
                        date: datum.timestamp.to_string()
                    };
                    let obj = sr.to_object();
                    self.list_store.append(&obj);
                }
            }
        }
        Command::None
    }

    fn view(&self) -> &Self::View {
        &self.container
    }
}