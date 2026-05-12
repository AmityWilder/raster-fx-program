#![warn(clippy::undocumented_unsafe_blocks, clippy::missing_safety_doc)]
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

/// Types that can be used to create vertices in rlgl
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

impl RlVertex for [f32; 2] {
    unsafe fn rl_vertex(self) {
        let [x, y] = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector2::new(x, y).rl_vertex();
        }
    }
}

impl RlVertex for (f32, f32) {
    unsafe fn rl_vertex(self) {
        let (x, y) = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector2::new(x, y).rl_vertex();
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

impl RlVertex for [f32; 3] {
    unsafe fn rl_vertex(self) {
        let [x, y, z] = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector3::new(x, y, z).rl_vertex();
        }
    }
}

impl RlVertex for (f32, f32, f32) {
    unsafe fn rl_vertex(self) {
        let (x, y, z) = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector3::new(x, y, z).rl_vertex();
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

/// Types that can be used to create texture coordinates in rlgl
pub trait RlTexCoord {
    /// # Safety
    /// Caller must guarantee
    /// - [`ffi::rlBegin`] has been called without a corresponding [`ffi::rlEnd`] yet
    /// - number of vertices is valid for the rl mode ([`ffi::RL_LINES`]/[`ffi::RL_TRIANGLES`]/[`ffi::RL_QUADS`])
    unsafe fn rl_tex_coord(self);
}

impl RlTexCoord for [f32; 2] {
    unsafe fn rl_tex_coord(self) {
        let [x, y] = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector2::new(x, y).rl_tex_coord();
        }
    }
}

impl RlTexCoord for (f32, f32) {
    unsafe fn rl_tex_coord(self) {
        let (x, y) = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector2::new(x, y).rl_tex_coord();
        }
    }
}

impl RlTexCoord for Vector2 {
    unsafe fn rl_tex_coord(self) {
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlTexCoord2f(self.x, self.y);
        }
    }
}

/// Types that can be used to create normals in rlgl
pub trait RlNormal {
    /// # Safety
    /// Caller must guarantee [`ffi::rlBegin`] has been called without a corresponding [`ffi::rlEnd`] yet
    unsafe fn rl_normal(self);
}

impl RlNormal for [f32; 3] {
    unsafe fn rl_normal(self) {
        let [x, y, z] = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector3::new(x, y, z).rl_normal();
        }
    }
}

impl RlNormal for (f32, f32, f32) {
    unsafe fn rl_normal(self) {
        let (x, y, z) = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector3::new(x, y, z).rl_normal();
        }
    }
}

impl RlNormal for Vector3 {
    unsafe fn rl_normal(self) {
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlNormal3f(self.x, self.y, self.z);
        }
    }
}

/// Types that can be used to create colors in rlgl
pub trait RlColor {
    /// # Safety
    /// Caller must guarantee [`ffi::rlBegin`] has been called without a corresponding [`ffi::rlEnd`] yet
    unsafe fn rl_color(self);
}

impl RlColor for [u8; 4] {
    unsafe fn rl_color(self) {
        let [r, g, b, a] = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Color::new(r, g, b, a).rl_color();
        }
    }
}

impl RlColor for (u8, u8, u8, u8) {
    unsafe fn rl_color(self) {
        let (r, g, b, a) = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Color::new(r, g, b, a).rl_color();
        }
    }
}

impl RlColor for Color {
    unsafe fn rl_color(self) {
        // SAFETY: Caller's responsibility
        unsafe {
            ffi::rlColor4ub(self.r, self.g, self.b, self.a);
        }
    }
}

impl RlColor for [f32; 3] {
    unsafe fn rl_color(self) {
        let [r, g, b] = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector3::new(r, g, b).rl_color();
        }
    }
}

