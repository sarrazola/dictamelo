//! Platform-independent shortcut edges. Native callbacks feed one ordered worker.

use crate::{settings::RecordingMode, status::Status};

#[derive(Clone, Copy, Debug)]
pub enum Event { Pressed, Released, Cancel, Deadline(u64), Reset }

#[derive(Debug, PartialEq)]
pub enum Action { Start, Stop, None }

#[derive(Default)]
pub struct Edges { down: bool }

impl Edges {
    pub fn handle(&mut self, event: Event, mode: RecordingMode, status: &Status) -> Action {
        match event {
            Event::Pressed if !self.down => {
                self.down = true;
                if matches!(status, Status::Recording) && mode == RecordingMode::Toggle {
                    Action::Stop
                } else if !status.is_busy() { Action::Start } else { Action::None }
            }
            Event::Released if self.down => {
                self.down = false;
                if matches!(status, Status::Recording) && mode == RecordingMode::Hold {
                    Action::Stop
                } else { Action::None }
            }
            _ => Action::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_stops_only_on_its_matching_release() {
        let mut e = Edges::default();
        assert_eq!(e.handle(Event::Released, RecordingMode::Hold, &Status::Recording), Action::None);
        assert_eq!(e.handle(Event::Pressed, RecordingMode::Hold, &Status::Idle), Action::Start);
        assert_eq!(e.handle(Event::Pressed, RecordingMode::Hold, &Status::Recording), Action::None);
        assert_eq!(e.handle(Event::Released, RecordingMode::Hold, &Status::Recording), Action::Stop);
    }

    #[test]
    fn toggle_ignores_release_and_repeat_then_stops_on_next_press() {
        let mut e = Edges::default();
        assert_eq!(e.handle(Event::Pressed, RecordingMode::Toggle, &Status::Idle), Action::Start);
        assert_eq!(e.handle(Event::Pressed, RecordingMode::Toggle, &Status::Recording), Action::None);
        assert_eq!(e.handle(Event::Released, RecordingMode::Toggle, &Status::Recording), Action::None);
        assert_eq!(e.handle(Event::Pressed, RecordingMode::Toggle, &Status::Recording), Action::Stop);
        assert_eq!(e.handle(Event::Released, RecordingMode::Toggle, &Status::Transcribing), Action::None);
        assert_eq!(e.handle(Event::Pressed, RecordingMode::Toggle, &Status::Idle), Action::Start);
    }

    #[test]
    fn cancel_and_deadline_do_not_turn_repeat_into_a_new_recording() {
        for mode in [RecordingMode::Hold, RecordingMode::Toggle] {
            let mut e = Edges::default();
            assert_eq!(e.handle(Event::Pressed, mode, &Status::Idle), Action::Start);
            let done = Status::Done { message: String::new() };
            assert_eq!(e.handle(Event::Pressed, mode, &done), Action::None);
            assert_eq!(e.handle(Event::Released, mode, &done), Action::None);
            assert_eq!(e.handle(Event::Pressed, mode, &done), Action::Start);
        }
    }

    #[test]
    fn press_while_transcribing_cannot_be_replayed_after_completion() {
        let mut e = Edges::default();
        assert_eq!(e.handle(Event::Pressed, RecordingMode::Toggle, &Status::Transcribing), Action::None);
        assert_eq!(e.handle(Event::Pressed, RecordingMode::Toggle, &Status::Idle), Action::None);
        assert_eq!(e.handle(Event::Released, RecordingMode::Toggle, &Status::Idle), Action::None);
        assert_eq!(e.handle(Event::Pressed, RecordingMode::Toggle, &Status::Idle), Action::Start);
    }

    #[tokio::test]
    async fn quick_edges_wait_for_microphone_startup_in_order() {
        for mode in [RecordingMode::Hold, RecordingMode::Toggle] {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            tx.send(Event::Pressed).unwrap();
            tx.send(Event::Released).unwrap();
            if mode == RecordingMode::Toggle { tx.send(Event::Pressed).unwrap(); }
            drop(tx);
            let mut edges = Edges::default();
            let mut status = Status::Idle;
            let mut actions = Vec::new();
            while let Some(event) = rx.recv().await {
                let action = edges.handle(event, mode, &status);
                match action {
                    Action::Start => {
                        tokio::task::yield_now().await;
                        status = Status::Recording;
                    }
                    Action::Stop => status = Status::Transcribing,
                    Action::None => continue,
                }
                actions.push(action);
            }
            assert_eq!(actions, vec![Action::Start, Action::Stop]);
        }
    }
}
