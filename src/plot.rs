use glow::*;
use glow::{Context, HasContext, Texture};

const SHADER_VERSION: &str = "#version 140";
const TEXTURE_WIDTH: u32 = 2048;
const TEXTURE_HEIGHT: u32 = 1024;
const NUM_TILES: u32 = 8;
const MAX_HEIGHT: u32 = NUM_TILES * TEXTURE_HEIGHT;

pub struct WaterfallPlot {
    gl: Context,
    pingpong: usize,
    waterfall_fb: Framebuffer,
    waterfall_textures: [Texture; NUM_TILES as usize + 1],
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
    u_y_offset: Option<UniformLocation>,
    y_offset: u32,
    u_resolution: Option<UniformLocation>,
    u_power_offset: Option<UniformLocation>,
    u_power_scale: Option<UniformLocation>,
    // Control variables
    u_cm_offset: Option<UniformLocation>,
    time_position: usize,
    scroll_advance: bool,
}

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
        let waterfall_textures: [Texture; NUM_TILES as usize + 1] = [(); NUM_TILES as usize + 1]
            .map(|_| gl.create_texture().expect("Cannot create texture"));

        let level = 0;
        let internal_format: i32 = glow::RGBA as i32;
        let format: u32 = glow::RGBA;
        let border = 0;
        let ty = glow::UNSIGNED_BYTE;

        for texture in waterfall_textures {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                level,
                internal_format,
                TEXTURE_WIDTH as i32,
                TEXTURE_HEIGHT as i32,
                border,
                format,
                ty,
                None,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);
        }

        // Framebuffer that will be used to render to our waterfall texture
        let waterfall_fb = gl.create_framebuffer().expect("Cannot create framebuffer");

        //
        let window_width: i32 = 1024;
        let window_height: i32 = 1024;
        let power_offset: f32 = 30.0;
        let power_min: f32 = 0.0;
        let power_max: f32 = 100.0;
        let pingpong: usize = 0;
        let time_position: usize = 0;

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
            TEXTURE_WIDTH as f32,
            TEXTURE_HEIGHT as f32,
        );

        let u_waterfall_texture =
            gl.get_uniform_location(waterfall_program.unwrap(), "waterfallTexture");
        gl.uniform_1_i32(u_waterfall_texture.as_ref(), 0);

        let u_samples = gl.get_uniform_location(waterfall_program.unwrap(), "samples");

        let u_y_offset = gl.get_uniform_location(waterfall_program.unwrap(), "yOffset");

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

        let u_cm_offset = gl.get_uniform_location(colormap_program.unwrap(), "offset");
        let u_waterfall_texture_0 =
            gl.get_uniform_location(colormap_program.unwrap(), "waterfallTexture0");
        gl.uniform_1_i32(u_waterfall_texture_0.as_ref(), 0);
        let u_waterfall_texture_1 =
            gl.get_uniform_location(colormap_program.unwrap(), "waterfallTexture1");
        gl.uniform_1_i32(u_waterfall_texture_1.as_ref(), 1);

        gl.clear_color(0.0, 0.0, 0.0, 1.0);

        let y_offset = 0;
        let scroll_advance = true;

        Self {
            gl,
            pingpong,
            waterfall_fb,
            waterfall_textures,
            window_width,
            window_height,
            power_offset,
            power_max,
            power_min,
            waterfall_program,
            colormap_program,
            u_samples,
            u_y_offset,
            y_offset,
            u_resolution,
            u_power_offset,
            u_power_scale,
            u_cm_offset,
            time_position,
            scroll_advance,
        }
    }

    pub unsafe fn drop(&mut self) {
        self.gl.delete_program(self.colormap_program.unwrap());
        self.gl.delete_program(self.waterfall_program.unwrap());
    }

    pub unsafe fn update_plot(&mut self, samples_block: &[f32]) {
        // Update waterfall program logic
        // We want to update (back_texture) with the new FFT data, but we cannot
        // write directly to it. Instead, we use a secondary texture (front_texture)
        // as a target and swap them every frame.
        self.y_offset = (self.y_offset + 1).rem_euclid(MAX_HEIGHT);
        let target_texture = (self.y_offset / TEXTURE_HEIGHT) as usize;
        let ztexture = NUM_TILES as usize;
        let (front_texture, back_texture) = if self.pingpong == 0 {
            (target_texture, ztexture)
        } else {
            (ztexture, target_texture)
        };

        // Update colormap program logic
        // Because we want to support scrolling, we select here the 2 textures that
        // are going to get drawn to the screen: top (cm_tex0) and bottom (cm_tex1).
        // cm_offset controls how much is drawn of each one.
        if self.scroll_advance == false
            && self.time_position < ((NUM_TILES - 1) * TEXTURE_HEIGHT - 1) as usize
        {
            self.time_position = self.time_position + 1;
        }
        let scroll_offset = (self.y_offset as i32 - self.time_position as i32)
            .rem_euclid(MAX_HEIGHT as i32) as usize;
        let cm_offset = scroll_offset.rem_euclid(TEXTURE_HEIGHT as usize);

        let tex_idx0 = (scroll_offset / TEXTURE_HEIGHT as usize) as usize;
        let tex_idx1 = (tex_idx0 as i32 - 1).rem_euclid(NUM_TILES as i32) as usize;
        let cm_tex0 = if tex_idx0 == target_texture {
            front_texture
        } else {
            tex_idx0
        };
        let cm_tex1 = if tex_idx1 == target_texture {
            front_texture
        } else {
            tex_idx1
        };

        // Actual OpenGL calls start here
        let gl = &self.gl;

        // Update waterfall texture first
        gl.use_program(self.waterfall_program);

        // Update samples uniform with current FFT window and time position to update
        gl.uniform_1_f32_slice(self.u_samples.as_ref(), samples_block);
        gl.uniform_1_u32(
            self.u_y_offset.as_ref(),
            self.y_offset.rem_euclid(TEXTURE_HEIGHT),
        );
        // Use the framebuffer to render the updated waterfall to another texture
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.waterfall_fb));
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(
            glow::TEXTURE_2D,
            Some(self.waterfall_textures[back_texture]),
        );
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(self.waterfall_textures[front_texture]),
            0,
        );
        gl.viewport(0, 0, TEXTURE_WIDTH as i32, TEXTURE_HEIGHT as i32);
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        gl.draw_buffer(glow::COLOR_ATTACHMENT0);

        // Draw final scene (only colormap atm)
        gl.use_program(self.colormap_program);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.waterfall_textures[cm_tex0]));
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.waterfall_textures[cm_tex1]));
        gl.uniform_1_u32(self.u_cm_offset.as_ref(), cm_offset as u32);
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

    pub unsafe fn scroll(&mut self, val: i32) {
        self.time_position = (self.time_position as i32 + val)
            .max(0)
            .min((NUM_TILES as i32 - 1) * TEXTURE_HEIGHT as i32 - 1)
            as usize;
        self.scroll_advance = self.time_position == 0;
    }
}