impl RlColor for (f32, f32, f32) {
    unsafe fn rl_color(self) {
        let (r, g, b) = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector3::new(r, g, b).rl_color();
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

impl RlColor for [f32; 4] {
    unsafe fn rl_color(self) {
        let [r, g, b, a] = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector4::new(r, g, b, a).rl_color();
        }
    }
}

impl RlColor for (f32, f32, f32, f32) {
    unsafe fn rl_color(self) {
        let (r, g, b, a) = self;
        // SAFETY: Caller's responsibility
        unsafe {
            Vector4::new(r, g, b, a).rl_color();
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

/// Implementors must guarantee Self is exactly 2 (impl [`RlTexCoord`], impl [`RlVertex`])
pub trait LineVerts {
    /// # Safety
    /// Callers must guarantee [`ffi::rlBegin`] has been called with [`ffi::RL_LINES`] without a corresponding [`ffi::rlEnd`] yet
    unsafe fn line(self);
}

impl<T, U> LineVerts for [(T, U); 2]
where
    T: RlTexCoord,
    U: RlVertex,
{
    unsafe fn line(self) {
        for (tex_coord, vertex) in self {
            // SAFETY: Caller responsibility
            unsafe {
                tex_coord.rl_tex_coord();
                vertex.rl_vertex();
            }
        }
    }
}

impl<T1, U1, T2, U2> LineVerts for ((T1, U1), (T2, U2))
where
    T1: RlTexCoord,
    T2: RlTexCoord,
    U1: RlVertex,
    U2: RlVertex,
{
    unsafe fn line(self) {
        let ((tex_coord1, vertex1), (tex_coord2, vertex2)) = self;

        // SAFETY: Caller responsibility
        unsafe {
            tex_coord1.rl_tex_coord();
            vertex1.rl_vertex();

            tex_coord2.rl_tex_coord();
            vertex2.rl_vertex();
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

    pub fn line(&mut self, verts: impl LineVerts) {
        // SAFETY: Construction guarantees rlgl active, array guarantees valid quantity of vertices
        unsafe { verts.line() }
    }
}

// SAFETY: Rlgl is activated on construction and does not end until dropped
unsafe impl<D: RlglExt> Rlgl for RlLines<'_, D> {}

/// Implementors must guarantee Self is exactly 3 (impl [`RlTexCoord`], impl [`RlVertex`])
pub trait TriangleVerts {
    /// # Safety
    /// Callers must guarantee [`ffi::rlBegin`] has been called with [`ffi::RL_TRIANGLES`] without a corresponding [`ffi::rlEnd`] yet
    unsafe fn triangle(self);
}

impl<T, U> TriangleVerts for [(T, U); 3]
where
    T: RlTexCoord,
    U: RlVertex,
{
    unsafe fn triangle(self) {
        for (tex_coord, vertex) in self {
            // SAFETY: Caller responsibility
            unsafe {
                tex_coord.rl_tex_coord();
                vertex.rl_vertex();
            }
        }
    }
}

impl<T1, U1, T2, U2, T3, U3> TriangleVerts for ((T1, U1), (T2, U2), (T3, U3))
where
    T1: RlTexCoord,
    T2: RlTexCoord,
    T3: RlTexCoord,
    U1: RlVertex,
    U2: RlVertex,
    U3: RlVertex,
{
    unsafe fn triangle(self) {
        let ((tex_coord1, vertex1), (tex_coord2, vertex2), (tex_coord3, vertex3)) = self;

        // SAFETY: Caller responsibility
        unsafe {
            tex_coord1.rl_tex_coord();
            vertex1.rl_vertex();

            tex_coord2.rl_tex_coord();
            vertex2.rl_vertex();

            tex_coord3.rl_tex_coord();
            vertex3.rl_vertex();
        }
    }
}

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

    pub fn triangle(&mut self, verts: impl TriangleVerts) {
        // SAFETY: Construction guarantees rlgl active, TriangleVerts guarantees valid quantity of vertices
        unsafe {
            verts.triangle();
        }
    }
}

// SAFETY: Rlgl is activated on construction and does not end until dropped
unsafe impl<D: RlglExt> Rlgl for RlTriangles<'_, D> {}

/// Implementors must guarantee Self is exactly 4 (impl [`RlTexCoord`], impl [`RlVertex`])
pub trait QuadVerts {
    /// # Safety
    /// Callers must guarantee [`ffi::rlBegin`] has been called with [`ffi::RL_QUADS`] without a corresponding [`ffi::rlEnd`] yet
    unsafe fn quad(self);
}

impl<T, U> QuadVerts for [(T, U); 4]
where
    T: RlTexCoord,
    U: RlVertex,
{
    unsafe fn quad(self) {
        for (tex_coord, vertex) in self {
            // SAFETY: Caller responsibility
            unsafe {
                tex_coord.rl_tex_coord();
                vertex.rl_vertex();
            }
        }
    }
}

impl<T1, U1, T2, U2, T3, U3, T4, U4> QuadVerts for ((T1, U1), (T2, U2), (T3, U3), (T4, U4))
where
    T1: RlTexCoord,
    T2: RlTexCoord,
    T3: RlTexCoord,
    T4: RlTexCoord,
    U1: RlVertex,
    U2: RlVertex,
    U3: RlVertex,
    U4: RlVertex,
{
    unsafe fn quad(self) {
        let (
            (tex_coord1, vertex1),
            (tex_coord2, vertex2),
            (tex_coord3, vertex3),
            (tex_coord4, vertex4),
        ) = self;

        // SAFETY: Caller responsibility
        unsafe {
            tex_coord1.rl_tex_coord();
            vertex1.rl_vertex();

            tex_coord2.rl_tex_coord();
            vertex2.rl_vertex();

            tex_coord3.rl_tex_coord();
            vertex3.rl_vertex();

            tex_coord4.rl_tex_coord();
            vertex4.rl_vertex();
        }
    }
}

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

    pub fn quad(&mut self, verts: impl QuadVerts) {
        // SAFETY: Construction guarantees rlgl active, QuadVerts guarantees valid quantity of vertices
        unsafe {
            verts.quad();
        }
    }
}

// SAFETY: Rlgl is activated on construction and does not end until dropped
unsafe impl<D: RlglExt> Rlgl for RlQuads<'_, D> {}
