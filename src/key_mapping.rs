use defmt::warn;

pub fn char_to_hid_u8(c: char) -> Option<u8> {
    match c {
        'a' => Some(0x04),
        'b' => Some(0x05),
        'c' => Some(0x06),
        'd' => Some(0x07),
        'e' => Some(0x08),
        'f' => Some(0x09),
        'g' => Some(0x0A),
        'h' => Some(0x0B),
        'i' => Some(0x0C),
        'j' => Some(0x0D),
        'k' => Some(0x0E),
        'l' => Some(0x0F),
        'm' => Some(0x10),
        'n' => Some(0x11),
        'o' => Some(0x12),
        'p' => Some(0x13),
        'q' => Some(0x14),
        'r' => Some(0x15),
        's' => Some(0x16),
        't' => Some(0x17),
        'u' => Some(0x18),
        'v' => Some(0x19),
        'w' => Some(0x1A),
        'x' => Some(0x1B),
        'y' => Some(0x1C),
        'z' => Some(0x1D),
        '0' => Some(0x27),
        '1' => Some(0x1E),
        '2' => Some(0x1F),
        '3' => Some(0x20),
        '4' => Some(0x21),
        '5' => Some(0x22),
        '6' => Some(0x23),
        '7' => Some(0x24),
        '8' => Some(0x25),
        '9' => Some(0x26),
        ' ' => Some(0x2C),
        c => {
            warn!("unsupported character: {}", c);
            None
        }
    }
}
