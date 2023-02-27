use glow::*;
use glow::{Context, HasContext, Texture};

pub struct WaterfallPlot {
    gl: Context,
    pingpong: usize,
    waterfall_fb: Framebuffer,
    waterfall_textures: [Texture; 2],
    texture_width: i32,
    texture_height: i32,
    window_width: i32,
    window_height: i32,
    power_offset: f32,
    power_max: f32,
    power_min: f32,
    // Programs
    waterfall_program: Option<NativeProgram>,
    colormap_program: Option<NativeProgram>,
    // Uniforms
    u_samples: Option<UniformLocation>,
    u_resolution: Option<UniformLocation>,
    u_power_offset: Option<UniformLocation>,
    u_power_scale: Option<UniformLocation>,
}

const SHADER_VERSION: &str = "#version 140";

impl WaterfallPlot {
    unsafe fn create_program(
        gl: &glow::Context,
        vertex_src: &str,
        fragment_src: &str,
    ) -> Option<NativeProgram> {
        let program: NativeProgram = gl.create_program().expect("Cannot create program");

        let shader_sources = [
            (glow::VERTEX_SHADER, vertex_src),
            (glow::FRAGMENT_SHADER, fragment_src),
        ];

        let mut shaders = Vec::with_capacity(shader_sources.len());

        for (shader_type, shader_source) in shader_sources.iter() {
            let shader = gl
                .create_shader(*shader_type)
                .expect("Cannot create shader");
            gl.shader_source(shader, &format!("{}\n{}", SHADER_VERSION, shader_source));
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                panic!("{}", gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }

        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            panic!("{}", gl.get_program_info_log(program));
        }

        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }

