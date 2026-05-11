use crate::layer::rtex_from_image;
use raylib::prelude::*;
use std::io;

pub trait Serialize<Ctx: ?Sized = ()> {
    fn serialize<W>(&self, dst: &mut W, ctx: &Ctx) -> io::Result<()>
    where
        W: ?Sized + io::Write;
}

pub trait Deserialize<Ctx: ?Sized = ()> {
    fn deserialize<R>(src: &mut R, ctx: &mut Ctx) -> io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + io::Read;
}

pub trait SerializeArr<Ctx: ?Sized = ()> {
    type Item;

    fn serialize_arr<W>(&self, dst: &mut W, ctx: &Ctx) -> io::Result<()>
    where
        W: ?Sized + io::Write;
}

pub trait DeserializeArr<Ctx: ?Sized = ()> {
    type Item;

    fn deserialize_arr<R>(src: &mut R, ctx: &mut Ctx) -> io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + io::Read;
}

pub trait SerializeSlice<Ctx: ?Sized = ()> {
    type Item;

    fn serialize_slice<W>(&self, dst: &mut W, ctx: &Ctx) -> io::Result<()>
    where
        W: ?Sized + io::Write;
}

pub trait DeserializeSlice<T, Ctx: ?Sized = ()> {
    fn deserialize_slice<R>(src: &mut R, ctx: &mut Ctx) -> io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + io::Read;
}

impl<T, Ctx: ?Sized, const N: usize> SerializeArr<Ctx> for [T; N]
where
    T: Serialize<Ctx>,
{
    type Item = T;

    fn serialize_arr<W>(&self, dst: &mut W, ctx: &Ctx) -> io::Result<()>
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

    fn deserialize_arr<R>(src: &mut R, ctx: &mut Ctx) -> io::Result<Self>
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
{
    type Item = T;

    fn serialize_slice<W>(&self, dst: &mut W, ctx: &Ctx) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.len().serialize(dst, &())?;
        self.iter().try_for_each(|item| item.serialize(dst, ctx))
    }
}

impl<T, Ctx, FromT> DeserializeSlice<T, Ctx> for FromT
where
    T: Deserialize<Ctx>,
    FromT: FromIterator<T>,
{
    fn deserialize_slice<R>(src: &mut R, ctx: &mut Ctx) -> io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + io::Read,
    {
        let len = usize::deserialize(src, &mut ())?;
        (0..len).map(|_| T::deserialize(src, ctx)).collect()
    }
}

impl Serialize for u8 {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        dst.write_all(std::slice::from_ref(self))
    }
}

impl Deserialize for u8 {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
    where
        R: ?Sized + io::Read,
    {
        let mut byte = 0;
        src.read_exact(std::slice::from_mut(&mut byte))?;
        Ok(byte)
    }
}

impl<const N: usize> Serialize for [u8; N] {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        dst.write_all(self)
    }
}

impl<const N: usize> Deserialize for [u8; N] {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
    where
        R: ?Sized + io::Read,
    {
        let mut bytes = [0; N];
        src.read_exact(bytes.as_mut_slice())?;
        Ok(bytes)
    }
}

impl Serialize for [u8] {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.len().serialize(dst, &())?;
        dst.write_all(self)
    }
}

impl Deserialize for Vec<u8> {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
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
            fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
            where
                W: ?Sized + io::Write
            {
                dst.write_all(self.to_le_bytes().as_slice())
            }
        }

        impl Deserialize for $Type {
            fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
            where
                R: ?Sized + io::Read
            {
                <[u8; _]>::deserialize(src, &mut ()).map(Self::from_le_bytes)
            }
        }
    )*};
}

serde_int!(u16, u32, u64, u128, i8, i16, i32, i64, i128);

impl Serialize for usize {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        match u64::try_from(*self).map_err(io::Error::other) {
            Ok(x) => x.serialize(dst, &()),
            Err(e) => Err(e),
        }
    }
}

impl Deserialize for usize {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
    where
        R: ?Sized + io::Read,
    {
        match u64::deserialize(src, &mut ()) {
            Ok(x) => Self::try_from(x).map_err(io::Error::other),
            Err(e) => Err(e),
        }
    }
}

impl Serialize for isize {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        i64::try_from(*self)
            .map_err(io::Error::other)
            .and_then(|x| x.serialize(dst, &()))
    }
}

impl Deserialize for isize {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
    where
        R: ?Sized + io::Read,
    {
        i64::deserialize(src, &mut ()).and_then(|x| Self::try_from(x).map_err(io::Error::other))
    }
}

