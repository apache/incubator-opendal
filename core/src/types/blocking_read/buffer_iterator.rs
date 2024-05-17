use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::Buf;
use rayon::prelude::*;

use crate::raw::*;
use crate::*;

pub(super) struct BufferIterator {
    inner: Arc<dyn oio::BlockingRead>,
    it: RangeIterator,

    chunk: usize,
    concurrent: usize,
}

impl BufferIterator {
    pub fn new(
        inner: Arc<dyn oio::BlockingRead>,
        options: OpReader,
        offset: u64,
        end: Option<u64>,
    ) -> Self {
        let chunk = options.chunk().unwrap_or(4 * 1024 * 1024);
        let it = RangeIterator {
            offset,
            chunk: chunk as u64,
            end,
            finished: Arc::new(AtomicBool::new(false)),
        };
        Self {
            inner,
            it,
            chunk,
            concurrent: options.concurrent(),
        }
    }
}

impl Iterator for BufferIterator {
    type Item = Result<Buffer>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut bufs = Vec::with_capacity(self.concurrent);

        let intervals: Vec<Range<u64>> = (0..self.concurrent)
            .filter_map(|_| self.it.next())
            .collect();

        if intervals.is_empty() {
            return None;
        }

        let results: Vec<Result<(usize, Buffer)>> = intervals
            .into_par_iter()
            .map(|range| -> Result<(usize, Buffer)> {
                let limit = (range.end - range.start) as usize;

                let bs = self.inner.read_at(range.start, limit)?;
                let n = bs.remaining();

                Ok((n, bs))
            })
            .collect();
        for result in results {
            match result {
                Ok((n, buf)) => {
                    bufs.push(buf);
                    if n < self.chunk {
                        self.it.finished();
                        return Some(Ok(bufs.into_iter().flatten().collect()));
                    }
                }
                Err(err) => return Some(Err(err)),
            }
        }

        Some(Ok(bufs.into_iter().flatten().collect()))
    }
}

pub(super) struct RangeIterator {
    offset: u64,
    chunk: u64,
    end: Option<u64>,
    finished: Arc<AtomicBool>,
}

impl RangeIterator {
    fn finished(&mut self) {
        self.finished.store(true, Ordering::Relaxed);
    }
}

impl Iterator for RangeIterator {
    type Item = Range<u64>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished.load(Ordering::Relaxed) {
            return None;
        }

        let offset = self.offset;
        if let Some(end) = self.end {
            if self.offset >= end {
                return None;
            }
            if self.offset + self.chunk > end {
                self.finished();
                return Some(offset..end);
            }
        }
        self.offset += self.chunk;
        Some(offset..offset + self.chunk)
    }
}
