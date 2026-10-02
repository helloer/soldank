use miniquad::*;

// main rendering pipeline

pub const VERT_SOURCE: &str = r#"#version 100
attribute vec2 in_pos;
attribute vec2 in_texcoords;
attribute vec4 in_color;
uniform mat4 transform;
varying mediump vec2 texcoords;
varying lowp vec4 color;

void main() {
    vec4 c = in_color / 255.0;
    color = vec4(c.rgb * c.a, c.a);
    texcoords = in_texcoords;
    gl_Position = transform * vec4(in_pos, 0.0, 1.0);
}
"#;

pub const FRAG_SOURCE: &str = r#"#version 100
varying mediump vec2 texcoords;
varying lowp vec4 color;
uniform sampler2D sampler;

void main() {
    gl_FragColor = texture2D(sampler, texcoords) * color;
}
"#;

pub fn meta() -> ShaderMeta {
    ShaderMeta {
        images: vec!["sampler".to_string()],
        uniforms: UniformBlockLayout {
            uniforms: vec![UniformDesc::new("transform", UniformType::Mat4)],
        },
    }
}

pub fn attributes() -> [VertexAttribute; 3] {
    [
        VertexAttribute::new("in_pos", VertexFormat::Float2),
        VertexAttribute::new("in_texcoords", VertexFormat::Float2),
        VertexAttribute::new("in_color", VertexFormat::Byte4),
    ]
}

#[repr(C)]
pub struct Uniforms {
    pub transform: glam::Mat4,
}

// premultiplied alpha blending
pub fn params() -> PipelineParams {
    let blend = BlendState::new(
        Equation::Add,
        BlendFactor::One,
        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
    );

    PipelineParams {
        color_blend: Some(blend),
        alpha_blend: Some(blend),
        ..Default::default()
    }
}
