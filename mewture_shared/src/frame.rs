//! Reassembles DDAA frames from a serial byte stream.
//!
//! A single `read()` from a serial port can return part of a frame, or several frames at once.
//! `FrameAccumulator` buffers the bytes between reads and hands back one complete frame at a time.
//!
//! The end of a frame is found using the data length byte, then the stop bytes are checked at that position.
//! The protocol has no escaping, so scanning for the stop bytes would split frames whose data contains `0x17 0x04`.

const START_BYTES: [u8; 3] = [0x07, 0x06, 0x15]; // Request, response success, response failure.
const START_BYTE_2: u8 = 0x01;
const STOP_BYTES: [u8; 2] = [0x17, 0x04];
const LENGTH_OFFSET: usize = 4;
const FRAME_OVERHEAD: usize = 7; // 2 start + command + variable + length + 2 stop.
const MAX_DATA_LENGTH: usize = 32;

#[derive(Debug, Default)]
pub struct FrameAccumulator {
    buffer: Vec<u8>,
}

impl FrameAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add bytes read from the serial port.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Drop anything buffered (e.g. after a reconnect).
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Take the next complete frame, if there is one. Bytes that can't be part of a frame are discarded.
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        loop {
            // Skip to the next start byte pair.
            match self
                .buffer
                .windows(2)
                .position(|pair| START_BYTES.contains(&pair[0]) && pair[1] == START_BYTE_2)
            {
                Some(start) => {
                    self.buffer.drain(..start);
                }
                None => {
                    // No frame start. Keep a trailing start byte, its second start byte may be in the next read.
                    let keep = matches!(self.buffer.last(), Some(byte) if START_BYTES.contains(byte));
                    let discard = self.buffer.len() - usize::from(keep);
                    self.buffer.drain(..discard);
                    return None;
                }
            }

            if self.buffer.len() <= LENGTH_OFFSET {
                return None; // Need more bytes to know the length.
            }

            let data_length = self.buffer[LENGTH_OFFSET] as usize;
            if data_length > MAX_DATA_LENGTH {
                // Not a valid frame, skip this start byte and look again.
                self.buffer.drain(..1);
                continue;
            }

            let frame_length = FRAME_OVERHEAD + data_length;
            if self.buffer.len() < frame_length {
                return None; // Rest of the frame hasn't arrived yet.
            }

            if self.buffer[frame_length - 2..frame_length] == STOP_BYTES {
                return Some(self.buffer.drain(..frame_length).collect());
            }

            // Stop bytes aren't where the length says they should be, so this wasn't a real frame start.
            self.buffer.drain(..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MUTE: [u8; 8] = [0x07, 0x01, 0x1A, 0x00, 0x01, 0x01, 0x17, 0x04];
    const UNMUTE: [u8; 8] = [0x07, 0x01, 0x1A, 0x00, 0x01, 0x00, 0x17, 0x04];

    fn frames(acc: &mut FrameAccumulator) -> Vec<Vec<u8>> {
        std::iter::from_fn(|| acc.next_frame()).collect()
    }

    #[test]
    fn single_frame() {
        let mut acc = FrameAccumulator::new();
        acc.push(&MUTE);
        assert_eq!(frames(&mut acc), vec![MUTE.to_vec()]);
    }

    #[test]
    fn frame_split_across_reads() {
        let mut acc = FrameAccumulator::new();
        acc.push(&MUTE[..1]);
        assert_eq!(acc.next_frame(), None);
        acc.push(&MUTE[1..3]);
        assert_eq!(acc.next_frame(), None);
        acc.push(&MUTE[3..]);
        assert_eq!(acc.next_frame(), Some(MUTE.to_vec()));
    }

    #[test]
    fn multiple_frames_in_one_read() {
        let mut acc = FrameAccumulator::new();
        acc.push(&[MUTE, UNMUTE].concat());
        assert_eq!(frames(&mut acc), vec![MUTE.to_vec(), UNMUTE.to_vec()]);
    }

    #[test]
    fn garbage_is_skipped() {
        let mut acc = FrameAccumulator::new();
        acc.push(&[0x00, 0x55, 0x01, 0x17, 0x04]);
        acc.push(&MUTE);
        assert_eq!(frames(&mut acc), vec![MUTE.to_vec()]);
    }

    #[test]
    fn data_containing_stop_bytes() {
        // Data looks like the end of a frame followed by a whole mute frame.
        let mut frame = vec![0x07, 0x01, 0x1A, 0x05, 0x0A, 0x17, 0x04];
        frame.extend_from_slice(&MUTE);
        frame.extend_from_slice(&[0x17, 0x04]);
        let mut acc = FrameAccumulator::new();
        acc.push(&frame);
        acc.push(&UNMUTE);
        assert_eq!(frames(&mut acc), vec![frame, UNMUTE.to_vec()]);
    }

    #[test]
    fn bad_stop_bytes_resync() {
        let mut acc = FrameAccumulator::new();
        acc.push(&[0x07, 0x01, 0x1A, 0x00, 0x01, 0x00, 0x99, 0x99]);
        acc.push(&MUTE);
        assert_eq!(frames(&mut acc), vec![MUTE.to_vec()]);
    }

    #[test]
    fn oversized_length_resync() {
        let mut acc = FrameAccumulator::new();
        acc.push(&[0x07, 0x01, 0x1A, 0x00, 0x50]);
        acc.push(&MUTE);
        assert_eq!(frames(&mut acc), vec![MUTE.to_vec()]);
    }

    #[test]
    fn nak_frame() {
        let nak = [0x15, 0x01, 0x1A, 0x00, 0x01, 0x09, 0x17, 0x04];
        let mut acc = FrameAccumulator::new();
        acc.push(&nak);
        assert_eq!(frames(&mut acc), vec![nak.to_vec()]);
    }

    #[test]
    fn buffer_does_not_grow_on_garbage() {
        let mut acc = FrameAccumulator::new();
        for _ in 0..100 {
            acc.push(&[0x42; 64]);
            assert_eq!(acc.next_frame(), None);
        }
        assert!(acc.buffer.is_empty());
    }

    #[test]
    fn clear_drops_partial_frame() {
        let mut acc = FrameAccumulator::new();
        acc.push(&MUTE[..4]);
        acc.clear();
        acc.push(&UNMUTE);
        assert_eq!(frames(&mut acc), vec![UNMUTE.to_vec()]);
    }
}
