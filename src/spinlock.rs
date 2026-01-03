use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct Spinlock<T> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

pub struct SpinlockGuard<'a, T> {
    lock: &'a Spinlock<T>,
    data: &'a mut T,
}

unsafe impl<T> Sync for Spinlock<T> {}
unsafe impl<T> Send for Spinlock<T> {}

impl<T> Spinlock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinlockGuard<T> {
        // Disable interrupts to avoid deadlock with ISR
        unsafe { core::arch::asm!("cli") };

        while self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while self.lock.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }

        SpinlockGuard {
            lock: self,
            data: unsafe { &mut *self.data.get() },
        }
    }

    pub fn as_ptr(&self) -> *mut T {
        self.data.get()
    }
}

impl<'a, T> Deref for SpinlockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.data
    }
}

impl<'a, T> DerefMut for SpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}

impl<'a, T> Drop for SpinlockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.lock.store(false, Ordering::Release);
        // Re-enable interrupts?
        // WARNING: This is simplistic. Nested locks or if interrupts were already disabled matter.
        // For simple xv6-like OS, push/pop cli is better.
        // But for now, we assume simple usage.
        unsafe { core::arch::asm!("sti") };
    }
}
