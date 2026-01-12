use crate::proc;
use crate::spinlock::{Spinlock, SpinlockGuard};
use core::cell::UnsafeCell;

pub struct SleepLockSafe<T> {
    lock: Spinlock<bool>,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SleepLockSafe<T> {}

impl<T> SleepLockSafe<T> {
    pub const fn new(data: T) -> Self {
        Self {
            lock: Spinlock::new(false, "SLEEPLOCK"),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SleepLockGuard<T> {
        let mut lk = self.lock.lock();
        while *lk {
            proc::sleep(self as *const _ as usize, Some(lk));
            lk = self.lock.lock();
        }
        *lk = true;
        SleepLockGuard { lock: self }
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
}

pub struct SleepLockGuard<'a, T> {
    lock: &'a SleepLockSafe<T>,
}

impl<'a, T> Drop for SleepLockGuard<'a, T> {
    fn drop(&mut self) {
        let mut lk = self.lock.lock.lock();
        *lk = false;
        proc::wakeup(self.lock as *const _ as usize);
    }
}

use core::ops::{Deref, DerefMut};

impl<'a, T> Deref for SleepLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for SleepLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}
