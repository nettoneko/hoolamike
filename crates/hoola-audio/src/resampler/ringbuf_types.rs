use {
    ringbuf::{storage::Heap, traits::Split},
    tap::prelude::*,
};

pub(super) type Buffer<T> = ringbuf::LocalRb<Heap<T>>;
pub type Reader<T> = <self::Buffer<T> as Split>::Cons;
pub type Writer<T> = <self::Buffer<T> as Split>::Prod;

pub struct BufferSplit<T> {
    pub reader: Reader<T>,
    pub writer: Writer<T>,
}

impl<T> BufferSplit<T> {
    pub fn new(capacity: usize) -> Self {
        self::Buffer::new(capacity)
            .split()
            .pipe(|(writer, reader)| Self { reader, writer })
    }
}
