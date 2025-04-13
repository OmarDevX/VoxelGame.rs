pub mod windows{
    use egui::{Modifiers, Slider, Ui};
    use crate::VoxelType;

    // Define ShaderType enum at the top level
    #[derive(Clone, Copy, PartialEq)]
    pub enum ShaderType {
        Basic,
        Organic,
        Balanced,
        Cubes,
    }

    #[derive(Clone)]
    pub struct SandboxWindow {
        pub selected_block: VoxelType,
        pub movement_speed: f32,
        pub mouse_sensitivity: f32,
    }
    
    impl SandboxWindow {
        pub fn new() -> Self {
            Self {
                selected_block: VoxelType::Dirt,
                movement_speed: 1.0,
                mouse_sensitivity: 0.1,
            }
        }
    
        pub fn ui(&mut self, ctx: &egui::Context, ui: &mut Ui) {
            let _ = ctx;
            self.scene_settings(ui);
        
        }
        pub fn scene_settings(&mut self, ui: &mut Ui) {
            ui.heading("Block Selection");
            ui.separator();
            
            ui.horizontal(|ui| {
                if ui.selectable_label(self.selected_block == VoxelType::Dirt, "Dirt").clicked() {
                    self.selected_block = VoxelType::Dirt;
                }
                if ui.selectable_label(self.selected_block == VoxelType::Grass, "Grass").clicked() {
                    self.selected_block = VoxelType::Grass;
                }
                if ui.selectable_label(self.selected_block == VoxelType::Stone, "Stone").clicked() {
                    self.selected_block = VoxelType::Stone;
                }
            });
            
            ui.horizontal(|ui| {
                if ui.selectable_label(self.selected_block == VoxelType::Wood, "Wood").clicked() {
                    self.selected_block = VoxelType::Wood;
                }
                if ui.selectable_label(self.selected_block == VoxelType::Leaves, "Leaves").clicked() {
                    self.selected_block = VoxelType::Leaves;
                }
                if ui.selectable_label(self.selected_block == VoxelType::Light, "Light").clicked() {
                    self.selected_block = VoxelType::Light;
                }
            });
            
            ui.separator();
            ui.heading("Movement Settings");
            
            ui.add(Slider::new(&mut self.movement_speed, 0.1..=2.0).text("Movement Speed"));
            ui.add(Slider::new(&mut self.mouse_sensitivity, 0.01..=0.3).text("Mouse Sensitivity"));
            
            ui.separator();
            ui.label("Controls:");
            ui.label("WASD - Move");
            ui.label("Space/Shift - Up/Down");
            ui.label("Left Click - Break Block");
            ui.label("Right Click - Place Block");
            ui.label("ESC - Toggle Mouse Capture");
        }
        
    }
    
    pub struct BlockSelection {
        pub selected_block: VoxelType,
    }
    
    impl BlockSelection {
        pub fn new() -> Self {
            Self {
                selected_block: VoxelType::Dirt,
            }
        }
        
        pub fn render(&mut self, ui: &mut Ui) {
            ui.group(|ui| {
                ui.label("Block Selection");
                ui.radio_value(&mut self.selected_block, VoxelType::Air, "Air");
                ui.radio_value(&mut self.selected_block, VoxelType::Dirt, "Dirt");
                ui.radio_value(&mut self.selected_block, VoxelType::Grass, "Grass");
                ui.radio_value(&mut self.selected_block, VoxelType::Stone, "Stone");
                ui.radio_value(&mut self.selected_block, VoxelType::Wood, "Wood");
                ui.radio_value(&mut self.selected_block, VoxelType::Leaves, "Leaves");
                ui.radio_value(&mut self.selected_block, VoxelType::Light, "Light Block");
            });
        }
    }
    
    #[derive(Clone)]
    pub struct MovementSettings {
        pub movement_speed: f32,
        pub mouse_sensitivity: f32,
    }
    
    impl MovementSettings {
        pub fn new() -> Self {
            Self {
                movement_speed: 1.0,
                mouse_sensitivity: 0.1,
            }
        }
        
        pub fn render(&mut self, ui: &mut Ui) {
            ui.add(Slider::new(&mut self.movement_speed, 0.1..=2.0).text("Movement Speed"));
            ui.add(Slider::new(&mut self.mouse_sensitivity, 0.01..=0.3).text("Mouse Sensitivity"));
        }
    }
        
    pub struct MainWindow<'a> {
        pub show_sandbox_window: bool,
        pub sandbox_window: &'a mut SandboxWindow,
        pub block_selection: BlockSelection,
        pub movement_settings: MovementSettings,
        pub selected_shader: ShaderType,
        pub show_settings: bool,
    }
    
    impl<'a> MainWindow<'a> {
        pub fn new(sandbox_window: &'a mut SandboxWindow) -> Self {
            Self {
                show_sandbox_window: true,
                sandbox_window,
                block_selection: BlockSelection::new(),
                movement_settings: MovementSettings::new(),
                selected_shader: ShaderType::Balanced,
                show_settings: true,
            }
        }
    
        pub fn ui(&mut self, ctx: &egui::Context) {
            self.desktop_ui(ctx);
            if self.show_settings {
                self.render(ctx);
            }
        }
    
        pub fn desktop_ui(&mut self, ctx: &egui::Context) {
            egui::SidePanel::left("egui_demo_panel")
                .resizable(true)
                .default_width(250.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("✒ Voxel Game");
                    });
                    ui.separator();
                    use egui::special_emojis::{GITHUB, TWITTER};
                    if self.show_sandbox_window {
                        egui::Window::new("Block Selection")
                            .resizable(true)
                            .default_width(400.0)
                            .show(ctx, |ui| {
                                self.sandbox_window.ui(ctx, ui);
                            });
                    }
                    ui.hyperlink_to(
                        format!("{GITHUB} Resource Code"),
                        "https://github.com/OmarDevX",
                    );
                    ui.separator();
                    self.demo_list_ui(ui);
                });
    
            egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    file_menu_button(ui);
                    ui.menu_button("View", |ui| {
                        if ui.checkbox(&mut self.show_settings, "Settings").clicked() {
                            ui.close_menu();
                        }
                    });
                });
            });
        }
    
        pub fn demo_list_ui(&mut self, ui: &mut egui::Ui) {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    ui.label("Controls");
                    if ui.button("Toggle Block Selection").clicked() {
                        self.show_sandbox_window = !self.show_sandbox_window;
                    }
            
                    if ui.button("Organize windows").clicked() {
                        ui.ctx().memory_mut(|mem| mem.reset_areas());
                    }
                });
            });
        }
        pub fn get_sandbox_window(&self) -> &SandboxWindow {
                return self.sandbox_window;
        }
        
        pub fn render(&mut self, ctx: &egui::Context) {
            egui::Window::new("Settings")
                .resizable(false)
                .default_pos([400.0, 100.0])
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.heading("Block Selection");
                    self.block_selection.render(ui);
                    
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);
                    
                    ui.heading("Movement Settings");
                    self.movement_settings.render(ui);
                    
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);
                    
                    ui.heading("Shader Selection");
                    ui.radio_value(&mut self.selected_shader, ShaderType::Balanced, "Balanced");
                    ui.radio_value(&mut self.selected_shader, ShaderType::Cubes, "Cubes");
                    
                    // Disable incompatible shaders
                    ui.add_enabled(false, egui::RadioButton::new(false, "Basic (Incompatible)"));
                    ui.add_enabled(false, egui::RadioButton::new(false, "Organic (Incompatible)"));
                    
                    // Add a note about shader compatibility
                    ui.add_space(5.0);
                    ui.label(egui::RichText::new("Note: Only Balanced and Cubes shaders are currently compatible with this version.").small());
                });
        }
    }
    
        pub fn file_menu_button(ui: &mut Ui) {
        let organize_shortcut =
            egui::KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, egui::Key::O);
        let reset_shortcut =
            egui::KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, egui::Key::R);
    
        // NOTE: we must check the shortcuts OUTSIDE of the actual "File" menu,
        // or else they would only be checked if the "File" menu was actually open!
    
        if ui.input_mut(|i| i.consume_shortcut(&organize_shortcut)) {
            ui.ctx().memory_mut(|mem| mem.reset_areas());
        }
    
        if ui.input_mut(|i| i.consume_shortcut(&reset_shortcut)) {
            ui.ctx().memory_mut(|mem| *mem = Default::default());
        }
    
        ui.menu_button("File", |ui| {
            ui.set_min_width(220.0);
            ui.style_mut().wrap = Some(false);
    
            // On the web the browser controls the zoom
            #[cfg(not(target_arch = "wasm32"))]
            {
                egui::gui_zoom::zoom_menu_buttons(ui);
                ui.weak(format!(
                    "Current zoom: {:.0}%",
                    100.0 * ui.ctx().zoom_factor()
                ))
                .on_hover_text("The UI zoom level, on top of the operating system's default value");
                ui.separator();
            }
    
            if ui
                .add(
                    egui::Button::new("Organize Windows")
                        .shortcut_text(ui.ctx().format_shortcut(&organize_shortcut)),
                )
                .clicked()
            {
                ui.ctx().memory_mut(|mem| mem.reset_areas());
                ui.close_menu();
            }
    
            if ui
                .add(
                    egui::Button::new("Reset egui memory")
                        .shortcut_text(ui.ctx().format_shortcut(&reset_shortcut)),
                )
                .on_hover_text("Forget scroll, positions, sizes etc")
                .clicked()
            {
                ui.ctx().memory_mut(|mem| *mem = Default::default());
                ui.close_menu();
            }
        });
    }
}

// Remove the duplicate ShaderType enum and MainWindow implementation
// as they're now properly defined inside the windows module
