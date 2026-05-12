use crate::layer::rtex_from_image;
use raylib::prelude::*;
use std::io;
use thiserror::Error;

pub trait Serialize<Ctx: ?Sized = ()> {
    type Error;

    fn serialize<W>(&self, dst: &mut W, ctx: &Ctx) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write;
}

pub trait Deserialize<Ctx: ?Sized = ()> {
    type Error;

    fn deserialize<R>(src: &mut R, ctx: &mut Ctx) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + io::Read;
}

pub trait SerializeArr<Ctx: ?Sized = ()> {
    type Item;
    type Error;

    fn serialize_arr<W>(&self, dst: &mut W, ctx: &Ctx) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write;
}

pub trait DeserializeArr<Ctx: ?Sized = ()> {
    type Item;
    type Error;

    fn deserialize_arr<R>(src: &mut R, ctx: &mut Ctx) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + io::Read;
}

pub trait SerializeSlice<Ctx: ?Sized = ()> {
    type Item;
    type Error;

    fn serialize_slice<W>(&self, dst: &mut W, ctx: &Ctx) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write;
}

pub trait DeserializeSlice<T, Ctx: ?Sized = ()> {
    type Error;

    fn deserialize_slice<R>(src: &mut R, ctx: &mut Ctx) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + io::Read;
}

impl<T, Ctx: ?Sized, const N: usize> SerializeArr<Ctx> for [T; N]
where
    T: Serialize<Ctx>,
{
    type Item = T;
    type Error = T::Error;

    fn serialize_arr<W>(&self, dst: &mut W, ctx: &Ctx) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        self.iter().try_for_each(|item| item.serialize(dst, ctx))
    }
}

impl<T, Ctx: ?Sized, const N: usize> DeserializeArr<Ctx> for [T; N]
where
    T: Deserialize<Ctx>,
{
    type Item = T;
    type Error = T::Error;

    fn deserialize_arr<R>(src: &mut R, ctx: &mut Ctx) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + io::Read,
    {
        std::array::try_from_fn(|_| T::deserialize(src, ctx))
    }
}

impl<T, Ctx> SerializeSlice<Ctx> for [T]
where
    T: Serialize<Ctx>,
    SerUsizeError: Into<T::Error>,
{
    type Item = T;
    type Error = T::Error;

    fn serialize_slice<W>(&self, dst: &mut W, ctx: &Ctx) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        self.len().serialize(dst, &()).map_err(Into::into)?;
        self.iter().try_for_each(|item| item.serialize(dst, ctx))
    }
}

impl<T, Ctx, FromT> DeserializeSlice<T, Ctx> for FromT
where
    T: Deserialize<Ctx>,
    DeUsizeError: Into<T::Error>,
    FromT: FromIterator<T>,
{
    type Error = T::Error;

    fn deserialize_slice<R>(src: &mut R, ctx: &mut Ctx) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + io::Read,
    {
        let len = usize::deserialize(src, &mut ()).map_err(Into::into)?;
        (0..len).map(|_| T::deserialize(src, ctx)).collect()
    }
}

impl Serialize for u8 {
    type Error = io::Error;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        dst.write_all(std::slice::from_ref(self))
    }
}

impl Deserialize for u8 {
    type Error = io::Error;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        R: ?Sized + io::Read,
    {
        let mut byte = 0;
        src.read_exact(std::slice::from_mut(&mut byte))?;
        Ok(byte)
    }
}

impl<const N: usize> Serialize for [u8; N] {
    type Error = io::Error;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        dst.write_all(self)
    }
}

impl<const N: usize> Deserialize for [u8; N] {
    type Error = io::Error;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        R: ?Sized + io::Read,
    {
        let mut bytes = [0; N];
        src.read_exact(bytes.as_mut_slice())?;
        Ok(bytes)
    }
}

impl Serialize for [u8] {
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        self.len().serialize(dst, &())?;
        dst.write_all(self).map_err(SerUsizeError::Write)
    }
}

impl Deserialize for Vec<u8> {
    type Error = DeUsizeError;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        R: ?Sized + io::Read,
    {
        let len = usize::deserialize(src, &mut ())?;
        let mut buf = vec![0; len];
        src.read_exact(buf.as_mut_slice())?;
        Ok(buf)
    }
}

macro_rules! serde_int {
    ($($Type:ident),*) => {$(
        impl Serialize for $Type {
            type Error = std::io::Error;

            fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
            where
                W: ?Sized + std::io::Write
            {
                dst.write_all(self.to_le_bytes().as_slice())
            }
        }

        impl Deserialize for $Type {
            type Error = std::io::Error;

            fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
            where
                R: ?Sized + std::io::Read
            {
                <[u8; _]>::deserialize(src, &mut ()).map(Self::from_le_bytes)
            }
        }
    )*};
}

serde_int!(u16, u32, u64, u128, i8, i16, i32, i64, i128);

