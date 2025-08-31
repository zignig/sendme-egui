// Incoming events
pub enum Event {
    Message(String),
    Progress((String, f32)),
    Finished
}

// Outgoing Commands
#[derive(Debug)]
pub enum Command {
    Send(String),
    Fetch(String),
}

// Progress bar works
