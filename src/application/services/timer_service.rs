//! TimerService - centralized timer management
//!
//! Manages all Win32 timers used by the application, providing a clean
//! interface for starting and stopping various timers.

use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{KillTimer, SetTimer},
};

/// Timer identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum TimerId {
    /// Cursor blink animation
    CursorBlink = 1,
    /// Theme file watcher for hot-reload
    FileWatch = 2,
    /// Window animation (fade in/out)
    Animation = 3,
    /// Clock widget updates
    Clock = 4,
    /// Background task polling
    TaskPoll = 5,
    /// Tail view refresh
    TailRefresh = 6,
}

impl TimerId {
    /// Try to convert a usize to a TimerId
    pub fn from_usize(value: usize) -> Option<Self> {
        match value {
            1 => Some(TimerId::CursorBlink),
            2 => Some(TimerId::FileWatch),
            3 => Some(TimerId::Animation),
            4 => Some(TimerId::Clock),
            5 => Some(TimerId::TaskPoll),
            6 => Some(TimerId::TailRefresh),
            _ => None,
        }
    }
}

/// Timer intervals in milliseconds
#[derive(Debug, Clone)]
pub struct TimerIntervals {
    pub cursor_blink: u32,
    pub file_watch: u32,
    pub animation_frame: u32,
    pub clock_update: u32,
    pub task_poll: u32,
    pub tail_refresh: u32,
}

impl Default for TimerIntervals {
    fn default() -> Self {
        Self {
            cursor_blink: 530,
            file_watch: 500,
            animation_frame: 16, // ~60fps
            clock_update: 1000,
            task_poll: 100,
            tail_refresh: 200,
        }
    }
}

/// Centralized timer management service
#[derive(Debug)]
pub struct TimerService {
    hwnd: HWND,
    intervals: TimerIntervals,
    /// Track which timers are currently active
    active_timers: u8, // bitfield
}

impl TimerService {
    /// Create a new timer service for the given window
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            intervals: TimerIntervals::default(),
            active_timers: 0,
        }
    }

    /// Create with custom intervals
    pub fn with_intervals(hwnd: HWND, intervals: TimerIntervals) -> Self {
        Self {
            hwnd,
            intervals,
            active_timers: 0,
        }
    }

    /// Start a specific timer
    pub fn start(&mut self, timer: TimerId) {
        let (id, interval) = match timer {
            TimerId::CursorBlink => (1, self.intervals.cursor_blink),
            TimerId::FileWatch => (2, self.intervals.file_watch),
            TimerId::Animation => (3, self.intervals.animation_frame),
            TimerId::Clock => (4, self.intervals.clock_update),
            TimerId::TaskPoll => (5, self.intervals.task_poll),
            TimerId::TailRefresh => (6, self.intervals.tail_refresh),
        };

        unsafe {
            SetTimer(self.hwnd, id, interval, None);
        }
        self.active_timers |= 1 << (id - 1);
    }

    /// Stop a specific timer
    pub fn stop(&mut self, timer: TimerId) {
        let id = timer as usize;
        unsafe {
            let _ = KillTimer(self.hwnd, id);
        }
        self.active_timers &= !(1 << (id - 1));
    }

    /// Check if a timer is currently active
    pub fn is_active(&self, timer: TimerId) -> bool {
        let id = timer as usize;
        (self.active_timers & (1 << (id - 1))) != 0
    }

    /// Start cursor blink timer
    #[inline]
    pub fn start_cursor(&mut self) {
        self.start(TimerId::CursorBlink);
    }

    /// Stop cursor blink timer
    #[inline]
    pub fn stop_cursor(&mut self) {
        self.stop(TimerId::CursorBlink);
    }

    /// Start file watch timer for theme hot-reload
    #[inline]
    pub fn start_file_watch(&mut self) {
        self.start(TimerId::FileWatch);
    }

    /// Stop file watch timer
    #[inline]
    pub fn stop_file_watch(&mut self) {
        self.stop(TimerId::FileWatch);
    }

    /// Start animation timer
    #[inline]
    pub fn start_animation(&mut self) {
        self.start(TimerId::Animation);
    }

    /// Stop animation timer
    #[inline]
    pub fn stop_animation(&mut self) {
        self.stop(TimerId::Animation);
    }

    /// Start clock update timer
    #[inline]
    pub fn start_clock(&mut self) {
        self.start(TimerId::Clock);
    }

    /// Stop clock update timer
    #[inline]
    pub fn stop_clock(&mut self) {
        self.stop(TimerId::Clock);
    }

    /// Start task poll timer
    #[inline]
    pub fn start_task_poll(&mut self) {
        self.start(TimerId::TaskPoll);
    }

    /// Stop task poll timer
    #[inline]
    pub fn stop_task_poll(&mut self) {
        self.stop(TimerId::TaskPoll);
    }

    /// Start tail refresh timer
    #[inline]
    pub fn start_tail_refresh(&mut self) {
        self.start(TimerId::TailRefresh);
    }

    /// Stop tail refresh timer
    #[inline]
    pub fn stop_tail_refresh(&mut self) {
        self.stop(TimerId::TailRefresh);
    }

    /// Stop all active timers
    pub fn stop_all(&mut self) {
        for id in 1..=6 {
            if (self.active_timers & (1 << (id - 1))) != 0 {
                unsafe {
                    let _ = KillTimer(self.hwnd, id);
                }
            }
        }
        self.active_timers = 0;
    }
}
