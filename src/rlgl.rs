#![warn(clippy::undocumented_unsafe_blocks, reason = "need to determine")]
#![allow(clippy::multiple_unsafe_ops_per_block)]
use raylib::prelude::*;
use std::marker::PhantomData;

/// Guarantees drawing is active and we are on the Raylib thread
pub struct RlTexture<'a, 'b, D, T>(PhantomData<(&'a mut D, &'b T, *mut ())>);

impl<D, T> Drop for RlTexture<'_, '_, D, T> {
    fn drop(&mut self) {
        // SAFETY: Guaranteed by construction
        unsafe {
            ffi::rlSetTexture(0);
        }
    }
}

impl<'a, 'b, D: RaylibDraw, T: RaylibTexture2D> RlTexture<'a, 'b, D, T> {
    fn new(_: &'a mut D, texture: &'b T) -> Self {
        // SAFETY: RaylibDraw proves drawing is active
        unsafe {
            ffi::rlSetTexture(texture.as_ref().id);
        }
        Self(PhantomData)
    }
}

pub trait RlTextureExt: Sized + RaylibDraw {
    fn rl_set_texture<'a, 'b, T>(&'a mut self, texture: &'b T) -> RlTexture<'a, 'b, Self, T>
    where
        T: RaylibTexture2D,
    {
        RlTexture::new(self, texture)
    }
}

impl<D: RaylibDraw> RlTextureExt for D {}

/// # Safety
/// Implementor must guarantee [`ffi::rlBegin`] is safe to call
pub unsafe trait RlglExt: Sized {
    fn rl_begin_lines(&mut self) -> RlLines<'_, Self> {
        RlLines::new(self)
    }

    fn rl_begin_triangles(&mut self) -> RlTriangles<'_, Self> {
        RlTriangles::new(self)
    }

    fn rl_begin_quads(&mut self) -> RlQuads<'_, Self> {
        RlQuads::new(self)
    }
}

// SAFETY: This is the only rlgl mode
unsafe impl<D: RaylibDraw, T: RaylibTexture2D> RlglExt for RlTexture<'_, '_, D, T> {}

pub trait RlVertex {
    /// # Safety
    /// Caller must guarantee
    /// - [`ffi::rlBegin`] has been called without a corresponding [`ffi::rlEnd`] yet
    /// - number of vertices is valid for the rl mode ([`ffi::RL_LINES`]/[`ffi::RL_TRIANGLES`]/[`ffi::RL_QUADS`])
    unsafe fn rl_vertex(self);
}

impl RlVertex for [std::ffi::c_int; 2] {
    unsafe fn rl_vertex(self) {
        let [x, y] = self;
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlVertex2i(x, y);
        }
    }
}

impl RlVertex for (std::ffi::c_int, std::ffi::c_int) {
    unsafe fn rl_vertex(self) {
        let (x, y) = self;
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlVertex2i(x, y);
        }
    }
}

impl RlVertex for Vector2 {
    unsafe fn rl_vertex(self) {
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlVertex2f(self.x, self.y);
        }
    }
}

impl RlVertex for Vector3 {
    unsafe fn rl_vertex(self) {
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlVertex3f(self.x, self.y, self.z);
        }
    }
}

pub trait RlTexCoord {
    /// # Safety
    /// Caller must guarantee
    /// - [`ffi::rlBegin`] has been called without a corresponding [`ffi::rlEnd`] yet
    /// - number of vertices is valid for the rl mode ([`ffi::RL_LINES`]/[`ffi::RL_TRIANGLES`]/[`ffi::RL_QUADS`])
    unsafe fn rl_tex_coord(self);
}

impl RlTexCoord for Vector2 {
    unsafe fn rl_tex_coord(self) {
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlTexCoord2f(self.x, self.y);
        }
    }
}

pub trait RlNormal {
    /// # Safety
    /// Caller must guarantee [`ffi::rlBegin`] has been called without a corresponding [`ffi::rlEnd`] yet
    unsafe fn rl_normal(self);
}

impl RlNormal for Vector3 {
    unsafe fn rl_normal(self) {
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlNormal3f(self.x, self.y, self.z);
        }
    }
}

pub trait RlColor {
    /// # Safety
    /// Caller must guarantee [`ffi::rlBegin`] has been called without a corresponding [`ffi::rlEnd`] yet
    unsafe fn rl_color(self);
}

