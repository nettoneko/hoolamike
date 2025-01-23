use {multichannel_chunk_reader::MultichannelChunkBuffer, rubato::FftFixedIn};

// type ResamplerInner = FftFixedIn<f32>;

pub mod ringbuf_types;

pub mod multichannel_chunk_reader;

pub mod multichannel_converter;

pub struct StreamingResampler {
    pub input: MultichannelChunkBuffer<f32>,
    pub resampler: FftFixedIn<f32>,
    pub output: MultichannelChunkBuffer<f32>,
}

pub type RetBuffers<T> = Vec<Vec<T>>;

// impl StreamingResampler {
//     fn input_buf_len(&self) -> usize {
//         self.storage_buffers.first().reader.occupied_len()
//     }

//     fn load_chunk(&mut self) -> Option<&NonEmpty<Vec<f32>>> {
//         match self.input_buf_len() > self.chunk_size {
//             true => {
//                 // self.ret_buffers.iter_mut().for_each(|b| b.clear());
//                 self.storage_buffers
//                     .iter_mut()
//                     .zip(self.ret_buffers.iter_mut())
//                     .for_each(|(channel, ret)| {
//                         channel
//                             .reader
//                             .pop_iter()
//                             .zip(ret.iter_mut())
//                             .for_each(|(from, to)| *to = from);
//                     })
//                     .pipe(|_| &self.ret_buffers)
//                     .pipe(Some)
//             }
//             false => None,
//         }
//     }

//     fn process_chunk(&mut self) -> Option<&self::RetBuffers<f32>> {
//         self.load_chunk()
//     }

//     fn load<Inner>(&mut self, chunk: &[Inner])
//     where
//         Inner: AsRef<[f32]>,
//     {
//         chunk
//             .iter()
//             .map(|c| c.as_ref())
//             .zip(self.storage_buffers.iter_mut())
//             .for_each(|(input, buf)| {
//                 buf.extend(input.iter().copied());
//             });
//     }

//     pub fn new(from: u32, to: u32, chunk_size: usize, channels: usize) -> Result<Self> {
//         ResamplerInner::new(from as _, to as _, chunk_size, 2, channels)
//             .context("Creating sinc interpolation resampler")
//             .map(|resampler| Self {
//                 resampler,
//                 input: MultichannelChunkBuffer::new(chunk_size, channels),
//                 output: todo!(),
//             })
//     }
// }
