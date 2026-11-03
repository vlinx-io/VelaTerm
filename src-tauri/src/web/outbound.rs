//! Socket staging has byte and item quotas, including encrypted/writer copies still in flight.
//!
//! Pushed messages fail fast: a connection that cannot keep up is closed and resynchronizes on reconnect.
//! Replies answer the client's own requests, including the snapshots that reconnect resynchronization
//! itself asks for, so they wait for the socket to drain instead. Failing them would close every
//! reconnect whose snapshot burst exceeds the quota, and the client would retry the same burst forever.
//! Terminal attach replays are the same kind of answer: a reconnect reattaches every open terminal and each
//! replays up to its whole scrollback. They reserve room before the scrollback is read and then travel in
//! order with live output, but are charged to their own class so they never count as live backlog.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool,AtomicUsize,Ordering};
use std::time::Duration;
use axum::extract::ws::Message;
use tokio::sync::{mpsc,Notify};

const CONTROL_BYTES:usize=3*1024*1024;
const REPLY_BYTES:usize=8*1024*1024;
const REPLY_RECHECK:Duration=Duration::from_millis(50);
/// Charge reserved for one attach replay: the whole scrollback plus mode prelude and frame header, at the
/// same copy multiplier `charge` applies.
const REPLAY_CHARGE:usize=(crate::pty::manager::SCROLLBACK_CAP+4096+256)*4;
/// Attach replays in flight per connection. Further attaches wait without reading their scrollback yet.
const REPLAY_BYTES:usize=4*REPLAY_CHARGE;
const PRESENTATION_BYTES:usize=5*1024*1024;
const ATTACHMENT_BYTES:usize=32*1024*1024;
const GLOBAL_BYTES:usize=64*1024*1024;
static GLOBAL:AtomicUsize=AtomicUsize::new(0);
/// Woken whenever any connection releases bytes, so a waiting reply can retry its reservation.
static RELEASED:Notify=Notify::const_new();

#[derive(Clone,Copy)]
enum Class {Control,Reply,Presentation,Attachment,Replay}
/// `released` wakes this connection's waiters when one of its own frames leaves, which is how a replay
/// waiter tells a slow connection that is still draining from one that is stuck.
struct Budget {control:AtomicUsize,reply:AtomicUsize,presentation:AtomicUsize,attachment:AtomicUsize,replay:AtomicUsize,closed:AtomicBool,pressure:Notify,released:Notify}
#[derive(Clone)]
pub(super) struct Sender {control:mpsc::Sender<Queued>,reply:mpsc::Sender<Queued>,presentation:mpsc::Sender<Queued>,attachment:mpsc::Sender<Queued>,budget:Arc<Budget>}
pub(super) struct Receiver {control:mpsc::Receiver<Queued>,reply:mpsc::Receiver<Queued>,presentation:mpsc::Receiver<Queued>,attachment:mpsc::Receiver<Queued>,budget:Arc<Budget>,control_closed:bool,reply_closed:bool,presentation_closed:bool,attachment_closed:bool}
pub(super) struct Queued {pub message:Message,_credit:Credit}
struct Credit {budget:Arc<Budget>,class:Class,bytes:usize}
/// Room reserved for one attach replay; see `Sender::replay_ticket`.
pub(super) struct ReplayTicket(Credit);
#[derive(Debug)]
pub(super) struct Pressure;

