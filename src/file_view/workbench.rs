use gtk::prelude::*;
use crate::file_view::{FileView};
use crate::file_view::toolbar::FileViewToolbar;
use std::rc::Rc;
use glib::bitflags::_core::cell::RefCell;
use std::path::PathBuf;
use gtk::Orientation;


pub struct FileViewWorkbench {
    toolbar: FileViewToolbar,
    file_view: Rc<RefCell<FileView>>,
    container: gtk::Box,
}

impl FileViewWorkbench {
    pub fn new(path: PathBuf) -> Self  {
        let toolbar = FileViewToolbar::new();
        let file_view = Rc::new(RefCell::new(FileView::new(path)));

        let file = file_view.clone();
        toolbar.on_toggle_autoscroll(move || {
            file.borrow_mut().toggle_autoscroll();
        });

        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.add(toolbar.view());
        container.add(file_view.borrow().view());

        Self {
            toolbar,
            file_view,
            container
        }
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }
}