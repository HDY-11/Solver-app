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


use std::ops::{Deref, DerefMut};

/// 一个可恢复的独占访问令牌。
///
/// `Token<T>` 拥有一个 `T` 类型的数据，并且在同一时刻只允许一处访问它：
/// - 当数据在 `Token` 手中时，可以通过 [`try_get`](Token::try_get) 获取可变引用。
/// - 通过 [`lend`](Token::lend) 可以将数据"借出"给 [`Lent`] 守卫，
///   Token 在此期间变为空状态，直到 `Lent` 被丢弃时数据自动归还。
///
/// # 使用场景
///
/// 特别适合异步上下文中需要临时"取出"数据、在多个 `await` 点之间持有的场景。
///
/// # 安全性
///
/// 此类型对外 API 完全 safe。内部使用裸指针和 `unsafe` 实现，
/// 通过以下不变式保证安全：
///
/// - `slot` 要么指向一个有效的 `T`，要么为 `null`（表示数据已被借出）。
/// - `Lent` 存活期间，`Token.slot` 必为 `null`，且外界无法通过 `&mut self`
///   方法访问 `Token`（因为 `Lent` 持有对 `Token` 的可变借用）。
/// - `Lent` 被 drop 时，数据指针被写回 `Token.slot`，恢复有效状态。
///
/// # 线程安全
///
/// - `Token<T>: Send` 当 `T: Send`
/// - `Token<T>` **未**实现 `Sync`，不可多线程共享引用
/// - `Lent<'a, T>: Send` 当 `T: Send`
///
/// # 示例
///
/// ```
/// # use your_crate::Token;
/// let mut token = Token::new(42);
///
/// // 数据在 Token 手中时，可以访问
/// assert_eq!(*token.try_get().unwrap(), 42);
/// *token.try_get().unwrap() = 100;
///
/// // 借出数据
/// let mut lent = token.lend();
/// *lent = 200;
///
/// // 此时 Token 为空，返回 None
/// assert!(token.try_get().is_none());
///
/// // 归还数据
/// drop(lent);
///
/// // Token 恢复访问
/// assert_eq!(*token.try_get().unwrap(), 200);
/// ```
///
/// 在异步代码中跨 `await` 点持有：
///
/// ```ignore
/// async fn example(mut token: Token<Connection>) {
///     let mut conn = token.lend();
///     do_something(&mut conn).await;
///     do_another(&mut conn).await;
///     // conn 在这里被 drop，数据归还给 token
/// }
/// ```
pub struct Token<T> {
    /// 指向数据的裸指针。
    /// - `NonNull` 时：数据在 Token 手中，可通过 `try_get` 安全访问。
    /// - `null` 时：数据已被借出给某个 `Lent`，Token 处于空状态。
    slot: *mut T,
}

/// 从 [`Token`] 借出数据的 RAII 守卫。
///
/// `Lent` 通过 [`Token::lend`] 创建，持有对 `Token` 的可变借用，
/// 阻止在 `Lent` 存活期间通过原始 `Token` 访问数据。
///
/// 当 `Lent` 被丢弃时，数据自动归还给 `Token`。
///
/// # Deref 行为
///
/// - `Deref<Target = T>` 提供对数据的不可变引用
/// - `DerefMut` 提供对数据的可变引用
///
/// # 注意
///
/// `Lent` 的存在会阻止对 `Token` 的任何 `&mut self` 方法调用
///（如 `lend()`、`try_get()`），因为 `Lent` 持有对 `Token` 的可变借用。
/// 这是编译期保证，而非运行期检查。
///
/// # 线程安全
///
/// `Lent<'a, T>: Send` 当 `T: Send`，因此可以在异步任务之间传递。
pub struct Lent<'a, T> {
    /// 指向被借出数据的裸指针。始终有效（非 null）。
    data: *mut T,
    /// 对原始 Token 的可变引用，用于在 drop 时归还数据。
    token: &'a mut Token<T>,
}