impl Drop for Credit {
    fn drop(&mut self) {
        self.budget.counter(self.class).fetch_sub(self.bytes,Ordering::AcqRel);
        GLOBAL.fetch_sub(self.bytes,Ordering::AcqRel);
        RELEASED.notify_waiters();self.budget.released.notify_waiters();
    }
}
impl Credit {
    /// Settles a reservation to the frame's real charge. Growing skips the quota checks: the reservation
    /// already admitted the frame, and the scrollback cap bounds it.
    fn settle(&mut self,bytes:usize) {
        if bytes>self.bytes {
            let extra=bytes-self.bytes;
            self.budget.counter(self.class).fetch_add(extra,Ordering::AcqRel);GLOBAL.fetch_add(extra,Ordering::AcqRel);
        } else if bytes<self.bytes {
            let spare=self.bytes-bytes;
            self.budget.counter(self.class).fetch_sub(spare,Ordering::AcqRel);GLOBAL.fetch_sub(spare,Ordering::AcqRel);
            RELEASED.notify_waiters();self.budget.released.notify_waiters();
        }
        self.bytes=bytes;
    }
}
impl Budget {
    fn counter(&self,class:Class) -> &AtomicUsize {match class {Class::Control=>&self.control,Class::Reply=>&self.reply,Class::Presentation=>&self.presentation,Class::Attachment=>&self.attachment,Class::Replay=>&self.replay}}
    fn fail(&self) {self.closed.store(true,Ordering::Release);self.pressure.notify_one();}
    /// Takes credit only when it fits. A single reply larger than its class quota is admitted while it is
    /// the only reply in flight, so one large snapshot cannot be refused forever.
    fn take(self:&Arc<Self>,class:Class,bytes:usize) -> Option<Credit> {
        if GLOBAL.fetch_update(Ordering::AcqRel,Ordering::Acquire,|used|used.checked_add(bytes).filter(|next|*next<=GLOBAL_BYTES)).is_err() {return None;}
        let limit=match class {Class::Control=>CONTROL_BYTES,Class::Reply=>REPLY_BYTES,Class::Presentation=>PRESENTATION_BYTES,Class::Attachment=>ATTACHMENT_BYTES,Class::Replay=>REPLAY_BYTES};
        let alone=matches!(class,Class::Reply);
        if self.counter(class).fetch_update(Ordering::AcqRel,Ordering::Acquire,|used|used.checked_add(bytes).filter(|next|*next<=limit||alone&&used==0)).is_err() {
            // No wakeup here: waiters would wake each other on every refused attempt and spin. A transient
            // hold that refused someone is covered by the waiters' periodic recheck instead.
            GLOBAL.fetch_sub(bytes,Ordering::AcqRel);return None;
        }
        Some(Credit {budget:self.clone(),class,bytes})
    }
    fn reserve(self:&Arc<Self>,class:Class,bytes:usize) -> Result<Credit,Pressure> {
        if self.closed.load(Ordering::Acquire) {return Err(Pressure);}
        self.take(class,bytes).ok_or_else(||{self.fail();Pressure})
    }
}

/// Original staging, JSON/encryption temporaries, and the socket writer each own a copy.
/// A minimum charge also bounds an adversarial stream of empty frames.
fn charge(message:&Message) -> usize {
    let bytes=match message {Message::Text(text)=>text.capacity(),Message::Binary(bytes)|Message::Ping(bytes)|Message::Pong(bytes)=>bytes.capacity(),
        Message::Close(frame)=>frame.as_ref().map_or(0,|frame|frame.reason.len())};
    bytes.saturating_add(256).saturating_mul(4)
}

pub(super) fn channel() -> (Sender,Receiver) {
    let budget=Arc::new(Budget {control:AtomicUsize::new(0),reply:AtomicUsize::new(0),presentation:AtomicUsize::new(0),attachment:AtomicUsize::new(0),replay:AtomicUsize::new(0),closed:AtomicBool::new(false),pressure:Notify::new(),released:Notify::new()});
    // Presentation carries live terminal output queued behind attach replays, so it allows more items than the
    // other classes. Its byte quota and the minimum per-frame charge still bound what those items can hold.
    let (control,control_rx)=mpsc::channel(64);let (reply,reply_rx)=mpsc::channel(64);let (presentation,presentation_rx)=mpsc::channel(1024);
    let (attachment,attachment_rx)=mpsc::channel(1);
    (Sender {control,reply,presentation,attachment,budget:budget.clone()},Receiver {control:control_rx,reply:reply_rx,presentation:presentation_rx,attachment:attachment_rx,budget,control_closed:false,reply_closed:false,presentation_closed:false,attachment_closed:false})
}

