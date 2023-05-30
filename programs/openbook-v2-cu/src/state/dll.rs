use super::MAX_NUM_EVENTS;
use anchor_lang::prelude::*;
use openbook_v2::{error::OpenBookError, state::AnyEvent};
use static_assertions::const_assert_eq;

pub const NULL: u16 = u16::MAX;
pub const LAST_SLOT: usize = MAX_NUM_EVENTS - 1;

#[account(zero_copy)]
pub struct DLLEventQueue {
    pub header: DLLHeader,
    pub nodes: [Node; MAX_NUM_EVENTS],
    pub reserved: [u8; 64],
}
const_assert_eq!(std::mem::size_of::<DLLEventQueue>(), 16 + 488 * 208 + 64);
const_assert_eq!(std::mem::size_of::<DLLEventQueue>(), 101584);
const_assert_eq!(std::mem::size_of::<DLLEventQueue>() % 8, 0);

impl DLLEventQueue {
    pub fn init(&mut self) {
        self.header = DLLHeader {
            free_head: 0,
            used_head: NULL,
            count: 0,
            seq_num: 0,
            _padd: Default::default(),
        };

        for i in 0..MAX_NUM_EVENTS {
            self.nodes[i].set_next(i + 1);
            self.nodes[i].set_prev(NULL as usize);
        }
        self.nodes[LAST_SLOT].set_next(NULL as usize);
    }

    pub fn len(&self) -> usize {
        self.header.count() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_full(&self) -> bool {
        self.len() == self.nodes.len()
    }

    pub fn push_back(&mut self, value: AnyEvent) {
        assert!(!self.is_full());

        let slot = self.header.free_head();
        let new_next: usize;
        let new_prev: usize;

        if self.is_empty() {
            new_next = slot;
            new_prev = slot;

            self.header.set_free_head(self.nodes[slot].next() as u16);
            self.header.set_used_head(slot as u16);
        } else {
            new_next = self.header.used_head();
            new_prev = self.nodes[new_next].prev as usize;

            self.nodes[new_prev].set_next(slot);
            self.nodes[new_next].set_prev(slot);
            self.header.set_free_head(self.nodes[slot].next() as u16);
        }

        self.header.incr_count();
        self.header.incr_event_id();
        self.nodes[slot].event = value;
        self.nodes[slot].set_next(new_next);
        self.nodes[slot].set_prev(new_prev);
    }

    pub fn front(&self) -> Option<&AnyEvent> {
        if self.is_empty() {
            return None;
        } else {
            Some(&self.nodes[self.header.used_head()].event)
        }
    }

    pub fn at(&self, slot: usize) -> Option<&AnyEvent> {
        if self.nodes[slot].is_free() {
            return None;
        } else {
            Some(&self.nodes[slot].event)
        }
    }

    pub fn delete(&mut self) -> Result<AnyEvent> {
        self.delete_slot(self.header.used_head())
    }

    pub fn delete_slot(&mut self, slot: usize) -> Result<AnyEvent> {
        if self.is_empty() || self.nodes[slot].is_free() {
            return Err(OpenBookError::SomeError.into());
        }

        let prev_slot = self.nodes[slot].prev();
        let next_slot = self.nodes[slot].next();
        let next_free = self.header.free_head();

        self.nodes[prev_slot].set_next(next_slot);
        self.nodes[next_slot].set_prev(prev_slot);

        self.header.set_free_head(slot as u16);

        if self.header.count() == 1 {
            self.header.set_used_head(NULL);
        } else if self.header.used_head() == slot {
            self.header.set_used_head(next_slot as u16);
        };

        self.header.decr_count();
        self.nodes[slot].set_next(next_free);
        self.nodes[slot].set_prev(NULL as usize);

        Ok(self.nodes[slot].event)
    }

    pub fn iter(&self) -> impl Iterator<Item = EventWithSlot> {
        DLLEventQueueIterator {
            queue: self,
            slot: self.header.used_head(),
            index: 0,
        }
    }
}

pub struct EventWithSlot<'a> {
    event: &'a AnyEvent,
    slot: usize,
}

