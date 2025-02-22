//! `ggml` is a semi-idiomatic wrapper for the `ggml` C library.
//!
//! It exposes a subset of operations (currently used to implement the [llm](https://crates.io/crates/llm) library).
//! Note that it does not expose a fully-idiomatic safe Rust interface; operations that could be potentially unsafe are marked as such.
//!
//! `ggml` operates on a computational graph; no values will be computed until [Context::graph_compute] is executed.
//! All [Tensor]s are nodes in this computational graph, and values cannot be retrieved until computation is completed.
#![deny(missing_docs)]

use std::os::raw::{c_int, c_void};

mod context;
mod tensor;

pub mod format;
pub mod legacy;
pub mod util;

pub use context::Context;
pub use tensor::Tensor;

pub(crate) use ggml_sys as sys;

#[cfg(test)]
mod tests;

/// The type of a tensor element.
pub type ElementType = Type;

#[derive(Debug, PartialEq, Clone, Copy)]
/// The format of the file containing the model.
pub enum ContainerType {
    /// Legacy format, oldest ggml tensor file format
    Ggml,
    /// Legacy format. Introduces versioning. Newer than GGML, older than GGJT.
    Ggmf(u32),
    /// [mmap](https://en.wikipedia.org/wiki/Mmap)-able format. Current version of the format.
    Ggjt(u32),
    /// LoRA adapter format.
    Ggla(u32),
}
impl ContainerType {
    /// Does this container type support mmap?
    pub fn support_mmap(&self) -> bool {
        match self {
            ContainerType::Ggml => false,
            ContainerType::Ggmf(_) => false,
            ContainerType::Ggla(_) => false,
            ContainerType::Ggjt(_) => true,
        }
    }

    /// Read the container type from a reader.
    pub fn read<E: std::error::Error>(
        reader: &mut dyn std::io::BufRead,
    ) -> Result<Self, crate::format::LoadError<E>> {
        // Verify magic
        let magic = util::read_u32(reader)?;
        let container_type: ContainerType = match magic {
            crate::FILE_MAGIC_GGML => ContainerType::Ggml,
            crate::FILE_MAGIC_GGMF => {
                let version = util::read_u32(reader)?;
                ContainerType::Ggmf(version)
            }
            crate::FILE_MAGIC_GGJT => {
                let version = util::read_u32(reader)?;
                ContainerType::Ggjt(version)
            }
            crate::FILE_MAGIC_GGLA => {
                let version = util::read_u32(reader)?;
                ContainerType::Ggla(version)
            }
            magic => return Err(crate::format::LoadError::InvalidMagic(magic)),
        };

        Ok(container_type)
    }

    /// Write the container type to a writer.
    pub fn write(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            ContainerType::Ggml => {
                util::write_u32(writer, FILE_MAGIC_GGML)?;
            }
            ContainerType::Ggmf(version) => {
                util::write_u32(writer, FILE_MAGIC_GGMF)?;
                util::write_u32(writer, *version)?;
            }
            ContainerType::Ggjt(version) => {
                util::write_u32(writer, FILE_MAGIC_GGJT)?;
                util::write_u32(writer, *version)?;
            }
            ContainerType::Ggla(version) => {
                util::write_u32(writer, FILE_MAGIC_GGLA)?;
                util::write_u32(writer, *version)?;
            }
        }
        Ok(())
    }
}

/// Magic constant for `ggml` files (unversioned).
pub const FILE_MAGIC_GGML: u32 = 0x67676d6c;
/// Magic constant for `ggml` files (versioned, ggmf).
pub const FILE_MAGIC_GGMF: u32 = 0x67676d66;
/// Magic constant for `ggml` files (versioned, ggjt).
pub const FILE_MAGIC_GGJT: u32 = 0x67676a74;
/// Magic constant for `ggla` files (LoRA adapter).
pub const FILE_MAGIC_GGLA: u32 = 0x67676C61;

/// The current quantization version.
pub const QNT_VERSION: u32 = sys::GGML_QNT_VERSION;
/// The factor by which to divide `ftype` to determine the current quantization version.
pub const QNT_VERSION_FACTOR: u32 = sys::GGML_QNT_VERSION_FACTOR;

