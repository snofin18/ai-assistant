//! 定容 ring buffer。
//!
//! 为什么不用无界 `Vec`：无界缓冲会把"批量"变成"攒到 OOM 再一起失败"。容量到顶时
//! [`RingBuffer::push`] 返回 [`PushOutcome::Full`]，调用方**必须**立刻 flush —— 本 crate
//! 从不静默丢事件（铁律 1：要么写成功，要么报错）。
//!
//! 边界：本模块不认识事件、不认识 SQL，只提供"定容 FIFO"这一件事。

use std::collections::VecDeque;

/// 入队结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    /// 已入队，尚未到容量上限。
    Buffered,
    /// 已入队**且**已到容量上限 —— 调用方必须立刻 flush。
    Full,
}

/// 定容 FIFO 缓冲（容量至少 1）。
#[derive(Debug)]
pub struct RingBuffer<T> {
    items: VecDeque<T>,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    /// 建一个容量为 `capacity` 的空缓冲。
    ///
    /// 容量会被抬到至少 1：容量 0 等于"每个事件都必须立刻 flush"，那是
    /// [`crate::Durability::Immediate`] 的语义，不该由 ring buffer 表达。
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: VecDeque::with_capacity(capacity),
            capacity: capacity.max(1),
        }
    }

    /// 入队；返回是否已到容量上限。
    pub fn push(&mut self, item: T) -> PushOutcome {
        self.items.push_back(item);
        if self.items.len() >= self.capacity {
            PushOutcome::Full
        } else {
            PushOutcome::Buffered
        }
    }

    /// 当前条数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 容量（已抬到至少 1）。
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// 按入队顺序迭代（返回的迭代器自带 `#[must_use]`，这里不再重复标注）。
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// 最后入队的一项（用于求链尾）。
    #[must_use]
    pub fn last(&self) -> Option<&T> {
        self.items.back()
    }

    /// 清空。**只在 flush 成功之后调用** —— 提前清空就是丢事件。
    pub fn clear(&mut self) {
        self.items.clear();
    }
}
