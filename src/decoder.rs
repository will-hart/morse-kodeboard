use defmt::{Format, info};
use embassy_time::Instant;

#[derive(Clone, Copy, Debug, Format, Default, Eq, PartialEq)]
enum MorseValue {
    #[default]
    Empty,
    Dit,
    Dah,
    Break,
}

#[derive(Clone, Copy, Debug)]
enum MorseDecodingResult {
    Char(char),
    Error,
    NotReady,
}

/// Buffer size here is six - the longest possible character + break
const BUFFER_SIZE: usize = 6;

pub struct Decoder {
    /// The length of a dit  
    pub dit_ms: u64,

    /// Holds the dits and dahs for the current character
    value_buffer: [MorseValue; BUFFER_SIZE],
    /// The current index we're editing in the value buffer
    index: usize,
    /// Whether the signal is currently high or low
    is_high: bool,
    /// When the signal last changed
    time_last_changed: Instant,
}

impl Decoder {
    pub fn new(dit_ms: u64) -> Self {
        Self {
            dit_ms,
            value_buffer: [MorseValue::Empty; BUFFER_SIZE],
            index: 0,
            is_high: true,
            time_last_changed: Instant::now(),
        }
    }
}

/// Private methods
impl Decoder {
    fn has_break(&self) -> bool {
        self.value_buffer.contains(&MorseValue::Break)
    }

    fn buffer_to_char(&self) -> MorseDecodingResult {
        if !self.has_break() {
            return MorseDecodingResult::NotReady;
        }

        use MorseValue::*;
        if let Some(c) = match self.value_buffer {
            [Dit, Dah, Break, Empty, Empty, Empty] => Some('a'),
            [Dah, Dit, Dit, Dit, Break, Empty] => Some('b'),
            [Dah, Dit, Dah, Dit, Break, Empty] => Some('c'),
            [Dah, Dit, Dit, Break, Empty, Empty] => Some('d'),
            [Dit, Break, Empty, Empty, Empty, Empty] => Some('e'),
            [Dit, Dit, Dah, Dit, Break, Empty] => Some('f'),
            [Dah, Dah, Dit, Break, Empty, Empty] => Some('g'),
            [Dit, Dit, Dit, Dit, Break, Empty] => Some('h'),
            [Dit, Dit, Break, Empty, Empty, Empty] => Some('i'),
            [Dit, Dah, Dah, Dah, Break, Empty] => Some('j'),
            [Dah, Dit, Dah, Break, Empty, Empty] => Some('k'),
            [Dit, Dah, Dit, Dit, Break, Empty] => Some('l'),
            [Dah, Dah, Break, Empty, Empty, Empty] => Some('m'),
            [Dah, Dit, Break, Empty, Empty, Empty] => Some('n'),
            [Dah, Dah, Dah, Break, Empty, Empty] => Some('o'),
            [Dit, Dah, Dah, Dit, Break, Empty] => Some('p'),
            [Dah, Dah, Dit, Dah, Break, Empty] => Some('q'),
            [Dit, Dah, Dit, Break, Empty, Empty] => Some('r'),
            [Dit, Dit, Dit, Break, Empty, Empty] => Some('s'),
            [Dah, Break, Empty, Empty, Empty, Empty] => Some('t'),
            [Dit, Dit, Dah, Break, Empty, Empty] => Some('u'),
            [Dit, Dit, Dit, Dah, Break, Empty] => Some('v'),
            [Dit, Dah, Dah, Break, Empty, Empty] => Some('w'),
            [Dah, Dit, Dit, Dah, Break, Empty] => Some('x'),
            [Dah, Dit, Dah, Dah, Break, Empty] => Some('y'),
            [Dah, Dah, Dit, Dit, Break, Empty] => Some('z'),
            [Dit, Dah, Dah, Dah, Dah, Break] => Some('1'),
            [Dit, Dit, Dah, Dah, Dah, Break] => Some('2'),
            [Dit, Dit, Dit, Dah, Dah, Break] => Some('3'),
            [Dit, Dit, Dit, Dit, Dah, Break] => Some('4'),
            [Dit, Dit, Dit, Dit, Dit, Break] => Some('5'),
            [Dah, Dit, Dit, Dit, Dit, Break] => Some('6'),
            [Dah, Dah, Dit, Dit, Dit, Break] => Some('7'),
            [Dah, Dah, Dah, Dit, Dit, Break] => Some('8'),
            [Dah, Dah, Dah, Dah, Dit, Break] => Some('9'),
            [Dah, Dah, Dah, Dah, Dah, Break] => Some('0'),
            _ => None,
        } {
            MorseDecodingResult::Char(c)
        } else {
            // info!(
            //     "Unknown encoding [{},{},{},{},{},{}]",
            //     self.value_buffer[0],
            //     self.value_buffer[1],
            //     self.value_buffer[2],
            //     self.value_buffer[3],
            //     self.value_buffer[4],
            //     self.value_buffer[5]
            // );
            MorseDecodingResult::Error
        }
    }