struct DLLEventQueueIterator<'a> {
    queue: &'a DLLEventQueue,
    slot: usize,
    index: usize,
}

impl<'a> Iterator for DLLEventQueueIterator<'a> {
    type Item = EventWithSlot<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.queue.len() {
            None
        } else {
            let slot = self.slot;
            let item = &self.queue.nodes[slot].event;
            self.slot = self.queue.nodes[slot].next();
            self.index += 1;
            Some(EventWithSlot { event: item, slot })
        }
    }
}

#[zero_copy]
#[derive(Debug)]
pub struct DLLHeader {
    free_head: u16,
    used_head: u16,
    count: u16,
    _padd: u16,
    pub seq_num: u64,
}
const_assert_eq!(std::mem::size_of::<DLLHeader>(), 16);
const_assert_eq!(std::mem::size_of::<DLLHeader>() % 8, 0);

impl DLLHeader {
    pub fn count(&self) -> usize {
        self.count as usize
    }

    pub fn free_head(&self) -> usize {
        self.free_head as usize
    }

    pub fn used_head(&self) -> usize {
        self.used_head as usize
    }

    fn set_free_head(&mut self, value: u16) {
        self.free_head = value;
    }

    fn set_used_head(&mut self, value: u16) {
        self.used_head = value;
    }

    fn incr_count(&mut self) {
        self.count += 1;
    }

    fn decr_count(&mut self) {
        self.count -= 1;
    }

    fn incr_event_id(&mut self) {
        self.seq_num += 1;
    }
}

#[zero_copy]
#[derive(Debug)]
pub struct Node {
    next: u16,
    prev: u16,
    _pad: [u8; 4],
    pub event: AnyEvent,
}
const_assert_eq!(std::mem::size_of::<Node>(), 8 + 200);
const_assert_eq!(std::mem::size_of::<Node>() % 8, 0);

impl Node {
    pub fn is_free(&self) -> bool {
        self.prev == NULL
    }

    pub fn next(&self) -> usize {
        self.next as usize
    }

    pub fn prev(&self) -> usize {
        self.prev as usize
    }

    fn set_next(&mut self, next: usize) {
        self.next = next as u16;
    }

    fn set_prev(&mut self, prev: usize) {
        self.prev = prev as u16;
    }
}

#[cfg(test)]
mod test_event_queue {
    use super::*;
    use bytemuck::Zeroable;

    const LAST_SLOT: usize = MAX_NUM_EVENTS - 1;

    fn count_free_nodes(event_queue: &DLLEventQueue) -> usize {
        event_queue.nodes.iter().filter(|n| n.is_free()).count()
    }

    #[test]
    fn init() {
        let mut eq = DLLEventQueue::zeroed();
        eq.init();

        assert_eq!(eq.header.count(), 0);
        assert_eq!(eq.header.free_head(), 0);
        assert_eq!(eq.header.used_head(), NULL as usize);
        assert_eq!(count_free_nodes(&eq), MAX_NUM_EVENTS as usize);
    }

    #[test]
    #[should_panic]
    fn cannot_insert_if_full() {
        let mut eq = DLLEventQueue::zeroed();
        eq.init();
        for _ in 0..MAX_NUM_EVENTS + 1 {
            eq.push_back(AnyEvent::zeroed());
        }
    }

    #[test]
    #[should_panic]
    fn cannot_delete_if_empty() {
        let mut eq = DLLEventQueue::zeroed();
        eq.init();
        eq.delete().unwrap();
    }

