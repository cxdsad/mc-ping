//
// Stolen from mc_varint



//! An implementation for Minecraft[^mc]'s VarInt and VarLong types, focusing on minimum memory
//! usage and maximum performance.
//!
//! [^mc]: A well-known video game whose servers and clients are able to be built by third-party authors.
//!
//! This crate contains two structs for VarInt and VarLong, and four traits for conversations and
//! IO operations on them. You may refer to the paragraphs following to get their usages.
//!
//! Algorithms and structures herein are built according to [a wiki.vg page]
//!
//! [a wiki.vg page]: http://wiki.vg/Protocol#VarInt_and_VarLong
//!
//! # VarInt and VarLong struct
//!
//! These two structs represents the two types mentioned above. Data stored in these two structs
//! are guaranteed a valid VarInt or VarLong by their conversation traits.
//!
//! You may create these structs using function `VarInt::from(i32)` and `VarLong::from(i64)`.
//! And using `i32::from(VarInt)` and `i64::from(VarLong)` can simply convert two structs into
//! actual values in order to use them in following logic.
//!
//! These two structs implements `Default`, which leads to easier use in codes.
//!
//! # Two 'Read' traits and two 'Write' traits
//!
//! They are VarIntRead, VarLongRead for 'Read', and VarIntWrite, VarLongWrite for 'Write'.
//!
//! Both two 'Read' traits are implemented for all `R`'s where `R: io::Read`. You may use it to
//! read `VarInt`'s and `VarLong`'s directly from IO streams, such as, network connections or files.
//!
//! And for the two 'Write' traits, they are implemented for all `W`'s where `W: io::Write` for your
//! convenience.
//!
//! # How this crate reduces memory usage
//!
//! As only VarInt and VarLong struct performs the allocation, firstly we should minimize the space
//! these two structs use in memory. As These two structs only stores the sized integer data
//! instead of something combined with pointers and sizes, the memory usage is reduced to minimal,
//! which means, the VarInt only uses 5 bytes and VarLong only uses 10.
//!
//! When writing to IO, reading from IO or performing type conversations, this crate only allocate
//! one `[u8; 1]` array as buffer, and for the Rust's sake, can free it safely even without a GC.
//! By this way we save more memory in calculating, resulting in more memory able to be used for
//! network buffers, databases and your following logic code.

#![deny(missing_docs)]

use std::io;

macro_rules! var_impl {
    ($store_struct: ident, $read_trait: ident, $write_trait: ident, $read_func: ident, $write_func: ident,
    $conversation_type: ident, $size: expr, $error_too_long: expr) => {

/// The struct representing a VarInt or VarLong.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct $store_struct {
    pub inner: [u8; $size]
}

impl Default for $store_struct {
    fn default() -> Self {
        $store_struct {
            inner: [0u8; $size]
        }
    }
}

pub trait $read_trait {
    /// Reads a VarInt or Varlong from `self`.
    ///
    /// The current position is advanced according to the length of VarInt or VarLong.
    ///
    /// # Errors
    ///
    /// If the VarInt or VarLong to read from `self` is too long (is invalid) or this function
    /// encounters any form of underlying I/O or other error, an error variant will be returned.
    fn $read_func(&mut self) -> io::Result<$store_struct>;
}

impl<R> $read_trait for R where R: io::Read {
    fn $read_func(&mut self) -> Result<$store_struct, io::Error> {
        let mut ans = $store_struct {
            inner: [0u8; $size]
        };
        let mut ptr = 0;
        let mut buf = [0u8];
        loop {
            self.read_exact(&mut buf)?;
            if ptr >= $size {
                return Err(io::Error::new(io::ErrorKind::InvalidData, $error_too_long));
            }
            ans.inner[ptr] = buf[0];
            ptr += 1;
            if buf[0] & 0b1000_0000 == 0 {
                return Ok(ans);
            }
        }
    }
}

/// The Write trait for this VarInt or VarLong struct.
///
/// This trait is implemented for all `io::Write`'s.
///
/// # Examples
///
/// `Cursor`s implement `io::Write`, thus implement `VarIntWrite` and `VarLongWrite`:
///
/// ```
pub trait $write_trait {
    /// Writes a VarInt or Varlong to `self`.
    ///
    /// The current position is advanced according to the length of VarInt or VarLong.
    ///
    /// # Errors
    ///
    /// If this function encounters any form of underlying I/O or other error, an error variant
    /// will be returned.
    fn $write_func(&mut self, n: $store_struct) -> io::Result<()>;
}

impl<W> $write_trait for W where W: io::Write {
    fn $write_func(&mut self, n: $store_struct) -> io::Result<()> {
        let mut buf = [0x00];
        let mut ptr = 0;
        loop {
            if n.inner[ptr] == 0 {
                break;
            }
            buf[0] = n.inner[ptr];
            self.write_all(&buf)?;
            ptr += 1;
            if ptr >= $size {
                break;
            }
        }
        // If no bytes written, that is, the $store_struct is equal to 0
        if ptr == 0 {
            // At that time, `buf` is still [0x00], let's write it
            self.write_all(&buf)?;
        }
        Ok(())
    }
}

impl From<$store_struct> for $conversation_type {
    fn from(v: $store_struct) -> Self {
        let mut ans = 0 as Self;
        let mut ptr = 0;
        loop {
            let value = $conversation_type::from(v.inner[ptr] & 0b0111_1111);
            ans |= value << (7 * ptr as Self);
            if v.inner[ptr] & 0b1000_0000 == 0 {
                return ans;
            }
            ptr += 1;
        }
    }
}

impl From<$conversation_type> for $store_struct {
    fn from(n: $conversation_type) -> Self {
        let mut ans = $store_struct {
            inner: [0u8; $size]
        };
        let mut n = n;
        let mut ptr = 0;
        loop {
            let mut tmp = (n & 0b0111_1111) as u8;
            // There isn't a logical right-shift operator in Rust
            n = (n >> 7) & ($conversation_type::max_value() >> 6);
            if n != 0 {
                tmp |= 0b1000_0000;
            }
            ans.inner[ptr] = tmp;
            ptr += 1;
            if n == 0 || ptr >= $size {
                break;
            }
        }
        ans
    }
}
    };
}

impl VarInt {
    pub fn size(&self) -> usize {
        for i in 0..self.inner.len() {
            if self.inner[i] & 0x80 == 0 {
                return i + 1;
            }
        }
        self.inner.len()
    }
}


var_impl!(VarInt, VarIntRead, VarIntWrite, read_var_int, write_var_int,
            i32, 5, "varint too long (length > 5)");
var_impl!(VarLong, VarLongRead, VarLongWrite, read_var_long, write_var_long,
            i64, 10, "varlong too long (length > 10)");

