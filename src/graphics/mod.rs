use std::{borrow::Cow, hash::Hash, marker::PhantomData};

use crate::utils::uid::Uid;

use self::wrappers::{
    buffer::{BufferContainer, SendBufferContainer},
    framebuffer::{Framebuffer, FramebufferContainer, FramebufferHandle, SendFramebufferContainer},
    shader::{Program, ProgramContainer, ProgramHandle, SendProgramContainer},
    texture::{SendTextureContainer, TextureContainer},
    vertex_array::{
        SendVertexArrayContainer, VertexArray, VertexArrayContainer, VertexArrayHandle,
    },
};

pub mod blur;
pub mod context;
pub mod debug_callback;
pub mod quad_renderer;
pub mod transform_stack;
pub mod wrappers;

#[derive(Debug)]
pub struct GfxHandle<T> {
    pub handle: Uid,
    data: PhantomData<fn() -> T>,
}

impl<T> GfxHandle<T> {
    pub fn from_handle(handle: u64) -> Self {
        Self {
            handle: Uid::from_raw(handle),
            data: PhantomData,
        }
    }

    pub fn new() -> Self {
        Self {
            handle: Uid::new(),
            data: PhantomData,
        }
    }
}

impl<T> Hash for GfxHandle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.handle.hash(state);
    }
}

impl<T> PartialEq for GfxHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl<T> Eq for GfxHandle<T> {}

impl<T> Clone for GfxHandle<T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            data: self.data,
        }
    }
}

impl<T> Copy for GfxHandle<T> {}

impl<T> Default for GfxHandle<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct HandleContainer {
    pub vertex_arrays: VertexArrayContainer,
    pub buffers: BufferContainer,
    pub textures: TextureContainer,
    pub programs: ProgramContainer,
    pub framebuffers: FramebufferContainer,
}

#[derive(Default)]
pub struct SendHandleContainer {
    vertex_arrays: SendVertexArrayContainer,
    buffers: SendBufferContainer,
    textures: SendTextureContainer,
    programs: SendProgramContainer,
    framebuffers: SendFramebufferContainer,
}

impl HandleContainer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_vertex_array(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        handle: &VertexArrayHandle,
    ) -> anyhow::Result<VertexArray> {
        VertexArray::new(name).map(|v| self.vertex_arrays.insert(handle, v))
    }

    // pub fn create_buffer(
    //     &mut self,
    //     name: impl Into<Cow<'static, str>>,
    //     handle: &BufferHandle,
    // ) -> anyhow::Result<Buffer> {
    //     Buffer::new(name).map(|b| self.buffers.insert(handle, b))
    // }

    // pub fn create_texture(
    //     &mut self,
    //     name: impl Into<Cow<'static, str>>,
    //     handle: &TextureHandle,
    // ) -> anyhow::Result<Texture> {
    //     Texture::new(name).map(|t| self.textures.insert(handle, t))
    // }

    pub fn create_vf_program(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        handle: &ProgramHandle,
        vertex: &str,
        fragment: &str,
    ) -> anyhow::Result<Program> {
        let program = Program::new(name.into()).map(|p| self.programs.insert(handle, p))?;
        program.init_vf(vertex, fragment)?;
        Ok(program)
    }

    pub fn create_framebuffer(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        handle: &FramebufferHandle,
    ) -> anyhow::Result<Framebuffer> {
        Framebuffer::new(name).map(|f| self.framebuffers.insert(handle, f))
    }

    pub fn to_send(self) -> SendHandleContainer {
        SendHandleContainer {
            vertex_arrays: self.vertex_arrays.to_send(),
            buffers: self.buffers.to_send(),
            textures: self.textures.to_send(),
            programs: self.programs.to_send(),
            framebuffers: self.framebuffers.to_send(),
        }
    }
}

impl SendHandleContainer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn to_nonsend(self) -> HandleContainer {
        HandleContainer {
            vertex_arrays: self.vertex_arrays.to_nonsend(),
            buffers: self.buffers.to_nonsend(),
            textures: self.textures.to_nonsend(),
            programs: self.programs.to_nonsend(),
            framebuffers: self.framebuffers.to_nonsend(),
        }
    }
}
