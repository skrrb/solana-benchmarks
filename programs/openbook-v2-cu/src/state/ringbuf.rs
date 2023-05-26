use super::MAX_NUM_EVENTS;
use anchor_lang::prelude::*;
use openbook_v2::{error::OpenBookError, state::AnyEvent};
use static_assertions::const_assert_eq;

pub trait QueueHeader: bytemuck::Pod {
    type Item: bytemuck::Pod + Copy;

    fn head(&self) -> usize;
    fn set_head(&mut self, value: u32);
    fn count(&self) -> usize;
    fn set_count(&mut self, value: u32);

    fn incr_event_id(&mut self);
    fn decr_event_id(&mut self, n: u64);
}

#[account(zero_copy)]
pub struct EventQueue {
    pub header: EventQueueHeader,
    pub buf: [AnyEvent; MAX_NUM_EVENTS as usize],
    pub reserved: [u8; 64],
}
const_assert_eq!(std::mem::size_of::<EventQueue>(), 16 + 488 * 200 + 64);
const_assert_eq!(std::mem::size_of::<EventQueue>(), 97680);
const_assert_eq!(std::mem::size_of::<EventQueue>() % 8, 0);

impl EventQueue {
    pub fn len(&self) -> usize {
        self.header.count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn full(&self) -> bool {
        self.header.count() == self.buf.len()
    }

    pub fn push_back(&mut self, value: AnyEvent) -> std::result::Result<(), AnyEvent> {
        if self.full() {
            return Err(value);
        }
        let slot = (self.header.head() + self.header.count()) % self.buf.len();
        self.buf[slot] = value;

        let count = self.header.count();
        self.header.set_count((count + 1) as u32); // guaranteed because of full() check

        self.header.incr_event_id();
        Ok(())
    }

    pub fn peek_front(&self) -> Option<&AnyEvent> {
        if self.is_empty() {
            return None;
        }
        Some(&self.buf[self.header.head()])
    }

    pub fn peek_front_mut(&mut self) -> Option<&mut AnyEvent> {
        if self.is_empty() {
            return None;
        }
        Some(&mut self.buf[self.header.head()])
    }

    pub fn pop_front(&mut self) -> Result<AnyEvent> {
        require!(!self.is_empty(), OpenBookError::SomeError);

        let value = self.buf[self.header.head()];

        let count = self.header.count();
        self.header.set_count((count - 1) as u32);

        let head = self.header.head();
        self.header.set_head(((head + 1) % self.buf.len()) as u32);

        Ok(value)
    }

    pub fn revert_pushes(&mut self, desired_len: usize) -> Result<()> {
        require!(desired_len <= self.header.count(), OpenBookError::SomeError);
        let len_diff = self.header.count() - desired_len;
        self.header.set_count(desired_len as u32);
        self.header.decr_event_id(len_diff as u64);
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &AnyEvent> {
        EventQueueIterator {
            queue: self,
            index: 0,
        }
    }
}

struct EventQueueIterator<'a> {
    queue: &'a EventQueue,
    index: usize,
}

impl<'a> Iterator for EventQueueIterator<'a> {
    type Item = &'a AnyEvent;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.queue.len() {
            None
        } else {
            let item =
                &self.queue.buf[(self.queue.header.head() + self.index) % self.queue.buf.len()];
            self.index += 1;
            Some(item)
        }
    }
}

#[zero_copy]
pub struct EventQueueHeader {
    head: u32,
    count: u32,
    pub seq_num: u64,
}
const_assert_eq!(std::mem::size_of::<EventQueueHeader>(), 16);
const_assert_eq!(std::mem::size_of::<EventQueueHeader>() % 8, 0);

impl QueueHeader for EventQueueHeader {
    type Item = AnyEvent;

    fn head(&self) -> usize {
        self.head as usize
    }
    fn set_head(&mut self, value: u32) {
        self.head = value;
    }
    fn count(&self) -> usize {
        self.count as usize
    }
    fn set_count(&mut self, value: u32) {
        self.count = value;
    }
    fn incr_event_id(&mut self) {
        self.seq_num += 1;
    }
    fn decr_event_id(&mut self, n: u64) {
        self.seq_num -= n;
    }
}
