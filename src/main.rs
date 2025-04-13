use egui::*;
use egui_backend::{
    egui::{self, ClippedPrimitive},
    epi::{Frame, IntegrationInfo},
    get_frame_time, gl, sdl2,
    sdl2::event::Event,
    sdl2::video::GLProfile,
    sdl2::video::SwapInterval,
    DpiScaling, ShaderVersion, Signal,
};

use std::{fs, os::unix::raw::time_t, sync::Arc, time::Instant};


use epi::backend::FrameData;
use glm::{vec3, Vec3, Vector3};
use sdl2::{event::WindowEvent, keyboard::Keycode, sys::u_int};
// Alias the backend to something less mouthful
use egui_sdl2_gl::{self as egui_backend, painter::{compile_shader, link_program}};
use gl::types::*;
use std::ptr;
use std::ffi::CString;
mod window_manager;
use window_manager::{window_manager::windows::{MainWindow, SandboxWindow}, *};
use crate::window_manager::window_manager::windows::ShaderType;

// Voxel types
#[derive(Clone, Copy, PartialEq)]
enum VoxelType {
    Air,
    Dirt,
    Grass,
    Stone,
    Wood,
    Leaves,
    Light,  // New light block type
}

// Voxel data structure
struct Voxel {
    voxel_type: VoxelType,
}

// Chunk data structure (16x16x16 voxels)
struct Chunk {
    voxels: Vec<Voxel>,
    position: (i32, i32, i32), // Chunk position in world
}

impl Chunk {
    fn new(position: (i32, i32, i32)) -> Self {
        let mut voxels = Vec::with_capacity(16 * 16 * 16);
        for _ in 0..16 * 16 * 16 {
            voxels.push(Voxel { voxel_type: VoxelType::Air });
        }
        Self { voxels, position }
    }

    fn get_voxel(&self, x: usize, y: usize, z: usize) -> &Voxel {
        if x < 16 && y < 16 && z < 16 {
            &self.voxels[y * 16 * 16 + z * 16 + x]
        } else {
            &self.voxels[0] // Return air for out of bounds
        }
    }

    fn set_voxel(&mut self, x: usize, y: usize, z: usize, voxel_type: VoxelType) {
        if x < 16 && y < 16 && z < 16 {
            self.voxels[y * 16 * 16 + z * 16 + x] = Voxel { voxel_type };
        }
    }
}

// World data structure
struct World {
    chunks: Vec<Chunk>,
}

impl World {
    fn new() -> Self {
        let mut world = Self { chunks: Vec::new() };
        // Create a 3x3 grid of chunks on the same Y level (y=0)
        for x in -1..=1 {
            for z in -1..=1 {
                let mut chunk = Chunk::new((x, 0, z));
                // Generate some terrain
                for cx in 0..16 {
                    for cz in 0..16 {
                        // Calculate absolute world position
                        let world_x = cx as f32 + (x * 16) as f32;
                        let world_z = cz as f32 + (z * 16) as f32;
                        
                        // Generate height using world coordinates
                        let height = 4.0 + (world_x * 0.1).sin() * 1.0 + (world_z * 0.1).cos() * 1.0;  // Reduced height variation
                        
                        for cy in 0..16 {
                            let cy_f32 = cy as f32;
                            if cy_f32 <= height {
                                if cy_f32 > height - 1.0 {
                                    chunk.set_voxel(cx, cy, cz, VoxelType::Grass);
                                } else if cy_f32 > height - 3.0 {  // Reduced dirt layer
                                    chunk.set_voxel(cx, cy, cz, VoxelType::Dirt);
                                } else {
                                    chunk.set_voxel(cx, cy, cz, VoxelType::Stone);
                                }
                            }
                        }
                    }
                }
                world.chunks.push(chunk);
            }
        }
        world
    }

    fn get_voxel(&self, x: i32, y: i32, z: i32) -> VoxelType {
        // Calculate chunk coordinates
        let chunk_x = (x as f32 / 16.0).floor() as i32;
        let chunk_y = (y as f32 / 16.0).floor() as i32;
        let chunk_z = (z as f32 / 16.0).floor() as i32;
        
        // Calculate local coordinates within the chunk
        let local_x = x.rem_euclid(16) as usize;
        let local_y = y.rem_euclid(16) as usize;
        let local_z = z.rem_euclid(16) as usize;
        
        // Find the chunk
        for chunk in &self.chunks {
            if chunk.position == (chunk_x, chunk_y, chunk_z) {
                return chunk.get_voxel(local_x, local_y, local_z).voxel_type;
            }
        }
        
        // If chunk not found, return air
        VoxelType::Air
    }

