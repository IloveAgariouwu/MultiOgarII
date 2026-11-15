use std::cell::Cell;
use std::mem::offset_of;
use std::rc::Rc;

use bytemuck::Pod;
use js_sys::{JsNullable, Number};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    GpuAdapter, GpuAddressMode, GpuBindGroup, GpuBindGroupDescriptor, GpuBindGroupEntry,
    GpuBlendComponent, GpuBlendFactor, GpuBlendState, GpuBuffer, GpuBufferBinding,
    GpuBufferDescriptor, GpuCanvasAlphaMode, GpuCanvasConfiguration, GpuCanvasContext,
    GpuColorDict, GpuColorTargetState, GpuCopyExternalImageDestInfo,
    GpuCopyExternalImageSourceInfo, GpuDevice, GpuDeviceDescriptor, GpuDeviceLostInfo,
    GpuDeviceLostReason, GpuFilterMode, GpuFragmentState, GpuLoadOp, GpuMipmapFilterMode,
    GpuPrimitiveState, GpuPrimitiveTopology, GpuRenderPassColorAttachment, GpuRenderPassDescriptor,
    GpuRenderPipeline, GpuRenderPipelineDescriptor, GpuSampler, GpuSamplerDescriptor,
    GpuShaderModuleDescriptor, GpuStoreOp, GpuTexelCopyBufferLayout, GpuTexelCopyTextureInfo,
    GpuTexture, GpuTextureDescriptor, GpuTextureDimension, GpuTextureFormat,
    GpuTextureViewDescriptor, GpuTextureViewDimension, GpuUncapturedErrorEvent, GpuVertexAttribute,
    GpuVertexBufferLayout, GpuVertexFormat, GpuVertexState, GpuVertexStepMode, HtmlCanvasElement,
};

use super::backdrop::Backdrop;
use super::jelly::ROW as JELLY_ROW;
use super::overlap::{cover_offset, OverlapMark, COVER_VEC4S, OVERLAP_BYTES};
use super::primitive::{pack_into, shader_source, Instance, ShaderLang, INSTANCE_BYTES};
use super::skin_atlas::{mip_sides, SkinCache, SkinTexture};
use super::text::{TextAtlas, ATLAS_SIDE};
use super::{ContextState, FrameView, Layer, Layers};

const WGSL: &str = include_str!("shaders/sprite.wgsl");
const OVERLAP_WGSL: &str = include_str!("shaders/overlap.wgsl");
const BUFFER_VERTEX: u32 = 0x20;
const BUFFER_UNIFORM: u32 = 0x40;
const BUFFER_COPY_DST: u32 = 0x08;
const TEXTURE_COPY_SRC: u32 = 0x01;
const TEXTURE_COPY_DST: u32 = 0x02;
const TEXTURE_BINDING: u32 = 0x04;
const TEXTURE_RENDER_ATTACHMENT: u32 = 0x10;
const UNIFORM_BYTES: u32 = 32;
const UNCAPTURED_ERROR: &str = "uncapturederror";

struct Batch {
    buffer: Option<GpuBuffer>,
    capacity: usize,
}

pub(crate) struct GpuBackend {
    device: GpuDevice,
    context: GpuCanvasContext,
    pipeline: GpuRenderPipeline,
    uniform: GpuBuffer,
    minimap_uniform: GpuBuffer,
    atlas: GpuTexture,
    atlas_layers: u32,
    backdrop: GpuTexture,
    jelly: GpuTexture,
    jelly_rows: usize,
    backdrop_generation: Option<u32>,
    sampler: GpuSampler,
    skins: Option<SkinTexture<GpuTexture>>,
    bind_group: Option<[GpuBindGroup; 3]>,
    screen_pipeline: GpuRenderPipeline,
    batches: [Batch; Layer::COUNT],
    packed: Vec<Instance>,
    overlap_pipeline: GpuRenderPipeline,
    overlap_group: GpuBindGroup,
    overlap_batch: Batch,
    max_dimension: u32,
    owns_context: bool,
    on_error: Closure<dyn FnMut(GpuUncapturedErrorEvent)>,
}

fn nums(values: &[u32]) -> Vec<Number> {
    values.iter().map(|&v| Number::from(v)).collect()
}

pub(crate) async fn adapter() -> Result<GpuAdapter, JsValue> {
    let navigator = web_sys::window().expect("window").navigator();
    if !js_sys::Reflect::has(&navigator, &"gpu".into()).expect("navigator is an object") {
        return Err("navigator.gpu is missing".into());
    }
    JsFuture::from(navigator.gpu().request_adapter())
        .await?
        .dyn_into()
        .map_err(|_| "requestAdapter returned no adapter".into())
}

