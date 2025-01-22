use std::io;
use std::mem;

pub trait SizedNumber: Sized {
    const SIZE: usize;
    type Bytes: AsRef<[u8]> + Default;
}

pub trait FromBytes: SizedNumber {
    fn from_le_bytes(bytes: &[u8]) -> Self;
    fn from_be_bytes(bytes: &[u8]) -> Self;
}

pub trait ToBytes: SizedNumber {
    fn to_le_bytes(self) -> Self::Bytes;
    fn to_be_bytes(self) -> Self::Bytes;
}

macro_rules! impl_bitio_traits_for {
    ($($t:ty),*) => {
        $(
            impl SizedNumber for $t {
                const SIZE: usize = mem::size_of::<$t>();
                type Bytes = [u8; mem::size_of::<$t>()];
            }

            impl FromBytes for $t {
                fn from_le_bytes(bytes: &[u8]) -> $t {
                    <$t>::from_le_bytes(bytes.try_into().unwrap())
                }

                fn from_be_bytes(bytes: &[u8]) -> $t {
                    <$t>::from_be_bytes(bytes.try_into().unwrap())
                }
            }

            impl ToBytes for $t {
                fn to_le_bytes(self) -> Self::Bytes {
                    <$t>::to_le_bytes(self)
                }

                fn to_be_bytes(self) -> Self::Bytes {
                    <$t>::to_be_bytes(self)
                }
            }
        )*
    }
}
impl_bitio_traits_for!(u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, f32, f64);

/// reads only the exact amount of bytes required to serialize primitive nums 
pub struct BitReader<R> {
    inner: R,
    /// is big endian
    be: bool,
}

impl<R: io::Read> BitReader<R> {
    pub fn new(inner: R, big_endian: bool) -> Self {
        BitReader { inner, be: big_endian }
    }

    /// switches on `T::SIZE`, which is const-generated for every impl of `FromBytes`
    pub fn read<T: FromBytes>(&mut self) -> io::Result<T> {
        match T::SIZE {
            1 => self.read_helper::<1>().map(
                |b| if self.be { T::from_be_bytes(&b) } else { T::from_le_bytes(&b) }
            ),
            2 => self.read_helper::<2>().map(
                |b| if self.be { T::from_be_bytes(&b) } else { T::from_le_bytes(&b) }
            ),
            4 => self.read_helper::<4>().map(
                |b| if self.be { T::from_be_bytes(&b) } else { T::from_le_bytes(&b) }
            ),
            8 => self.read_helper::<8>().map(
                |b| if self.be { T::from_be_bytes(&b) } else { T::from_le_bytes(&b) }
            ),
            16 => self.read_helper::<16>().map(
                |b| if self.be { T::from_be_bytes(&b) } else { T::from_le_bytes(&b) }
            ),
            _ => panic!("Unsupported size for type T: `{}`", T::SIZE),
        }
    }

    /// turns into monomorphs for each invokation site of unique `const N` 
    /// purpose is to allocate a buffer on the stack and read N bytes from the internal reader
    fn read_helper<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut buf = [0u8; N];
        self.inner.read_exact(&mut buf)?;
        Ok(buf)
    }
}

/// writes the bytes for any impl of [ToBytes] to the enclosed writer.
pub struct BitWriter<W> {
    inner: W,
    be: bool,
}

impl<W: io::Write> BitWriter<W> {
    pub fn new(inner: W, big_endian: bool) -> Self {
        BitWriter { inner, be: big_endian }
    }

    pub fn write<T: ToBytes>(&mut self, t: T) -> io::Result<()> {
        let bytes = if self.be {
            t.to_be_bytes()
        } else {
            t.to_le_bytes()
        };
        self.inner.write_all(bytes.as_ref())
    }
}