    fn set_voxel(&mut self, x: i32, y: i32, z: i32, voxel_type: VoxelType) {
        let chunk_x = (x as f32 / 16.0).floor() as i32;
        let chunk_y = (y as f32 / 16.0).floor() as i32;
        let chunk_z = (z as f32 / 16.0).floor() as i32;
        
        let local_x = (x.rem_euclid(16)) as usize;
        let local_y = (y.rem_euclid(16)) as usize;
        let local_z = (z.rem_euclid(16)) as usize;
        
        // Find existing chunk
        for chunk in &mut self.chunks {
            if chunk.position == (chunk_x, chunk_y, chunk_z) {
                chunk.set_voxel(local_x, local_y, local_z, voxel_type);
                return;
            }
        }
        
        // If chunk doesn't exist, create it
        let mut new_chunk = Chunk::new((chunk_x, chunk_y, chunk_z));
        new_chunk.set_voxel(local_x, local_y, local_z, voxel_type);
        self.chunks.push(new_chunk);
    }
}

// Camera structure
struct Camera {
    position: Vec3,
    front: Vec3,
    up: Vec3,
    right: Vec3,
    yaw: f32,
    pitch: f32,
    movement_speed: f32,
    mouse_sensitivity: f32,
}

impl Camera {
    fn new() -> Self {
        let position = vec3(0.0, 5.0, 10.0);  // Start closer to the terrain
        let front = vec3(0.0, -0.5, -1.0);    // Look slightly downward
        let up = vec3(0.0, 1.0, 0.0);
        let right = glm::normalize(glm::cross(front, up));
        
        Self {
            position,
            front,
            up,
            right,
            yaw: -90.0,
            pitch: -30.0,  // Start looking down at the terrain
            movement_speed: 2.0,
            mouse_sensitivity: 0.1,
        }
    }
    
    fn process_keyboard(&mut self, direction: &str, delta_time: f32) {
        // Ensure delta_time is reasonable to prevent huge jumps
        let delta_time = delta_time.min(0.1);
        
        // Calculate base velocity with a fixed time step
        let base_velocity = self.movement_speed * delta_time;
        
        // Scale velocity based on direction
        let velocity = match direction {
            "FORWARD" | "BACKWARD" => base_velocity * 0.5,  // Reduce forward/backward speed
            "LEFT" | "RIGHT" => base_velocity * 0.7,       // Slightly reduce strafing speed
            "UP" | "DOWN" => base_velocity * 0.3,          // Reduce vertical movement speed
            _ => base_velocity
        };

        match direction {
            "FORWARD" => self.position = self.position + self.front * velocity,
            "BACKWARD" => self.position = self.position - self.front * velocity,
            "LEFT" => self.position = self.position - self.right * velocity,
            "RIGHT" => self.position = self.position + self.right * velocity,
            "UP" => self.position = self.position + self.up * velocity,
            "DOWN" => self.position = self.position - self.up * velocity,
            _ => {}
        }
    }
    
    fn process_mouse_movement(&mut self, x_offset: f32, y_offset: f32) {
        let x_offset = x_offset * self.mouse_sensitivity;
        let y_offset = y_offset * self.mouse_sensitivity;
        
        self.yaw += x_offset;
        self.pitch += y_offset;
        
        // Constrain pitch
        if self.pitch > 89.0 {
            self.pitch = 89.0;
        }
        if self.pitch < -89.0 {
            self.pitch = -89.0;
        }
        
        // Update front vector
        let x = self.yaw.to_radians().cos() * self.pitch.to_radians().cos();
        let y = self.pitch.to_radians().sin();
        let z = self.yaw.to_radians().sin() * self.pitch.to_radians().cos();
        self.front = glm::normalize(vec3(x, y, z));
        self.right = glm::normalize(glm::cross(self.front, vec3(0.0, 1.0, 0.0)));
        self.up = glm::normalize(glm::cross(self.right, self.front));
    }
    