impl<T> Token<T> {
    /// 创建一个新的 `Token<T>`，持有给定的数据。
    ///
    /// 数据被分配在堆上，`Token` 获得其独占所有权。
    ///
    /// # 示例
    ///
    /// ```
    /// # use your_crate::Token;
    /// let token = Token::new("hello");
    /// assert_eq!(*token.try_get().unwrap(), "hello");
    /// ```
    pub fn new(data: T) -> Self {
        Token {
            slot: Box::into_raw(Box::new(data)),
        }
    }

    /// 将数据借出，返回一个 [`Lent`] 守卫，同时将自身标记为空。
    ///
    /// 此方法需要 `&mut self`，因此只要返回的 `Lent` 存活，
    /// 编译器将阻止对 `Token` 的任何其他可变借用。
    ///
    /// # 返回值
    ///
    /// 返回一个 [`Lent`] 守卫，它实现了 `Deref<Target = T>` 和 `DerefMut`，
    /// 可以像 `&mut T` 一样使用。当守卫被丢弃时，数据自动归还。
    ///
    /// # 示例
    ///
    /// ```
    /// # use your_crate::Token;
    /// let mut token = Token::new(vec![1, 2, 3]);
    /// let mut lent = token.lend();
    /// lent.push(4);
    /// assert_eq!(lent.len(), 4);
    /// drop(lent);
    /// assert_eq!(token.try_get().unwrap().len(), 4);
    /// ```
    pub fn lend(&mut self) -> Lent<'_, T> {
        let ptr = self.slot;
        // SAFETY: 将 slot 置空，确保 Token 不再认为它拥有数据。
        // Lent 的 Drop 实现会负责归还此指针。
        self.slot = std::ptr::null_mut();
        Lent {
            data: ptr,
            token: self,
        }
    }

    /// 尝试获取对内部数据的可变引用。
    ///
    /// # 返回值
    ///
    /// - `Some(&mut T)`：如果数据当前在 `Token` 手中（未被借出）。
    /// - `None`：如果数据已被借出给某个 `Lent`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use your_crate::Token;
    /// let mut token = Token::new(42);
    ///
    /// // 数据在手中，可以访问
    /// let val = token.try_get().unwrap();
    /// *val = 100;
    ///
    /// // 借出后，Token 为空
    /// let lent = token.lend();
    /// assert!(token.try_get().is_none());//无法编译，不能二次借用
    /// drop(lent);
    ///
    /// // 归还后，又可以访问了
    /// assert_eq!(*token.try_get().unwrap(), 100);
    /// ```
    pub fn try_get(&mut self) -> Option<&mut T> {
        if self.slot.is_null() {
            None
        } else {
            // SAFETY: slot 非 null 意味着数据在 Token 手中，
            // 指针来自 Box::into_raw，始终有效对齐。
            // 此方法需要 &mut self，且 slot 非 null 意味着
            // 没有 Lent 存在，因此这是对数据的唯一引用。
            Some(unsafe { &mut *self.slot })
        }
    }

    /// 消耗 Token 并取出内部数据。
    ///
    /// 仅在没有任何活跃的 `Lent` 时才能调用（由 `&mut self` 保证）。
    pub fn take(self) -> T {
        // SAFETY: self 被消耗，slot 非空且为 Box 指针，我们重获所有权
        let ptr = self.slot;
        // 不能调用 drop，因为我们要取出数据；手动防止 drop 释放
        std::mem::forget(self);
        unsafe { *Box::from_raw(ptr) }
    }

    /// 替换内部数据，返回旧值。
    ///
    /// # 安全性
    /// - 要求 `&mut self`，保证此时没有活跃的 `Lent`（它们需要 `&mut Token`）。
    /// - 内部用 unsafe 完成原地析构与写新值，指针始终有效。
    pub fn replace(&mut self, new: T) -> T {
        let old_ptr = self.slot;
        // 创建新 Box 并获取裸指针
        self.slot = Box::into_raw(Box::new(new));
        // SAFETY: old_ptr 非空（由 Token 不变式保证），且没有 Lent 引用它，
        // 我们重新获得所有权并立即释放。
        unsafe { *Box::from_raw(old_ptr) }
    }
}

