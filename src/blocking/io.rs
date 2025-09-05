use core::error::Error;


/// [BlockingWrite] is the library's abstraction for blocking write I/O.
///
/// It is similar to `std::io::Write`, and there is a blanket implementation of [BlockingWrite] for
///  any implementation of `Write`. The reason for introducing [BlockingWrite] is that it allows
///  json-streaming to be used in a `no-std` environment.
///
/// Note that json-streaming writes data to a [BlockingWrite] in many small chunks without any I/O
///  buffering. It is the client's responsibility to use `std::io::BufWriter` or similar for
///  improved performance where desired.
pub trait BlockingWrite {
    type Error: Error;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error>;
}

#[cfg(feature = "std")]
/// Blanket implementation that allows any [std::io::Write] implementation to be used seamlessly as
///  [BlockingWrite].
impl <W: std::io::Write> BlockingWrite for W {
    type Error = std::io::Error;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        W::write_all(self, buf)
    }
}

/// [BlockingRead] is the library's abstraction for blocking read I/O.
///
/// It is similar to `std:io::Read`, and there is a blanket implementation of [BlockingRead] for
///  any implementation of `Read`. The reason for introducing [BlockingRead] is that it allows
///  json-streaming in a `no-std` environment.
///
/// Note that json-streaming reads data from a [BlockingRead] in many small chunks without any I/O
///  buffering. It is the client's responsibility to use `std::io::BufRead` or similar for
///  improved performance where desired.
pub trait BlockingRead {
    type Error: Error;

    fn read(&mut self) -> Result<Option<u8>, Self::Error>;
}

#[cfg(feature = "std")]
/// Blanket implementation that allows any [std::io::Read] implementation to be used seamlessly as
///  [BlockingRead].
impl <R: std::io::Read> BlockingRead for R {
    type Error = std::io::Error;

    fn read(&mut self) -> Result<Option<u8>, Self::Error> {
        let mut result = [0u8; 1];
        let num_read = R::read(self, &mut result)?;
        if num_read == 1 {
            Ok(Some(result[0]))
        }
        else {
            Ok(None)
        }
    }
}