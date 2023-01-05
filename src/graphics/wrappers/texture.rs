use gl::types::{GLenum, GLuint};

use crate::exec::server::draw;

use super::{GLGfxHandle, GLHandle, GLHandleContainer, GLHandleTrait, SendGLHandleContainer};

pub struct TextureTrait;
pub type Texture = GLHandle<TextureTrait>;
pub type TextureContainer = GLHandleContainer<TextureTrait>;
pub type SendTextureContainer = SendGLHandleContainer<TextureTrait>;
pub type TextureHandle = GLGfxHandle<TextureTrait>;

impl GLHandleTrait for TextureTrait {
    fn create(_: ()) -> GLuint {
        let mut handle = 0;
        unsafe { gl::GenTextures(1, &mut handle) };
        handle
    }

    fn delete(handle: GLuint) {
        Self::delete_mul(&[handle])
    }

    fn identifier() -> GLenum {
        gl::TEXTURE
    }

    fn delete_mul(handles: &[GLuint]) {
        unsafe { gl::DeleteTextures(handles.len().try_into().unwrap(), handles.as_ptr()) }
    }

    fn get_container_mut(server: &mut draw::Server) -> Option<&mut GLHandleContainer<Self, ()>>
    {
        Some(&mut server.handles.textures)
    }

    fn get_container(server: &draw::Server) -> Option<&GLHandleContainer<Self, ()>>
    {
        Some(&server.handles.textures)
    }
}
