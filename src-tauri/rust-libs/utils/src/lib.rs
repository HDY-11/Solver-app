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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_usage() {
        let mut token = Token::new(42);
        assert_eq!(*token.try_get().unwrap(), 42);

        let mut lent = token.lend();
        *lent = 100;
        assert_eq!(*lent, 100);

        drop(lent);
        assert_eq!(*token.try_get().unwrap(), 100);
    }

    #[test]
    fn test_nested_lend_not_possible() {
        let mut token = Token::new(1);
        let _lent = token.lend();
        // 编译错误：token 已被可变借用，无法再次 lend
        // let _lent2 = token.lend();
    }

    #[test]
    fn test_drop_token_with_data() {
        let token = Token::new(String::from("hello"));
        drop(token); // 不应泄漏内存
    }

    #[test]
    fn test_lent_deref() {
        let mut token = Token::new(vec![1, 2, 3]);
        let lent = token.lend();
        assert_eq!(lent.len(), 3);
        assert_eq!(lent[0], 1);
    }

    #[test]
    fn test_lent_deref_mut() {
        let mut token = Token::new(vec![1, 2, 3]);
        let mut lent = token.lend();
        lent.push(4);
        assert_eq!(lent.len(), 4);
    }

    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Token<i32>>();
        assert_send::<Token<String>>();
        // 非 Send 类型不能放在 Send Token 中
        // assert_send::<Token<std::rc::Rc<i32>>>();
    }
}