#[derive(Debug, Error)]
pub enum DeNonZeroError {
    #[error("nonzero cannot be zero")]
    Zero,

    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    Read(#[from] io::Error),
}

impl From<DeUsizeError> for DeNonZeroError {
    fn from(e: DeUsizeError) -> Self {
        match e {
            DeUsizeError::Conversion(e) => Self::Conversion(e),
            DeUsizeError::Read(e) => Self::Read(e),
        }
    }
}

macro_rules! serde_nonzero {
    ($($Type:ty),*) => {$(
        impl Serialize for $Type {
            type Error = SerUsizeError;

            fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
            where
                W: ?Sized + std::io::Write
            {
                self.get().serialize(dst, &())?;
                Ok(())
            }
        }

        impl Deserialize for $Type {
            type Error = DeNonZeroError;

            fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
            where
                R: ?Sized + std::io::Read
            {
                Self::new(Deserialize::deserialize(src, &mut ())?)
                    .ok_or(DeNonZeroError::Zero)

            }
        }
    )*};
}

serde_nonzero!(
    std::num::NonZeroU8,
    std::num::NonZeroU16,
    std::num::NonZeroU32,
    std::num::NonZeroU64,
    std::num::NonZeroU128,
    std::num::NonZeroUsize,
    std::num::NonZeroI8,
    std::num::NonZeroI16,
    std::num::NonZeroI32,
    std::num::NonZeroI64,
    std::num::NonZeroI128,
    std::num::NonZeroIsize
);

#[derive(Debug, Error)]
pub enum SerUsizeError {
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    Write(#[from] io::Error),
}

impl Serialize for usize {
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        u64::try_from(*self)?
            .serialize(dst, &())
            .map_err(Into::into)
    }
}

#[derive(Debug, Error)]
pub enum DeUsizeError {
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    Read(#[from] io::Error),
}

impl Deserialize for usize {
    type Error = DeUsizeError;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        R: ?Sized + io::Read,
    {
        u64::deserialize(src, &mut ())?
            .try_into()
            .map_err(Into::into)
    }
}

impl Serialize for isize {
    type Error = io::Error;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        i64::try_from(*self)
            .map_err(io::Error::other)
            .and_then(|x| x.serialize(dst, &()))
    }
}

impl Deserialize for isize {
    type Error = io::Error;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        R: ?Sized + io::Read,
    {
        i64::deserialize(src, &mut ()).and_then(|x| Self::try_from(x).map_err(io::Error::other))
    }
}

impl Serialize for str {
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        self.as_bytes().serialize(dst, &())
    }
}

#[derive(Debug, Error)]
pub enum DeStringError {
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    Read(#[from] io::Error),

    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
}

impl From<DeUsizeError> for DeStringError {
    fn from(e: DeUsizeError) -> Self {
        match e {
            DeUsizeError::Conversion(e) => Self::Conversion(e),
            DeUsizeError::Read(e) => Self::Read(e),
        }
    }
}

impl Deserialize for String {
    type Error = DeStringError;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        R: ?Sized + io::Read,
    {
        let bytes = Vec::deserialize(src, &mut ())?;
        Self::from_utf8(bytes).map_err(DeStringError::Utf8)
    }
}

impl Serialize for std::ffi::OsStr {
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        self.as_encoded_bytes().serialize(dst, &())
    }
}

impl Deserialize for std::ffi::OsString {
    type Error = DeUsizeError;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        R: ?Sized + io::Read,
    {
        let bytes = Vec::deserialize(src, &mut ())?;
        // SAFETY: iunno
        unsafe { Ok(Self::from_encoded_bytes_unchecked(bytes)) }
    }
}

impl Serialize for std::ffi::CStr {
    type Error = io::Error;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        dst.write_all(self.to_bytes_with_nul())
    }
}

impl Deserialize for std::ffi::CString {
    type Error = io::Error;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        R: ?Sized + io::Read,
    {
        let mut buf = Vec::new();
        loop {
            let ch = Deserialize::deserialize(src, &mut ())?;
            buf.push(ch);
            if ch == 0 {
                break;
            }
        }
        // SAFETY: if buf does not end with 0, loop would not have ended
        unsafe { Ok(std::ffi::CString::from_vec_with_nul_unchecked(buf)) }
    }
}

impl Serialize for std::path::Path {
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        self.canonicalize()
            .map_err(SerUsizeError::Write)?
            .as_os_str()
            .serialize(dst, &())
    }
}

impl Deserialize for std::path::PathBuf {
    type Error = DeUsizeError;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        R: ?Sized + io::Read,
    {
        std::ffi::OsString::deserialize(src, &mut ()).map(Self::from)
    }
}

macro_rules! serde_float {
    ($($Type:ident),*) => {$(
        impl Serialize for $Type {
            type Error = io::Error;

            fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
            where
                W: ?Sized + io::Write
            {
                self.to_bits().serialize(dst, &())
            }
        }

        impl Deserialize for $Type {
            type Error = io::Error;

            fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
            where
                R: ?Sized + io::Read
            {
                Deserialize::deserialize(src, &mut ()).map(Self::from_bits)
            }
        }
    )*};
}

serde_float!(f32, f64);

