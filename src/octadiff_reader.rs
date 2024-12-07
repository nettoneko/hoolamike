use {
    anyhow::{Context, Result},
    binrw::{
        meta::{ReadEndian, WriteEndian},
        prelude::*,
    },
    num::ToPrimitive,
    std::{
        fmt,
        io::{self, Read, Seek, SeekFrom},
        num::NonZeroUsize,
    },
    tap::prelude::*,
};

const BINARY_VERSION: BinaryVersion = [0x01];

type HashAlgorithmName = Keyword<4>;
const DEFAULT_HASH_ALGORITM_NAME: HashAlgorithmName = *b"SHA1";
const DEFAULT_HASH_ALGORITHM_HASH_LEN: usize = 20;

type Keyword<const SIZE: usize> = [u8; SIZE];

type BinaryEndOfMetadata = Keyword<3>;
pub const BINARY_END_OF_METADATA: BinaryEndOfMetadata = *b">>>";

type BinaryVersion = Keyword<1>;

#[macro_export]
macro_rules! zip_results {
    (Error = $ret:ty, $($result:expr),*) => {
        {
            let __extract = move || -> std::result::Result<_, _> {
                std::result::Result::<_, $ret>::Ok(($($result?),*))
            };
            __extract()
        }
    };
    ($($result:expr),*) => {
        {
            let mut __extract = || -> std::result::Result<_, _> {
                Ok(($($result?),*))
            };
            __extract()
        }
    };

}

#[binrw::binrw]
pub struct LengthPrefixedString {
    #[bw(calc = (bytes.len()) as u8)]
    len: u8,
    #[br(little, count = len)]
    bytes: Vec<u8>,
}

#[binrw::binrw]
pub struct ConstantSizedString<const SIZE: usize> {
    bytes: [u8; SIZE],
}

#[binrw::binrw]
#[br(little)]
struct WithEof<T>
where
    for<'a> T: BinRead<Args<'a> = ()> + BinWrite<Args<'a> = ()> + ReadEndian + WriteEndian,
{
    inner: T,
    #[br(assert(eof == BINARY_END_OF_METADATA, "expected b'{}', got b'{}'", dbg_bytes(&BINARY_END_OF_METADATA), dbg_bytes(&eof)))]
    eof: BinaryEndOfMetadata,
}

#[extension_traits::extension(pub trait MaybeBinRead)]
impl<T> T
where
    T: for<'a> BinRead<Args<'a> = ()> + ReadEndian,
{
    fn read_opt<R: Read + Seek>(mut reader: R) -> std::result::Result<Option<T>, binrw::Error> {
        match reached_end_of_stream(&mut reader)? {
            true => Ok(None),
            false => {
                let pos = reader.stream_position()?;
                match T::read(&mut reader) {
                    Err(binrw::Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                        reader.seek(SeekFrom::Start(pos))?;
                        Ok(None)
                    }
                    other => other.map(Some),
                }
            }
        }
    }
}

#[binrw::binrw]
pub struct HashBytes(ConstantSizedString<DEFAULT_HASH_ALGORITHM_HASH_LEN>);

#[binrw::binrw]
#[brw(little, magic = b"OCTODELTA")]
pub struct OctodiffMetadata {
    #[br(assert(version == BINARY_VERSION, "binary version missmatch"))]
    pub version: BinaryVersion,
    #[br(assert(hash_algorithm_name.bytes == DEFAULT_HASH_ALGORITM_NAME, "hash algorithm mismatch"))]
    pub hash_algorithm_name: LengthPrefixedString,
    #[br(assert(hash_length as usize == DEFAULT_HASH_ALGORITHM_HASH_LEN))]
    pub hash_length: i32,
    pub hash: HashBytes,
}

fn dbg_bytes(bytes: &[u8]) -> String {
    bytes.iter().copied().map(char::from).collect()
}

#[binrw::binrw]
#[brw(little)]
pub struct CopyDataCommand {
    start: i64,
    length: i64,
}

