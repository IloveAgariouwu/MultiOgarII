use std::mem::offset_of;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram, WebGlShader,
    WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

use super::backdrop::Backdrop;
use super::jelly::ROW as JELLY_ROW;
use super::overlap::{cover_offset, OverlapMark, COVER_VEC4S, OVERLAP_BYTES};
use super::primitive::{pack_into, shader_source, Instance, ShaderLang, INSTANCE_BYTES};
use super::skin_atlas::{mip_sides, SkinCache, SkinTexture};
use super::text::{TextAtlas, ATLAS_SIDE};
use super::{FrameView, Layer, Layers};
use bytemuck::Pod;

const VERT: &str = include_str!("shaders/sprite.vert");
const FRAG: &str = include_str!("shaders/sprite.frag");
const OVERLAP_VERT: &str = include_str!("shaders/overlap.vert");
const OVERLAP_FRAG: &str = include_str!("shaders/overlap.frag");
const UNMASKED_RENDERER: u32 = 0x9246;
const SOFTWARE_RENDERERS: [&str; 4] =
    ["SwiftShader", "Basic Render Driver", "llvmpipe", "softpipe"];

pub(crate) fn renders_in_software() -> bool {
    let Some(gl) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.create_element("canvas").ok())
        .and_then(|c| c.dyn_into::<HtmlCanvasElement>().ok())
        .and_then(|c| c.get_context("webgl2").ok().flatten())
        .and_then(|c| c.dyn_into::<Gl>().ok())
    else {
        return false;
    };
    let parameter = match gl.get_extension("WEBGL_debug_renderer_info") {
        Ok(Some(_)) => UNMASKED_RENDERER,
        _ => Gl::RENDERER,
    };
    let renderer = gl
        .get_parameter(parameter)
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    lose_context(&gl);
    SOFTWARE_RENDERERS.iter().any(|s| renderer.contains(s))
}

fn lose_context(gl: &Gl) {
    let Ok(Some(ext)) = gl.get_extension("WEBGL_lose_context") else {
        return;
    };
    if let Some(lose) = js_sys::Reflect::get(&ext, &"loseContext".into())
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
    {
        let _ = lose.call0(&ext);
    }
}

struct Batch {
    vao: WebGlVertexArrayObject,
    buffer: WebGlBuffer,
    capacity: usize,
}

pub(crate) struct GlBackend {
    gl: Gl,
    program: WebGlProgram,
    u_view: WebGlUniformLocation,
    u_px: WebGlUniformLocation,
    batches: [Batch; Layer::COUNT],
    packed: Vec<Instance>,
    overlap: OverlapPass,
    atlas: WebGlTexture,
    atlas_layers: u32,
    skins: Option<SkinTexture<WebGlTexture>>,
    backdrop: WebGlTexture,
    jelly: WebGlTexture,
    jelly_rows: usize,
    backdrop_generation: Option<u32>,
    max_dimension: u32,
    owns_context: bool,
}

fn compile(gl: &Gl, kind: u32, source: &str) -> Result<WebGlShader, JsValue> {
    let shader = gl.create_shader(kind).ok_or("create_shader failed")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl.get_shader_info_log(&shader).unwrap_or_default().into())
    }
}

fn link(gl: &Gl, vert: &str, frag: &str) -> Result<WebGlProgram, JsValue> {
    let program = gl.create_program().ok_or("create_program failed")?;
    gl.attach_shader(&program, &compile(gl, Gl::VERTEX_SHADER, vert)?);
    gl.attach_shader(&program, &compile(gl, Gl::FRAGMENT_SHADER, frag)?);
    gl.link_program(&program);
    if gl
        .get_program_parameter(&program, Gl::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl.get_program_info_log(&program).unwrap_or_default().into())
    }
}