fn attribute(format: GpuVertexFormat, offset: usize, location: u32) -> GpuVertexAttribute {
    GpuVertexAttribute::new(format, offset as u32, location)
}

fn blend_state(src: GpuBlendFactor, dst: GpuBlendFactor) -> GpuBlendState {
    let color = GpuBlendComponent::new();
    color.set_src_factor(src);
    color.set_dst_factor(dst);
    let alpha = GpuBlendComponent::new();
    alpha.set_src_factor(GpuBlendFactor::One);
    alpha.set_dst_factor(GpuBlendFactor::OneMinusSrcAlpha);
    GpuBlendState::new(&alpha, &color)
}

fn normal_blend() -> GpuBlendState {
    blend_state(GpuBlendFactor::One, GpuBlendFactor::OneMinusSrcAlpha)
}

fn screen_blend() -> GpuBlendState {
    blend_state(GpuBlendFactor::One, GpuBlendFactor::OneMinusSrc)
}

fn build_pipeline(
    device: &GpuDevice,
    format: GpuTextureFormat,
    source: &str,
    stride: u32,
    attributes: &[GpuVertexAttribute],
    blend: &GpuBlendState,
) -> Result<GpuRenderPipeline, JsValue> {
    let module = device.create_shader_module(&GpuShaderModuleDescriptor::new(source));
    let layout = GpuVertexBufferLayout::new(stride, attributes);
    layout.set_step_mode(GpuVertexStepMode::Instance);
    let vertex = GpuVertexState::new(&module);
    vertex.set_entry_point("vs");
    vertex.set_buffers(&[JsNullable::wrap(layout)]);
    let target = GpuColorTargetState::new(format);
    target.set_blend(blend);
    let fragment = GpuFragmentState::new(&module, &[JsNullable::wrap(target)]);
    fragment.set_entry_point("fs");
    let primitive = GpuPrimitiveState::new();
    primitive.set_topology(GpuPrimitiveTopology::TriangleStrip);
    let descriptor = GpuRenderPipelineDescriptor::new_with_gpu_auto_layout_mode(
        web_sys::GpuAutoLayoutMode::Auto,
        &vertex,
    );
    descriptor.set_fragment(&fragment);
    descriptor.set_primitive(&primitive);
    device.create_render_pipeline(&descriptor)
}

fn sampler(device: &GpuDevice, mipmap: bool) -> GpuSampler {
    let d = GpuSamplerDescriptor::new();
    d.set_mag_filter(GpuFilterMode::Linear);
    d.set_min_filter(GpuFilterMode::Linear);
    d.set_address_mode_u(GpuAddressMode::ClampToEdge);
    d.set_address_mode_v(GpuAddressMode::ClampToEdge);
    if mipmap {
        d.set_mipmap_filter(GpuMipmapFilterMode::Linear);
    }
    device.create_sampler_with_descriptor(&d)
}

impl Drop for GpuBackend {
    fn drop(&mut self) {
        self.device
            .remove_event_listener_with_callback(
                UNCAPTURED_ERROR,
                self.on_error.as_ref().unchecked_ref(),
            )
            .expect("remove uncapturederror listener");
        if self.owns_context {
            self.context.unconfigure();
        }
        self.device.destroy();
    }
}