impl Sender {
    pub(super) fn send(&self,message:Message) -> Result<(),Pressure> {
        let class=if matches!(message,Message::Binary(_)) {Class::Presentation} else {Class::Control};self.enqueue(message,class)
    }
    pub(super) fn presentation(&self,message:Message) -> Result<(),Pressure> {self.enqueue(message,Class::Presentation)}
    pub(super) fn attachment(&self,message:Message) -> Result<(),Pressure> {self.enqueue(message,Class::Attachment)}
    /// Queues the answer to a client request, waiting while earlier output drains. Only a reply that still
    /// cannot be admitted after `patience` treats the connection as stuck and closes it. Callers bound how
    /// many replies wait at once, which bounds the memory they hold.
    pub(super) async fn reply(&self,message:Message,patience:Duration) -> Result<(),Pressure> {
        let bytes=charge(&message);
        if bytes>GLOBAL_BYTES {self.budget.fail();return Err(Pressure);}
        let deadline=tokio::time::Instant::now()+patience;
        let credit=loop {
            // Register for the wakeup before trying, so a release between the attempt and the wait is not missed.
            let released=RELEASED.notified();tokio::pin!(released);released.as_mut().enable();
            if self.budget.closed.load(Ordering::Acquire) {return Err(Pressure);}
            if let Some(credit)=self.budget.take(Class::Reply,bytes) {break credit;}
            let now=tokio::time::Instant::now();
            if now>=deadline {self.budget.fail();return Err(Pressure);}
            let _=tokio::time::timeout_at(deadline.min(now+REPLY_RECHECK),released).await;
        };
        let queued=Queued {message,_credit:credit};
        if !matches!(tokio::time::timeout_at(deadline,self.reply.send(queued)).await,Ok(Ok(()))) {self.budget.fail();return Err(Pressure);}
        Ok(())
    }
    /// Reserves room for one terminal attach replay, waiting while earlier replays on this connection drain.
    /// Call it before the scrollback is read, so waiting attaches hold no output. Only a connection that sends
    /// none of its queued frames for `patience` counts as stuck and is closed.
    pub(super) async fn replay_ticket(&self,patience:Duration) -> Result<ReplayTicket,Pressure> {
        let mut deadline=tokio::time::Instant::now()+patience;
        loop {
            let released=self.budget.released.notified();tokio::pin!(released);released.as_mut().enable();
            if self.budget.closed.load(Ordering::Acquire) || self.presentation.is_closed() {return Err(Pressure);}
            if let Some(credit)=self.budget.take(Class::Replay,REPLAY_CHARGE) {return Ok(ReplayTicket(credit));}
            let now=tokio::time::Instant::now();
            if now>=deadline {self.budget.fail();return Err(Pressure);}
            // A frame of this connection left the queue: it is draining, so the patience starts over.
            if tokio::time::timeout_at(deadline.min(now+REPLY_RECHECK),released).await.is_ok() {deadline=tokio::time::Instant::now()+patience;}
        }
    }
    /// Queues an attach replay on the presentation channel, so it stays ordered with the session's live output,
    /// under the room its ticket reserved rather than the presentation quota.
    pub(super) fn replay(&self,message:Message,ticket:ReplayTicket) -> Result<(),Pressure> {
        if self.budget.closed.load(Ordering::Acquire) {return Err(Pressure);}
        let mut credit=ticket.0;credit.settle(charge(&message));
        if self.presentation.try_send(Queued {message,_credit:credit}).is_err() {self.budget.fail();return Err(Pressure);}Ok(())
    }
    fn enqueue(&self,message:Message,class:Class) -> Result<(),Pressure> {
        let credit=self.budget.reserve(class,charge(&message))?;
        let queued=Queued {message,_credit:credit};
        let sender=match class {Class::Control=>&self.control,Class::Reply=>&self.reply,Class::Presentation|Class::Replay=>&self.presentation,Class::Attachment=>&self.attachment};
        if sender.try_send(queued).is_err() {self.budget.fail();return Err(Pressure);}Ok(())
    }
}
impl Receiver {
    pub(super) fn pressured(&self) -> bool {self.budget.closed.load(Ordering::Acquire)}
    pub(super) async fn recv(&mut self) -> Option<Queued> {
        loop {
            if self.pressured() || self.control_closed && self.reply_closed && self.presentation_closed && self.attachment_closed {return None;}
            tokio::select! {biased;
                _=self.budget.pressure.notified()=>return None,
                message=self.control.recv(),if !self.control_closed=>match message {Some(message)=>return Some(message),None=>self.control_closed=true},
                message=self.reply.recv(),if !self.reply_closed=>match message {Some(message)=>return Some(message),None=>self.reply_closed=true},
                message=self.attachment.recv(),if !self.attachment_closed=>match message {Some(message)=>return Some(message),None=>self.attachment_closed=true},
                message=self.presentation.recv(),if !self.presentation_closed=>match message {Some(message)=>return Some(message),None=>self.presentation_closed=true},
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static TEST_GATE:std::sync::Mutex<()>=std::sync::Mutex::new(());
    #[tokio::test]
    async fn control_has_reserved_bytes_and_priority_while_socket_copies_remain_charged() {
        let _gate=TEST_GATE.lock().unwrap();
        let (sender,mut receiver)=channel();
        sender.presentation(Message::Text("x".repeat(512*1024))).unwrap();
        sender.presentation(Message::Text("y".repeat(512*1024))).unwrap();
        sender.send(Message::Text("permission".into())).unwrap();
        let control=receiver.recv().await.unwrap();assert!(matches!(&control.message,Message::Text(text) if text=="permission"));
        assert!(sender.budget.control.load(Ordering::Acquire)>0);drop(control);assert_eq!(sender.budget.control.load(Ordering::Acquire),0);
        let in_flight=receiver.recv().await.unwrap();
        assert!(sender.presentation(Message::Text("z".repeat(512*1024))).is_err());
        assert!(receiver.pressured());assert!(receiver.recv().await.is_none());
        assert!(sender.budget.presentation.load(Ordering::Acquire)>0);
        drop(in_flight);drop(receiver);assert_eq!(sender.budget.presentation.load(Ordering::Acquire),0);
    }
    #[tokio::test]
    async fn empty_frames_and_oversized_controls_cannot_accumulate_unbounded_items() {
        let _gate=TEST_GATE.lock().unwrap();
        let (sender,mut receiver)=channel();for _ in 0..64 {sender.send(Message::Text(String::new())).unwrap();}
        assert!(sender.send(Message::Text(String::new())).is_err());assert!(receiver.recv().await.is_none());
        drop(receiver);assert_eq!(sender.budget.control.load(Ordering::Acquire),0);
        let (sender,mut receiver)=channel();assert!(sender.send(Message::Text("x".repeat(CONTROL_BYTES))).is_err());
        assert!(receiver.pressured());assert!(receiver.recv().await.is_none());assert_eq!(sender.budget.control.load(Ordering::Acquire),0);
    }
    #[tokio::test]
    async fn independent_connections_share_a_process_byte_quota_and_release_on_disconnect() {
        let _gate=TEST_GATE.lock().unwrap();
        let mut receivers=Vec::new();let mut senders=Vec::new();let mut pressured=false;
        for _ in 0..40 {let (sender,receiver)=channel();
            for _ in 0..2 {if sender.presentation(Message::Text("x".repeat(512*1024))).is_err() {pressured=true;break;}}
            senders.push(sender);receivers.push(receiver);if pressured {break;}
        }
        assert!(pressured);assert!(GLOBAL.load(Ordering::Acquire)<=GLOBAL_BYTES);
        drop(receivers);for sender in senders {assert_eq!(sender.budget.presentation.load(Ordering::Acquire),0);}
    }
    #[tokio::test]
    async fn explicit_original_image_uses_one_separate_attachment_slot_without_consuming_control_reserve() {
        let _gate=TEST_GATE.lock().unwrap();let (sender,mut receiver)=channel();
        sender.attachment(Message::Text("x".repeat(7*1024*1024))).unwrap();
        sender.send(Message::Text("permission".into())).unwrap();let control=receiver.recv().await.unwrap();
        assert!(matches!(&control.message,Message::Text(text) if text=="permission"));drop(control);
        let image=receiver.recv().await.unwrap();assert!(sender.budget.attachment.load(Ordering::Acquire)>=28*1024*1024);
        assert!(sender.attachment(Message::Text("x".repeat(7*1024*1024))).is_err());assert!(receiver.pressured());
        drop(image);drop(receiver);assert_eq!(sender.budget.attachment.load(Ordering::Acquire),0);
    }
    #[tokio::test]
    async fn reply_burst_beyond_the_quota_waits_for_the_writer_instead_of_closing() {
        let _gate=TEST_GATE.lock().unwrap();let (sender,mut receiver)=channel();
        // Reconnect resynchronization asks for several snapshots at once; together they exceed the reply quota.
        let burst=sender.clone();
        let producer=tokio::spawn(async move {
            for n in 0..8 {burst.reply(Message::Text(n.to_string().repeat(512*1024)),Duration::from_secs(5)).await.unwrap();}
        });
        for n in 0..8 {
            let reply=receiver.recv().await.unwrap();
            assert!(matches!(&reply.message,Message::Text(text) if text.starts_with(&n.to_string())));
        }
        producer.await.unwrap();assert!(!receiver.pressured());
        drop(receiver);assert_eq!(sender.budget.reply.load(Ordering::Acquire),0);
    }
    #[tokio::test]
    async fn oversized_reply_is_admitted_alone_and_events_keep_their_reserve() {
        let _gate=TEST_GATE.lock().unwrap();let (sender,mut receiver)=channel();
        sender.reply(Message::Text("x".repeat(3*1024*1024)),Duration::from_secs(1)).await.unwrap();
        assert!(sender.budget.reply.load(Ordering::Acquire)>REPLY_BYTES);
        sender.send(Message::Text("permission".into())).unwrap();
        let control=receiver.recv().await.unwrap();assert!(matches!(&control.message,Message::Text(text) if text=="permission"));
        let big=receiver.recv().await.unwrap();assert!(!receiver.pressured());
        drop(big);drop(control);drop(receiver);assert_eq!(sender.budget.reply.load(Ordering::Acquire),0);
    }
    #[tokio::test]
    async fn attach_replays_beyond_the_quota_wait_and_live_output_keeps_its_own_quota() {
        let _gate=TEST_GATE.lock().unwrap();let (sender,mut receiver)=channel();
        // A reconnect reattaches many terminals at once, each replaying a full scrollback.
        let scrollback=crate::pty::manager::SCROLLBACK_CAP;
        let burst=sender.clone();
        let producer=tokio::spawn(async move {
            for n in 0..12u8 {
                let ticket=burst.replay_ticket(Duration::from_secs(5)).await.unwrap();
                burst.replay(Message::Binary(vec![n;scrollback]),ticket).unwrap();
            }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Replays in flight do not count as live backlog.
        sender.presentation(Message::Binary(vec![0xff;1024*1024])).unwrap();
        let mut replays=0u8;
        while replays<12 {
            let frame=receiver.recv().await.unwrap();
            if let Message::Binary(bytes)=&frame.message {if bytes.len()==scrollback {assert_eq!(bytes[0],replays);replays+=1;}}
        }
        producer.await.unwrap();assert!(!receiver.pressured());
        assert!(sender.budget.presentation.load(Ordering::Acquire)<=PRESENTATION_BYTES);
        drop(receiver);assert_eq!(sender.budget.replay.load(Ordering::Acquire),0);
    }
    #[tokio::test]
    async fn replay_ticket_on_a_connection_that_never_drains_closes_it_after_its_patience() {
        let _gate=TEST_GATE.lock().unwrap();let (sender,receiver)=channel();
        let mut held=Vec::new();
        for _ in 0..REPLAY_BYTES/REPLAY_CHARGE {held.push(sender.replay_ticket(Duration::from_secs(1)).await.unwrap());}
        assert!(sender.replay_ticket(Duration::from_millis(50)).await.is_err());assert!(receiver.pressured());
        drop(held);drop(receiver);assert_eq!(sender.budget.replay.load(Ordering::Acquire),0);
    }
    #[tokio::test]
    async fn reply_that_never_drains_closes_the_connection_after_its_patience() {
        let _gate=TEST_GATE.lock().unwrap();let (sender,receiver)=channel();
        sender.reply(Message::Text("x".repeat(1024*1024)),Duration::from_secs(1)).await.unwrap();
        assert!(sender.reply(Message::Text("y".repeat(1024*1024)),Duration::from_millis(50)).await.is_err());
        assert!(receiver.pressured());
        drop(receiver);assert_eq!(sender.budget.reply.load(Ordering::Acquire),0);
    }
}