fn batch(gl: &Gl) -> Result<Batch, JsValue> {
    let vao = gl
        .create_vertex_array()
        .ok_or("create_vertex_array failed")?;
    let buffer = gl.create_buffer().ok_or("create_buffer failed")?;
    gl.bind_vertex_array(Some(&vao));
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
    let stride = INSTANCE_BYTES as i32;
    let attribute = |loc: u32, size: i32, ty: u32, normalized: bool, offset: usize| {
        gl.enable_vertex_attrib_array(loc);
        gl.vertex_attrib_pointer_with_i32(loc, size, ty, normalized, stride, offset as i32);
        gl.vertex_attrib_divisor(loc, 1);
    };
    attribute(0, 4, Gl::FLOAT, false, offset_of!(Instance, geom));
    attribute(1, 4, Gl::UNSIGNED_SHORT, true, offset_of!(Instance, aux));
    attribute(2, 4, Gl::UNSIGNED_BYTE, true, offset_of!(Instance, color0));
    attribute(3, 4, Gl::UNSIGNED_BYTE, true, offset_of!(Instance, color1));
    attribute(4, 1, Gl::FLOAT, false, offset_of!(Instance, scalar));
    gl.enable_vertex_attrib_array(5);
    let head = offset_of!(Instance, head) as i32;
    gl.vertex_attrib_i_pointer_with_i32(5, 1, Gl::UNSIGNED_INT, stride, head);
    gl.vertex_attrib_divisor(5, 1);
    gl.bind_vertex_array(None);
    Ok(Batch {
        vao,
        buffer,
        capacity: 0,
    })
}

fn overlap_batch(gl: &Gl) -> Result<Batch, JsValue> {
    let vao = gl
        .create_vertex_array()
        .ok_or("create_vertex_array failed")?;
    let buffer = gl.create_buffer().ok_or("create_buffer failed")?;
    gl.bind_vertex_array(Some(&vao));
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
    let stride = OVERLAP_BYTES as i32;
    let attribute = |loc: u32, size: i32, ty: u32, normalized: bool, offset: usize| {
        gl.enable_vertex_attrib_array(loc);
        gl.vertex_attrib_pointer_with_i32(loc, size, ty, normalized, stride, offset as i32);
        gl.vertex_attrib_divisor(loc, 1);
    };
    attribute(0, 3, Gl::FLOAT, false, offset_of!(OverlapMark, x));
    attribute(
        1,
        4,
        Gl::UNSIGNED_BYTE,
        true,
        offset_of!(OverlapMark, color),
    );
    for k in 0..COVER_VEC4S {
        attribute(2 + k as u32, 4, Gl::FLOAT, false, cover_offset(k));
    }
    gl.bind_vertex_array(None);
    Ok(Batch {
        vao,
        buffer,
        capacity: 0,
    })
}

struct OverlapPass {
    program: WebGlProgram,
    u_view: WebGlUniformLocation,
    u_px: WebGlUniformLocation,
    batch: Batch,
}

impl OverlapPass {
    fn new(gl: &Gl) -> Result<Self, JsValue> {
        let program = link(
            gl,
            OVERLAP_VERT,
            &shader_source(ShaderLang::Glsl, OVERLAP_FRAG),
        )?;
        let uniform = |name: &str| {
            gl.get_uniform_location(&program, name)
                .ok_or_else(|| JsValue::from_str(name))
        };
        Ok(Self {
            u_view: uniform("u_view")?,
            u_px: uniform("u_px")?,
            batch: overlap_batch(gl)?,
            program,
        })
    }
}

impl Drop for GlBackend {
    fn drop(&mut self) {
        if self.owns_context {
            lose_context(&self.gl);
        }
    }
}

