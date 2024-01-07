use std::cmp;
use std::io::BufRead;
use std::io::Read;
use std::io::Result;

pub(crate) trait Filter: Sized {
    fn init() -> Self;
    fn filter(&mut self, input: &[u8], output: &mut Vec<u8>);
}

pub(crate) struct BufFilter<F: Filter, R: BufRead> {
    inner: R,
    buffer: Vec<u8>,
    pos: usize,
    filter: F,
}

impl<F: Filter, R: BufRead> BufFilter<F, R> {
    pub(crate) fn new(inner: R) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
            pos: 0,
            filter: F::init(),
        }
    }
}

impl<F: Filter, R: BufRead> Read for BufFilter<F, R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut nread = 0;
        let mut len = self.buffer.len() - self.pos;
        if len != 0 {
            nread = cmp::min(len, buf.len());
            buf[..nread].copy_from_slice(&self.buffer[self.pos..self.pos + nread]);
            self.consume(nread);
        }

        if nread != buf.len() {
            self.fill_buf()?;
            len = cmp::min(self.buffer.len(), buf.len() - nread);
            buf[nread..nread + len].copy_from_slice(&self.buffer[self.pos..self.pos + len]);
            self.consume(len);
            nread += len;
        }

        Ok(nread)
    }
}

impl<F: Filter, R: BufRead> BufRead for BufFilter<F, R> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        if self.pos >= self.buffer.len() {
            self.buffer.clear();
            let buffer = self.inner.fill_buf()?;
            self.filter.filter(buffer, &mut self.buffer);
            let buffer_len = buffer.len();
            self.inner.consume(buffer_len);
            self.pos = 0;
        }

        Ok(&self.buffer[self.pos..])
    }

    fn consume(&mut self, amt: usize) {
        self.pos = cmp::min(self.pos + amt, self.buffer.len());
    }
}