impl GpuBackend {
    pub(crate) async fn new(
        canvas: &HtmlCanvasElement,
        adapter: GpuAdapter,
        state: &Rc<Cell<ContextState>>,
    ) -> Result<Self, JsValue> {
        let max_dimension = adapter.limits().max_texture_dimension_2d();
        let limits: js_sys::Object<Number> = js_sys::Object::new().unchecked_into();
        js_sys::Reflect::set(
            &limits,
            &"maxTextureDimension2D".into(),
            &max_dimension.into(),
        )?;
        let descriptor = GpuDeviceDescriptor::new();
        descriptor.set_required_limits(&limits);
        let device: GpuDevice = JsFuture::from(adapter.request_device_with_descriptor(&descriptor))
            .await?
            .dyn_into()?;
        let lost = JsFuture::from(device.lost());
        let lost_state = Rc::clone(state);
        wasm_bindgen_futures::spawn_local(async move {
            let info: GpuDeviceLostInfo = lost.await.expect("device lost info").unchecked_into();
            if info.reason() == GpuDeviceLostReason::Destroyed {
                return;
            }
            super::error(&format!("WebGPU device lost: {}", info.message()));
            lost_state.set(ContextState::Lost);
        });
        let on_error =
            Closure::<dyn FnMut(GpuUncapturedErrorEvent)>::new(|e: GpuUncapturedErrorEvent| {
                super::error(&format!("WebGPU error: {}", e.error().message()));
            });
        let navigator = web_sys::window().expect("window").navigator();
        let format = navigator.gpu().get_preferred_canvas_format();
        let sprite_attributes = [
            attribute(GpuVertexFormat::Float32x4, offset_of!(Instance, geom), 0),
            attribute(GpuVertexFormat::Unorm16x4, offset_of!(Instance, aux), 1),
            attribute(GpuVertexFormat::Unorm8x4, offset_of!(Instance, color0), 2),
            attribute(GpuVertexFormat::Unorm8x4, offset_of!(Instance, color1), 3),
            attribute(GpuVertexFormat::Float32, offset_of!(Instance, scalar), 4),
            attribute(GpuVertexFormat::Uint32, offset_of!(Instance, head), 5),
        ];
        let sprite_source = shader_source(ShaderLang::Wgsl, WGSL);
        let pipeline = build_pipeline(
            &device,
            format,
            &sprite_source,
            INSTANCE_BYTES,
            &sprite_attributes,
            &normal_blend(),
        )?;
        let screen_pipeline = build_pipeline(
            &device,
            format,
            &sprite_source,
            INSTANCE_BYTES,
            &sprite_attributes,
            &screen_blend(),
        )?;
        let mut overlap_attributes = vec![
            attribute(GpuVertexFormat::Float32x3, offset_of!(OverlapMark, x), 0),
            attribute(GpuVertexFormat::Unorm8x4, offset_of!(OverlapMark, color), 1),
        ];
        overlap_attributes.extend(
            (0..COVER_VEC4S)
                .map(|k| attribute(GpuVertexFormat::Float32x4, cover_offset(k), 2 + k as u32)),
        );
        let overlap_pipeline = build_pipeline(
            &device,
            format,
            &shader_source(ShaderLang::Wgsl, OVERLAP_WGSL),
            OVERLAP_BYTES,
            &overlap_attributes,
            &normal_blend(),
        )?;

        let uniform = device.create_buffer(&GpuBufferDescriptor::new(
            UNIFORM_BYTES,
            BUFFER_UNIFORM | BUFFER_COPY_DST,
        ))?;
        let minimap_uniform = device.create_buffer(&GpuBufferDescriptor::new(
            UNIFORM_BYTES,
            BUFFER_UNIFORM | BUFFER_COPY_DST,
        ))?;
        let overlap_group = device.create_bind_group(&GpuBindGroupDescriptor::new(
            &[GpuBindGroupEntry::new_with_gpu_buffer_binding(
                0,
                &GpuBufferBinding::new(&uniform),
            )],
            &overlap_pipeline.get_bind_group_layout(0),
        ));
        let atlas = Self::atlas_texture(&device, 1)?;
        let sampler = sampler(&device, true);
        let backdrop = device.create_texture(&GpuTextureDescriptor::new(
            GpuTextureFormat::Rgba8unorm,
            &nums(&[1, 1]),
            TEXTURE_BINDING | TEXTURE_COPY_DST | TEXTURE_RENDER_ATTACHMENT,
        ))?;
        let jelly = Self::jelly_texture(&device, 1)?;
        let context: GpuCanvasContext = canvas
            .get_context("webgpu")?
            .ok_or("webgpu context unavailable")?
            .dyn_into()?;
        let config = GpuCanvasConfiguration::new(&device, format);
        config.set_alpha_mode(GpuCanvasAlphaMode::Opaque);
        context.configure(&config)?;
        device.add_event_listener_with_callback(
            UNCAPTURED_ERROR,
            on_error.as_ref().unchecked_ref(),
        )?;
        Ok(Self {
            device,
            context,
            pipeline,
            screen_pipeline,
            uniform,
            minimap_uniform,
            atlas,
            atlas_layers: 1,
            backdrop,
            backdrop_generation: None,
            jelly,
            jelly_rows: 1,
            sampler,
            skins: None,
            bind_group: None,
            batches: std::array::from_fn(|_| Batch {
                buffer: None,
                capacity: 0,
            }),
            packed: Vec::new(),
            overlap_pipeline,
            overlap_group,
            overlap_batch: Batch {
                buffer: None,
                capacity: 0,
            },
            max_dimension,
            owns_context: true,
            on_error,
        })
    }

