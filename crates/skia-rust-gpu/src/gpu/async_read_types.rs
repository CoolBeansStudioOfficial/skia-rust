// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/AsyncReadTypes.h (TClientMappedBufferManager), src/core/SkMessageBus.h

//! `skgpu::TClientMappedBufferManager`: buffers handed to clients while still mapped.
//!
//! We sometimes hand clients objects that contain mapped buffers. The client may consume the
//! mapped buffer on another thread. The manager receives messages that buffers are ready to be
//! unmapped (on the owner's thread). It also handles cleaning up mapped buffers if the owner is
//! destroyed before the client has finished with the buffer.
//!
//! Skia delivers the messages through the process-wide `SkMessageBus<BufferFinishedMessage, ID>`,
//! which picks the inbox whose id equals the message's intended recipient. The port has no
//! global state: each manager owns its inbox, and the [`BufferFinishedSender`] handles it gives
//! out post into that inbox (messages for another recipient are dropped, as the bus does when no
//! inbox matches).
//!
//! `TAsyncReadResult` (the `SkImage::AsyncReadResult` implementation) comes with the context's
//! async readback (G9b).

use std::sync::{Arc, Mutex};

/// What the manager needs from a mapped buffer.
pub trait ClientMappedBuffer: Send + 'static {
    /// `unmap()`.
    fn unmap(&self);

    /// True if both handles are the same buffer.
    fn is_same_buffer(&self, other: &Self) -> bool;
}

/// The message internal users post to unmap a buffer: `fBuffer` must have been previously passed
/// to `insert()`.
// Port of: src/gpu/AsyncReadTypes.h#L31-L45 (chrome/m156)
#[doc(alias = "TClientMappedBufferManager::BufferFinishedMessage")]
#[derive(Debug)]
pub struct BufferFinishedMessage<T, Id> {
    /// `fBuffer`.
    pub buffer: T,
    /// `fIntendedRecipient`.
    pub intended_recipient: Id,
}

impl<T, Id> BufferFinishedMessage<T, Id> {
    /// `BufferFinishedMessage(buffer, intendedRecipient)`.
    #[must_use]
    pub fn new(buffer: T, intended_recipient: Id) -> Self {
        Self {
            buffer,
            intended_recipient,
        }
    }
}

// `SkMessageBus<BufferFinishedMessage, IDType, false>::Inbox`.
#[derive(Debug)]
struct Inbox<T, Id> {
    unique_id: Id,
    messages: Mutex<Vec<BufferFinishedMessage<T, Id>>>,
}

impl<T, Id> Inbox<T, Id> {
    // Port of: src/core/SkMessageBus.h#L106-L111 (chrome/m156)
    fn receive(&self, message: BufferFinishedMessage<T, Id>) {
        self.messages
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(message);
    }