impl<T> Drop for Token<T> {
    fn drop(&mut self) {
        if !self.slot.is_null() {
            // SAFETY: slot 非 null 时指向由 Box::into_raw 分配的 Box<T>。
            // 由于 Token 拥有该数据的所有权，在 drop 时必须释放。
            //
            // 如果 slot 为 null，说明数据已被 Lent 持有，
            // 此时 Lent 持有对 self 的可变引用，因此 Token 不可能被 drop
            // （Rust 的借用规则保证：当 &mut T 存在时，T 不能被 drop）。
            // 所以到这里时 slot 要么非 null（正常情况），
            // 要么已经发生了严重的 unsafe 代码错误。
            unsafe {
                drop(Box::from_raw(self.slot));
            }
        }
    }
}

impl<'a, T> Drop for Lent<'a, T> {
    /// 归还数据给 Token。
    ///
    /// 将数据指针写回 `Token.slot`，恢复其有效状态。
    fn drop(&mut self) {
        // SAFETY: data 始终有效（来自 Token 的 slot，而 Token 保证 slot 非 null 时有效）。
        // 归还时 Token.slot 必为 null（由 lend 保证），没有其他指针指向该数据。
        self.token.slot = self.data;
    }
}

impl<'a, T> Deref for Lent<'a, T> {
    type Target = T;

    /// 提供对借出数据的不可变引用。
    fn deref(&self) -> &T {
        // SAFETY: data 始终指向有效的 T。Lent 存活期间持有对 Token 的可变引用，
        // 阻止了其他人通过 Token 访问数据，因此这里是唯一的访问者。
        unsafe { &*self.data }
    }
}

impl<'a, T> DerefMut for Lent<'a, T> {
    /// 提供对借出数据的可变引用。
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: data 始终指向有效的 T。&mut self 保证这是唯一的可变引用。
        unsafe { &mut *self.data }
    }
}

// SAFETY: Token 拥有 T 的所有权语义。当 Token 被发送到其他线程时，
// 内部的 T 也随之发送。要求 T: Send 以确保内部数据可以安全跨线程。
// Token 未实现 Sync，因此不存在多个线程共享同一个 Token 的情况。
unsafe impl<T: Send> Send for Token<T> {}

// SAFETY: Lent 持有对 T 的可变引用语义（独占访问）。
// 将 Lent 发送到其他线程相当于将 &mut T 发送过去，这需要 T: Send。
// Lent 未实现 Sync，因此不存在多个线程共享的情况。
unsafe impl<T: Send> Send for Lent<'_, T> {}



use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::mem::ManuallyDrop;

pub struct Slot<T> {
    ptr: AtomicPtr<T>,
}

pub struct Lease<T> {
    ptr: *mut T,
    _slot: Arc<Slot<T>>,
}

impl<T> Slot<T> {
    pub fn new(value: T) -> Arc<Self> {
        Arc::new(Self {
            ptr: AtomicPtr::new(Box::into_raw(Box::new(value))),
        })
    }
    
    /// 尝试借出数据。如果已被借出，返回 None。
    pub fn try_lease(self: &Arc<Self>) -> Option<Lease<T>> {
        let ptr = self.ptr.load(Ordering::Acquire);
        if ptr.is_null() {
            return None;  // 已被借出
        }
        // CAS：将 ptr 从非 null 替换为 null
        match self.ptr.compare_exchange(
            ptr,
            std::ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // 成功获取
                Some(Lease {
                    ptr,
                    _slot: Arc::clone(self),
                })
            }
            Err(_) => {
                // 其他线程抢先了
                None
            }
        }
    }
    
    /// 忙等借出（循环 CAS 直到成功）
    pub fn lease(self: &Arc<Self>) -> Lease<T> {
        loop {
            if let Some(lease) = self.try_lease() {
                return lease;
            }
            std::hint::spin_loop();
        }
    }
}