    pub(crate) fn max_dimension(&self) -> u32 {
        self.max_dimension
    }

    pub(crate) fn release_context(&mut self) {
        self.owns_context = false;
    }

    fn atlas_texture(device: &GpuDevice, layers: u32) -> Result<GpuTexture, JsValue> {
        let d = GpuTextureDescriptor::new(
            GpuTextureFormat::Rgba8unorm,
            &nums(&[ATLAS_SIDE, ATLAS_SIDE, layers]),
            TEXTURE_BINDING | TEXTURE_COPY_SRC | TEXTURE_COPY_DST | TEXTURE_RENDER_ATTACHMENT,
        );
        d.set_dimension(GpuTextureDimension::N2d);
        device.create_texture(&d)
    }

    fn grow_atlas(&mut self, layers: u32) -> Result<(), JsValue> {
        let grown = Self::atlas_texture(&self.device, layers)?;
        let encoder = self.device.create_command_encoder();
        encoder.copy_texture_to_texture_with_u32_sequence(
            &GpuTexelCopyTextureInfo::new(&self.atlas),
            &GpuTexelCopyTextureInfo::new(&grown),
            &nums(&[ATLAS_SIDE, ATLAS_SIDE, self.atlas_layers]),
        )?;
        self.device.queue().submit(&[encoder.finish()]);
        std::mem::replace(&mut self.atlas, grown).destroy();
        self.atlas_layers = layers;
        self.bind_group = None;
        Ok(())
    }

    fn sync_atlas(&mut self, atlas: &mut TextAtlas) -> Result<(), JsValue> {
        if atlas.pages() > self.atlas_layers {
            self.grow_atlas(atlas.pages())?;
        }
        let queue = self.device.queue();
        for u in atlas.take_uploads() {
            let dest = GpuCopyExternalImageDestInfo::new(&self.atlas);
            dest.set_origin(&nums(&[u.x, u.y, u.page]));
            dest.set_premultiplied_alpha(true);
            queue.copy_external_image_to_texture_with_u32_sequence(
                &GpuCopyExternalImageSourceInfo::new(&u.bitmap),
                &dest,
                &nums(&[u.bitmap.width(), u.bitmap.height()]),
            )?;
            u.bitmap.close();
        }
        Ok(())
    }