    /// Resets the buffer ready for the next character
    fn reset_buffer(&mut self) {
        self.index = 0;
        self.value_buffer = [MorseValue::Empty; BUFFER_SIZE];
    }

    /// Pushes an item on to the value buffer. If the  
    fn push_buffer_item(&mut self, value: MorseValue) {
        if self.index == BUFFER_SIZE {
            // we're overflowing the buffer, start popping items off the front
            for idx in 1..BUFFER_SIZE {
                self.value_buffer[idx - 1] = self.value_buffer[idx];
            }
            self.value_buffer[self.index - 1] = value;
        } else {
            // we're still filling out the buffer, just add the item and increment the index
            self.value_buffer[self.index] = value;
            self.index += 1;
        }
    }
}

/// Public inteface
impl Decoder {
    /// Takes in an input and attempts to parse it into morse code dits and dahs.
    ///  Returns `Some(char)` if a character is ready and  None if no character is ready
    ///
    /// A character is delineated by a "break" (or a low signal) at least as long
    /// as 7x the length of the dit.  This may either be explicit (as in measuring
    /// the time between low and high signals) or may occur if the buffer has some values
    /// and there has been a long enough delay with the marker in a low state.
    pub fn push(&mut self, currently_high: bool, change_time: Instant) -> Option<char> {
        if self.is_high && currently_high {
            // nop
            return None;
        }

        let elapsed_in_dits = (change_time - self.time_last_changed).as_millis() / self.dit_ms;

        let is_high = self.is_high;
        self.is_high = currently_high;

        match (is_high, currently_high) {
            (true, true) => {
                // nop above, but for completeness
                unreachable!();
            }
            (true, false) => {
                self.time_last_changed = change_time;

                // falling edge, we've either added a dit or a dah
                self.push_buffer_item(if elapsed_in_dits <= 2 {
                    info!(".");
                    MorseValue::Dit
                } else {
                    info!("_");
                    MorseValue::Dah
                });

                // no character to return here as we're waiting on a break
                return None;
            }
            (false, true) => {
                self.time_last_changed = change_time;

                // rising edge - if we've got a long pause then its a break, otherwise
                // we just record the time and keep listening for the next dit or dah
                if elapsed_in_dits < 7 {
                    return None;
                }

                info!("BREAK");
            }
            (false, false) => {
                // if we've been low for ages, consider this a break. We don't
                // do this if the index is 0 because this just means we're idle
                if self.index == 0 || elapsed_in_dits < 7 {
                    return None;
                }

                info!("Pseudo-BREAK");
            }
        }

        // if we've got here its either a rising edge or a continuous low signal
        // lets handle it. First push a break
        self.push_buffer_item(MorseValue::Break);

        // then we see what we have in the value_buffer

        match self.buffer_to_char() {
            MorseDecodingResult::Char(c) => {
                info!("Found morse character {}", c);
                self.reset_buffer();
                Some(c)
            }
            MorseDecodingResult::Error => {
                // info!("Found invalid morse buffer");
                self.reset_buffer();
                None
            }
            MorseDecodingResult::NotReady => {
                // info!("Buffer not ready - not sure how we got here :D");
                None
            }
        }
    }
}