impl<T> Lease<T> {
    /// 主动归还，此后 Lease 为空。
    pub fn restore(&mut self) {
        if self.ptr.is_null() {
            return ;
        }
        let old = self._slot.ptr.swap(self.ptr, Ordering::AcqRel);
        self.ptr = std::ptr::null_mut();
        if !old.is_null() {
            unsafe { drop(Box::from_raw(old)); }
        }
    }

    /// 尝试重新租借（仅在已归还后才能调用）。
    pub fn reclaim(&mut self) -> bool {
        assert!(self.ptr.is_null(), "Lease still holds data, call restore() first");
        if let Some(new_lease) = self._slot.try_lease() {
            let mut new_lease = ManuallyDrop::new(new_lease);
            self.ptr = new_lease.ptr;
            // 不让 new_lease 的 drop 写回
            unsafe {
                std::ptr::drop_in_place(&mut new_lease._slot);
            }
            true
        } else {
            false
        }
    }
}

impl<T> std::ops::Deref for Lease<T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T> std::ops::DerefMut for Lease<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

impl<T> Drop for Lease<T> {
    fn drop(&mut self) {
        // 归还数据指针
        if !self.ptr.is_null(){
            self._slot.ptr.store(self.ptr, Ordering::Release);
        }
    }
}

unsafe impl<T: Send> Send for Slot<T> {}
unsafe impl<T: Send + Sync> Sync for Slot<T> {}
unsafe impl<T: Send> Send for Lease<T> {}