#[binrw::binrw]
#[brw(little)]
pub struct WriteDataCommand {
    length: i64,
}

#[binrw::binrw]
#[brw(little)]
pub enum OctodiffCommand {
    #[br(magic(0x60_u8))]
    Copy(CopyDataCommand),
    #[br(magic(0x80_u8))]
    Write(WriteDataCommand),
}

pub struct ApplyDetla<S: Read + Seek, D: Read + Seek> {
    pub metadata: OctodiffMetadata,
    current_command: Option<OctodiffCommandProgress>,
    source: S,
    delta: D,
}

fn must_be_usize(value: i64) -> std::io::Result<usize> {
    value
        .to_usize()
        .context("expected value to be usize")
        .map_err(std::io::Error::other)
}
fn must_be_u64(value: i64) -> std::io::Result<u64> {
    value
        .to_u64()
        .context("expected value to be usize")
        .map_err(std::io::Error::other)
}

fn must_be_non_zero_usize(value: i64) -> std::io::Result<NonZeroUsize> {
    must_be_usize(value)
        .context("not even a usize")
        .and_then(|value| NonZeroUsize::new(value).context("must be non-zero"))
        .map_err(std::io::Error::other)
}

fn read_at_most<R: Seek + Read>(mut source: R, buf: &mut [u8], remaining_length: usize) -> std::io::Result<Option<NonZeroUsize>> {
    remaining_length
        .min(buf.len())
        .pipe(|buffer_size| (&mut buf[..buffer_size]).pipe(|buf| source.read_exact(buf).map(|_| buffer_size)))
        .map(NonZeroUsize::new)
}

fn reached_end_of_stream<T: Seek + Read>(source: &mut T) -> std::io::Result<bool> {
    let (position, len) = (source.stream_position()?, source.stream_len()?);
    Ok(position == len)
}

fn read_next_command<T: Read + Seek>(mut source: T) -> Result<Option<OctodiffCommand>> {
    OctodiffCommand::read_opt(&mut source).context("reading next octodiff command")
}

pub enum CommandSummary {
    Copy { start: usize, length: usize },
    Write(Vec<u8>),
}

impl std::fmt::Display for CommandSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandSummary::Copy { start, length } => write!(f, "copy start={start}, length={length}"),
            CommandSummary::Write(bytes) => write!(f, "write {}", hex::encode(bytes)),
        }
    }
}

impl OctodiffMetadata {
    pub fn explain<T: Read + Seek>(mut reader: T) -> Result<(Self, Vec<CommandSummary>)> {
        let metadata = WithEof::<Self>::read(&mut reader)
            .context("reading metadata")
            .map(|WithEof { inner, eof: _ }| inner)?;
        let mut command_summary = vec![];
        while let Some(chunk) = read_next_command(&mut reader).with_context(|| {
            //
            let position = reader.stream_position().unwrap();
            let total_len = reader.stream_len().unwrap();
            let remaining_len = total_len - position;
            format!("nothing matched at cursor\nposition [{position}]\nremaining_len: {remaining_len }\ntotal_len: {total_len}")
        })? {
            match chunk {
                OctodiffCommand::Copy(CopyDataCommand { start, length }) => command_summary.push(CommandSummary::Copy {
                    start: start as _,
                    length: length as _,
                }),
                OctodiffCommand::Write(WriteDataCommand { length }) => {
                    let mut buf = vec![0; length as usize];
                    reader
                        .read_exact(&mut buf)
                        .with_context(|| "reading [{length}] bytes for write summary")?;
                    command_summary.push(CommandSummary::Write(buf))
                }
            }
        }
        Ok((metadata, command_summary))
    }
}

