use ctypes::BlockHash;

use parking_lot::RwLock;
use std::sync::mpsc::{sync_channel, Receiver, RecvError, SyncSender};
use std::sync::{Arc, Weak};

pub fn create() -> (NotifySender, NotifyReceiverSource) {
    let (tx, rx) = sync_channel(1);
    let tx = Arc::new(RwLock::new(Some(tx)));
    let tx_weak = Arc::downgrade(&tx);
    (
        NotifySender {
            tx,
        },
        NotifyReceiverSource(
            ReceiverCanceller {
                tx: tx_weak,
            },
            NotifyReceiver {
                rx,
            },
        ),
    )
}

pub struct NotifySender {
    tx: Arc<RwLock<Option<SyncSender<BlockHash>>>>,
}

impl NotifySender {
    pub fn notify(&self, block_hash: BlockHash) {
        let guard = self.tx.read();
        if let Some(tx) = guard.as_ref() {
            // TODO: Ignore the error. Receiver thread might be terminated or congested.
            let _ = tx.try_send(block_hash);
        } else {
            // ReceiverCanceller is dropped.
        }
    }
}

pub struct NotifyReceiverSource(pub ReceiverCanceller, pub NotifyReceiver);

/// Dropping this makes the receiver stopped.
///
/// `recv()` method  of the `Receiver` will stop and return `RecvError` when corresponding `Sender` is dropped.
/// This is an inherited behaviour of `std::sync::mpsc::{Sender, Receiver}`.
/// However, we need another way to stop the `Receiver`, since `Sender` is usually shared throughout our codes.
/// We can't collect them all and destory one by one. We need a kill switch.
///
/// `ReceiverCanceller` holds weak reference to the `Sender`, so it doesn't prohibit the default behaviour.
/// Then, we can upgrade the weak reference and get the shared reference to `Sender` itself, and manually drop it with this.
pub struct ReceiverCanceller {
    tx: Weak<RwLock<Option<SyncSender<BlockHash>>>>,
}

impl Drop for ReceiverCanceller {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.upgrade() {
            let mut guard = tx.write();
            if let Some(sender) = guard.take() {
                drop(sender)
            }
        } else {
            // All NotifySender is dropped. No droppable Sender.
        }
    }
}

/// Receiver is dropped when
/// 1. There are no NotifySenders out there.
/// 2. ReceiverCanceller is dropped. See the comment of `ReceiverCanceller`.
pub struct NotifyReceiver {
    rx: Receiver<BlockHash>,
}

impl NotifyReceiver {
    pub fn recv(&self) -> Result<BlockHash, RecvError> {
        self.rx.recv()
    }
}