        Some(program)
    }

    pub unsafe fn new(gl: Context) -> Self {
        // Create a pair of textures that will be used to create our waterfall
        // - One serves as destination for the framebuffer render
        // - The other is the last rendered texture, that we use to copy from
        // We will switch roles between them every new frame
        let waterfall_textures: [Texture; 2] = [
            gl.create_texture().expect("Cannot create texture"),
            gl.create_texture().expect("Cannot create texture"),
        ];

        let level = 0;
        let internal_format: i32 = glow::RGBA as i32;
        let format: u32 = glow::RGBA;
        let texture_width = 2048;
        let texture_height = 768;
        let border = 0;
        let ty = glow::UNSIGNED_BYTE;

        for texture in waterfall_textures {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                level,
                internal_format,
                texture_width,
                texture_height,
                border,
                format,
                ty,
                None,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);
        }

        // Framebuffer that will be used to render to our waterfall texture
        let waterfall_fb = gl.create_framebuffer().expect("Cannot create framebuffer");

        //
        let window_width: i32 = 1024;
        let window_height: i32 = 768;
        let power_offset: f32 = 30.0;
        let power_min: f32 = 0.0;
        let power_max: f32 = 100.0;
        let pingpong: usize = 0;

        // Create GPU pipelines for:
        // 1. Creation of the waterfall texture
        // 2. Application of the color map
        let vs_quad_src: &str = include_str!("shaders/vs_quad.glsl");
        let fs_waterfall_src: &str = include_str!("shaders/fs_waterfall.glsl");
        let fs_colormap_src: &str = concat!(
            include_str!("shaders/turbo_colormap.glsl"),
            include_str!("shaders/fs_colormap.glsl"),
        );

        // Define program 1 and its uniforms
        let waterfall_program = WaterfallPlot::create_program(&gl, vs_quad_src, fs_waterfall_src);
        gl.use_program(waterfall_program);

        let u_power_offset =
            gl.get_uniform_location(waterfall_program.unwrap(), "waterfallScaleAdd");
        let u_power_scale =
            gl.get_uniform_location(waterfall_program.unwrap(), "waterfallScaleMult");
        gl.uniform_1_f32(u_power_offset.as_ref(), power_offset);
        gl.uniform_1_f32(u_power_scale.as_ref(), 1.0 / (power_max - power_min).abs());

        let u_resolution = gl.get_uniform_location(waterfall_program.unwrap(), "resolution");
        gl.uniform_2_f32(
            u_resolution.as_ref(),
            texture_width as f32,
            texture_height as f32,
        );

        let u_waterfall_texture =
            gl.get_uniform_location(waterfall_program.unwrap(), "waterfallTexture");
        gl.uniform_1_i32(u_waterfall_texture.as_ref(), 0);

        let u_samples = gl.get_uniform_location(waterfall_program.unwrap(), "samples");

        gl.bind_framebuffer(glow::FRAMEBUFFER, None);

        // Define program 2 and its uniforms
        let colormap_program = WaterfallPlot::create_program(&gl, vs_quad_src, fs_colormap_src);
        gl.use_program(colormap_program);

        let u_resolution = gl.get_uniform_location(colormap_program.unwrap(), "resolution");
        gl.uniform_2_f32(
            u_resolution.as_ref(),
            window_width as f32,
            window_height as f32,
        );

        let u_waterfall_texture =
            gl.get_uniform_location(colormap_program.unwrap(), "waterfallTexture");
        gl.uniform_1_i32(u_waterfall_texture.as_ref(), 0);

        gl.clear_color(0.0, 0.0, 0.0, 1.0);

        Self {
            gl,
            pingpong,
            waterfall_fb,
            waterfall_textures,
            texture_width,
            texture_height,
            window_width,
            window_height,
            power_offset,
            power_max,
            power_min,
            waterfall_program,
            colormap_program,
            u_samples,
            u_resolution,
            u_power_offset,
            u_power_scale,
        }
    }

    pub unsafe fn drop(&mut self) {
        self.gl.delete_program(self.colormap_program.unwrap());
        self.gl.delete_program(self.waterfall_program.unwrap());
    }

    pub unsafe fn update_plot(&mut self, samples_block: &[f32]) {
        let front_texture = Some(self.waterfall_textures[self.pingpong]);
        let back_texture = Some(self.waterfall_textures[self.pingpong ^ 1]);

        let gl = &self.gl;

        // Update waterfall texture first
        gl.use_program(self.waterfall_program);

        gl.uniform_1_f32_slice(self.u_samples.as_ref(), samples_block);

        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.waterfall_fb));
        gl.bind_texture(glow::TEXTURE_2D, back_texture);
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            front_texture,
            0,
        );
        gl.viewport(0, 0, self.texture_width, self.texture_height);
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        gl.draw_buffer(glow::COLOR_ATTACHMENT0);

        // Draw final scene (only colormap atm)
        gl.use_program(self.colormap_program);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.bind_texture(glow::TEXTURE_2D, front_texture);
        gl.viewport(0, 0, self.window_width, self.window_height);
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

        self.pingpong ^= 1;
    }

    pub unsafe fn set_window_size(&mut self, width: u32, height: u32) {
        let gl = &self.gl;
        self.window_width = width as i32;
        self.window_height = height as i32;
        gl.use_program(self.colormap_program);
        gl.uniform_2_f32(self.u_resolution.as_ref(), width as f32, height as f32);
    }

    pub unsafe fn incr_offset(&mut self, val: f32) {
        let gl = &self.gl;
        self.power_offset += val;
        gl.use_program(self.waterfall_program);
        gl.uniform_1_f32(self.u_power_offset.as_ref(), self.power_offset);
    }

    pub unsafe fn incr_max(&mut self, val: f32) {
        let gl = &self.gl;
        self.power_max += val;
        gl.use_program(self.waterfall_program);
        gl.uniform_1_f32(
            self.u_power_scale.as_ref(),
            1.0 / (self.power_max - self.power_min).abs(),
        );
    }

    pub unsafe fn incr_min(&mut self, val: f32) {
        let gl = &self.gl;
        self.power_min += val;
        gl.use_program(self.waterfall_program);
        gl.uniform_1_f32(
            self.u_power_scale.as_ref(),
            1.0 / (self.power_max - self.power_min).abs(),
        );
    }
}
