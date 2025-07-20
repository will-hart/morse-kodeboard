# Morse Kodeboard

I can type 1,250 words per minute on a keyboard with only the minor side effects
of deafening people within 50 meters and wearing out the delete key inside a week.
It occurs to me that If a TKL keyboard makes me 82.24563x more of an engineer,
then how much more engineer would you become by removing 86 of the other keys?

Of course disruption requires innovation, and clearly you can't be a 1,000,000x
engineer with just an "a" key. I considered using an LLM to interpret my intentions
from the velocity and anger with which the key was pressed, but then after a
period of quiet reflection I realised training an AI to do this would immediately
make it sentient.

So to save all of humanity from an AI overlord, I instead applied my vast and
terrifying engineering skills to come up with something the likes of which
has never been seen before. This technological marvel is both simple and truly
disruptive. I invented the idea of using a sequence of long and short pulses to
represent the characters I want to type. I decided to call it a "Morse" coding
sequence, named after [Inspector Morse](https://www.imdb.com/title/tt0092379/)
because I once lived somewhere where an episode was filmed.

This new type of keyboard is clearly more than a keyboard, as without it being
a 1,000,000x engineer clearly isn't possible. So in honor of the great Inspector,
I will call this marvel of modern science a

> **Morse Kodeboard**.

## TLDR

Somehow somebody thought of this idea and I though it would be fun. This is (rust)
firmware for a RP Pico that uses a ~single button~ three buttons and morse code
and pretends to be a USB HID keyboard. Technically it doesn't have a delete button
so good luck if you morse code wrong, which is highly likely because I don't even
know morse code.

## License

* Software: MIT or Apache 2.0
* Hardware: CERN Open Hardware 2.0 - Permissive

## Things I (re/)learned while doing this

- USB Descriptors are always a pain, but I guess with embassy its kind of straightforward?
- Most embassy examples don't use tasks for USB. Its because the code is messier
  as you need to make a lot of the descriptor buffers etc `static`. Using
  `StaticCell` was pretty handy here (there was one embassy example).
- Wireshark + USBpcap are your friend! (although its still tricky working out which
  interface you're meant to look at in Windows).
  - Filter messages to a specific address using `usb.addr[0:4] == "1.1."`
- Make sure to throttle loops, this was the cause of the keyboard not being recognised
  as the USB listening loop was running without any throttling.
  