#[derive(Debug, Error)]
pub enum SerImageError {
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    Write(#[from] io::Error),

    #[error(transparent)]
    Raylib(#[from] raylib::error::Error),
}

impl From<SerUsizeError> for SerImageError {
    fn from(e: SerUsizeError) -> Self {
        match e {
            SerUsizeError::Conversion(e) => Self::Conversion(e),
            SerUsizeError::Write(e) => Self::Write(e),
        }
    }
}

impl Serialize for Image {
    type Error = SerImageError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        self.export_image_to_memory(".png")?
            .serialize(dst, &())
            .map_err(Into::into)
    }
}

#[derive(Debug, Error)]
pub enum DeImageError {
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    Read(#[from] io::Error),

    #[error(transparent)]
    Raylib(#[from] raylib::error::Error),
}

impl From<DeUsizeError> for DeImageError {
    fn from(e: DeUsizeError) -> Self {
        match e {
            DeUsizeError::Conversion(e) => Self::Conversion(e),
            DeUsizeError::Read(e) => Self::Read(e),
        }
    }
}

impl Deserialize for Image {
    type Error = DeImageError;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + io::Read,
    {
        Image::load_image_from_mem(".png", Vec::deserialize(src, &mut ())?.as_slice())
            .map_err(Into::into)
    }
}

impl Serialize for Texture2D {
    type Error = SerImageError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        self.load_image()?.serialize(dst, &())
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread)> for Texture2D {
    type Error = DeImageError;

    fn deserialize<R>(
        src: &mut R,
        (rl, thread): &mut (&mut RaylibHandle, &RaylibThread),
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + io::Read,
    {
        rl.load_texture_from_image(thread, &Image::deserialize(src, &mut ())?)
            .map_err(Into::into)
    }
}

impl Serialize for RenderTexture2D {
    type Error = SerImageError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + io::Write,
    {
        self.load_image()?.serialize(dst, &())
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread)> for RenderTexture2D {
    type Error = DeImageError;

    fn deserialize<R>(
        src: &mut R,
        (rl, thread): &mut (&mut RaylibHandle, &RaylibThread),
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + io::Read,
    {
        rtex_from_image(rl, thread, &Image::deserialize(src, &mut ())?)
            .map_err(DeImageError::Raylib)
    }
}

#[macro_export]
macro_rules! serde_arr_like {
    ($($Struct:ty { $($field:ident),* }),*) => {$(
        impl Serialize for $Struct {
            type Error = io::Error;

            fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
            where
                W: ?Sized + std::io::Write,
            {
                [$(self.$field),*].serialize_arr(dst, &())
            }
        }

        impl Deserialize for $Struct {
            type Error = io::Error;

            fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
            where
                Self: Sized,
                R: ?Sized + std::io::Read,
            {
                let [$($field),*] = DeserializeArr::deserialize_arr(src, &mut ())?;
                Ok(Self { $($field),* })
            }
        }
    )*};
}

#[macro_export]
macro_rules! serde_pod {
    ($($Struct:ty { $($field:ident),* }),*) => {$(
        impl Serialize for $Struct {
            type Error = std::io::Error;

            fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
            where
                W: ?Sized + std::io::Write,
            {
                $(self.$field.serialize(dst, &())?;)*
                Ok(())
            }
        }

        impl Deserialize for $Struct {
            type Error = std::io::Error;

            fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
            where
                Self: Sized,
                R: ?Sized + std::io::Read,
            {
                Ok(Self {
                    $($field: Deserialize::deserialize(src, &mut ())?),*
                })
            }
        }
    )*};
}

serde_arr_like!(
    Vector2 { x, y },
    Vector3 { x, y, z },
    Vector4 { x, y, z, w },
    Matrix {
        m0,
        m4,
        m8,
        m12,
        m1,
        m5,
        m9,
        m13,
        m2,
        m6,
        m10,
        m14,
        m3,
        m7,
        m11,
        m15
    },
    Color { r, g, b, a }
);

macro_rules! serde_discrim_enum {
    ($($Enum:ty : $Repr:ty { $($Variant:ident),* }),*) => {$(
        impl Serialize for $Enum {
            type Error = io::Error;

            fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
            where
                W: ?Sized + std::io::Write,
            {
                (*self as $Repr).serialize(dst, &())
            }
        }

        impl Deserialize for $Enum {
            type Error = io::Error;

            fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
            where
                Self: Sized,
                R: ?Sized + std::io::Read,
            {
                $(const $Variant: $Repr = <$Enum>::$Variant as $Repr;)*
                match <$Repr>::deserialize(src, &mut ())? {
                    $($Variant => Ok(Self::$Variant),)*
                    x => Err(std::io::Error::other(format!("unknown variant: {x} ({x:#X})"))),
                }
            }
        }
    )*};
}

serde_discrim_enum!(
    BlendMode : u8 {
        BLEND_ALPHA,
        BLEND_ADDITIVE,
        BLEND_MULTIPLIED,
        BLEND_ADD_COLORS,
        BLEND_SUBTRACT_COLORS,
        BLEND_ALPHA_PREMULTIPLY,
        BLEND_CUSTOM,
        BLEND_CUSTOM_SEPARATE
    }
);