    #[test]
    fn insert_until_full() {
        let mut eq = DLLEventQueue::zeroed();
        eq.init();

        // insert one event in the first slot; the single used node should point to himself
        eq.push_back(AnyEvent::zeroed());
        assert_eq!(eq.header.used_head(), 0);
        assert_eq!(eq.header.free_head(), 1);
        assert_eq!(eq.nodes[0].prev(), 0);
        assert_eq!(eq.nodes[0].next(), 0);
        assert_eq!(eq.nodes[1].next(), 2);

        for i in 1..MAX_NUM_EVENTS - 2 {
            eq.push_back(AnyEvent::zeroed());
            assert_eq!(eq.header.used_head(), 0);
            assert_eq!(eq.header.free_head(), i + 1);
            assert_eq!(eq.nodes[0].prev(), i);
            assert_eq!(eq.nodes[0].next(), 1);
            assert_eq!(eq.nodes[i + 1].next(), i + 2);
        }

        // insert another one, afterwards only one free node pointing to himself should be left
        eq.push_back(AnyEvent::zeroed());
        assert_eq!(eq.header.used_head(), 0);
        assert_eq!(eq.header.free_head(), LAST_SLOT);
        assert_eq!(eq.nodes[0].prev(), LAST_SLOT - 1);
        assert_eq!(eq.nodes[0].next(), 1);
        assert_eq!(eq.nodes[LAST_SLOT].next(), NULL as usize);

        // insert last available event
        eq.push_back(AnyEvent::zeroed());
        assert_eq!(eq.header.used_head(), 0);
        assert_eq!(eq.header.free_head(), NULL as usize);
        assert_eq!(eq.nodes[0].prev(), LAST_SLOT);
        assert_eq!(eq.nodes[0].next(), 1);
    }

    #[test]
    fn delete_full() {
        let mut eq = DLLEventQueue::zeroed();
        eq.init();
        for _ in 0..MAX_NUM_EVENTS {
            eq.push_back(AnyEvent::zeroed());
        }

        eq.delete().unwrap();
        assert_eq!(eq.header.free_head(), 0);
        assert_eq!(eq.header.used_head(), 1);
        assert_eq!(eq.nodes[0].next(), NULL as usize);
        assert_eq!(eq.nodes[1].prev(), LAST_SLOT);
        assert_eq!(eq.nodes[1].next(), 2);

        for i in 1..MAX_NUM_EVENTS - 2 {
            eq.delete().unwrap();
            assert_eq!(eq.header.free_head(), i);
            assert_eq!(eq.header.used_head(), i + 1);
            assert_eq!(eq.nodes[i].next(), i - 1);
            assert_eq!(eq.nodes[i + 1].prev(), LAST_SLOT);
            assert_eq!(eq.nodes[i + 1].next(), i + 2);
        }

        eq.delete().unwrap();
        assert_eq!(eq.header.free_head(), LAST_SLOT - 1);
        assert_eq!(eq.header.used_head(), LAST_SLOT);
        assert_eq!(eq.nodes[LAST_SLOT - 1].next(), LAST_SLOT - 2);
        assert_eq!(eq.nodes[LAST_SLOT].prev(), LAST_SLOT);
        assert_eq!(eq.nodes[LAST_SLOT].next(), LAST_SLOT);

        eq.delete().unwrap();
        assert_eq!(eq.header.used_head(), NULL as usize);
        assert_eq!(eq.header.free_head(), LAST_SLOT);
        assert_eq!(eq.nodes[LAST_SLOT].next(), LAST_SLOT - 1);

        assert_eq!(eq.header.count(), 0);
        assert_eq!(count_free_nodes(&eq), MAX_NUM_EVENTS);

        // cannot delete more
        assert_eq!(eq.delete().is_err(), true);
    }

    #[test]
    fn delete_at_given_position() {
        let mut eq = DLLEventQueue::zeroed();
        eq.init();
        for _ in 0..5 {
            eq.push_back(AnyEvent::zeroed());
        }
        eq.delete_slot(2).unwrap();
        assert_eq!(eq.header.free_head(), 2);
        assert_eq!(eq.header.used_head(), 0);
    }