    // Port of: src/core/SkMessageBus.h#L113-L119 (chrome/m156)
    fn poll(&self) -> Vec<BufferFinishedMessage<T, Id>> {
        std::mem::take(
            &mut *self
                .messages
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

/// Posts `BufferFinishedMessage`s to one manager (`SkMessageBus::Post`). It can be used from any
/// thread.
#[derive(Debug)]
pub struct BufferFinishedSender<T, Id> {
    inbox: Arc<Inbox<T, Id>>,
}

impl<T, Id> Clone for BufferFinishedSender<T, Id> {
    fn clone(&self) -> Self {
        Self {
            inbox: self.inbox.clone(),
        }
    }
}

impl<T, Id: PartialEq> BufferFinishedSender<T, Id> {
    /// `Post(message)`: delivers the message if the manager is its intended recipient.
    // Port of: src/core/SkMessageBus.h#L151-L173 (chrome/m156)
    pub fn post(&self, message: BufferFinishedMessage<T, Id>) {
        if message.intended_recipient == self.inbox.unique_id {
            self.inbox.receive(message);
        }
    }
}

/// See the module docs.
// Port of: src/gpu/AsyncReadTypes.h#L24-L120 (chrome/m156)
#[doc(alias = "skgpu::TClientMappedBufferManager")]
#[derive(Debug)]
pub struct TClientMappedBufferManager<T: ClientMappedBuffer, Id: Copy + PartialEq> {
    finished_buffer_inbox: Arc<Inbox<T, Id>>,
    client_held_buffers: Vec<T>,
    abandoned: bool,
}

impl<T: ClientMappedBuffer, Id: Copy + PartialEq> TClientMappedBufferManager<T, Id> {
    /// `TClientMappedBufferManager(ownerID)`.
    #[must_use]
    pub fn new(owner_id: Id) -> Self {
        Self {
            finished_buffer_inbox: Arc::new(Inbox {
                unique_id: owner_id,
                messages: Mutex::new(Vec::new()),
            }),
            client_held_buffers: Vec::new(),
            abandoned: false,
        }
    }

    /// `ownerID()`: initialize `BufferFinishedMessage::intended_recipient` to this value. It is
    /// the unique ID of the object that owns this buffer manager.
    #[doc(alias = "ownerID")]
    #[must_use]
    pub fn owner_id(&self) -> Id {
        self.finished_buffer_inbox.unique_id
    }

    /// A handle to post `BufferFinishedMessage`s to this manager with.
    #[must_use]
    pub fn sender(&self) -> BufferFinishedSender<T, Id> {
        BufferFinishedSender {
            inbox: self.finished_buffer_inbox.clone(),
        }
    }

    /// `insert()`: lets the manager know to expect a message with buffer `b`. It's illegal for a
    /// buffer to be inserted again before it is unmapped by `process()`.
    // Port of: src/gpu/AsyncReadTypes.h#L68-L74 (chrome/m156)
    pub fn insert(&mut self, b: T) {
        debug_assert!(
            !self
                .client_held_buffers
                .iter()
                .any(|held| held.is_same_buffer(&b))
        );
        // std::forward_list::emplace_front
        self.client_held_buffers.insert(0, b);
    }

    /// `process()`: polls for messages and unmaps any incoming buffers.
    // Port of: src/gpu/AsyncReadTypes.h#L76-L86 (chrome/m156)
    pub fn process(&mut self) {
        let messages = self.finished_buffer_inbox.poll();
        if !self.abandoned {
            for m in messages {
                self.remove(&m.buffer);
                m.buffer.unmap();
            }
        }
    }

    /// `abandon()`: notifies the manager that the context has been abandoned. No more `unmap()`s
    /// will occur.
    // Port of: src/gpu/AsyncReadTypes.h#L88-L91 (chrome/m156)
    pub fn abandon(&mut self) {
        self.abandoned = true;
        self.client_held_buffers.clear();
    }

    // Removes only the first element that equals `b`.
    // Port of: src/gpu/AsyncReadTypes.h#L97-L110 (chrome/m156)
    fn remove(&mut self, b: &T) {
        let position = self
            .client_held_buffers
            .iter()
            .position(|held| held.is_same_buffer(b));
        debug_assert!(position.is_some());
        if let Some(position) = position {
            self.client_held_buffers.remove(position);
        }
        debug_assert!(
            !self
                .client_held_buffers
                .iter()
                .any(|held| held.is_same_buffer(b))
        );
    }

    /// The number of buffers the clients still hold.
    #[must_use]
    pub fn num_client_held_buffers(&self) -> usize {
        self.client_held_buffers.len()
    }
}

impl<T: ClientMappedBuffer, Id: Copy + PartialEq> Drop for TClientMappedBufferManager<T, Id> {
    // Port of: src/gpu/AsyncReadTypes.h#L52-L61 (chrome/m156)
    fn drop(&mut self) {
        self.process();
        if !self.abandoned {
            // If we're going down before we got the messages we go ahead and unmap all the
            // buffers. It's up to the client to ensure that they aren't being accessed on
            // another thread while this is happening (or afterwards on any thread).
            for b in &self.client_held_buffers {
                b.unmap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct Fake {
        id: usize,
        unmaps: Arc<AtomicUsize>,
    }

    impl ClientMappedBuffer for Fake {
        fn unmap(&self) {
            self.unmaps.fetch_add(1, Ordering::SeqCst);
        }

        fn is_same_buffer(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }

    #[test]
    fn messages_unmap_and_remove_buffers() {
        let unmaps = Arc::new(AtomicUsize::new(0));
        let mut manager = TClientMappedBufferManager::<Fake, u32>::new(7);
        manager.insert(Fake {
            id: 1,
            unmaps: unmaps.clone(),
        });
        manager.insert(Fake {
            id: 2,
            unmaps: unmaps.clone(),
        });
        let sender = manager.sender();
        sender.post(BufferFinishedMessage::new(
            Fake {
                id: 1,
                unmaps: unmaps.clone(),
            },
            7,
        ));
        // Not for this manager.
        sender.post(BufferFinishedMessage::new(
            Fake {
                id: 2,
                unmaps: unmaps.clone(),
            },
            8,
        ));
        manager.process();
        assert_eq!(manager.num_client_held_buffers(), 1);
        // The message's buffer unmapped once.
        assert_eq!(unmaps.load(Ordering::SeqCst), 1);
        drop(manager);
        // The remaining buffer is unmapped on drop.
        assert_eq!(unmaps.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn abandoned_manager_does_not_unmap() {
        let unmaps = Arc::new(AtomicUsize::new(0));
        let mut manager = TClientMappedBufferManager::<Fake, u32>::new(1);
        manager.insert(Fake {
            id: 1,
            unmaps: unmaps.clone(),
        });
        manager.abandon();
        drop(manager);
        assert_eq!(unmaps.load(Ordering::SeqCst), 0);
    }
}
