use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Clone)]
pub enum Event { Partial(String), Final(String), Replace { backspace: usize, text: String }, Ended, Error(String) }

pub struct Fake { pub audio: mpsc::Receiver<Vec<f32>>, pub events: mpsc::Sender<Event>, pub cancel: oneshot::Receiver<()> }
impl Fake {
    pub async fn run(mut self) {
        tokio::select! {
            _ = &mut self.cancel => return,
            result = self.audio.recv() => {
                if result.is_none() { let _ = self.events.send(Event::Partial("hello ".into())).await; let _ = self.events.send(Event::Final("world".into())).await; let _ = self.events.send(Event::Ended).await; return; }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                while self.audio.recv().await.is_some() {}
                let _ = self.events.send(Event::Partial("hellp".into())).await;
                let _ = self.events.send(Event::Replace { backspace: 1, text: "o ".into() }).await;
                let _ = self.events.send(Event::Final("world".into())).await;
                let _ = self.events.send(Event::Ended).await;
            }
        }
    }
}
