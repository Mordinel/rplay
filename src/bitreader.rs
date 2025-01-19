use std::io;
use std::mem;

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

pub trait FromBytes: Sized {
    /// The size of the implementor in bytes
    const SIZE: usize;
    fn from_le_bytes(bytes: &[u8]) -> Self;
    fn from_be_bytes(bytes: &[u8]) -> Self;
}

/// takes typenames (which all must have `::from_be_bytes` and `::from_be_bytes`, and be sized)
/// and generates `FromBytes` impls for each one
macro_rules! impl_from_bytes_for_int {
    ($($t:ty),*) => {
        $(
            impl FromBytes for $t {
                const SIZE: usize = mem::size_of::<$t>();
                fn from_le_bytes(bytes: &[u8]) -> Self {
                    <$t>::from_le_bytes(bytes.try_into().unwrap())
                }

                fn from_be_bytes(bytes: &[u8]) -> Self {
                    <$t>::from_be_bytes(bytes.try_into().unwrap())
                }
            }
        )*
    };
}

impl_from_bytes_for_int!(u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, f32, f64);