impl<S, D> ApplyDetla<S, D>
where
    S: Read + Seek,
    D: Read + Seek,
{
    pub fn new(source: S, mut delta: D) -> Result<Option<Self>> {
        OctodiffMetadata::read_le(&mut delta)
            .context("reading metadata of delta file")
            .map(|metadata| Self {
                metadata,
                source,
                delta,
                current_command: None,
            })
            .and_then(|mut delta| {
                delta
                    .read_next_command_progress()
                    .map(|current_command| {
                        current_command.map(|current_command| {
                            delta.tap_mut(|delta| {
                                delta.current_command = Some(current_command);
                            })
                        })
                    })
                    .context("preparing reader for first command")
            })
    }

    fn read_next_command(&mut self) -> std::io::Result<Option<OctodiffCommand>> {
        read_next_command(&mut self.delta).map_err(std::io::Error::other)
    }

    fn read_next_command_progress(&mut self) -> std::io::Result<Option<OctodiffCommandProgress>> {
        self.read_next_command().and_then(|next_command| {
            next_command
                .map(|next_command| match next_command {
                    OctodiffCommand::Copy(CopyDataCommand { start, length }) => {
                        let (start, length) = zip_results![Error = std::io::Error, must_be_u64(start), must_be_non_zero_usize(length)]?;
                        self.source
                            .seek(SeekFrom::Start(start))
                            .map(|_| OctodiffCommandProgress::Copy { remaining_length: length })
                    }
                    OctodiffCommand::Write(WriteDataCommand { length }) => {
                        must_be_non_zero_usize(length).map(|length| OctodiffCommandProgress::Write { remaining_length: length })
                    }
                })
                .transpose()
        })
    }

    fn handle_progress(
        &mut self,
        buf: &mut [u8],
        progress: OctodiffCommandProgress,
    ) -> std::io::Result<Option<(NonZeroUsize, Option<OctodiffCommandProgress>)>> {
        fn read_with_progress<T: Read + Seek>(
            mut source: T,
            buf: &mut [u8],
            remaining_length: NonZeroUsize,
        ) -> std::io::Result<Option<(NonZeroUsize, Option<NonZeroUsize>)>> {
            read_at_most(&mut source, buf, remaining_length.get()).and_then(|read| {
                read.and_then(|read| remaining_length.get().checked_sub(read.get()))
                    .context("read too much")
                    .map_err(std::io::Error::other)
                    .map(|new_remaining_length| {
                        new_remaining_length
                            .pipe(NonZeroUsize::new)
                            .pipe(|remaining_length| (read.map(|read| (read, remaining_length))))
                    })
            })
        }
        match progress {
            OctodiffCommandProgress::Copy { remaining_length } => read_with_progress(&mut self.source, buf, remaining_length).map(|progress| {
                progress.map(|(progress, remaining)| (progress, remaining.map(|remaining_length| OctodiffCommandProgress::Copy { remaining_length })))
            }),
            OctodiffCommandProgress::Write { remaining_length } => read_with_progress(&mut self.delta, buf, remaining_length).map(|progress| {
                progress.map(|(progress, remaining)| (progress, remaining.map(|remaining_length| OctodiffCommandProgress::Write { remaining_length })))
            }),
        }
    }
    pub fn continue_read(&mut self, buf: &mut [u8]) -> std::io::Result<Option<NonZeroUsize>> {
        match self
            .current_command
            .take()
            .map(Ok)
            .or_else(|| self.read_next_command_progress().transpose())
        {
            Some(progress) => progress
                .and_then(|progress| self.handle_progress(buf, progress))
                .map(|progress| match progress {
                    Some((progress, next_command)) => {
                        self.current_command = next_command;
                        Some(progress)
                    }
                    None => None,
                }),
            None => Ok(None),
        }
    }
}

enum OctodiffCommandProgress {
    Copy { remaining_length: NonZeroUsize },
    Write { remaining_length: NonZeroUsize },
}

#[extension_traits::extension(pub trait NonZeroUsizeExt)]
impl NonZeroUsize
where
    Self: Sized,
{
    fn checked_sub(&self, other: usize) -> Option<NonZeroUsize> {
        self.get().checked_sub(other).and_then(Self::new)
    }
}

impl<S, D> Read for ApplyDetla<S, D>
where
    S: Read + Seek,
    D: Read + Seek,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.continue_read(buf)
            .map(|read| read.map(NonZeroUsize::get).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests;