impl GlBackend {
    pub(crate) fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let attrs = web_sys::WebGlContextAttributes::new();
        attrs.set_alpha(false);
        attrs.set_antialias(false);
        attrs.set_depth(false);
        attrs.set_stencil(false);
        attrs.set_premultiplied_alpha(false);
        let gl: Gl = canvas
            .get_context_with_context_options("webgl2", &attrs)?
            .ok_or("webgl2 unavailable")?
            .dyn_into()?;
        let number = |name: u32| {
            gl.get_parameter(name)
                .expect("limit query")
                .as_f64()
                .expect("limit is a number") as u32
        };
        let viewport: js_sys::Int32Array = gl.get_parameter(Gl::MAX_VIEWPORT_DIMS)?.dyn_into()?;
        let max_dimension = number(Gl::MAX_RENDERBUFFER_SIZE)
            .min(viewport.get_index(0) as u32)
            .min(viewport.get_index(1) as u32);
        let program = link(
            &gl,
            &shader_source(ShaderLang::Glsl, VERT),
            &shader_source(ShaderLang::Glsl, FRAG),
        )?;
        gl.use_program(Some(&program));
        let uniform = |name: &str| {
            gl.get_uniform_location(&program, name)
                .ok_or_else(|| JsValue::from_str(name))
        };
        let u_view = uniform("u_view")?;
        let u_px = uniform("u_px")?;
        gl.uniform1i(Some(&uniform("u_atlas")?), 0);
        gl.uniform1i(Some(&uniform("u_skins")?), 1);
        gl.uniform1i(Some(&uniform("u_backdrop")?), 2);
        let backdrop = gl.create_texture().ok_or("create_texture failed")?;
        gl.active_texture(Gl::TEXTURE2);
        gl.bind_texture(Gl::TEXTURE_2D, Some(&backdrop));
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            Gl::TEXTURE_2D,
            0,
            Gl::RGBA as i32,
            1,
            1,
            0,
            Gl::RGBA,
            Gl::UNSIGNED_BYTE,
            Some(&[0; 4]),
        )?;
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, Gl::LINEAR as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
        gl.uniform1i(Some(&uniform("u_jelly")?), 3);
        let jelly = gl.create_texture().ok_or("create_texture failed")?;
        Self::resize_jelly(&gl, &jelly, 1)?;
        let atlas = Self::atlas_texture(&gl, 1)?;
        gl.enable(Gl::BLEND);
        Self::normal_blend(&gl);
        let batches = [
            batch(&gl)?,
            batch(&gl)?,
            batch(&gl)?,
            batch(&gl)?,
            batch(&gl)?,
            batch(&gl)?,
            batch(&gl)?,
            batch(&gl)?,
            batch(&gl)?,
        ];
        let overlap = OverlapPass::new(&gl)?;
        Ok(Self {
            gl,
            program,
            u_view,
            u_px,
            batches,
            packed: Vec::new(),
            overlap,
            atlas,
            atlas_layers: 1,
            skins: None,
            backdrop,
            backdrop_generation: None,
            jelly,
            jelly_rows: 1,
            max_dimension,
            owns_context: true,
        })
    }

    pub(crate) fn max_dimension(&self) -> u32 {
        self.max_dimension
    }

    pub(crate) fn release_context(&mut self) {
        self.owns_context = false;
    }

    pub(crate) fn drawable(&self) -> (u32, u32) {
        (
            self.gl.drawing_buffer_width() as u32,
            self.gl.drawing_buffer_height() as u32,
        )
    }

    fn resize_jelly(gl: &Gl, texture: &WebGlTexture, rows: usize) -> Result<(), JsValue> {
        gl.active_texture(Gl::TEXTURE3);
        gl.bind_texture(Gl::TEXTURE_2D, Some(texture));
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
            Gl::TEXTURE_2D,
            0,
            Gl::R32F as i32,
            JELLY_ROW as i32,
            rows as i32,
            0,
            Gl::RED,
            Gl::FLOAT,
            None,
        )?;
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, Gl::NEAREST as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MAG_FILTER, Gl::NEAREST as i32);
        Ok(())
    }

    fn sync_jelly(&mut self, rows: &[f32]) -> Result<(), JsValue> {
        let count = rows.len() / JELLY_ROW;
        if count == 0 {
            return Ok(());
        }
        if count > self.jelly_rows {
            self.jelly_rows = count.next_power_of_two();
            Self::resize_jelly(&self.gl, &self.jelly, self.jelly_rows)?;
        }
        let memory: js_sys::WebAssembly::Memory = wasm_bindgen::memory().unchecked_into();
        let view = js_sys::Float32Array::new_with_byte_offset_and_length(
            &memory.buffer(),
            rows.as_ptr() as u32,
            rows.len() as u32,
        );
        let gl = &self.gl;
        gl.active_texture(Gl::TEXTURE3);
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.jelly));
        gl.tex_sub_image_2d_with_i32_and_i32_and_u32_and_type_and_opt_array_buffer_view(
            Gl::TEXTURE_2D,
            0,
            0,
            0,
            JELLY_ROW as i32,
            count as i32,
            Gl::RED,
            Gl::FLOAT,
            Some(&view),
        )
    }

    fn normal_blend(gl: &Gl) {
        gl.blend_func_separate(
            Gl::ONE,
            Gl::ONE_MINUS_SRC_ALPHA,
            Gl::ONE,
            Gl::ONE_MINUS_SRC_ALPHA,
        );
    }

    fn atlas_texture(gl: &Gl, layers: u32) -> Result<WebGlTexture, JsValue> {
        let atlas = gl.create_texture().ok_or("create_texture failed")?;
        gl.active_texture(Gl::TEXTURE0);
        gl.bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&atlas));
        gl.tex_storage_3d(
            Gl::TEXTURE_2D_ARRAY,
            1,
            Gl::RGBA8,
            ATLAS_SIDE as i32,
            ATLAS_SIDE as i32,
            layers as i32,
        );
        let param = |name, value: u32| gl.tex_parameteri(Gl::TEXTURE_2D_ARRAY, name, value as i32);
        param(Gl::TEXTURE_MIN_FILTER, Gl::LINEAR);
        param(Gl::TEXTURE_MAG_FILTER, Gl::LINEAR);
        param(Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE);
        param(Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE);
        Ok(atlas)
    }

    fn grow_atlas(&mut self, layers: u32) -> Result<(), JsValue> {
        let grown = Self::atlas_texture(&self.gl, layers)?;
        let gl = &self.gl;
        let read = gl.create_framebuffer().ok_or("create_framebuffer failed")?;
        gl.bind_framebuffer(Gl::READ_FRAMEBUFFER, Some(&read));
        for layer in 0..self.atlas_layers as i32 {
            gl.framebuffer_texture_layer(
                Gl::READ_FRAMEBUFFER,
                Gl::COLOR_ATTACHMENT0,
                Some(&self.atlas),
                0,
                layer,
            );
            gl.copy_tex_sub_image_3d(
                Gl::TEXTURE_2D_ARRAY,
                0,
                0,
                0,
                layer,
                0,
                0,
                ATLAS_SIDE as i32,
                ATLAS_SIDE as i32,
            );
        }
        gl.bind_framebuffer(Gl::READ_FRAMEBUFFER, None);
        gl.delete_framebuffer(Some(&read));
        gl.delete_texture(Some(&std::mem::replace(&mut self.atlas, grown)));
        self.atlas_layers = layers;
        Ok(())
    }

    fn sync_atlas(&mut self, atlas: &mut TextAtlas) -> Result<(), JsValue> {
        if atlas.pages() > self.atlas_layers {
            self.grow_atlas(atlas.pages())?;
        }
        let gl = &self.gl;
        gl.active_texture(Gl::TEXTURE0);
        gl.bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.atlas));
        for u in atlas.take_uploads() {
            gl.tex_sub_image_3d_with_image_bitmap(
                Gl::TEXTURE_2D_ARRAY,
                0,
                u.x as i32,
                u.y as i32,
                u.page as i32,
                u.bitmap.width() as i32,
                u.bitmap.height() as i32,
                1,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                &u.bitmap,
            )?;
            u.bitmap.close();
        }
        Ok(())
    }

    fn sync_skins(&mut self, skins: &mut SkinCache) -> Result<(), JsValue> {
        let gl = &self.gl;
        gl.active_texture(Gl::TEXTURE1);
        let reset = self
            .skins
            .as_ref()
            .is_none_or(|t| t.side != skins.side() || t.generation != skins.generation());
        let grown = !reset
            && self
                .skins
                .as_ref()
                .is_some_and(|t| t.capacity != skins.capacity());
        if reset || grown {
            let texture = gl.create_texture().ok_or("create_texture failed")?;
            gl.bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&texture));
            let levels = mip_sides(skins.side()).len() as i32;
            gl.tex_storage_3d(
                Gl::TEXTURE_2D_ARRAY,
                levels,
                Gl::RGBA8,
                skins.side() as i32,
                skins.side() as i32,
                skins.capacity() as i32,
            );
            gl.tex_parameteri(
                Gl::TEXTURE_2D_ARRAY,
                Gl::TEXTURE_MIN_FILTER,
                Gl::LINEAR_MIPMAP_LINEAR as i32,
            );
            gl.tex_parameteri(
                Gl::TEXTURE_2D_ARRAY,
                Gl::TEXTURE_MAG_FILTER,
                Gl::LINEAR as i32,
            );
            gl.tex_parameteri(
                Gl::TEXTURE_2D_ARRAY,
                Gl::TEXTURE_WRAP_S,
                Gl::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameteri(
                Gl::TEXTURE_2D_ARRAY,
                Gl::TEXTURE_WRAP_T,
                Gl::CLAMP_TO_EDGE as i32,
            );
            if let Some(old) = self.skins.take() {
                if grown {
                    let read = gl.create_framebuffer().ok_or("create_framebuffer failed")?;
                    gl.bind_framebuffer(Gl::READ_FRAMEBUFFER, Some(&read));
                    for (level, side) in mip_sides(old.side).into_iter().enumerate() {
                        for layer in skins.all_layers().filter(|&l| l < old.capacity) {
                            gl.framebuffer_texture_layer(
                                Gl::READ_FRAMEBUFFER,
                                Gl::COLOR_ATTACHMENT0,
                                Some(&old.texture),
                                level as i32,
                                layer as i32,
                            );
                            gl.copy_tex_sub_image_3d(
                                Gl::TEXTURE_2D_ARRAY,
                                level as i32,
                                0,
                                0,
                                layer as i32,
                                0,
                                0,
                                side as i32,
                                side as i32,
                            );
                        }
                    }
                    gl.bind_framebuffer(Gl::READ_FRAMEBUFFER, None);
                    gl.delete_framebuffer(Some(&read));
                }
                gl.delete_texture(Some(&old.texture));
            }
            self.skins = Some(SkinTexture {
                texture,
                side: skins.side(),
                capacity: skins.capacity(),
                generation: skins.generation(),
            });
        }
        let tex = self.skins.as_ref().expect("skin texture created above");
        gl.bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&tex.texture));
        for upload in skins.take_uploads() {
            for (level, bitmap) in upload.mips.iter().enumerate() {
                gl.tex_sub_image_3d_with_image_bitmap(
                    Gl::TEXTURE_2D_ARRAY,
                    level as i32,
                    0,
                    0,
                    upload.layer as i32,
                    bitmap.width() as i32,
                    bitmap.height() as i32,
                    1,
                    Gl::RGBA,
                    Gl::UNSIGNED_BYTE,
                    bitmap,
                )?;
                bitmap.close();
            }
        }
        Ok(())
    }

    fn sync_backdrop(&mut self, backdrop: &Backdrop) -> Result<(), JsValue> {
        if self.backdrop_generation == Some(backdrop.generation()) {
            return Ok(());
        }
        self.backdrop_generation = Some(backdrop.generation());
        let gl = &self.gl;
        gl.active_texture(Gl::TEXTURE2);
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.backdrop));
        let Some(mips) = backdrop.mips() else {
            return Ok(());
        };
        for (level, bitmap) in mips.iter().enumerate() {
            gl.tex_image_2d_with_u32_and_u32_and_image_bitmap(
                Gl::TEXTURE_2D,
                level as i32,
                Gl::RGBA as i32,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                bitmap,
            )?;
        }
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MAX_LEVEL, mips.len() as i32 - 1);
        gl.tex_parameteri(
            Gl::TEXTURE_2D,
            Gl::TEXTURE_MIN_FILTER,
            Gl::LINEAR_MIPMAP_LINEAR as i32,
        );
        Ok(())
    }

    fn upload<T: Pod>(gl: &Gl, batch: &mut Batch, items: &[T]) {
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&batch.buffer));
        let bytes: &[u8] = bytemuck::cast_slice(items);
        if items.len() > batch.capacity {
            batch.capacity = items.len().next_power_of_two();
            gl.buffer_data_with_i32(
                Gl::ARRAY_BUFFER,
                (batch.capacity * std::mem::size_of::<T>()) as i32,
                Gl::DYNAMIC_DRAW,
            );
        }
        gl.buffer_sub_data_with_i32_and_u8_array(Gl::ARRAY_BUFFER, 0, bytes);
    }

    fn set_view(
        gl: &Gl,
        u_view: &WebGlUniformLocation,
        u_px: &WebGlUniformLocation,
        view: &FrameView,
    ) {
        gl.uniform4f(
            Some(u_view),
            view.cam_x,
            view.cam_y,
            2.0 * view.scale / view.width as f32,
            2.0 * view.scale / view.height as f32,
        );
        gl.uniform1f(Some(u_px), view.scale);
    }

    pub(crate) fn draw(
        &mut self,
        view: &FrameView,
        layers: &Layers,
        food_changed: bool,
        atlas: &mut TextAtlas,
        skins: &mut SkinCache,
        backdrop: &Backdrop,
    ) -> Result<(), JsValue> {
        self.sync_atlas(atlas)?;
        self.sync_skins(skins)?;
        self.sync_backdrop(backdrop)?;
        self.sync_jelly(layers.jelly)?;
        let gl = &self.gl;
        gl.viewport(0, 0, view.width as i32, view.height as i32);
        let [r, g, b, a] = view.clear_color();
        gl.clear_color(r as f32, g as f32, b as f32, a as f32);
        gl.clear(Gl::COLOR_BUFFER_BIT);
        gl.use_program(Some(&self.program));
        Self::set_view(gl, &self.u_view, &self.u_px, view);
        let lists = layers.prim_lists();
        for (batch, (layer, prims)) in self.batches.iter_mut().zip(lists) {
            if layer == Layer::Over && !layers.overlaps.is_empty() {
                let pass = &mut self.overlap;
                gl.use_program(Some(&pass.program));
                Self::set_view(gl, &pass.u_view, &pass.u_px, view);
                Self::upload(gl, &mut pass.batch, layers.overlaps);
                gl.bind_vertex_array(Some(&pass.batch.vao));
                gl.draw_arrays_instanced(Gl::TRIANGLE_STRIP, 0, 4, layers.overlaps.len() as i32);
                gl.use_program(Some(&self.program));
            }
            if prims.is_empty() {
                continue;
            }
            if layer != Layer::Food || food_changed || batch.capacity == 0 {
                pack_into(prims, &mut self.packed);
                Self::upload(gl, batch, &self.packed);
            }
            gl.bind_vertex_array(Some(&batch.vao));
            if layer.screen_space() {
                Self::set_view(gl, &self.u_view, &self.u_px, &view.screen());
            }
            let clip = layers.clip.filter(|_| layer == Layer::MinimapClip);
            if let Some(c) = clip {
                gl.enable(Gl::SCISSOR_TEST);
                gl.scissor(
                    c.x as i32,
                    view.height as i32 - (c.y + c.h) as i32,
                    c.w as i32,
                    c.h as i32,
                );
            }
            if layer.screen_blend() {
                gl.blend_func_separate(
                    Gl::ONE,
                    Gl::ONE_MINUS_SRC_COLOR,
                    Gl::ONE,
                    Gl::ONE_MINUS_SRC_ALPHA,
                );
            }
            gl.draw_arrays_instanced(Gl::TRIANGLE_STRIP, 0, 4, prims.len() as i32);
            if clip.is_some() {
                gl.disable(Gl::SCISSOR_TEST);
            }
            if layer.screen_blend() {
                Self::normal_blend(gl);
            }
        }
        gl.bind_vertex_array(None);
        Ok(())
    }
}
