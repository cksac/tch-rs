//! Dataset iterators.
use crate::{kind, Device, Kind, Tensor};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Result};

/// An iterator over a pair of tensors which have the same first dimension
/// size.
/// The typical use case is to iterate over batches. Each batch is a pair
/// containing a (potentially random) slice of each of the two input
/// tensors.
#[derive(Debug)]
pub struct Iter2 {
    xs: Tensor,
    ys: Tensor,
    batch_index: i64,
    batch_size: i64,
    total_size: i64,
    device: Device,
    return_smaller_last_batch: bool,
}

impl Iter2 {
    /// Returns a new iterator.
    ///
    /// This takes as input two tensors which first dimension must match. The
    /// returned iterator can be used to range over mini-batches of data of
    /// specified size.
    ///
    /// # Arguments
    ///
    /// * `xs` - the features to be used by the model.
    /// * `ys` - the targets that the model attempts to predict.
    /// * `batch_size` - the size of batches to be returned.
    pub fn new(xs: &Tensor, ys: &Tensor, batch_size: i64) -> Iter2 {
        let total_size = xs.size()[0];
        if ys.size()[0] != total_size {
            panic!("different dimension for the two inputs {:?} {:?}", xs, ys)
        }
        Iter2 {
            xs: xs.shallow_clone(),
            ys: ys.shallow_clone(),
            batch_index: 0,
            batch_size,
            total_size,
            device: Device::Cpu,
            return_smaller_last_batch: false,
        }
    }

    /// Shuffles the dataset.
    ///
    /// The iterator would still run over the whole dataset but the order in
    /// which elements are grouped in mini-batches is randomized.
    pub fn shuffle(&mut self) -> &mut Iter2 {
        let index = Tensor::randperm(self.total_size, kind::INT64_CPU);
        self.xs = self.xs.index_select(0, &index);
        self.ys = self.ys.index_select(0, &index);
        self
    }

    /// Transfers the mini-batches to a specified device.
    pub fn to_device(&mut self, device: Device) -> &mut Iter2 {
        self.device = device;
        self
    }

    /// When set, returns the last batch even if smaller than the batch size.
    pub fn return_smaller_last_batch(&mut self) -> &mut Iter2 {
        self.return_smaller_last_batch = true;
        self
    }
}

impl Iterator for Iter2 {
    type Item = (Tensor, Tensor);

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.batch_index * self.batch_size;
        let size = std::cmp::min(self.batch_size, self.total_size - start);
        if size <= 0 || (!self.return_smaller_last_batch && size < self.batch_size) {
            None
        } else {
            self.batch_index += 1;
            Some((
                self.xs.narrow(0, start, size).to_device(self.device),
                self.ys.narrow(0, start, size).to_device(self.device),
            ))
        }
    }
}

/// Text data holder.
#[derive(Debug)]
pub struct TextData {
    data: Tensor,
    char_for_label: Vec<char>,
}

#[derive(Debug)]
pub struct TextDataIter {
    data: Tensor,
    seq_len: i64,
    batch_index: i64,
    batch_size: i64,
    indexes: Tensor,
    indexes_len: i64,
}

impl TextData {
    /// Creates a text dataset from a file.
    pub fn new<P: AsRef<std::path::Path>>(filename: P) -> Result<TextData> {
        let mut buf_reader = BufReader::new(File::open(filename)?);
        let mut buffer = Vec::new();
        buf_reader.read_to_end(&mut buffer)?;

        let mut label_for_char = HashMap::<u8, u8>::new();
        let mut char_for_label = Vec::<char>::new();
        for c in buffer.iter_mut() {
            *c = *label_for_char.entry(*c).or_insert_with(|| {
                let label = char_for_label.len() as u8;
                char_for_label.push(*c as char);
                label
            })
        }

        Ok(TextData {
            data: Tensor::of_data(&buffer, Kind::Uint8),
            char_for_label,
        })
    }

    /// Returns the number of different characters/labels used by the dataset.
    pub fn labels(&self) -> i64 {
        self.char_for_label.len() as i64
    }

    /// Returns a shallow copy of the data.
    pub fn data(&self) -> Tensor {
        self.data.shallow_clone()
    }

    pub fn label_to_char(&self, label: i64) -> char {
        self.char_for_label[label as usize]
    }

    /// Returns a batch iterator over the dataset.
    /// Each sample is made of seq_len characters.
    pub fn iter_shuffle(&self, seq_len: i64, batch_size: i64) -> TextDataIter {
        let indexes_len = self.data.size()[0] - seq_len + 1;
        TextDataIter {
            data: self.data.shallow_clone(),
            seq_len,
            batch_index: 0,
            batch_size,
            indexes: Tensor::randperm(indexes_len, kind::INT64_CPU),
            indexes_len,
        }
    }
}

impl Iterator for TextDataIter {
    type Item = Tensor;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.batch_index * self.batch_size;
        let size = std::cmp::min(self.batch_size, self.indexes_len - start);
        if size < self.batch_size {
            None
        } else {
            self.batch_index += 1;
            let indexes = Vec::<i64>::from(&self.indexes.narrow(0, start, size));
            let batch: Vec<_> = indexes
                .iter()
                .map(|&i| self.data.narrow(0, i, self.seq_len))
                .collect();
            let batch: Vec<_> = batch.iter().map(|b| b).collect();
            Some(Tensor::stack(&batch, 0))
        }
    }
}
