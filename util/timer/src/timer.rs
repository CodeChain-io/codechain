// Copyright 2018 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::cmp::Reverse;
use std::collections::binary_heap::BinaryHeap;
use std::collections::hash_map::{Entry, HashMap};
use std::collections::VecDeque;
use std::string::ToString;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex, RwLock};

pub type TimerName = &'static str;
pub type TimerToken = usize;

pub trait TimeoutHandler: Send + Sync {
    fn on_timeout(&self, _token: TimerToken) {}
}

#[derive(Clone)]
pub struct TimerLoop {
    timer_id_nonce: Arc<AtomicUsize>,
    scheduler: Arc<Scheduler>,
}

impl TimerLoop {
    pub fn new(worker_size: usize) -> TimerLoop {
        let scheduler = Arc::new(Scheduler::new());

        let worker_queue = Arc::new(WorkerQueue::new());

        spawn_workers(worker_size, &worker_queue);
        {
            let worker_queue = Arc::clone(&worker_queue);
            let scheduler = Arc::clone(&scheduler);
            thread::Builder::new()
                .name("timer.scheduler".to_string())
                .spawn(move || scheduler.run(&worker_queue))
                .unwrap();
        }

        TimerLoop {
            timer_id_nonce: Arc::new(AtomicUsize::new(0)),
            scheduler,
        }
    }

    pub fn new_timer(&self, name: TimerName) -> TimerApi {
        let timer_id = self.timer_id_nonce.fetch_add(1, Ordering::SeqCst);
        TimerApi {
            timer_id,
            handler: Arc::new(RwLock::new(None)),
            timer_name: name,
            scheduler: Arc::downgrade(&self.scheduler),
        }
    }
}

type TimerId = usize;

#[derive(Clone)]
pub struct TimerApi {
    timer_id: TimerId,
    timer_name: TimerName,
    handler: Arc<RwLock<Option<Weak<TimeoutHandler>>>>,
    scheduler: Weak<Scheduler>,
}

#[derive(Eq, PartialEq, Debug)]
pub enum ScheduleError {
    TokenAlreadyScheduled,
    TimerLoopDropped,
}

impl TimerApi {
    pub fn set_handler<T>(&self, handler: &Arc<T>)
    where
        T: 'static + TimeoutHandler, {
        let mut handler_guard = self.handler.write();
        assert!(handler_guard.is_none(), "Timer handler cannot be changed once it is set");
        *handler_guard = Some(Arc::downgrade(handler) as Weak<TimeoutHandler>);
    }

    pub fn schedule_once(&self, after: Duration, timer_token: TimerToken) -> Result<(), ScheduleError> {
        let scheduler = self.scheduler.upgrade().ok_or(ScheduleError::TimerLoopDropped)?;
        scheduler.schedule(self, timer_token, after, None)
    }

    pub fn schedule_repeat(&self, after: Duration, timer_token: TimerToken) -> Result<(), ScheduleError> {
        let scheduler = self.scheduler.upgrade().ok_or(ScheduleError::TimerLoopDropped)?;
        scheduler.schedule(self, timer_token, after, Some(after))
    }

    pub fn cancel(&self, timer_token: TimerToken) -> Result<bool, ScheduleError> {
        let scheduler = self.scheduler.upgrade().ok_or(ScheduleError::TimerLoopDropped)?;
        let result = scheduler.cancel(self, timer_token);
        Ok(result)
    }
}

struct Scheduler {
    inner: Mutex<SchedulerInner>,
    condvar: Condvar,
}

impl Scheduler {
    fn new() -> Scheduler {
        Scheduler {
            inner: Mutex::new(SchedulerInner::new()),
            condvar: Condvar::new(),
        }
    }

    fn schedule(
        &self,
        requested_timer: &TimerApi,
        timer_token: TimerToken,
        after: Duration,
        repeat: Option<Duration>,
    ) -> Result<(), ScheduleError> {
        let mut scheduler = self.inner.lock();
        scheduler.schedule(requested_timer, timer_token, after, repeat)?;
        self.condvar.notify_one();
        Ok(())
    }

    fn cancel(&self, requested_timer: &TimerApi, timer_token: TimerToken) -> bool {
        let mut scheduler = self.inner.lock();
        let result = scheduler.cancel(requested_timer, timer_token);
        self.condvar.notify_all();
        result
    }