/// The size of a `ggml` object.
pub const OBJECT_SIZE: usize = sys::GGML_OBJECT_SIZE;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
/// The type of a value in `ggml`.
pub enum Type {
    /// Quantized 4-bit (type 0).
    #[default]
    Q4_0,
    /// Quantized 4-bit (type 1).
    Q4_1,
    /// Quantized 5-bit (type 0).
    Q5_0,
    /// Quantized 5-bit (type 1).
    Q5_1,
    /// Quantized 8-bit (type 0).
    Q8_0,
    /// Quantized 8-bit (type 1).
    Q8_1,
    /// Integer 32-bit.
    I32,
    /// Float 16-bit.
    F16,
    /// Float 32-bit.
    F32,

    /// Legacy: Quantized 4-bit (type 2).
    /// This is not supported by modern `ggml` and is only here for use with [legacy].
    LegacyQ4_2,
}
impl From<Type> for sys::ggml_type {
    fn from(t: Type) -> Self {
        match t {
            Type::Q4_0 => sys::ggml_type_GGML_TYPE_Q4_0,
            Type::Q4_1 => sys::ggml_type_GGML_TYPE_Q4_1,
            Type::Q5_0 => sys::ggml_type_GGML_TYPE_Q5_0,
            Type::Q5_1 => sys::ggml_type_GGML_TYPE_Q5_1,
            Type::Q8_0 => sys::ggml_type_GGML_TYPE_Q8_0,
            Type::Q8_1 => sys::ggml_type_GGML_TYPE_Q8_1,
            Type::I32 => sys::ggml_type_GGML_TYPE_I32,
            Type::F16 => sys::ggml_type_GGML_TYPE_F16,
            Type::F32 => sys::ggml_type_GGML_TYPE_F32,
            // Legacy
            Type::LegacyQ4_2 => 4,
        }
    }
}
impl TryFrom<sys::ggml_type> for Type {
    type Error = ();
    fn try_from(t: sys::ggml_type) -> Result<Self, Self::Error> {
        match t {
            sys::ggml_type_GGML_TYPE_Q4_0 => Ok(Type::Q4_0),
            sys::ggml_type_GGML_TYPE_Q4_1 => Ok(Type::Q4_1),
            sys::ggml_type_GGML_TYPE_Q5_0 => Ok(Type::Q5_0),
            sys::ggml_type_GGML_TYPE_Q5_1 => Ok(Type::Q5_1),
            sys::ggml_type_GGML_TYPE_Q8_0 => Ok(Type::Q8_0),
            sys::ggml_type_GGML_TYPE_Q8_1 => Ok(Type::Q8_1),
            sys::ggml_type_GGML_TYPE_I32 => Ok(Type::I32),
            sys::ggml_type_GGML_TYPE_F16 => Ok(Type::F16),
            sys::ggml_type_GGML_TYPE_F32 => Ok(Type::F32),
            // Legacy
            4 => Ok(Type::LegacyQ4_2),

            _ => Err(()),
        }
    }
}
impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Q4_0 => write!(f, "q4_0"),
            Type::Q4_1 => write!(f, "q4_1"),
            Type::Q5_0 => write!(f, "q5_0"),
            Type::Q5_1 => write!(f, "q5_1"),
            Type::Q8_0 => write!(f, "q8_0"),
            Type::Q8_1 => write!(f, "q8_1"),
            Type::I32 => write!(f, "i32"),
            Type::F16 => write!(f, "f16"),
            Type::F32 => write!(f, "f32"),
            // Legacy
            Type::LegacyQ4_2 => write!(f, "q4_2"),
        }
    }
}
impl Type {
    /// Returns whether this type is quantized.
    pub fn is_quantized(&self) -> bool {
        match self {
            Type::Q4_0 => true,
            Type::Q4_1 => true,
            Type::Q5_0 => true,
            Type::Q5_1 => true,
            Type::Q8_0 => true,
            Type::Q8_1 => true,
            Type::I32 => false,
            Type::F16 => false,
            Type::F32 => false,
            Type::LegacyQ4_2 => true,
        }
    }
}

/// A buffer of memory that can be used as a scratch buffer for a [Context].
///
/// See [Context::use_scratch].
pub struct Buffer {
    data: Box<[u8]>,
}

