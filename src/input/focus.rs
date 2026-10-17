use std::cell::RefCell;

use std::rc::Rc;

use crate::Key;

use super::KeyHandler;

// what a text input does with pasted text
pub type PasteHandler = Rc<dyn Fn(&str)>;

// the text input that typed keys go to, and what it does with them
struct Focus {
    id: &'static str,
    on_key: KeyHandler,
    on_paste: PasteHandler,
}

thread_local! {
    // only one text input takes keys at a time
    static FOCUS: RefCell<Option<Focus>> = const { RefCell::new(None) };
}

pub fn set(id: &'static str, on_key: KeyHandler, on_paste: PasteHandler) {
    FOCUS.set(Some(Focus { id, on_key, on_paste }));
}

// whether some text input takes keys right now
pub fn active() -> bool {
    FOCUS.with_borrow(Option::is_some)
}

// returns whether a text input took the text
pub fn paste(text: &str) -> bool {
    // copied out first, so the handler is free to change the focus itself
    let on_paste = FOCUS.with_borrow(|focus| focus.as_ref().map(|focus| focus.on_paste.clone()));

    let Some(on_paste) = on_paste else {
        return false;
    };

    on_paste(text);

    true
}

pub fn clear() {
    FOCUS.set(None);
}

pub fn has(id: &'static str) -> bool {
    FOCUS.with_borrow(|focus| matches!(focus, Some(focus) if focus.id == id))
}

// returns whether a text input took the key
pub fn send(key: Key) -> bool {
    // a text input has no use for these, so they go on to the window's on_key
    if matches!(key, Key::Up | Key::Down | Key::Tab | Key::Other) {
        return false;
    }

    // copied out first, so the handler is free to change the focus itself
    let on_key = FOCUS.with_borrow(|focus| focus.as_ref().map(|focus| focus.on_key.clone()));

    let Some(on_key) = on_key else {
        return false;
    };

    on_key(key);

    // escape leaves the input and still reaches the window, which may want to close
    key != Key::Escape
}
