use std::mem::MaybeUninit;

pub struct Queue<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    head: usize,
    tail: usize,
    len: usize,
}

impl<T, const N: usize> Queue<T, N> {

    pub fn new() -> Self {
        let data = std::array::from_fn(|_| MaybeUninit::uninit());
        Self {
            data,
            head: 0,
            tail: 0,
            len: 0,
        }
    }
    
    pub fn push(&mut self, value: T) -> Result<(), T> {
        if self.len == N {
            return Err(value);
        }

        unsafe {
            self.data.get_unchecked_mut(self.tail).write(value);
        }

        self.tail = (self.tail + 1) % N;
        self.len += 1;
        Ok(())
    }
    
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        let value = unsafe { self.data[self.head].assume_init_read() };

        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(value)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len == N
    }
    
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
}

impl<T, const N: usize> Drop for Queue<T, N> {
    fn drop(&mut self) {
        while let Some(_) = self.pop() {}
    }
}