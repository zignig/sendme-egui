// Incoming events
pub enum Event {
    Message(String),
    Progress((String, f32)),
}

// Outgoing Commands
#[derive(Debug)]
pub enum Command {
    Message,
    Send(String),
    Receive(String),
}