    fn sync_skins(&mut self, skins: &mut SkinCache) -> Result<(), JsValue> {
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
            let d = GpuTextureDescriptor::new(
                GpuTextureFormat::Rgba8unorm,
                &nums(&[skins.side(), skins.side(), skins.capacity()]),
                TEXTURE_BINDING | TEXTURE_COPY_SRC | TEXTURE_COPY_DST | TEXTURE_RENDER_ATTACHMENT,
            );
            d.set_dimension(GpuTextureDimension::N2d);
            d.set_mip_level_count(mip_sides(skins.side()).len() as u32);
            let texture = self.device.create_texture(&d)?;
            if let Some(old) = self.skins.take() {
                if grown {
                    let encoder = self.device.create_command_encoder();
                    for (level, side) in mip_sides(old.side).into_iter().enumerate() {
                        let src = GpuTexelCopyTextureInfo::new(&old.texture);
                        src.set_mip_level(level as u32);
                        let dst = GpuTexelCopyTextureInfo::new(&texture);
                        dst.set_mip_level(level as u32);
                        encoder.copy_texture_to_texture_with_u32_sequence(
                            &src,
                            &dst,
                            &nums(&[side, side, old.capacity]),
                        )?;
                    }
                    self.device.queue().submit(&[encoder.finish()]);
                }
                old.texture.destroy();
            }
            self.skins = Some(SkinTexture {
                texture,
                side: skins.side(),
                capacity: skins.capacity(),
                generation: skins.generation(),
            });
            self.bind_group = None;
        }
        let tex = self.skins.as_ref().expect("skin texture created above");
        let queue = self.device.queue();
        for upload in skins.take_uploads() {
            for (level, bitmap) in upload.mips.iter().enumerate() {
                let dest = GpuCopyExternalImageDestInfo::new(&tex.texture);
                dest.set_mip_level(level as u32);
                dest.set_premultiplied_alpha(true);
                dest.set_origin(&nums(&[0, 0, upload.layer]));
                queue.copy_external_image_to_texture_with_u32_sequence(
                    &GpuCopyExternalImageSourceInfo::new(bitmap),
                    &dest,
                    &nums(&[bitmap.width(), bitmap.height()]),
                )?;
                bitmap.close();
            }
        }
        Ok(())
    }

    fn jelly_texture(device: &GpuDevice, rows: usize) -> Result<GpuTexture, JsValue> {
        device.create_texture(&GpuTextureDescriptor::new(
            GpuTextureFormat::R32float,
            &nums(&[JELLY_ROW as u32, rows as u32]),
            TEXTURE_BINDING | TEXTURE_COPY_DST,
        ))
    }

    fn sync_jelly(&mut self, rows: &[f32]) -> Result<(), JsValue> {
        let count = rows.len() / JELLY_ROW;
        if count == 0 {
            return Ok(());
        }
        if count > self.jelly_rows {
            self.jelly_rows = count.next_power_of_two();
            let texture = Self::jelly_texture(&self.device, self.jelly_rows)?;
            std::mem::replace(&mut self.jelly, texture).destroy();
            self.bind_group = None;
        }
        let layout = GpuTexelCopyBufferLayout::new();
        layout.set_bytes_per_row((JELLY_ROW * 4) as u32);
        self.device
            .queue()
            .write_texture_with_u8_slice_and_u32_sequence(
                &GpuTexelCopyTextureInfo::new(&self.jelly),
                bytemuck::cast_slice(rows),
                &layout,
                &nums(&[JELLY_ROW as u32, count as u32]),
            )
    }

    fn sync_backdrop(&mut self, backdrop: &Backdrop) -> Result<(), JsValue> {
        if self.backdrop_generation == Some(backdrop.generation()) {
            return Ok(());
        }
        self.backdrop_generation = Some(backdrop.generation());
        let Some(mips) = backdrop.mips() else {
            return Ok(());
        };
        let first = &mips[0];
        let d = GpuTextureDescriptor::new(
            GpuTextureFormat::Rgba8unorm,
            &nums(&[first.width(), first.height()]),
            TEXTURE_BINDING | TEXTURE_COPY_DST | TEXTURE_RENDER_ATTACHMENT,
        );
        d.set_mip_level_count(mips.len() as u32);
        let texture = self.device.create_texture(&d)?;
        let queue = self.device.queue();
        for (level, bitmap) in mips.iter().enumerate() {
            let dest = GpuCopyExternalImageDestInfo::new(&texture);
            dest.set_mip_level(level as u32);
            dest.set_premultiplied_alpha(true);
            queue.copy_external_image_to_texture_with_u32_sequence(
                &GpuCopyExternalImageSourceInfo::new(bitmap),
                &dest,
                &nums(&[bitmap.width(), bitmap.height()]),
            )?;
        }
        std::mem::replace(&mut self.backdrop, texture).destroy();
        self.bind_group = None;
        Ok(())
    }

    fn bind_groups(&mut self) -> Result<[GpuBindGroup; 3], JsValue> {
        if let Some(g) = &self.bind_group {
            return Ok(g.clone());
        }
        let skins = self.skins.as_ref().expect("skin texture created in sync");
        let array = GpuTextureViewDescriptor::new();
        array.set_dimension(GpuTextureViewDimension::N2dArray);
        let atlas = self.atlas.create_view_with_descriptor(&array)?;
        let skin_view = skins.texture.create_view_with_descriptor(&array)?;
        let backdrop = self.backdrop.create_view()?;
        let jelly = self.jelly.create_view()?;
        let group = |uniform: &GpuBuffer, pipeline: &GpuRenderPipeline| {
            let binding = GpuBufferBinding::new(uniform);
            let entries = [
                GpuBindGroupEntry::new_with_gpu_buffer_binding(0, &binding),
                GpuBindGroupEntry::new_with_gpu_texture_view(1, &atlas),
                GpuBindGroupEntry::new(2, &self.sampler),
                GpuBindGroupEntry::new_with_gpu_texture_view(3, &skin_view),
                GpuBindGroupEntry::new(4, &self.sampler),
                GpuBindGroupEntry::new_with_gpu_texture_view(5, &backdrop),
                GpuBindGroupEntry::new_with_gpu_texture_view(6, &jelly),
            ];
            self.device.create_bind_group(&GpuBindGroupDescriptor::new(
                &entries,
                &pipeline.get_bind_group_layout(0),
            ))
        };
        let groups = [
            group(&self.uniform, &self.pipeline),
            group(&self.uniform, &self.screen_pipeline),
            group(&self.minimap_uniform, &self.pipeline),
        ];
        self.bind_group = Some(groups.clone());
        Ok(groups)
    }

    fn upload<T: Pod>(device: &GpuDevice, batch: &mut Batch, items: &[T]) -> Result<(), JsValue> {
        if items.len() > batch.capacity {
            if let Some(old) = batch.buffer.take() {
                old.destroy();
            }
            batch.capacity = items.len().next_power_of_two();
            batch.buffer = Some(device.create_buffer(&GpuBufferDescriptor::new(
                (batch.capacity * std::mem::size_of::<T>()) as u32,
                BUFFER_VERTEX | BUFFER_COPY_DST,
            ))?);
        }
        let buffer = batch.buffer.as_ref().expect("buffer allocated above");
        device.queue().write_buffer_with_u32_and_u8_slice(
            buffer,
            0,
            bytemuck::cast_slice(items),
        )?;
        Ok(())
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
        let uniforms = |v: &FrameView| -> [f32; 8] {
            [
                v.cam_x,
                v.cam_y,
                2.0 * v.scale / v.width as f32,
                2.0 * v.scale / v.height as f32,
                v.scale,
                0.0,
                0.0,
                0.0,
            ]
        };
        let queue = self.device.queue();
        queue.write_buffer_with_u32_and_u8_slice(
            &self.uniform,
            0,
            bytemuck::cast_slice(&uniforms(view)),
        )?;
        queue.write_buffer_with_u32_and_u8_slice(
            &self.minimap_uniform,
            0,
            bytemuck::cast_slice(&uniforms(&view.screen())),
        )?;
        let lists = layers.prim_lists();
        for (batch, (layer, prims)) in self.batches.iter_mut().zip(lists) {
            if !prims.is_empty() && (layer != Layer::Food || food_changed || batch.buffer.is_none())
            {
                pack_into(prims, &mut self.packed);
                Self::upload(&self.device, batch, &self.packed)?;
            }
        }
        if !layers.overlaps.is_empty() {
            Self::upload(&self.device, &mut self.overlap_batch, layers.overlaps)?;
        }
        let [group, screen_group, minimap_group] = self.bind_groups()?;
        let target = self.context.get_current_texture()?.create_view()?;
        let attachment = GpuRenderPassColorAttachment::new_with_gpu_texture_view(
            GpuLoadOp::Clear,
            GpuStoreOp::Store,
            &target,
        );
        let [r, g, b, a] = view.clear_color();
        let clear = GpuColorDict::new(a, b, g, r);
        attachment.set_clear_value_gpu_color_dict(&clear);
        let encoder = self.device.create_command_encoder();
        let pass =
            encoder.begin_render_pass(&GpuRenderPassDescriptor::new(&[JsNullable::wrap(
                attachment,
            )]))?;
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, Some(&group));
        for (batch, (layer, prims)) in self.batches.iter().zip(lists) {
            if let (Layer::Over, Some(buffer), false) = (
                layer,
                &self.overlap_batch.buffer,
                layers.overlaps.is_empty(),
            ) {
                pass.set_pipeline(&self.overlap_pipeline);
                pass.set_bind_group(0, Some(&self.overlap_group));
                pass.set_vertex_buffer(0, Some(buffer));
                pass.draw_with_instance_count(4, layers.overlaps.len() as u32);
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, Some(&group));
            }
            if let (Some(buffer), false) = (&batch.buffer, prims.is_empty()) {
                if layer.screen_space() {
                    pass.set_bind_group(0, Some(&minimap_group));
                }
                let clip = layers.clip.filter(|_| layer == Layer::MinimapClip);
                if let Some(c) = clip {
                    let x = c.x.min(view.width);
                    let y = c.y.min(view.height);
                    pass.set_scissor_rect(x, y, c.w.min(view.width - x), c.h.min(view.height - y));
                }
                if layer.screen_blend() {
                    pass.set_pipeline(&self.screen_pipeline);
                    pass.set_bind_group(0, Some(&screen_group));
                }
                pass.set_vertex_buffer(0, Some(buffer));
                pass.draw_with_instance_count(4, prims.len() as u32);
                if clip.is_some() {
                    pass.set_scissor_rect(0, 0, view.width, view.height);
                }
                if layer.screen_blend() {
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, Some(&group));
                }
            }
        }
        pass.end();
        queue.submit(&[encoder.finish()]);
        Ok(())
    }
}