#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn test_basic_lease_and_restore() {
        let slot = Slot::new(42);
        let mut lease = slot.try_lease().unwrap();
        assert_eq!(*lease, 42);
        
        *lease = 100;
        assert_eq!(*lease, 100);
        
        // 归还前无法再次借出
        assert!(slot.try_lease().is_none());
        
        lease.restore();
        assert!(lease.ptr.is_null());
        
        // 归还后可以再次借出
        let lease2 = slot.try_lease().unwrap();
        assert_eq!(*lease2, 100);
    }

    #[test]
    fn test_deref_and_deref_mut() {
        let slot = Slot::new(String::from("hello"));
        let mut lease = slot.try_lease().unwrap();
        
        // Deref
        assert_eq!(lease.len(), 5);
        
        // DerefMut
        lease.push_str(" world");
        assert_eq!(*lease, "hello world");
    }

    #[test]
    fn test_try_lease_while_leased() {
        let slot = Slot::new(1);
        let _lease = slot.try_lease().unwrap();
        
        // 已经被借出，应该返回 None
        assert!(slot.try_lease().is_none());
    }

    #[test]
    fn test_drop_auto_restore() {
        let slot = Slot::new(1);
        
        {
            let _lease = slot.try_lease().unwrap();
            // _lease 离开作用域，自动归还
        }
        
        // 应该能再次借出
        let lease = slot.try_lease().unwrap();
        assert_eq!(*lease, 1);
    }

    #[test]
    fn test_restore_then_reclaim() {
        let slot = Slot::new(42);
        let mut lease = slot.try_lease().unwrap();
        
        *lease = 99;
        lease.restore();
        
        // reclaim 成功
        assert!(lease.reclaim());
        assert_eq!(*lease, 99);
        
        // 归还后再次 reclaim
        lease.restore();
        assert!(lease.reclaim());
    }

    #[test]
    fn test_reclaim_fails_when_leased_by_other() {
        let slot = Slot::new(1);
        let mut lease1 = slot.try_lease().unwrap();
        *lease1 = 100;
        lease1.restore();
        
        // 另一个借出
        let _lease2 = slot.try_lease().unwrap();
        
        // lease1 的 reclaim 应该失败
        assert!(!lease1.reclaim());
    }

    #[test]
    #[should_panic(expected = "Lease still holds data, call restore() first")]
    fn test_reclaim_panics_when_still_holding() {
        let slot = Slot::new(1);
        let mut lease = slot.try_lease().unwrap();
        
        // 没有 restore，直接 reclaim 应该 panic
        lease.reclaim();
    }

    #[test]
    fn test_multiple_restore() {
        let slot = Slot::new(1);
        let mut lease = slot.try_lease().unwrap();
        
        lease.restore();
        lease.restore();  // 第二次 restore 应该无害
        lease.restore();  // 第三次也无害
        
        assert!(lease.reclaim());
    }

    #[test]
    fn test_lease_spin_loop() {
        let slot = Slot::new(42);
        let slot = Arc::new(slot);
        
        let slot_clone = Arc::clone(&slot);
        
        // 先借出
        let mut lease = slot.lease();
        *lease = 100;
        
        // 在另一个线程中，忙等借出
        let handle = thread::spawn(move || {
            let mut lease = slot_clone.lease();
            assert_eq!(*lease, 100);
            *lease = 200;
            lease
        });
        
        // 稍后归还，让另一个线程能借到
        thread::sleep(std::time::Duration::from_millis(10));
        lease.restore();
        
        let lease2 = handle.join().unwrap();
        assert_eq!(*lease2, 200);
    }

    #[test]
    fn test_concurrent_try_lease() {
        let slot = Slot::new(0);
        let barrier = Arc::new(Barrier::new(4));
        let mut handles = vec![];
        
        for i in 0..4 {
            let slot = slot.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                for _ in 0..100 {
                    if let Some(mut lease) = slot.try_lease() {
                        *lease += 1;
                        lease.restore();
                        break;
                    }
                    std::hint::spin_loop();
                }
            }));
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        let lease = slot.try_lease().unwrap();
        assert_eq!(*lease, 4);  // 4 个线程各加 1
    }

    #[test]
    fn test_restore_handles_concurrent_corruption() {
        // 测试 restore 中防御性处理 old 非 null 的情况
        let slot = Slot::new(1);
        let mut lease = slot.try_lease().unwrap();
        
        // 模拟异常：手动将 slot 的 ptr 设为非 null（模拟并发归还）
        let fake_ptr = Box::into_raw(Box::new(999));
        slot.ptr.store(fake_ptr, Ordering::Release);
        
        // restore 应该处理这种情况，释放掉虚假的指针
        lease.restore();
        
        // 槽应该处于可用状态（ptr 不为 null，是 lease 归还的）
        let lease2 = slot.try_lease().unwrap();
        assert_eq!(*lease2, 1);  // 原值被归还
    }

    #[test]
    fn test_reclaim_after_multiple_cycles() {
        let slot = Slot::new(String::from("test"));
        let mut lease = slot.try_lease().unwrap();
        
        for i in 0..10 {
            lease.push_str(&format!("_{}", i));
            lease.restore();
            assert!(lease.reclaim());
        }
        
        assert_eq!(*lease, "test_0_1_2_3_4_5_6_7_8_9");
    }

    #[test]
    fn test_drop_without_restore() {
        let slot = Slot::new(42);
        let slot_clone = Arc::clone(&slot);
        
        {
            let mut lease = slot.try_lease().unwrap();
            *lease = 100;
            // 不显式调用 restore，依赖 Drop
        }
        
        // Drop 后应该可以重新借出，且值被保留
        let lease = slot.try_lease().unwrap();
        assert_eq!(*lease, 100);
    }

    #[test]
    fn test_lease_holds_unique_access() {
        struct NonCopy(i32);
        
        let slot = Slot::new(NonCopy(42));
        let mut lease = slot.try_lease().unwrap();
        
        lease.0 = 100;
        assert_eq!(lease.0, 100);
        
        // 无法同时借出
        assert!(slot.try_lease().is_none());
    }
}