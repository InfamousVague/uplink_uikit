use std::rc::Rc;

use dioxus::prelude::*;

#[derive(Clone)]
pub struct LifeCycle<D: FnOnce() + Clone> {
    ondestroy: Option<D>,
}

pub fn use_component_lifecycle<C, D>(create: C, destroy: D) -> Rc<LifeCycle<D>>
where
    C: FnOnce() + 'static + Clone,
    D: FnOnce() + 'static + Clone,
{
    use_hook(|| {
        spawn(async move {
            // This will be run once the component is mounted
            std::future::ready::<()>(()).await;
            create();
        });

        Rc::new(LifeCycle {
            ondestroy: Some(destroy),
        })
    })
}

impl<D: FnOnce() + Clone> Drop for LifeCycle<D> {
    fn drop(&mut self) {
        if let Some(f) = self.ondestroy.take() {
            f();
        }
    }
}
