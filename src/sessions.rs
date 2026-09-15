use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SessionStore {
    visits: HashMap<String, usize>,
    next_id: u64,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            visits: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn visit(&mut self, cookie_header: Option<&str>) -> (String, usize, bool) {
        if let Some(session_id) = get_session_id(cookie_header) {
            if let Some(visits) = self.visits.get_mut(&session_id) {
                *visits += 1;
                return (session_id, *visits, false);
            }
        }

        self.next_id += 1;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let session_id = format!("{timestamp}-{}", self.next_id);

        self.visits.insert(session_id.clone(), 1);

        (session_id, 1, true)
    }
}

fn get_session_id(cookie_header: Option<&str>) -> Option<String> {
    let cookie_header = cookie_header?;

    for cookie in cookie_header.split(';') {
        let (name, value) = cookie.trim().split_once('=')?;

        if name == "session_id" {
            return Some(value.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_new_session() {
        let mut sessions = SessionStore::new();

        let (_, visits, is_new) = sessions.visit(None);

        assert_eq!(visits, 1);
        assert!(is_new);
    }

    #[test]
    fn remembers_an_existing_session() {
        let mut sessions = SessionStore::new();

        let (session_id, _, _) = sessions.visit(None);

        let cookie = format!("session_id={session_id}");
        let (_, visits, is_new) = sessions.visit(Some(&cookie));

        assert_eq!(visits, 2);
        assert!(!is_new);
    }
}