    fn run(&self, worker_queue: &WorkerQueue) {
        let mut scheduler = self.inner.lock();
        while !scheduler.stop {
            let wait_for = scheduler.handle_timeout(worker_queue);
            match wait_for {
                Some(timeout) => {
                    self.condvar.wait_for(&mut scheduler, timeout);
                }
                None => self.condvar.wait(&mut scheduler),
            }
        }
    }
}

/// Def 1. A 'state_control' for a 'schedule' that contained in 'self.heap' have two implicit states.
///     Attached: It is also contained in 'self.states'.
///     Detached: It is not contained in 'self.states'.
/// Def 2. Garbage: A 'state_control' that is contained in 'self.states' but not in 'self.heap'
///
/// Rule 1. All detached 'state_control' is in 'Cancelled' state.
///     A detached one has no way to 'cancel' it. so it should be in 'Cancelled' state.
/// Rule 2. All 'state_control' in 'self.states' are unique.
///     Otherwise, it leads to two different ScheduleId shares same 'state_control'.
/// Rule 3. All 'state_control' in 'Wait | WaitRepeating' state that contained in 'self.heap' are unique.
///     Otherwise, cancelling a 'state_control' makes two different schedules being cancelled.
///
/// Lemma 1. All 'state_control' found in 'ScheduleInner' falls in one of these three states,
///     Attached', 'Detached', 'Garbage'
/// Lemma 2. All 'state_control' in 'Wait | WaitRepeating' state in 'ScheduleInner' is either an attached one or a garbage.
///     A detached ones are all in 'Cancelled' state (Rule 1) so it is in either 'Attached' or 'Garbage' state.
/// Lemma 3. We can find all 'state_control' for a 'schedule' that are in 'Wait | WaitRepeating' using a ScheduleId.
///     We can find a 'state_control' for a ScheduleId (Rule 2), and all 'state_control' that
///     'Wait | WaitRepeating' is either an attached one or a garbage (Lemma 2),
///     but a garbage is not in 'self.heap' (Def 2).
///
/// Corollary 1. An attached one that is in 'Cancelled' state can be detached. (Rule 1)
/// Corollary 2. We can precisely cancel a 'schedule' at a time.
///     We can find all 'Wait | WaitRepeating' 'state_control' for a 'schedule' (Lemma 3),
///     and they are unique (Rule 3).
///
/// Note 1. We don't know which one "is" actually a garbage, since 'self.heap' doesn't provide a cheap method to searching through it.
///     We should reattach, detach or remove it before it is become garbage. If it is properly done, There's no garbage.
/// Note 2. Timeout, Cancelled states never revive. (to ease the complexity)

struct SchedulerInner {
    states: HashMap<ScheduleId, Arc<ScheduleStateControl>>,
    heap: BinaryHeap<Reverse<TimeOrdered<Schedule>>>,
    stop: bool,
}
impl SchedulerInner {
    fn new() -> SchedulerInner {
        SchedulerInner {
            states: HashMap::new(),
            heap: BinaryHeap::new(),
            stop: false,
        }
    }

    fn schedule(
        &mut self,
        requested_timer: &TimerApi,
        timer_token: TimerToken,
        after: Duration,
        repeat: Option<Duration>,
    ) -> Result<(), ScheduleError> {
        let schedule_id = ScheduleId(requested_timer.timer_id, timer_token);
        let handler = {
            let guard = requested_timer.handler.read();
            let handler = guard.as_ref().expect("TimeoutHandler must be set");
            Weak::clone(handler)
        };

        let state_control = match self.states.entry(schedule_id) {
            Entry::Vacant(entry) => {
                // unique one(Rule 2). it is going to be attached.
                let state_control = Arc::new(ScheduleStateControl::new_auto(repeat));
                entry.insert(Arc::clone(&state_control));
                state_control
            }
            Entry::Occupied(mut entry) => {
                if !entry.get().is_cancelled() {
                    // Prevents violation of Rule 1. We can't detach it.
                    return Err(ScheduleError::TokenAlreadyScheduled)
                }
                // Detach the entry (Corollary 1) before it become garbage (Note 1),
                // create a unique one (Rule 2). it is going to be attached.
                let state_control = Arc::new(ScheduleStateControl::new_auto(repeat));
                *entry.get_mut() = Arc::clone(&state_control);
                state_control
            }
        };

        let schedule = Schedule {
            at: Instant::now() + after,
            schedule_id,
            repeat,
            state_control,
            handler,
        };
        // state_control become an attached one (Def 1)
        self.heap.push(Reverse(TimeOrdered(schedule)));
        Ok(())
    }