impl Serialize for str {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.as_bytes().serialize(dst, &())
    }
}

impl Deserialize for String {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
    where
        R: ?Sized + io::Read,
    {
        let bytes = Vec::deserialize(src, &mut ())?;
        Self::from_utf8(bytes).map_err(io::Error::other)
    }
}

impl Serialize for std::ffi::OsStr {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.as_encoded_bytes().serialize(dst, &())
    }
}

impl Deserialize for std::ffi::OsString {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
    where
        R: ?Sized + io::Read,
    {
        let bytes = Vec::deserialize(src, &mut ())?;
        // SAFETY: iunno
        unsafe { Ok(Self::from_encoded_bytes_unchecked(bytes)) }
    }
}

impl Serialize for std::ffi::CStr {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        dst.write_all(self.to_bytes_with_nul())
    }
}

impl Deserialize for std::ffi::CString {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
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
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.canonicalize()
            .and_then(|x| x.as_os_str().serialize(dst, &()))
    }
}

impl Deserialize for std::path::PathBuf {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
    where
        R: ?Sized + io::Read,
    {
        std::ffi::OsString::deserialize(src, &mut ()).map(Self::from)
    }
}

macro_rules! serde_float {
    ($($Type:ident),*) => {$(
        impl Serialize for $Type {
            fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
            where
                W: ?Sized + io::Write
            {
                self.to_bits().serialize(dst, &())
            }
        }

        impl Deserialize for $Type {
            fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
            where
                R: ?Sized + io::Read
            {
                Deserialize::deserialize(src, &mut ()).map(Self::from_bits)
            }
        }
    )*};
}

serde_float!(f32, f64);

impl Serialize for Image {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.export_image_to_memory(".png")
            .map_err(io::Error::other)
            .and_then(|bytes| bytes.serialize(dst, &()))
    }
}

impl Deserialize for Image {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + io::Read,
    {
        Vec::deserialize(src, &mut ()).and_then(|ref bytes| {
            Image::load_image_from_mem(".png", bytes).map_err(io::Error::other)
        })
    }
}

impl Serialize for Texture2D {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.load_image()
            .map_err(io::Error::other)
            .and_then(|img| img.serialize(dst, &()))
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread)> for Texture2D {
    fn deserialize<R>(
        src: &mut R,
        (rl, thread): &mut (&mut RaylibHandle, &RaylibThread),
    ) -> io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + io::Read,
    {
        Image::deserialize(src, &mut ()).and_then(|ref img| {
            rl.load_texture_from_image(thread, img)
                .map_err(io::Error::other)
        })
    }
}

impl Serialize for RenderTexture2D {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.load_image()
            .map_err(io::Error::other)
            .and_then(|img| img.serialize(dst, &()))
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread)> for RenderTexture2D {
    fn deserialize<R>(
        src: &mut R,
        (rl, thread): &mut (&mut RaylibHandle, &RaylibThread),
    ) -> io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + io::Read,
    {
        Image::deserialize(src, &mut ())
            .map_err(io::Error::other)
            .and_then(|ref img| rtex_from_image(rl, thread, img).map_err(io::Error::other))
    }
}

#[macro_export]
macro_rules! serde_arr_like {
    ($($Struct:ty { $($field:ident),* }),*) => {$(
        impl Serialize for $Struct {
            fn serialize<W>(&self, dst: &mut W, _: &()) -> std::io::Result<()>
            where
                W: ?Sized + std::io::Write,
            {
                [$(self.$field),*].serialize_arr(dst, &())
            }
        }

        impl Deserialize for $Struct {
            fn deserialize<R>(src: &mut R, _: &mut ()) -> std::io::Result<Self>
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
            fn serialize<W>(&self, dst: &mut W, _: &()) -> std::io::Result<()>
            where
                W: ?Sized + std::io::Write,
            {
                $(self.$field.serialize(dst, &())?;)*
                Ok(())
            }
        }

        impl Deserialize for $Struct {
            fn deserialize<R>(src: &mut R, _: &mut ()) -> std::io::Result<Self>
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
            fn serialize<W>(&self, dst: &mut W, _: &()) -> std::io::Result<()>
            where
                W: ?Sized + std::io::Write,
            {
                (*self as $Repr).serialize(dst, &())
            }
        }

        impl Deserialize for $Enum {
            fn deserialize<R>(src: &mut R, _: &mut ()) -> std::io::Result<Self>
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
