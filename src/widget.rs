use std::cell::{RefCell, Ref};
use std::rc::Rc;
use std::pin::Pin;
use futures::Future;
use glib::{MainContext, Sender};
use gtk::{Orientation};

pub enum WidgetMsg<Widget: CustomWidget> {
    None,
    Out(Widget::Output),
    Defer(Pin<Box<dyn Future<Output = Widget::Msg> + 'static>>)
}

#[allow(dead_code)]
pub trait CustomWidget: Sized + 'static {
    type Msg: Clone;
    type Output: Clone;
    type Input: Default;

    fn init(data: Self::Input) -> Self;
    fn create(&self, container: &gtk::Box, tx: Sender<Self::Msg>);
    fn update(&mut self, msg: Self::Msg) -> WidgetMsg<Self>;
    fn new() -> WidgetWrapper<Self> {
        WidgetWrapper::<Self>::new()
    }

    fn container() -> gtk::Box {
        gtk::Box::new(Orientation::Vertical, 0)
    }

    fn run_async<T: Future<Output = Self::Msg> + 'static>(&self, t: T) -> WidgetMsg<Self> {
        WidgetMsg::Defer(Box::pin(t))
    }

    fn msg_none(&self) -> WidgetMsg<Self> {
        WidgetMsg::None
    }

    fn msg_out(&self, msg: Self::Output) -> WidgetMsg<Self> {
        WidgetMsg::Out(msg)
    }

    fn with_options<T: 'static + Clone + Fn(Self::Output)>(tx: T, data: Self::Input) -> WidgetWrapper<Self> {
        WidgetWrapper::<Self>::with_options(tx, data)
    }
}

pub struct WidgetWrapper<T: CustomWidget> {
    view: Rc<RefCell<T>>,
    container: gtk::Box,
}

#[allow(dead_code)]
impl<W: 'static + CustomWidget> WidgetWrapper<W> {
    pub fn new() -> WidgetWrapper<W> {
        Self::with_options(|_| {}, W::Input::default())
    }

    pub fn with_options<T: 'static + Clone + Fn(W::Output)>(out_tx: T, data: W::Input) -> WidgetWrapper<W> {
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let widget_view = Rc::new(RefCell::new(W::init(data)));
        let view = widget_view.clone();
        let container = W::container();
        view.borrow().create(&container, tx.clone());

        let tx = tx.clone();
        rx.attach(None, move |msg| {
            let out_msg = view.borrow_mut().update(msg);
            match out_msg {
                WidgetMsg::Out(msg) => {
                    out_tx(msg);
                }
                WidgetMsg::Defer(f) => {
                    let tx = tx.clone();
                    MainContext::ref_thread_default().spawn_local(async move {
                        tx.send(f.await).expect("Could not send msg");
                    });
                }
                WidgetMsg::None => ()
            }
            glib::Continue(true)
        });

        Self {
            view: widget_view,
            container
        }
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }

    pub fn get(&self) -> Ref<W> {
        self.view.borrow()
    }
}