impl Buffer {
    /// Creates a new buffer of the specified size.
    pub fn new(size: usize) -> Self {
        let mut data: Vec<u8> = Vec::with_capacity(size);

        // SAFETY: The contents are intentionally uninitialized, as they will be passed to
        // the ggml C API which will fill them with data.
        #[allow(clippy::uninit_vec)]
        unsafe {
            data.set_len(size);
        }

        Buffer {
            data: data.into_boxed_slice(),
        }
    }
}

/// A `ggml` computation graph. Keeps track of all state during computation.
pub struct ComputationGraph {
    inner: sys::ggml_cgraph,
}

impl ComputationGraph {
    /// Create a new [ComputationGraph] with the specified `n_threads`.
    pub fn new(n_threads: usize) -> Self {
        Self {
            inner: sys::ggml_cgraph {
                n_threads: usize_to_i32(n_threads),
                // SAFETY: This should be safe to zero. The original C++ impl
                // just leaves it uninitialized
                ..unsafe { std::mem::zeroed::<sys::ggml_cgraph>() }
            },
        }
    }

    /// Build this computational graph in the forward direction in preparation for computation.
    pub fn build_forward_expand(&mut self, tensor: &Tensor) {
        unsafe { sys::ggml_build_forward_expand(&mut self.inner, tensor.ptr.as_ptr()) }
    }
}

/// The size of `t` as bytes.
pub fn type_size(t: Type) -> usize {
    unsafe { sys::ggml_type_size(t.into()) }
}

/// [type_size]/[blck_size] as float.
pub fn type_sizef(x: Type) -> f64 {
    (unsafe { sys::ggml_type_sizef(x.into()) }) as f64
}

/// The size of a block for `t`. Only relevant for quantized types.
pub fn blck_size(t: Type) -> usize {
    i32_to_usize(unsafe { sys::ggml_blck_size(t.into()) })
}

fn usize_to_i32(val: usize) -> i32 {
    i32::try_from(val).unwrap()
}

fn usize_to_i64(val: usize) -> i64 {
    i64::try_from(val).unwrap()
}

fn i32_to_usize(val: i32) -> usize {
    usize::try_from(val).unwrap()
}

fn i64_to_usize(val: i64) -> usize {
    usize::try_from(val).unwrap()
}

/// Contains the result of a quantization operation.
pub struct QuantizationResult {
    /// The quantized output.
    pub output: Vec<u8>,
    /// The quantization history.
    pub history: Vec<i64>,
}

/// Quantizes `src` into `dst` using `q4_0` quantization.
///
/// You must ensure that `src.len() == n_elements`, and `n_elements_0`
/// is the first dimension of `src`.
pub fn quantize_q4_0(src: &[f32], n_elements: usize, n_elements_0: usize) -> QuantizationResult {
    quantize_impl(src, n_elements, n_elements_0, sys::ggml_quantize_q4_0)
}

/// Quantizes `src` into `dst` using `q4_1` quantization.
///
/// You must ensure that `src.len() == n_elements`, and `n_elements_0`
/// is the first dimension of `src`.
pub fn quantize_q4_1(src: &[f32], n_elements: usize, n_elements_0: usize) -> QuantizationResult {
    quantize_impl(src, n_elements, n_elements_0, sys::ggml_quantize_q4_1)
}

fn quantize_impl(
    src: &[f32],
    n_elements: usize,
    n_elements_0: usize,
    quantizer: unsafe extern "C" fn(*const f32, *mut c_void, c_int, c_int, *mut i64) -> usize,
) -> QuantizationResult {
    assert_eq!(src.len(), n_elements);
    assert_eq!(n_elements % n_elements_0, 0);

    // A conservative multiplier of 4 is used here.
    let mut output = vec![0u8; n_elements * 4];
    let mut history = vec![0i64; 16];
    let output_size = unsafe {
        quantizer(
            src.as_ptr(),
            output.as_mut_ptr() as *mut c_void,
            n_elements.try_into().unwrap(),
            n_elements_0.try_into().unwrap(),
            history.as_mut_ptr(),
        )
    };

    output.resize(output_size, 0u8);
    QuantizationResult { output, history }
}
