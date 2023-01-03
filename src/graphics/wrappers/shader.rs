use std::{
    borrow::Cow,
    ffi::{CStr, CString},
    ptr::{null, null_mut},
};

use anyhow::bail;
use gl::types::{GLchar, GLenum, GLuint};

use crate::{exec::server::draw, graphics::GfxHandle};

use super::{GLHandle, GLHandleContainer, GLHandleTrait};

pub struct ShaderTrait;
pub enum ShaderType {
    Vertex = gl::VERTEX_SHADER as isize,
    Fragment = gl::FRAGMENT_SHADER as isize,
}
pub type Shader = GLHandle<ShaderTrait, ShaderType>;
pub type ShaderContainer = GLHandleContainer<ShaderTrait, ShaderType>;
pub type ShaderHandle = GfxHandle<Shader>;

impl GLHandleTrait<ShaderType> for ShaderTrait {
    fn create(typ: ShaderType) -> GLuint {
        unsafe { gl::CreateShader(typ as GLenum) }
    }

    fn delete(handle: GLuint) {
        unsafe { gl::DeleteShader(handle) }
    }

    fn identifier() -> GLenum {
        gl::SHADER
    }
}
pub struct ProgramTrait;
pub type Program = GLHandle<ProgramTrait, ()>;
pub type ProgramContainer = GLHandleContainer<ProgramTrait, ()>;
pub type ProgramHandle = GfxHandle<Program>;

impl GLHandleTrait for ProgramTrait {
    fn create(_: ()) -> GLuint {
        unsafe { gl::CreateProgram() }
    }

    fn delete(handle: GLuint) {
        unsafe { gl::DeleteProgram(handle) }
    }

    fn identifier() -> GLenum {
        gl::PROGRAM
    }
}

impl Shader {
    pub fn new_sourced(name: &str, typ: ShaderType, source: &str) -> anyhow::Result<Self> {
        let shader = Self::new(name, typ)?;
        unsafe {
            let c_source = CString::new(source)?;
            let ptr = c_source.as_ptr();
            gl::ShaderSource(*shader, 1, &ptr, null());
            gl::CompileShader(*shader);
            let mut status = 0;
            gl::GetShaderiv(*shader, gl::COMPILE_STATUS, &mut status);
            if status == gl::FALSE.into() {
                let mut length = 0;
                gl::GetShaderiv(*shader, gl::INFO_LOG_LENGTH, &mut length);
                let mut buffer = Vec::<u8>::new();
                buffer.resize(length.try_into()?, 0);
                gl::GetShaderInfoLog(
                    *shader,
                    length,
                    null_mut(),
                    buffer.as_mut_ptr() as *mut GLchar,
                );
                let log = CStr::from_bytes_with_nul(buffer.as_slice())
                    .map(|l| l.to_string_lossy())
                    .unwrap_or_else(|_| Cow::Borrowed("unknown error occurred"));
                bail!("unable to compile {}, log: {}", name, log);
            }
        }
        Ok(shader)
    }
}

impl Program {
    pub fn new_vf(name: &str, vertex: &str, fragment: &str) -> anyhow::Result<Self> {
        let program = Self::new_default(name)?;
        let vertex = Shader::new_sourced(
            format!("{name} vertex shader").as_str(),
            ShaderType::Vertex,
            vertex,
        )?;
        let fragment = Shader::new_sourced(
            format!("{name} fragment shader").as_str(),
            ShaderType::Fragment,
            fragment,
        )?;

        unsafe {
            gl::AttachShader(*program, *vertex);
            gl::AttachShader(*program, *fragment);
            gl::LinkProgram(*program);
            gl::ValidateProgram(*program);
            let mut status = 0;
            gl::GetProgramiv(*program, gl::LINK_STATUS, &mut status);
            if status == gl::FALSE.into() {
                let mut length = 0;
                gl::GetProgramiv(*program, gl::INFO_LOG_LENGTH, &mut length);
                let mut buffer = Vec::<u8>::new();
                buffer.resize(length.try_into()?, 0);
                gl::GetProgramInfoLog(
                    *program,
                    length,
                    null_mut(),
                    buffer.as_mut_ptr() as *mut GLchar,
                );
                let log = CStr::from_bytes_with_nul(buffer.as_slice())
                    .map(|l| l.to_string_lossy())
                    .unwrap_or_else(|_| Cow::Borrowed("unknown error occurred"));
                bail!("unable to link {}, log: {}", name, log);
            }
            gl::DetachShader(*program, *vertex);
            gl::DetachShader(*program, *fragment);
        }

        Ok(program)
    }
}

impl ProgramHandle {
    pub fn get(&self, server: &draw::Server) -> Option<GLuint> {
        server.handles.programs.get(self.handle)
    }
}