impl RlColor for Color {
    unsafe fn rl_color(self) {
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlColor4ub(self.r, self.g, self.b, self.a);
        }
    }
}

impl RlColor for Vector3 {
    unsafe fn rl_color(self) {
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlColor3f(self.x, self.y, self.z);
        }
    }
}

impl RlColor for Vector4 {
    unsafe fn rl_color(self) {
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlColor4f(self.x, self.y, self.z, self.w);
        }
    }
}

/// # Safety
/// Implementors must guarantee [`ffi::rlBegin`] has been called without a corresponding [`ffi::rlEnd`] yet
pub unsafe trait Rlgl {
    fn rl_color(&mut self, tint: impl RlColor) {
        // SAFETY: Implementor's responsibility
        unsafe {
            tint.rl_color();
        }
    }

    fn rl_normal(&mut self, v: Vector3) {
        // SAFETY: Implementor's responsibility
        unsafe {
            v.rl_normal();
        }
    }
}

pub struct RlLines<'a, D>(PhantomData<&'a mut D>);

impl<D> Drop for RlLines<'_, D> {
    fn drop(&mut self) {
        // SAFETY: Guaranteed by construction
        unsafe {
            ffi::rlEnd();
        }
    }
}

impl<'a, D: RlglExt> RlLines<'a, D> {
    fn new(_: &'a mut D) -> Self {
        // SAFETY: Guaranteed by RlglExt
        unsafe {
            ffi::rlBegin(ffi::RL_LINES as i32);
        }
        Self(PhantomData)
    }

    pub fn line(&mut self, verts: [(impl RlTexCoord, impl RlVertex); 2]) {
        for (tex_coord, vertex) in verts {
            // SAFETY: Construction guarantees rlgl active, array guarantees valid quantity of vertices
            unsafe {
                tex_coord.rl_tex_coord();
                vertex.rl_vertex();
            }
        }
    }
}

// SAFETY: Rlgl is activated on construction and does not end until dropped
unsafe impl<D: RlglExt> Rlgl for RlLines<'_, D> {}

pub struct RlTriangles<'a, D>(PhantomData<&'a mut D>);

impl<D> Drop for RlTriangles<'_, D> {
    fn drop(&mut self) {
        // SAFETY: Guaranteed by construction
        unsafe {
            ffi::rlEnd();
        }
    }
}

impl<'a, D: RlglExt> RlTriangles<'a, D> {
    fn new(_: &'a mut D) -> Self {
        // SAFETY: Guaranteed by RlglExt
        unsafe {
            ffi::rlBegin(ffi::RL_TRIANGLES as i32);
        }
        Self(PhantomData)
    }

    pub fn triangle(&mut self, verts: [(impl RlTexCoord, impl RlVertex); 3]) {
        for (tex_coord, vertex) in verts {
            // SAFETY: Construction guarantees rlgl active, array guarantees valid quantity of vertices
            unsafe {
                tex_coord.rl_tex_coord();
                vertex.rl_vertex();
            }
        }
    }
}

// SAFETY: Rlgl is activated on construction and does not end until dropped
unsafe impl<D: RlglExt> Rlgl for RlTriangles<'_, D> {}

pub struct RlQuads<'a, D>(PhantomData<&'a mut D>);

impl<D> Drop for RlQuads<'_, D> {
    fn drop(&mut self) {
        // SAFETY: Guaranteed by construction
        unsafe {
            ffi::rlEnd();
        }
    }
}

impl<'a, D: RlglExt> RlQuads<'a, D> {
    fn new(_: &'a mut D) -> Self {
        // SAFETY: Guaranteed by RlglExt
        unsafe {
            ffi::rlBegin(ffi::RL_QUADS as i32);
        }
        Self(PhantomData)
    }

    pub fn quad(&mut self, verts: [(impl RlTexCoord, impl RlVertex); 4]) {
        for (tex_coord, vertex) in verts {
            // SAFETY: Construction guarantees rlgl active, array guarantees valid quantity of vertices
            unsafe {
                tex_coord.rl_tex_coord();
                vertex.rl_vertex();
            }
        }
    }
}

// SAFETY: Rlgl is activated on construction and does not end until dropped
unsafe impl<D: RlglExt> Rlgl for RlQuads<'_, D> {}
