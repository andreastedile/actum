use futures::channel::mpsc;

pub struct MessageReceiver<M, D> {
    pub(crate) m_receiver: mpsc::Receiver<M>,
    pub(crate) dependency: D,
}

impl<M, D> MessageReceiver<M, D> {
    pub const fn new(m_receiver: mpsc::Receiver<M>, dependency: D) -> Self {
        Self { m_receiver, dependency }
    }
}
