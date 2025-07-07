//! Contains code for firmware debouncing of inputs using 12 consecutive checks.
//! The value `1` is considered "on".

const ON: u16 = 0b1111111111111111;
const OFF: u16 = 0;

/// A debounced input that checks `N` times whether a switch is "on" before
/// returning that it is "on".
#[derive(Clone, Copy)]
pub struct DebouncedInput {
    memory: u16,
    previous_state: bool,
}

impl DebouncedInput {
    pub fn new(is_on: bool) -> Self {
        Self {
            memory: if is_on { ON } else { OFF },
            previous_state: is_on,
        }
    }

    pub fn current(&self) -> bool {
        self.previous_state
    }

    /// Debounces the given input taking the current value and returning `true`
    /// if the input is on after debouncing. Note that "on" may be high or low
    /// in hardware, but a boolean should be passed here which is `true` if the
    /// input is currently on
    pub fn debounce(&mut self, is_on: bool) -> bool {
        self.memory = (self.memory << 1) | if is_on { 0 } else { 1 };

        if self.memory == ON {
            true
        } else if self.memory == OFF {
            false
        } else {
            self.previous_state
        }
    }
}