    fn try_reschedule(&mut self, mut schedule: Schedule) -> bool {
        schedule.at = Instant::now() + schedule.repeat.expect("Schedule should have repeat interval");
        match self.states.entry(schedule.schedule_id) {
            Entry::Vacant(_) => {
                // 'schedule.state_control' was detached one (Def 1).
                // Don't reschedule since it is Cancelled (Rule 1)
                // schedule is going to be removed
                false
            }
            Entry::Occupied(entry) => {
                if Arc::ptr_eq(entry.get(), &schedule.state_control) {
                    // schedule.state_control was attached one, (Def 1)
                    // just re-push to heap.
                    self.heap.push(Reverse(TimeOrdered(schedule)));
                    true
                } else if entry.get().is_cancelled() {
                    // Detach the entry (Corollary 1) before it become garbage (Note 1),
                    entry.remove();
                    // 'schedule.state_control' was detached one (Def 1).
                    // Don't reschedule since it is Cancelled (Rule 1)
                    // schedule is going to be removed
                    false
                } else {
                    unreachable!("Rule 1 was violated");
                }
            }
        }
    }

    fn cancel(&mut self, requested_timer: &TimerApi, timer_token: TimerToken) -> bool {
        let schedule_id = ScheduleId(requested_timer.timer_id, timer_token);
        // See Corollary 2.
        match self.states.entry(schedule_id) {
            Entry::Vacant(_) => false,
            Entry::Occupied(entry) => {
                // Detach and cancel it (Rule 1)
                let state_control = entry.remove();
                state_control.cancel()
            }
        }
    }