    fn get_view_matrix(&self) -> glm::Mat4 {
        // Create a look-at matrix manually since glm::look_at is not available
        let f = glm::normalize(self.front);
        let r = glm::normalize(glm::cross(f, vec3(0.0, 1.0, 0.0)));
        let u = glm::cross(r, f);
        
        // Create view matrix directly
        let mut view = glm::Mat4::new(
            glm::vec4(r.x, r.y, r.z, -glm::dot(r, self.position)),
            glm::vec4(u.x, u.y, u.z, -glm::dot(u, self.position)),
            glm::vec4(-f.x, -f.y, -f.z, glm::dot(f, self.position)),
            glm::vec4(0.0, 0.0, 0.0, 1.0)
        );
        
        view
    }
}

fn main() {
    let mut SCREEN_WIDTH = 1280;
    let mut SCREEN_HEIGHT = 700;
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_profile(GLProfile::Core);
    gl_attr.set_framebuffer_srgb_compatible(true);
    gl_attr.set_double_buffer(true);
    gl_attr.set_multisample_samples(4);
    gl_attr.set_context_version(3, 2);
        let last_frame_time: Instant = Instant::now();
    let window = video_subsystem
        .window(
            "Voxel Game",
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
        )
        .opengl()
        .resizable()
        .build()
        .unwrap();

    let _ctx = window.gl_create_context().unwrap();
    debug_assert_eq!(gl_attr.context_profile(), GLProfile::Core);
    debug_assert_eq!(gl_attr.context_version(), (3, 2));

    if let Err(error) = window.subsystem().gl_set_swap_interval(SwapInterval::VSync) {
        println!(
            "Failed to gl_set_swap_interval(SwapInterval::VSync): {}",
            error
        );
    }
    let (mut painter, mut egui_state) =
        egui_backend::with_sdl2(&window, ShaderVersion::Default, DpiScaling::Default);
    let egui_ctx = egui::Context::default();
    let mut event_pump = sdl_context.event_pump().unwrap();
    let start_time: Instant = Instant::now();
    let repaint_signal = Arc::new(Signal::default());

    gl::load_with(|s| video_subsystem.gl_get_proc_address(s) as *const _);

    // Load GLSL shader source from files
    let compute_shader_source = fs::read_to_string("shaders/compute_shader.glsl")
        .expect("Failed to read compute_shader.glsl");
    let quad_vertex_shader_source = fs::read_to_string("shaders/quad_vertex_shader.glsl")
        .expect("Failed to read quad_vertex_shader.glsl");
    let quad_fragment_shader_source = fs::read_to_string("shaders/quad_fragment_shader.glsl")
        .expect("Failed to read quad_fragment_shader.glsl");

    // Compile shaders
    let compute_shader = compile_shader(&compute_shader_source, gl::COMPUTE_SHADER);
    let quad_vertex_shader = compile_shader(&quad_vertex_shader_source, gl::VERTEX_SHADER);
    let quad_fragment_shader = compile_shader(&quad_fragment_shader_source, gl::FRAGMENT_SHADER);

    // Link shader programs
    let mut compute_shader_program = link_program(compute_shader, 0);
    let quad_shader_program = link_program(quad_vertex_shader, quad_fragment_shader);

    // Create a texture for the compute shader to write to
    let mut texture = create_texture(SCREEN_WIDTH,SCREEN_HEIGHT);

    // Set up a fullscreen quad
    let vertices: [f32; 8] = [
        -1.0, -1.0,
        1.0, -1.0,
        -1.0,  1.0,
        1.0,  1.0,
    ];

    let mut vao = 0;
    let mut vbo = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);

        gl::BindVertexArray(vao);

        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(gl::ARRAY_BUFFER, (vertices.len() * std::mem::size_of::<GLfloat>()) as GLsizeiptr, vertices.as_ptr() as *const _, gl::STATIC_DRAW);

        let pos_attrib = gl::GetAttribLocation(quad_shader_program, CString::new("in_pos").unwrap().as_ptr());
        gl::EnableVertexAttribArray(pos_attrib as GLuint);
        gl::VertexAttribPointer(pos_attrib as GLuint, 2, gl::FLOAT, gl::FALSE, 2 * std::mem::size_of::<GLfloat>() as GLsizei, ptr::null());
        
        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindVertexArray(0);
    }
    
    // Create world and camera
    let mut world = World::new();
    let mut camera = Camera::new();
    
    // Create world data buffer
    let mut world_buffer = 0;
    unsafe {
        gl::GenBuffers(1, &mut world_buffer);
        gl::BindBuffer(gl::SHADER_STORAGE_BUFFER, world_buffer);
        
        // Calculate buffer size (3x3 chunks, each 16x16x16 voxels)
        let buffer_size = 3 * 3 * 16 * 16 * 16 * std::mem::size_of::<i32>();
        gl::BufferData(gl::SHADER_STORAGE_BUFFER, buffer_size as GLsizeiptr, std::ptr::null(), gl::DYNAMIC_DRAW);
        
        // Bind buffer to binding point 0
        gl::BindBufferBase(gl::SHADER_STORAGE_BUFFER, 0, world_buffer);
    }
    
    // Mouse state
    let mut mouse_captured = false;
    let mut last_x = SCREEN_WIDTH as f32 / 2.0;
    let mut last_y = SCREEN_HEIGHT as f32 / 2.0;
    
    // Create sandbox window with block selection
    let mut sandbox_windowi = SandboxWindow::new();
    
    // Pass mutable reference to `MainWindow`
    let mut main_window = MainWindow::new(&mut sandbox_windowi);
    
    let now: Instant = Instant::now();
    let delta_time: f32 = now.duration_since(last_frame_time).as_secs_f32();
    
    let mut current_shader_path = String::new();
    
    'running: loop {
        let timernow: Instant = Instant::now();
        let timer: f32 = timernow.duration_since(last_frame_time).as_secs_f32();
        egui_state.input.time = Some(start_time.elapsed().as_secs_f64());

        egui_ctx.begin_frame(egui_state.input.take());

        let frame_time = get_frame_time(start_time);
        let frame = Frame::new(FrameData {
            info: IntegrationInfo {
                web_info: None,
                cpu_usage: Some(frame_time),
                native_pixels_per_point: Some(egui_state.native_pixels_per_point),
                prefer_dark_mode: None,
                name: "Voxel Game",
            },
            output: Default::default(),
            repaint_signal: repaint_signal.clone(),
        });

        // Process UI first
        main_window.ui(&egui_ctx);
        
        // Get the current selected block type and movement settings
        let selected_block = main_window.get_sandbox_window().selected_block;
        let movement_speed = main_window.get_sandbox_window().movement_speed;
        let mouse_sensitivity = main_window.get_sandbox_window().mouse_sensitivity;
        
        // Update camera settings
        camera.movement_speed = movement_speed;
        camera.mouse_sensitivity = mouse_sensitivity;
        
        // Update camera position based on keyboard input
        let keys: Vec<Keycode> = event_pump
            .keyboard_state()
            .pressed_scancodes()
            .filter_map(Keycode::from_scancode)
            .collect();
            
        for key in keys {
            match key {
                Keycode::W => camera.process_keyboard("FORWARD", timer),
                Keycode::S => camera.process_keyboard("BACKWARD", timer),
                Keycode::A => camera.process_keyboard("LEFT", timer),
                Keycode::D => camera.process_keyboard("RIGHT", timer),
                Keycode::Space => camera.process_keyboard("UP", timer),
                Keycode::LShift => camera.process_keyboard("DOWN", timer),
                Keycode::Escape => mouse_captured = !mouse_captured,
                _ => {}
            }
        }

        // Update world data buffer
        unsafe {
            gl::BindBuffer(gl::SHADER_STORAGE_BUFFER, world_buffer);
            
            // Create a temporary buffer to store all voxel data
            let mut voxel_data = Vec::with_capacity(3 * 3 * 16 * 16 * 16);
            
            // Initialize with air
            voxel_data.resize(3 * 3 * 16 * 16 * 16, 0);
            
            // Fill the buffer with voxel data
            for chunk in &world.chunks {
                // Convert chunk coordinates to array indices (0-2 range)
                let chunk_x = (chunk.position.0 + 1) as usize;  // Convert from -1..1 to 0..2
                let chunk_z = (chunk.position.2 + 1) as usize;
                let chunk_index = chunk_x + chunk_z * 3;  // 3x3 grid layout
                
                // Fill the chunk data
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let voxel_type = match chunk.get_voxel(x, y, z).voxel_type {
                                VoxelType::Air => 0,
                                VoxelType::Dirt => 1,
                                VoxelType::Grass => 2,
                                VoxelType::Stone => 3,
                                VoxelType::Wood => 4,
                                VoxelType::Leaves => 5,
                                VoxelType::Light => 6,
                            };
                            
                            // Calculate index in the same way as the shader
                            let local_index = x + y * 16 + z * 16 * 16;
                            let index = chunk_index * 16 * 16 * 16 + local_index;
                            
                            // Ensure we don't go out of bounds
                            if index < voxel_data.len() {
                                voxel_data[index] = voxel_type;
                            }
                        }
                    }
                }
            }
            
            // Update the buffer with the new data
            gl::BufferSubData(
                gl::SHADER_STORAGE_BUFFER,
                0,
                (voxel_data.len() * std::mem::size_of::<i32>()) as GLsizeiptr,
                voxel_data.as_ptr() as *const _,
            );
        }

        unsafe {
            gl::UseProgram(compute_shader_program);
            
            // Set uniforms
            let time_loc = gl::GetUniformLocation(compute_shader_program, CString::new("currentTime").unwrap().as_ptr());
            gl::Uniform1f(time_loc as GLint, timer);
            
            // Camera position
            let cam_pos_loc = gl::GetUniformLocation(compute_shader_program, CString::new("cameraPosition").unwrap().as_ptr());
            gl::Uniform3f(cam_pos_loc as GLint, camera.position.x, camera.position.y, camera.position.z);
            
            // Camera direction
            let cam_dir_loc = gl::GetUniformLocation(compute_shader_program, CString::new("cameraDirection").unwrap().as_ptr());
            gl::Uniform3f(cam_dir_loc as GLint, camera.front.x, camera.front.y, camera.front.z);
            
            // Camera up
            let cam_up_loc = gl::GetUniformLocation(compute_shader_program, CString::new("cameraUp").unwrap().as_ptr());
            gl::Uniform3f(cam_up_loc as GLint, camera.up.x, camera.up.y, camera.up.z);
            
            // Camera right
            let cam_right_loc = gl::GetUniformLocation(compute_shader_program, CString::new("cameraRight").unwrap().as_ptr());
            gl::Uniform3f(cam_right_loc as GLint, camera.right.x, camera.right.y, camera.right.z);
            
            // Screen resolution
            let screen_res_loc = gl::GetUniformLocation(compute_shader_program, CString::new("screenResolution").unwrap().as_ptr());
            gl::Uniform2f(screen_res_loc as GLint, SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32);
            
            // World size
            let world_size_loc = gl::GetUniformLocation(compute_shader_program, CString::new("worldSize").unwrap().as_ptr());
            gl::Uniform3i(world_size_loc as GLint, 3, 1, 3);  // 3x1x3 chunks

            gl::DispatchCompute(SCREEN_WIDTH / 8, SCREEN_HEIGHT / 8, 1);
            gl::MemoryBarrier(gl::SHADER_IMAGE_ACCESS_BARRIER_BIT);
        }

        //////
        let FullOutput {
            platform_output,
            textures_delta,
            shapes,
            pixels_per_point,
            viewport_output,
        } = egui_ctx.end_frame();
        egui_state.process_output(&window, &platform_output);

        if frame.take_app_output().quit {
            break 'running;
        }

        let repaint_after = viewport_output
            .get(&ViewportId::ROOT)
            .expect("Missing ViewportId::ROOT")
            .repaint_delay;

        // Event handling loop
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::Window{
                    win_event: WindowEvent::Resized(width,hegith),
                    ..
                }=>{
                    SCREEN_HEIGHT=hegith as u32;
                    SCREEN_WIDTH=width as u32;
                    unsafe {
                        gl::Viewport(0,0,SCREEN_WIDTH as i32,SCREEN_HEIGHT as i32);
                    };
                    texture = unsafe { create_texture(SCREEN_WIDTH, SCREEN_HEIGHT)}
                }
                Event::MouseMotion { x, y, xrel, yrel, .. } => {
                    // Only process mouse movement for camera if mouse is captured
                    if mouse_captured {
                        let x_offset = xrel as f32;
                        let y_offset = -yrel as f32; // Inverted Y-axis
                        camera.process_mouse_movement(x_offset, y_offset);
                    }
                    
                    // Always pass mouse motion to egui for UI interaction
                    egui_state.process_input(&window, event, &mut painter);
                }
                Event::MouseButtonDown { mouse_btn, .. } => {
                    // Handle block placement/removal only if mouse is captured
                    if mouse_captured {
                        match mouse_btn {
                            sdl2::mouse::MouseButton::Left => {
                                // Ray cast and remove block
                                let ray_dir = camera.front;
                                let ray_pos = camera.position;
                                let mut hit = false;
                                let mut t = 0.0;
                                
                                // Use smaller steps for more precise hit detection
                                while t < 10.0 && !hit {
                                    let pos = ray_pos + ray_dir * t;
                                    let block_x = pos.x.round() as i32;
                                    let block_y = pos.y.round() as i32;
                                    let block_z = pos.z.round() as i32;
                                    
                                    // Check if we're in a valid chunk
                                    let chunk_x = (block_x as f32 / 16.0).floor() as i32;
                                    let chunk_z = (block_z as f32 / 16.0).floor() as i32;
                                    
                                    if chunk_x >= -1 && chunk_x <= 1 && chunk_z >= -1 && chunk_z <= 1 {
                                        if world.get_voxel(block_x, block_y, block_z) != VoxelType::Air {
                                            // Remove block
                                            world.set_voxel(block_x, block_y, block_z, VoxelType::Air);
                                            hit = true;
                                            println!("Removed block at ({}, {}, {})", block_x, block_y, block_z);
                                        }
                                    }
                                    
                                    t += 0.05; // Smaller step size for more precision
                                }
                            }
                            sdl2::mouse::MouseButton::Right => {
                                // Ray cast and place block
                                let ray_dir = camera.front;
                                let ray_pos = camera.position;
                                let mut hit = false;
                                let mut t = 0.0;
                                let mut last_empty_pos = None;
                                
                                // Use smaller steps for more precise hit detection
                                while t < 10.0 && !hit {
                                    let pos = ray_pos + ray_dir * t;
                                    let block_x = pos.x.round() as i32;
                                    let block_y = pos.y.round() as i32;
                                    let block_z = pos.z.round() as i32;
                                    
                                    // Check if we're in a valid chunk
                                    let chunk_x = (block_x as f32 / 16.0).floor() as i32;
                                    let chunk_z = (block_z as f32 / 16.0).floor() as i32;
                                    
                                    if chunk_x >= -1 && chunk_x <= 1 && chunk_z >= -1 && chunk_z <= 1 {
                                        let current_voxel = world.get_voxel(block_x, block_y, block_z);
                                        
                                        if current_voxel != VoxelType::Air {
                                            // If we found a solid block and have a previous empty position
                                            if let Some((x, y, z)) = last_empty_pos {
                                                // Place block at the last empty position
                                                world.set_voxel(x, y, z, selected_block);
                                                hit = true;
                                                println!("Placed block at ({}, {}, {})", x, y, z);
                                            }
                                        } else {
                                            // Store this empty position
                                            last_empty_pos = Some((block_x, block_y, block_z));
                                        }
                                    }
                                    
                                    t += 0.05; // Smaller step size for more precision
                                }
                            }
                            _ => {}
                        }
                    }
                    
                    // Always pass mouse button events to egui for UI interaction
                    egui_state.process_input(&window, event, &mut painter);
                }
                Event::KeyDown { keycode, .. } => {
                    // Handle ESC key to toggle mouse capture
                    if let Some(key) = keycode {
                        if key == Keycode::Escape {
                            mouse_captured = !mouse_captured;
                            
                            // Show/hide cursor based on mouse capture state
                            if mouse_captured {
                                sdl_context.mouse().set_relative_mouse_mode(true);
                            } else {
                                sdl_context.mouse().set_relative_mouse_mode(false);
                            }
                        }
                    }
                    
                    // Pass key events to egui for UI interaction
                    egui_state.process_input(&window, event, &mut painter);
                }
                _ => {
                    // Pass other SDL2 events to egui for processing
                        egui_state.process_input(&window, event, &mut painter);
                }
                }
        }

        // Use the compute shader program to process the texture
        unsafe {
            gl::UseProgram(compute_shader_program);
            gl::DispatchCompute(SCREEN_WIDTH / 8, SCREEN_HEIGHT / 8, 1);
            gl::MemoryBarrier(gl::SHADER_IMAGE_ACCESS_BARRIER_BIT);
        }

        // Render the texture to the screen
        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::UseProgram(quad_shader_program);
            gl::BindVertexArray(vao);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
        }

        let paint_jobs: Vec<ClippedPrimitive> = egui_ctx.tessellate(shapes, pixels_per_point);
        painter.paint_jobs(None, textures_delta, paint_jobs);

        window.gl_swap_window();

        // Update shader based on selection
        let shader_path = match main_window.sandbox_window.selected_shader {
            ShaderType::Basic => "shaders/compute_shader_basic.glsl",
            ShaderType::Organic => "shaders/compute_shader_organic.glsl",
            ShaderType::Balanced => "shaders/compute_shader_balanced.glsl",
            ShaderType::Cubes => "shaders/compute_shader_cubes.glsl",
            ShaderType::Default => "shaders/compute_shader.glsl",
        };
        
        // Reload shader if changed
        if shader_path != current_shader_path {
            println!("Switching to shader: {}", shader_path);
            
            // Try to load the new shader source
            let new_shader_source = match fs::read_to_string(&shader_path) {
                Ok(source) => source,
                Err(e) => {
                    println!("Failed to read shader: {} - {}", shader_path, e);
                    continue 'running;
                }
            };
            
            // Check if the shader is compatible with our current setup
            let is_compatible = if shader_path.contains("balanced") || shader_path.contains("cubes") || shader_path == "shaders/compute_shader.glsl" {
                // These shaders use the new layout
                true
            } else {
                // Basic and organic shaders use a different layout
                println!("Warning: Basic and Organic shaders use a different layout and may not work correctly.");
                false
            };
            
            if is_compatible {
                // Compile and link the new shader
                let new_compute_shader = compile_shader(&new_shader_source, gl::COMPUTE_SHADER);
                let new_compute_shader_program = link_program(new_compute_shader, 0);
                
                // Only update if we successfully created a new shader program
                if new_compute_shader_program != 0 {
                    unsafe {
                        // Delete the old shader program
                        gl::DeleteProgram(compute_shader_program);
                        
                        // Update the shader program reference
                        compute_shader_program = new_compute_shader_program;
                        
                        // Set up uniforms for the new shader
                        gl::UseProgram(compute_shader_program);
                        
                        // Set uniforms
                        let time_loc = gl::GetUniformLocation(compute_shader_program, CString::new("currentTime").unwrap().as_ptr());
                        gl::Uniform1f(time_loc as GLint, timer);
                        
                        // Camera position
                        let cam_pos_loc = gl::GetUniformLocation(compute_shader_program, CString::new("cameraPosition").unwrap().as_ptr());
                        gl::Uniform3f(cam_pos_loc as GLint, camera.position.x, camera.position.y, camera.position.z);
                        
                        // Camera direction
                        let cam_dir_loc = gl::GetUniformLocation(compute_shader_program, CString::new("cameraDirection").unwrap().as_ptr());
                        gl::Uniform3f(cam_dir_loc as GLint, camera.front.x, camera.front.y, camera.front.z);
                        
                        // Camera up
                        let cam_up_loc = gl::GetUniformLocation(compute_shader_program, CString::new("cameraUp").unwrap().as_ptr());
                        gl::Uniform3f(cam_up_loc as GLint, camera.up.x, camera.up.y, camera.up.z);
                        
                        // Camera right
                        let cam_right_loc = gl::GetUniformLocation(compute_shader_program, CString::new("cameraRight").unwrap().as_ptr());
                        gl::Uniform3f(cam_right_loc as GLint, camera.right.x, camera.right.y, camera.right.z);
                        
                        // Screen resolution
                        let screen_res_loc = gl::GetUniformLocation(compute_shader_program, CString::new("screenResolution").unwrap().as_ptr());
                        gl::Uniform2f(screen_res_loc as GLint, SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32);
                        
                        // World size
                        let world_size_loc = gl::GetUniformLocation(compute_shader_program, CString::new("worldSize").unwrap().as_ptr());
                        gl::Uniform3i(world_size_loc as GLint, 3, 1, 3);  // 3x1x3 chunks
                    }
                    
                    // Only update the current shader path if we successfully switched
                    current_shader_path = shader_path.to_string();
                } else {
                    println!("Failed to create shader program for: {}", shader_path);
                }
            } else {
                println!("Shader {} is not compatible with the current setup. Using default shader.", shader_path);
            }
        }
    }
}


fn create_texture(width: u32, height: u32) -> GLuint {
    let mut texture = 0;
    unsafe {
        gl::GenTextures(1, &mut texture);
        gl::BindTexture(gl::TEXTURE_2D, texture);
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGBA32F as i32,
            width as i32,
            height as i32,
            0,
            gl::RGBA,
            gl::FLOAT,
            std::ptr::null(),
        );
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
        gl::BindImageTexture(0, texture, 0, gl::FALSE, 0, gl::WRITE_ONLY, gl::RGBA32F);
    }
    texture
}