    #[test]
    #[should_panic]
    fn cannot_delete_twice_same() {
        let mut eq = DLLEventQueue::zeroed();
        eq.init();
        for _ in 0..5 {
            eq.push_back(AnyEvent::zeroed());
        }
        eq.delete_slot(2).unwrap();
        eq.delete_slot(2).unwrap();
    }

    #[test]
    fn fifo_event_processing() {
        let event_1 = {
            let mut dummy_event = AnyEvent::zeroed().clone();
            dummy_event.event_type = 1;
            dummy_event
        };

        let event_2 = {
            let mut dummy_event = AnyEvent::zeroed();
            dummy_event.event_type = 2;
            dummy_event
        };

        let event_3 = {
            let mut dummy_event = AnyEvent::zeroed();
            dummy_event.event_type = 3;
            dummy_event
        };

        // [ | | | | ] init
        // [1| | | | ] insert
        // [1|2| | | ] insert
        // [ |2| | | ] delete
        // [3|2| | | ] insert
        // [3| | | | ] delete

        let mut eq = DLLEventQueue::zeroed();
        eq.init();
        assert_eq!(eq.nodes[0].is_free(), true);
        assert_eq!(eq.nodes[1].is_free(), true);
        assert_eq!(eq.nodes[2].is_free(), true);

        eq.push_back(event_1);
        assert_eq!(eq.nodes[0].event.event_type, 1);
        assert_eq!(eq.nodes[1].is_free(), true);
        assert_eq!(eq.nodes[2].is_free(), true);

        eq.push_back(event_2);
        assert_eq!(eq.nodes[0].event.event_type, 1);
        assert_eq!(eq.nodes[1].event.event_type, 2);
        assert_eq!(eq.nodes[2].is_free(), true);

        eq.delete().unwrap();
        assert_eq!(eq.nodes[0].is_free(), true);
        assert_eq!(eq.nodes[1].event.event_type, 2);
        assert_eq!(eq.nodes[2].is_free(), true);

        eq.push_back(event_3);
        assert_eq!(eq.nodes[0].event.event_type, 3);
        assert_eq!(eq.nodes[1].event.event_type, 2);
        assert_eq!(eq.nodes[2].is_free(), true);

        eq.delete().unwrap();
        assert_eq!(eq.nodes[0].event.event_type, 3);
        assert_eq!(eq.nodes[1].is_free(), true);
        assert_eq!(eq.nodes[2].is_free(), true);
    }

    #[test]
    fn lifo_free_available_slots() {
        // [0|1|2|3|4] init
        // [ |0|1|2|3] insert
        // [ | |0|1|2] insert
        // [0| |1|2|3] delete
        // [1|0|2|3|4] delete
        // [0| |1|2|3] insert
        // [ | |0|1|2] insert

        let mut eq = DLLEventQueue::zeroed();

        eq.init();
        assert_eq!(eq.header.free_head(), 0);
        assert_eq!(eq.nodes[0].next(), 1);

        eq.push_back(AnyEvent::zeroed());
        assert_eq!(eq.header.free_head(), 1);
        assert_eq!(eq.nodes[1].next(), 2);

        eq.push_back(AnyEvent::zeroed());
        assert_eq!(eq.header.free_head(), 2);
        assert_eq!(eq.nodes[2].next(), 3);

        eq.delete().unwrap();
        assert_eq!(eq.header.free_head(), 0);
        assert_eq!(eq.nodes[0].next(), 2);

        eq.delete().unwrap();
        assert_eq!(eq.header.free_head(), 1);
        assert_eq!(eq.nodes[1].next(), 0);

        eq.push_back(AnyEvent::zeroed());
        assert_eq!(eq.header.free_head(), 0);
        assert_eq!(eq.nodes[0].next(), 2);

        eq.push_back(AnyEvent::zeroed());
        assert_eq!(eq.header.free_head(), 2);
        assert_eq!(eq.nodes[2].next(), 3);
    }
}
