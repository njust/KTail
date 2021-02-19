use std::cell::{RefCell, Ref};
use std::rc::Rc;

#[allow(dead_code)]
pub trait CustomWidget: Sized + 'static {
    type Msg: Clone;
    type Input: Default;

    fn init(data: Self::Input) -> Self;
    fn create<T: 'static + Clone + Fn(Self::Msg)>(&self, tx: T);
    fn view(&self) -> &gtk::Box;
    fn update(&mut self, msg: Self::Msg);
    fn new() -> WidgetWrapper<Self> {
        WidgetWrapper::<Self>::new()
    }

    fn with_options<T: 'static + Clone + Fn(Self::Msg)>(tx: T, data: Self::Input) -> WidgetWrapper<Self> {
        WidgetWrapper::<Self>::with_options(tx, data)
    }
}

pub struct WidgetWrapper<T: CustomWidget> {
    view: Rc<RefCell<T>>
}

impl<W: 'static + CustomWidget> WidgetWrapper<W> {
    pub fn new() -> WidgetWrapper<W> {
        Self::with_options(|_| {}, W::Input::default())
    }

    pub fn with_options<T: 'static + Clone + Fn(W::Msg)>(parent_tx: T, data: W::Input) -> WidgetWrapper<W> {
        let widget_view = Rc::new(RefCell::new(W::init(data)));
        let view = widget_view.clone();
        let widget_tx = Rc::new(move |msg: W::Msg| {
            view.borrow_mut().update(msg.clone());
            parent_tx(msg);
        });

        let tx = widget_tx.clone();
        let view = widget_view.clone();
        view.borrow().create(move |msg| {
            tx(msg);
        });

        Self {
            view: widget_view
        }
    }

    pub fn get(&self) -> Ref<W> {
        self.view.borrow()
    }
}