    fn handle_timeout(&mut self, worker_queue: &WorkerQueue) -> Option<Duration> {
        loop {
            let now = Instant::now();
            match self.heap.peek() {
                None => return None,
                Some(Reverse(TimeOrdered(earliest))) if now < earliest.at => return Some(earliest.at - now),
                _ => { /* lifetime prevents modifying heap from here. */ }
            }
            let Reverse(TimeOrdered(timed_out)) = self.heap.pop().expect("It always have an item");

            if timed_out.repeat.is_some() {
                if let Some(callback) = Callback::from_schedule(&timed_out) {
                    // timed_out.state_control is re-pushed only after it is popped (Rule 3)
                    if self.try_reschedule(timed_out.clone()) {
                        worker_queue.enqueue(callback);
                    }
                }
            } else {
                let enqueue = match self.states.entry(timed_out.schedule_id) {
                    Entry::Occupied(entry) => {
                        if Arc::ptr_eq(entry.get(), &timed_out.state_control) {
                            // 'timed_out.state_control' was attached one. (Def 1)
                            entry.remove();
                            !timed_out.state_control.is_cancelled()
                        } else {
                            false // detached one
                        }
                    }
                    _ => false, // also detached.
                };
                // timed_out is anyway removed.
                if enqueue {
                    if let Some(callback) = Callback::from_schedule(&timed_out) {
                        worker_queue.enqueue(callback);
                    }
                } else {
                    // 'timed_out.state_control' was detached one. (Def 1)
                    // It already be cancelled (Rule 1)
                    debug_assert!(timed_out.state_control.is_cancelled());
                }
            }
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
struct ScheduleId(TimerId, TimerToken);

/*
  valid state transition:
    Wait -> Timeout
    Wait -> Cancel
    WaitRepeat -> Cancel
*/
#[derive(Eq, PartialEq, Debug)]
enum ScheduleState {
    Wait,
    WaitRepeating,
    Timeout,
    Cancelled,
}

pub struct ScheduleStateControl {
    state: RwLock<ScheduleState>,
}

impl ScheduleStateControl {
    fn new() -> ScheduleStateControl {
        ScheduleStateControl {
            state: RwLock::new(ScheduleState::Wait),
        }
    }

    fn new_repeating() -> ScheduleStateControl {
        ScheduleStateControl {
            state: RwLock::new(ScheduleState::WaitRepeating),
        }
    }

    fn new_auto(repeat: Option<Duration>) -> ScheduleStateControl {
        match repeat {
            Some(_) => ScheduleStateControl::new_repeating(),
            None => ScheduleStateControl::new(),
        }
    }

    pub fn cancel(&self) -> bool {
        let mut state = self.state.write();
        match *state {
            ScheduleState::Wait | ScheduleState::WaitRepeating => {
                *state = ScheduleState::Cancelled;
                true
            }
            _ => false,
        }
    }

    fn is_cancelled(&self) -> bool {
        let state = self.state.read();
        match *state {
            ScheduleState::Cancelled => true,
            _ => false,
        }
    }

    fn within_lock<F, T>(&self, mut callback: F) -> T
    where
        F: FnMut(&mut ScheduleState) -> T, {
        let mut state = self.state.write();
        callback(&mut state)
    }

    fn set_timeout(state: &mut ScheduleState) {
        match state {
            ScheduleState::Wait => {
                *state = ScheduleState::Timeout;
            }
            _ => unreachable!("invalid transition"),
        }
    }
}

#[derive(Clone)]
struct Schedule {
    at: Instant,
    schedule_id: ScheduleId,
    repeat: Option<Duration>,
    state_control: Arc<ScheduleStateControl>,
    handler: Weak<TimeoutHandler>,
}

struct TimeOrdered<T>(T);

impl std::cmp::Eq for TimeOrdered<Schedule> {}

impl std::cmp::PartialEq for TimeOrdered<Schedule> {
    fn eq(&self, other: &TimeOrdered<Schedule>) -> bool {
        let a = self.0.at;
        let b = other.0.at;
        a.eq(&b)
    }
}

impl std::cmp::Ord for TimeOrdered<Schedule> {
    fn cmp(&self, other: &TimeOrdered<Schedule>) -> std::cmp::Ordering {
        let a = self.0.at;
        let b = other.0.at;
        a.cmp(&b)
    }
}

impl std::cmp::PartialOrd for TimeOrdered<Schedule> {
    fn partial_cmp(&self, other: &TimeOrdered<Schedule>) -> Option<std::cmp::Ordering> {
        let a = self.0.at;
        let b = other.0.at;
        a.partial_cmp(&b)
    }
}

struct Callback {
    schedule_id: ScheduleId,
    from_oneshot_schedule: bool,
    state_control: Arc<ScheduleStateControl>,
    handler: Arc<TimeoutHandler>,
}

impl Callback {
    fn from_schedule(schedule: &Schedule) -> Option<Callback> {
        if let Some(handler) = schedule.handler.upgrade() {
            Some(Callback {
                schedule_id: schedule.schedule_id,
                from_oneshot_schedule: schedule.repeat.is_none(),
                state_control: Arc::clone(&schedule.state_control),
                handler,
            })
        } else {
            None
        }
    }
}

fn spawn_workers(size: usize, queue: &Arc<WorkerQueue>) {
    for i in 0..size {
        let queue = Arc::clone(queue);
        thread::Builder::new().name(format!("timer.worker.{}", i)).spawn(move || worker_loop(&queue)).unwrap();
    }
}

fn worker_loop(queue: &Arc<WorkerQueue>) {
    while let Some(callback) = queue.dequeue() {
        let ScheduleId(_, timer_token) = callback.schedule_id;
        callback.state_control.within_lock(|state| {
            debug_assert_ne!(*state, ScheduleState::Timeout);
            if *state != ScheduleState::Cancelled {
                callback.handler.on_timeout(timer_token);
                debug_assert_eq!(callback.from_oneshot_schedule, *state == ScheduleState::Wait);
                if callback.from_oneshot_schedule {
                    ScheduleStateControl::set_timeout(state);
                }
            }
        });
    }
}

struct WorkerQueue {
    queue: Mutex<VecDeque<Callback>>,
    condvar: Condvar,
    stop: AtomicBool,
}

impl WorkerQueue {
    fn new() -> WorkerQueue {
        WorkerQueue {
            queue: Mutex::new(VecDeque::new()),
            condvar: Condvar::new(),
            stop: AtomicBool::new(false),
        }
    }

    fn enqueue(&self, callback: Callback) {
        let mut queue = self.queue.lock();
        if self.stop.load(Ordering::SeqCst) {
            return
        }
        queue.push_back(callback);
        self.condvar.notify_one();
    }

    fn dequeue(&self) -> Option<Callback> {
        let mut queue = self.queue.lock();
        while queue.is_empty() {
            if self.stop.load(Ordering::SeqCst) {
                return None
            }
            self.condvar.wait(&mut queue);
        }
        queue.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use std::panic;

    use super::*;
    use parking_lot::Mutex;

    struct CallbackHandler<F>(F);
    impl<F> TimeoutHandler for CallbackHandler<F>
    where
        F: Fn(TimerToken) + 'static + Sync + Send,
    {
        fn on_timeout(&self, token: TimerToken) {
            let callback = &self.0;
            callback(token);
        }
    }

    fn tick() -> Duration {
        Duration::from_millis(1000)
    }

    fn long_tick() -> Duration {
        tick() * 2
    }

    fn tick_epsilon() -> Duration {
        tick() / 5
    }

    fn similar(a: Instant, b: Instant) -> bool {
        let diff = if a > b {
            a - b
        } else {
            b - a
        };
        diff < tick_epsilon()
    }

    fn new_timer<T>(timer_loop: &TimerLoop, name: TimerName, handler: &Arc<T>) -> TimerApi
    where
        T: 'static + Sized + TimeoutHandler, {
        let timer = timer_loop.new_timer(name);
        timer.set_handler(handler);
        timer
    }

    #[test]
    fn test_timeout() {
        let timer_token = 100;
        let timer_loop = TimerLoop::new(1);
        let pair = Arc::new((Condvar::new(), Mutex::new(None)));
        let handler = {
            let pair = Arc::clone(&pair);
            Arc::new(CallbackHandler(move |token| {
                let (ref condvar, ref mutex) = *pair;
                let mut value = mutex.lock();
                *value = Some((Instant::now(), token));
                condvar.notify_all();
            }))
        };
        let timer = new_timer(&timer_loop, "test", &handler);

        let begin = Instant::now();
        timer.schedule_once(tick(), timer_token).unwrap();

        let (ref condvar, ref mutex) = *pair;
        let mut value = mutex.lock();
        condvar.wait(&mut value);
        assert!(value.is_some());
        let (called_at, token) = value.unwrap();
        assert_eq!(token, timer_token);
        assert!(similar(called_at, begin + tick())); // called_at = now + ticksufficiently small
    }

    #[test]
    fn test_cancel() {
        let timer_token = 100;
        let timer_loop = TimerLoop::new(1);
        let pair = Arc::new((Condvar::new(), Mutex::new(None)));
        let handler = {
            let pair = Arc::clone(&pair);
            Arc::new(CallbackHandler(move |_| {
                let (ref condvar, ref mutex) = *pair;
                let mut value = mutex.lock();
                *value = Some(());
                condvar.notify_all();
            }))
        };
        let timer = new_timer(&timer_loop, "test", &handler);

        assert_eq!(timer.schedule_once(tick(), timer_token), Ok(()));
        assert_eq!(timer.cancel(timer_token), Ok(true));

        let (ref condvar, ref mutex) = *pair;
        let mut value = mutex.lock();
        condvar.wait_for(&mut value, long_tick());
        assert!(value.is_none());
    }

    #[test]
    #[should_panic]
    fn test_timer_set_hander_twice() {
        let timer_loop = TimerLoop::new(1);
        let handler = Arc::new(CallbackHandler(|_| {}));
        let timer = timer_loop.new_timer("test");
        timer.set_handler(&handler);
        timer.set_handler(&handler);
    }

    #[test]
    fn test_handler_drop() {
        let timer_token = 100;
        let timer_loop = TimerLoop::new(1);
        let pair = Arc::new((Condvar::new(), Mutex::new(None)));
        let handler = {
            let pair = Arc::clone(&pair);
            Arc::new(CallbackHandler(move |_| {
                let (ref condvar, ref mutex) = *pair;
                let mut value = mutex.lock();
                *value = Some(());
                condvar.notify_all();
            }))
        };
        let timer = new_timer(&timer_loop, "test", &handler);

        assert_eq!(timer.schedule_once(tick(), timer_token), Ok(()));
        drop(handler);

        let (ref condvar, ref mutex) = *pair;
        let mut value = mutex.lock();
        condvar.wait_for(&mut value, long_tick());
        assert!(value.is_none());
    }

    #[test]
    fn test_schedule_twice() {
        let timer_token = 100;
        let timer_loop = TimerLoop::new(1);
        let handler = Arc::new(CallbackHandler(|_| {}));
        let timer = new_timer(&timer_loop, "test", &handler);

        assert_eq!(timer.schedule_once(tick(), timer_token), Ok(()));
        assert_eq!(timer.schedule_once(tick(), timer_token), Err(ScheduleError::TokenAlreadyScheduled));
    }

    #[test]
    fn test_schedule_twice_with_different_keys() {
        let timer_token_1 = 100;
        let timer_token_2 = 200;
        let timer_loop = TimerLoop::new(1);
        let handler = Arc::new(CallbackHandler(|_| {}));
        let timer = new_timer(&timer_loop, "test", &handler);

        assert_eq!(timer.schedule_once(tick(), timer_token_1), Ok(()));
        assert_eq!(timer.schedule_once(tick(), timer_token_2), Ok(()));
    }

    #[test]
    fn test_reschedule_after_timeout() {
        let timer_token = 100;
        let timer_loop = TimerLoop::new(1);
        let pair = Arc::new((Condvar::new(), Mutex::new(None)));
        let handler = {
            let pair = Arc::clone(&pair);
            Arc::new(CallbackHandler(move |token| {
                let (ref condvar, ref mutex) = *pair;
                let mut value = mutex.lock();
                *value = Some((Instant::now(), token));
                condvar.notify_all();
            }))
        };
        let timer = new_timer(&timer_loop, "test", &handler);

        assert_eq!(timer.schedule_once(tick(), timer_token), Ok(()));
        thread::sleep(long_tick());

        let begin = Instant::now();
        assert_eq!(timer.schedule_once(tick(), timer_token), Ok(()));

        let (ref condvar, ref mutex) = *pair;
        let mut value = mutex.lock();
        condvar.wait(&mut value);
        assert!(value.is_some());
        let (called_at, token) = value.unwrap();
        assert_eq!(token, timer_token);
        assert!(similar(called_at, begin + tick())); // called_at = now + ticksufficiently small
    }

    #[test]
    fn test_cancel_and_reschedule() {
        let timer_token = 100;
        let timer_loop = TimerLoop::new(1);
        let pair = Arc::new((Condvar::new(), Mutex::new(None)));
        let handler = {
            let pair = Arc::clone(&pair);
            Arc::new(CallbackHandler(move |token| {
                let (ref condvar, ref mutex) = *pair;
                let mut value = mutex.lock();
                *value = Some((Instant::now(), token));
                condvar.notify_all();
            }))
        };
        let timer = new_timer(&timer_loop, "test", &handler);

        let begin = Instant::now();
        assert_eq!(timer.schedule_once(tick(), timer_token), Ok(()));
        assert_eq!(timer.cancel(timer_token), Ok(true));
        assert_eq!(timer.schedule_once(tick(), timer_token), Ok(()));


        let (ref condvar, ref mutex) = *pair;
        let mut value = mutex.lock();
        condvar.wait(&mut value);
        assert!(value.is_some());
        let (called_at, token) = value.unwrap();
        assert_eq!(token, timer_token);
        assert!(similar(called_at, begin + tick())); // called_at = now + ticksufficiently small
    }
    #[test]
    fn test_repeat() {
        const TEST_COUNT: usize = 4;
        let timer_token = 100;
        let timer_loop = TimerLoop::new(1);
        let pair = Arc::new((Condvar::new(), Mutex::new(vec![])));
        let handler = {
            let pair = Arc::clone(&pair);
            Arc::new(CallbackHandler(move |_| {
                let (ref condvar, ref mutex) = *pair;
                let mut value = mutex.lock();
                value.push(Instant::now());

                condvar.notify_all();
            }))
        };
        let timer = new_timer(&timer_loop, "test", &handler);

        let begin = Instant::now();
        timer.schedule_repeat(tick(), timer_token).unwrap();

        let (ref condvar, ref mutex) = *pair;
        for i in 0..TEST_COUNT {
            let mut value = mutex.lock();
            assert_eq!(value.len(), i);
            condvar.wait(&mut value);
            assert_eq!(value.len(), i + 1);
        }
        assert_eq!(timer.cancel(timer_token), Ok(true));

        let (ref condvar, ref value) = *pair;
        let mut value = value.lock();
        condvar.wait_for(&mut value, long_tick()); // wait sufficiently
        assert_eq!(value.len(), TEST_COUNT);
        assert!(similar(value[0], begin + tick()));
        for i in 1..TEST_COUNT {
            assert!(similar(value[i - 1] + tick(), value[i]));
        }
    }
}
