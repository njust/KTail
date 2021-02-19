use std::cell::{RefCell, Ref};
use std::rc::Rc;

#[allow(dead_code)]
pub trait CustomWidget: Sized + 'static {
    type Msg: Clone;
    fn init() -> Self;
    fn create<T: 'static + Clone + Fn(Self::Msg)>(&self, tx: T);
    fn view(&self) -> &gtk::Box;
    fn update(&mut self, msg: Self::Msg);
    fn new() -> WidgetWrapper<Self> {
        WidgetWrapper::<Self>::new()
    }

    fn new_with_events<T: 'static + Clone + Fn(Self::Msg)>(tx: T) -> WidgetWrapper<Self> {
        WidgetWrapper::<Self>::new_with_events(tx)
    }
}

pub struct WidgetWrapper<T: CustomWidget> {
    view: Rc<RefCell<T>>
}

impl<W: 'static + CustomWidget> WidgetWrapper<W> {
    pub fn new() -> WidgetWrapper<W> {
        Self::new_with_events(|_| {})
    }

    pub fn new_with_events<T: 'static + Clone + Fn(W::Msg)>(parent_tx: T) -> WidgetWrapper<W> {
        let widget_view = Rc::new(RefCell::new(W::init()));
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