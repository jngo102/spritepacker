use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use eframe::{
    egui::{self, Button, ProgressBar, ScrollArea, SelectableLabel},
    emath::Vec2b,
    epaint::Vec2,
    glow,
};
use image::{GenericImage, GenericImageView};
use notify::{event::ModifyKind, EventKind, PollWatcher, RecursiveMode, Watcher};
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use serde::{Deserialize, Serialize};

use crate::components::switch::switch;
use crate::tk2d::{
    anim::Animation,
    clip::Clip,
    cln::Collection,
    info::{AnimInfo, SpriteInfo},
    sprite::{Sprite, SpriteImage},
};

use super::{i18n::translate, settings::Settings};

const APP_NAME: &str = "spritepacker";

#[derive(Default, Deserialize, Serialize, PartialEq)]
enum InspectMode {
    #[default]
    Animation,
    Backup,
    Collection,
}

#[derive(Default, Deserialize, Serialize)]
struct AppState {
    pub loaded_collections: Vec<Collection>,
    pub loaded_animations: Vec<Animation>,
    pub settings: Settings,

    pub current_animation: Animation,
    pub current_clip: Clip,
    pub current_collection: Collection,
    pub current_frame: Sprite,
    pub current_frame_index: usize,
    pub changed_sprites: Vec<Sprite>,
    pub pack_progress: f32,
    pub can_pack: bool,
    pub is_checking: bool,
    pub is_packing: bool,
    pub inspect_mode: InspectMode,
}

pub struct App {
    state: AppState,
    frame_timer: Option<Instant>,
    progress_sender: Option<Sender<f32>>,
    progress_receiver: Option<Receiver<f32>>,
    sprite_receiver: Option<Receiver<Sprite>>,
    watcher: Option<PollWatcher>,
}

impl eframe::App for App {
    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        confy::store(APP_NAME, APP_NAME, &self.state.settings).expect("Failed to store settings");
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_frame_timer();

        if self.state.is_checking {
            self.poll_changed_sprites();
        }

        ctx.set_visuals(if self.state.settings.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

        match self.state.settings.language.as_str() {
            "zh-CN" => {
                let mut fonts = egui::FontDefinitions::default();

                fonts.font_data.insert(
                    "NotoSansSC".to_owned(),
                    egui::FontData::from_static(include_bytes!("../../fonts/NotoSansSC.ttf")),
                );

                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "NotoSansSC".to_owned());

                ctx.set_fonts(fonts);
            }
            _ => {
                let mut fonts = egui::FontDefinitions::default();

                fonts.font_data.insert(
                    "NotoSans".to_owned(),
                    egui::FontData::from_static(include_bytes!("../../fonts/NotoSans.ttf")),
                );

                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "NotoSans".to_owned());

                ctx.set_fonts(fonts);
            }
        }

        egui::TopBottomPanel::new(egui::panel::TopBottomSide::Top, "topbar").show(ctx, |ui| {
            ui.heading(translate("Settings", self.state.settings.language.clone()));
            ui.horizontal(|ui| {
                ui.label(translate("Dark", self.state.settings.language.clone()));
                let dark_mode_switch = switch(&mut self.state.settings.dark);
                ui.add(dark_mode_switch);

                ui.label(translate(
                    "Sprites Path",
                    self.state.settings.language.clone(),
                ));
                ui.text_edit_singleline(&mut self.state.settings.sprites_path);
                if ui
                    .button(translate("Browse", self.state.settings.language.clone()))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.state.settings.sprites_path = path
                            .to_str()
                            .expect("Failed to convert path to str")
                            .to_string();
                    }
                }

                ui.label(translate("Language", self.state.settings.language.clone()));
                egui::ComboBox::new("languageselect", "")
                    .selected_text(self.state.settings.language.clone())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.state.settings.language,
                            "de".to_string(),
                            "Deutsch",
                        );
                        ui.selectable_value(
                            &mut self.state.settings.language,
                            "en-US".to_string(),
                            "English",
                        );
                        ui.selectable_value(
                            &mut self.state.settings.language,
                            "es".to_string(),
                            "Español",
                        );
                        ui.selectable_value(
                            &mut self.state.settings.language,
                            "fr".to_string(),
                            "Français",
                        );
                        ui.selectable_value(
                            &mut self.state.settings.language,
                            "zh-CN".to_string(),
                            "Chinese (Simplified)",
                        );
                    });
            });
        });
        egui::SidePanel::new(egui::panel::Side::Left, "animationspanel")
            .default_width(150.)
            .show(ctx, |ui| {
                ui.heading(translate(
                    "Animations",
                    self.state.settings.language.clone(),
                ));
                ui.separator();
                egui::ScrollArea::new(Vec2b::new(false, true)).show(ui, |ui| {
                    for animation in self.state.loaded_animations.iter() {
                        let list_item = SelectableLabel::new(
                            self.state.current_animation == *animation,
                            animation.name.clone(),
                        );
                        if ui.add_enabled(self.ui_enabled(), list_item).clicked() {
                            self.frame_timer = Some(Instant::now());
                            self.state.current_animation = animation.clone();
                            self.state.current_clip = self.state.current_animation.clips[0].clone();
                            self.state.current_frame = self.state.current_clip.frames[0].clone();
                            self.state.current_frame_index = 0;
                            self.state.inspect_mode = InspectMode::Animation;
                        }
                    }
                });
            });
        egui::SidePanel::new(egui::panel::Side::Left, "clipspanel")
            .default_width(150.)
            .show(ctx, |ui| {
                ui.heading(translate("Clips", self.state.settings.language.clone()));
                ui.separator();
                egui::ScrollArea::new(Vec2b::new(false, true)).show(ui, |ui| {
                    for clip in self.state.current_animation.clips.iter() {
                        let list_item = SelectableLabel::new(
                            self.state.current_clip == *clip,
                            clip.name.clone(),
                        );
                        if ui.add_enabled(self.ui_enabled(), list_item).clicked() {
                            self.frame_timer = Some(Instant::now());
                            self.state.current_clip = clip.clone();
                            self.state.current_frame = self.state.current_clip.frames[0].clone();
                            self.state.current_frame_index = 0;
                            self.state.inspect_mode = InspectMode::Animation;
                        }
                    }
                });
            });
        egui::SidePanel::new(egui::panel::Side::Left, "framespanel")
            .default_width(150.)
            .show(ctx, |ui| {
                ui.heading(translate("Frames", self.state.settings.language.clone()));
                ui.separator();
                egui::ScrollArea::new(Vec2b::new(false, true)).show(ui, |ui| {
                    for frame in self.state.current_clip.frames.iter() {
                        let list_item = SelectableLabel::new(
                            self.state.current_frame == *frame,
                            frame.name.clone(),
                        );
                        if ui.add_enabled(self.ui_enabled(), list_item).clicked() {
                            self.frame_timer = None;
                            self.state.current_frame = frame.clone();
                            self.state.current_frame_index = self
                                .state
                                .current_clip
                                .frames
                                .iter()
                                .position(|f| f == frame)
                                .expect("Failed to get frame index");
                            self.state.inspect_mode = InspectMode::Animation;
                        }
                    }
                });
            });
        egui::SidePanel::new(egui::panel::Side::Right, "changedpanel")
            .default_width(150.)
            .show(ctx, |ui| {
                ui.heading(translate("Changed", self.state.settings.language.clone()));
                ui.separator();
                egui::ScrollArea::new(Vec2b::new(false, true)).show(ui, |ui| {
                    for sprite in self.state.changed_sprites.iter() {
                        let list_item = SelectableLabel::new(
                            self.state.current_frame == *sprite,
                            sprite.name.clone(),
                        );
                        if ui.add_enabled(self.ui_enabled(), list_item).clicked() {
                            self.frame_timer = None;
                            self.state.inspect_mode = InspectMode::Backup;
                            let collection = self.get_collection(sprite.collection_name.clone());
                            self.state.current_collection = collection.clone();
                            let animation = self.get_animation_from_collection_name(collection);
                            self.state.current_animation = animation.clone();
                            let clip = animation
                                .clips
                                .par_iter()
                                .find_first(|clip| {
                                    clip.frames
                                        .par_iter()
                                        .find_first(|frame| frame.name == sprite.name)
                                        .is_some()
                                })
                                .expect("Failed to find clip from sprite")
                                .clone();
                            self.state.current_clip = clip.clone();
                            self.state.current_frame = clip
                                .frames
                                .par_iter()
                                .find_first(|frame| frame.name == sprite.name)
                                .expect("Failed to find frame from sprite")
                                .clone();
                        }
                    }
                });
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(translate("Inspector", self.state.settings.language.clone()));
            ui.separator();
            let preview_url = if self.state.inspect_mode == InspectMode::Animation
                || self.state.inspect_mode == InspectMode::Backup
            {
                let current_frame = self.state.current_frame.clone();
                let frame_path = current_frame.path.clone();
                let mut frame_url = format!("file://{frame_path}");
                if !Path::new(&frame_path).exists() {
                    frame_url = format!(
                        "file://{}/{}",
                        self.state.settings.sprites_path.clone(),
                        frame_path
                    );
                }

                frame_url
            } else if self.state.inspect_mode == InspectMode::Collection {
                let current_collection = self.state.current_collection.clone();
                let collection_path = current_collection.path.clone();
                let mut collection_url = format!("file://{}", collection_path.display());
                if !Path::new(&collection_path).exists() {
                    collection_url = format!(
                        "file://{}/{}",
                        self.state.settings.sprites_path.clone(),
                        collection_path.display()
                    );
                }

                collection_url
            } else {
                "".to_string()
            };

            let preview_image = egui::Image::new(preview_url)
                .max_size(Vec2::new(256., 256.))
                .maintain_aspect_ratio(true);
            ui.add(preview_image);

            ScrollArea::new(Vec2b::new(false, true))
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    for collection in self.state.loaded_collections.iter() {
                        let list_item = SelectableLabel::new(
                            self.state.current_collection == *collection,
                            collection.name.clone(),
                        );
                        if ui.add_enabled(self.ui_enabled(), list_item).clicked() {
                            self.frame_timer = None;
                            self.state.can_pack = false;
                            self.state.current_collection = collection.clone();
                            self.state.inspect_mode = InspectMode::Collection;
                        }
                    }
                });

            if !self.state.is_packing {
                if !self.state.can_pack {
                    let button =
                        Button::new(translate("Check", self.state.settings.language.clone()));
                    if ui
                        .add_enabled(
                            self.state.inspect_mode == InspectMode::Collection
                                && !self.state.is_checking,
                            button,
                        )
                        .clicked()
                    {
                        self.state.is_checking = true;
                        let sprites_path = self.state.settings.sprites_path.clone();
                        let mut collections = self.state.loaded_collections.clone();
                        let (tx_sprite, rx_sprite) = mpsc::channel();
                        self.sprite_receiver = Some(rx_sprite);
                        thread::spawn(move || {
                            App::check(sprites_path, &mut collections, tx_sprite)
                        });
                    }
                } else {
                    if ui
                        .button(translate("Pack", self.state.settings.language.clone()))
                        .clicked()
                    {
                        self.state.can_pack = false;
                        self.state.is_packing = true;
                        self.state.pack_progress = 0.;
                        self.pack_single_collection(self.state.current_collection.name.clone());
                    }
                }
            } else {
                if ui
                    .button(translate("Cancel", self.state.settings.language.clone()))
                    .clicked()
                {
                    self.cancel_pack();
                }
                self.poll_progress();
                let progress_bar = ProgressBar::new(self.state.pack_progress)
                    .animate(true)
                    .text(format!(
                        "{} {}: {:.2}%",
                        translate("Packing", self.state.settings.language.clone()),
                        self.state.current_collection.name,
                        self.state.pack_progress * 100.
                    ));
                ui.add(progress_bar);
            }
        });

        ctx.request_repaint();
    }
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = App {
            state: AppState::default(),
            frame_timer: Some(Instant::now()),
            progress_sender: None,
            progress_receiver: None,
            sprite_receiver: None,
            watcher: None,
        };

        // Load settings
        if let Ok(settings) = confy::load::<Settings>(APP_NAME, APP_NAME) {
            app.state.settings = settings;
        }

        while app.state.settings.sprites_path == "".to_string() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                app.state.settings.sprites_path = path
                    .to_str()
                    .expect("Failed to convert path to str")
                    .to_string();
            }
        }

        app.load_collections_and_animations();

        let sprites_path = app.state.settings.sprites_path.clone();

        let (tx_sprite, rx_sprite) = mpsc::channel();

        app.sprite_receiver = Some(rx_sprite);

        let (tx_watcher, rx_watcher) = mpsc::channel();
        let config = notify::Config::default()
            .with_compare_contents(true)
            .with_poll_interval(Duration::from_secs(1));

        let mut watcher =
            notify::PollWatcher::new(tx_watcher, config).expect("Failed to create watcher");
        watcher
            .watch(&Path::new(&sprites_path), RecursiveMode::Recursive)
            .expect("Failed to watch sprites path");
        app.watcher = Some(watcher);

        thread::spawn(move || match rx_watcher.recv() {
            Ok(result) => match result {
                Ok(event) => match &event.kind {
                    EventKind::Modify(modify_kind) => match modify_kind {
                        ModifyKind::Metadata(_) => {
                            println!("EVENT: {:?}", event);
                            for path in &event.paths {
                                let path = path
                                    .strip_prefix(sprites_path.clone())
                                    .expect("Failed to strip prefix from path");
                                let path_string = match path.to_str() {
                                    Some(str) => String::from(str),
                                    None => panic!("Failed to convert path to string"),
                                };
                                let paths = path_string.split(['/', '\\']).collect::<Vec<&str>>();
                                let sprite_info_path = PathBuf::from(sprites_path.clone())
                                    .join(paths[0])
                                    .join("0.Atlases")
                                    .join("SpriteInfo.json");
                                let collection_name = match fs::read_to_string(sprite_info_path) {
                                    Ok(text) => {
                                        let sprite_info: SpriteInfo =
                                            match serde_json::from_str(&text) {
                                                Ok(info) => info,
                                                Err(e) => {
                                                    panic!("Failed to parse SpriteInfo.json: {e}")
                                                }
                                            };
                                        sprite_info.collection_name[0].to_string()
                                    }
                                    Err(e) => {
                                        panic!("Failed to read SpriteInfo.json: {e}")
                                    }
                                };
                                if paths.len() < 3 {
                                    continue;
                                }
                                let sprite_name = paths[2].to_string();
                                let sprite_data = sprite_name.split("-").collect::<Vec<&str>>();
                                if sprite_data.len() < 3 {
                                    continue;
                                }
                                let sprite_id_string =
                                    sprite_data[sprite_data.len() - 1].replace(".png", "");
                                let sprite_id = match sprite_id_string.parse::<u32>() {
                                    Ok(id) => id,
                                    Err(e) => {
                                        panic!("Failed to parse sprite id {sprite_id_string}: {e}")
                                    }
                                };
                                let sprite = Sprite {
                                    id: sprite_id,
                                    name: sprite_name.to_string(),
                                    collection_name: collection_name.to_string(),
                                    path: path_string,
                                    flipped: false,
                                    x: 0,
                                    y: 0,
                                    xr: 0,
                                    yr: 0,
                                    width: 0,
                                    height: 0,
                                };

                                tx_sprite.send(sprite).expect("Failed to send sprite");
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                },
                Err(e) => panic!("Failed to receive event: {}", e.to_string()),
            },
            Err(e) => panic!("Watcher error: {}", e.to_string()),
        });

        return app;
    }

    /// Cancel the currently running pack task
    fn cancel_pack(&mut self) {
        self.state.is_packing = false;
        if let Some(sender) = &self.progress_sender {
            sender.send(-1.).expect("Failed to send cancel signal");
        }
        self.progress_sender = None;
        self.progress_receiver = None;
    }

    /// Check whether any sprites and their duplicates are not identical.
    fn check(
        sprites_path: String,
        collections: &mut Vec<Collection>,
        sprite_sender: Sender<Sprite>,
    ) {
        let mut problem_sprites = vec![];
        for collection in collections {
            let mut sprite_map = HashMap::<u32, Vec<Sprite>>::new();
            for sprite in &collection.sprites {
                let sprite_map_entry = sprite_map.get(&sprite.id);
                if let Some(entry) = sprite_map_entry {
                    for existing_sprite in entry {
                        let existing_sprite_path = existing_sprite.path.clone();
                        let mut path1 =
                            PathBuf::from(sprites_path.clone()).join(existing_sprite_path.clone());
                        if !path1.exists() {
                            path1 = PathBuf::from(existing_sprite_path.clone());
                        }
                        let mut path2 =
                            PathBuf::from(sprites_path.clone()).join(sprite.path.clone());
                        if !path2.exists() {
                            path2 = PathBuf::from(sprite.path.clone());
                        }
                        let image1 = image::open(path1.clone()).expect(
                            format!("Failed to open image at path {:?}", path1.display()).as_str(),
                        );
                        let image2 = image::open(path2.clone()).expect(
                            format!("Failed to open image at path {:?}", path2.display()).as_str(),
                        );

                        let sprite_image1 = SpriteImage {
                            sprite: existing_sprite.clone(),
                            image: image1,
                        };

                        let sprite_image2 = SpriteImage {
                            sprite: sprite.clone(),
                            image: image2,
                        };

                        if !sprite_image1.equals(&sprite_image2) {
                            for sprite in entry {
                                if !problem_sprites.contains(sprite) {
                                    problem_sprites.push(sprite.clone());
                                    sprite_sender
                                        .send(sprite.clone())
                                        .expect("Failed to send sprite");
                                }
                            }

                            if !problem_sprites.contains(sprite) {
                                problem_sprites.push(sprite.clone());
                                sprite_sender
                                    .send(sprite.clone())
                                    .expect("Failed to send sprite");
                            }

                            break;
                        }
                    }
                } else if sprite_map_entry.is_none() {
                    let sprite_data = sprite.name.split("-").collect::<Vec<&str>>();
                    let sprite_id_string = sprite_data[sprite_data.len() - 1].replace(".png", "");
                    let sprite_id = sprite_id_string.parse::<u32>().expect(
                        format!("Failed to convert Sprite ID string {sprite_id_string} to u32")
                            .as_str(),
                    );
                    sprite_map.insert(sprite_id, vec![sprite.clone()]);
                } else {
                    sprite_map
                        .get_mut(&sprite.id)
                        .expect("Sprite map is None")
                        .push(sprite.clone());
                }
            }
        }

        sprite_sender
            .send(Sprite::default())
            .expect("Failed to send cancel signal");
    }

    /// Check for any sprites that have been changed since the application started
    /// # Arguments
    /// * `already_changed_sprites` - A list of sprites that have already been marked as changed in the application
    /// # Returns
    /// * `Vec<Sprite>` A list of all changed sprites
    fn check_for_changed_sprites(&mut self) -> Vec<Sprite> {
        let changed_sprites = self.state.changed_sprites.clone();
        let mut sprites = changed_sprites
            .iter()
            .map(|sprite| {
                self.get_collection(sprite.collection_name.clone())
                    .sprites
                    .par_iter()
                    .find_first(|s| s.id == sprite.id)
                    .expect("Failed to find sprite in collection")
                    .clone()
            })
            .collect::<Vec<_>>();
        sprites.retain(|sprite| !self.state.changed_sprites.contains(sprite));
        return sprites;
    }

    /// Check the frame timer and update the current frame if necessary.
    fn check_frame_timer(&mut self) {
        if let Some(frame_timer) = self.frame_timer {
            if frame_timer.elapsed().as_secs_f32() > 1.0 / self.state.current_clip.fps {
                self.frame_timer = Some(Instant::now());
                self.state.current_frame_index += 1;
                if self.state.current_frame_index >= self.state.current_clip.frames.len() {
                    self.state.current_frame_index = self.state.current_clip.loop_start as usize;
                }
                self.state.current_frame =
                    self.state.current_clip.frames[self.state.current_frame_index].clone();
            }
        }
    }

    /// Get an animation from a collection.
    /// # Arguments
    /// * `collection_name` - The name of the collection
    /// * `state` - The application state
    /// # Returns
    /// * `Animation` - The found animation
    fn get_animation_from_collection_name(&self, collection: Collection) -> Animation {
        let animation = match self
            .state
            .loaded_animations
            .par_iter()
            .find_map_first(|anim| {
                anim.clips.par_iter().find_map_first(|clip| {
                    clip.frames.par_iter().find_map_first(|frame| {
                        if frame.collection_name == collection.name {
                            Some(anim)
                        } else {
                            None
                        }
                    })
                })
            }) {
            Some(anim) => anim.clone(),
            None => panic!(
                "Failed to find animation from collection {:?}",
                collection.name
            ),
        };

        animation
    }

    /// Get a collection by its name.
    /// # Arguments
    /// * `collection_name` - The name of the collection
    /// # Returns
    /// *`Collection` The collection with the given name
    fn get_collection(&self, collection_name: String) -> Collection {
        self.state
            .loaded_collections
            .par_iter()
            .find_first(|cln| cln.name == collection_name)
            .expect("Failed to find collection")
            .clone()
    }

    /// Get a collection from a sprite's name
    /// # Arguments
    /// * `sprite_name` - The name of the sprite
    /// * `state` - The application state
    /// # Returns
    /// * `Collection` - The found collection
    fn get_collection_from_sprite_name(&self, sprite_name: String) -> Collection {
        let collection =
            match self
                .state
                .loaded_collections
                .par_iter()
                .find_map_first(|collection| {
                    collection.sprites.par_iter().find_map_first(|sprite| {
                        if sprite.name == sprite_name {
                            Some(collection)
                        } else {
                            None
                        }
                    })
                }) {
                Some(collection) => collection.clone(),
                None => panic!("Failed to find collection from sprite name {sprite_name}"),
            };

        collection
    }

    /// Load collections and animations from sprite files on disk.
    fn load_collections_and_animations(&mut self) {
        let sprites_path = PathBuf::from(self.state.settings.sprites_path.clone());
        if let Ok(anim_paths) = fs::read_dir(sprites_path.clone()) {
            for anim_path in anim_paths {
                if let Ok(anim_entry) = anim_path {
                    if !anim_entry.path().is_dir() {
                        continue;
                    }
                    let sprite_info_path =
                        anim_entry.path().join("0.Atlases").join("SpriteInfo.json");
                    if let Ok(sprite_info_text) = fs::read_to_string(sprite_info_path) {
                        let sprite_info: SpriteInfo = serde_json::from_str(&sprite_info_text)
                            .expect("Failed to parse SpriteInfo.json");
                        for i in 0..sprite_info.id.len() {
                            if let Some(sprite) = sprite_info.at(i) {
                                if !PathBuf::from(sprite.path.clone()).exists()
                                    && !sprites_path.join(sprite.path.clone()).exists()
                                {
                                    continue;
                                }

                                if let Some(collection) = self
                                    .state
                                    .loaded_collections
                                    .iter()
                                    .find(|cln| cln.name == sprite.collection_name)
                                {
                                    let mut collection = collection.clone();
                                    collection.sprites.push(sprite);
                                    self.state
                                        .loaded_collections
                                        .retain(|cln| cln.name != collection.name);
                                    self.state.loaded_collections.push(collection);
                                } else {
                                    let collection_name = sprite.clone().collection_name;
                                    let mut cln = Collection {
                                        name: collection_name.clone(),
                                        path: anim_entry
                                            .path()
                                            .join("0.Atlases")
                                            .join(format!("{}.png", collection_name)),
                                        sprites: vec![],
                                    };
                                    cln.sprites.push(sprite);
                                    self.state.loaded_collections.push(cln);
                                }
                            }
                        }

                        let mut clips = vec![];
                        if let Ok(clip_paths) = fs::read_dir(anim_entry.path()) {
                            for clip_path in clip_paths {
                                if let Ok(clip_entry) = clip_path {
                                    if !clip_entry.path().is_dir()
                                        || clip_entry
                                            .path()
                                            .file_name()
                                            .expect("Failed to get file name of clip entry")
                                            == "0.Atlases"
                                    {
                                        continue;
                                    }
                                    let mut frames = vec![];
                                    let mut fps = 12.;
                                    let mut loop_start = 0;
                                    if let Ok(frame_paths) = fs::read_dir(clip_entry.path()) {
                                        for frame_path in frame_paths {
                                            if let Ok(frame_entry) = frame_path {
                                                if frame_entry.file_name() == "AnimInfo.json" {
                                                    if let Ok(anim_info_text) =
                                                        fs::read_to_string(frame_entry.path())
                                                    {
                                                        let anim_info: AnimInfo =
                                                            match serde_json::from_str(
                                                                &anim_info_text,
                                                            ) {
                                                                Ok(anim_info) => anim_info,
                                                                Err(_) => AnimInfo {
                                                                    fps: 12.0,
                                                                    loop_start: 0,
                                                                    num_frames: 0,
                                                                    collection_name: "".to_string(),
                                                                },
                                                            };
                                                        fps = anim_info.fps;
                                                        loop_start = anim_info.loop_start;
                                                    }
                                                    continue;
                                                } else if frame_entry
                                                    .path()
                                                    .extension()
                                                    .expect("Failed to get extension of frame path")
                                                    != "png"
                                                {
                                                    continue;
                                                }

                                                let index = sprite_info
                                                    .path
                                                    .par_iter()
                                                    .position_first(|path| {
                                                        let stripped_path = path.replace("./", "/").replace(".\\", "\\");
                                                        frame_entry.path().ends_with(stripped_path)
                                                    })
                                                    .expect(
                                                        format!(
                                                            "Failed to find sprite for frame at {:?}",
                                                            frame_entry.path()
                                                        )
                                                        .as_str(),
                                                    );

                                                let sprite = sprite_info.at(index).expect(
                                                    format!(
                                                        "Failed to get sprite at index {index}"
                                                    )
                                                    .as_str(),
                                                );

                                                frames.push(sprite);
                                            }
                                        }
                                    }

                                    if let Some(clip_name) = clip_entry.file_name().to_str() {
                                        clips.push(Clip::new(
                                            clip_name.to_string(),
                                            frames,
                                            fps,
                                            loop_start,
                                        ));
                                    }
                                }
                            }
                        }

                        let anim_file = anim_entry.file_name();
                        let anim_name = anim_file
                            .to_str()
                            .expect("Failed to get animation name from file name");

                        self.state.loaded_animations.push(Animation {
                            name: anim_name.to_string(),
                            clips: clips.to_vec(),
                        });
                    }
                }
            }
        }

        if self.state.loaded_animations.len() > 0 {
            self.state.current_animation = self.state.loaded_animations[0].clone();
            self.state.current_clip = self.state.current_animation.clips[0].clone();
            self.state.current_frame = self.state.current_clip.frames[0].clone();
        }
    }

    /// Replace all duplicate sprites in a collection.
    /// # Arguments
    /// * `source_sprite` - The sprite to replace duplicates with
    fn replace_duplicate_sprites(&mut self, source_sprite: Sprite) {
        let sprites_path = PathBuf::from(self.state.settings.sprites_path.clone());

        self.watcher
            .as_mut()
            .expect("Watcher is none")
            .unwatch(&sprites_path.clone())
            .expect("Failed to unwatch sprites path");

        let source_path = if sprites_path.join(source_sprite.path.clone()).exists() {
            sprites_path.join(source_sprite.path.clone())
        } else if PathBuf::from(source_sprite.path.clone()).exists() {
            PathBuf::from(source_sprite.path.clone())
        } else {
            panic!(
                "Failed to get a valid path from source sprite at {}",
                source_sprite.path
            );
        };

        let source_image = match image::open(source_path.clone()) {
            Ok(image) => image,
            Err(e) => panic!(
                "Failed to open image at path {:?}: {}",
                source_path.display(),
                e
            ),
        };

        let source_image = SpriteImage {
            sprite: source_sprite.clone(),
            image: source_image,
        };

        let collection = self.get_collection(source_sprite.collection_name.clone());
        for sprite in collection.sprites {
            if sprite.id != source_sprite.id {
                continue;
            }

            let sprite_path = if sprites_path.join(sprite.path.clone()).exists() {
                sprites_path.join(sprite.path.clone())
            } else if PathBuf::from(sprite.path.clone()).exists() {
                PathBuf::from(sprite.path.clone())
            } else {
                panic!("Failed to get a valid path from sprite at {}", sprite.path);
            };

            let sprite_image = image::open(sprite_path.clone()).expect(
                format!("Failed to open image at path {:?}", sprite_path.display()).as_str(),
            );

            let mut sprite_image = SpriteImage {
                sprite: sprite.clone(),
                image: sprite_image,
            };

            App::replace_sprite(source_image.clone(), &mut sprite_image);

            match sprite_image.image.save(sprite_path.clone()) {
                Ok(_) => println!(
                    "Replaced sprite at path {:?} with sprite at path {:?}",
                    sprite_path.display(),
                    source_path.display()
                ),
                Err(e) => panic!(
                    "Failed to save image at path {:?}: {}",
                    sprite_path.display(),
                    e
                ),
            }
        }

        self.watcher
            .as_mut()
            .expect("Failed to unwrap watcher")
            .watch(&Path::new(&sprites_path), RecursiveMode::Recursive)
            .expect("Failed to watch sprites path");
    }

    /// Replace a sprite with another sprite.
    /// # Arguments
    /// * `source_image` - The sprite to replace with
    /// * `target_image` - The sprite to replace
    fn replace_sprite(source_image: SpriteImage, target_image: &mut SpriteImage) {
        let target_ptr = Arc::new(Mutex::new(target_image));
        let sub_image = source_image.trim();
        (0..sub_image.width()).into_par_iter().for_each(|x| {
            (0..sub_image.height()).into_par_iter().for_each(|y| {
                let source_y = sub_image.height() - y - 1;
                let pixel = sub_image.get_pixel(x, source_y);
                let mut target_image = target_ptr.lock().unwrap();
                let target_x = x + target_image.sprite.xr as u32;
                let target_y =
                    target_image.image.height() - (y + target_image.sprite.yr as u32) - 1;
                target_image.image.put_pixel(target_x, target_y, pixel);
            });
        });
    }

    /// Packs a collection of sprites into an atlas.
    /// # Arguments
    /// * `collection` - The collection to pack
    /// * `sprites_path` - The path to the sprites folder
    /// * `tx` - The channel to send progress updates through
    fn pack_collection(collection: Collection, sprites_path: String, tx: Sender<f32>) {
        let atlas = image::open(collection.path.clone()).expect("Failed to open atlas file");
        let sprite_num_ptr = Arc::new(Mutex::new(0 as usize));
        let atlas_width = atlas.width() as i32;
        let atlas_height = atlas.height() as i32;
        let gen_atlas = Mutex::new(atlas);
        collection.sprites.par_iter().for_each(|sprite| {
            let frame_path = PathBuf::from_str(sprites_path.as_str())
                .expect("Failed to create frame path from string")
                .join(sprite.path.clone());
            let frame_image = image::open(frame_path.clone()).expect(
                format!("Failed to open frame image at {:?}", frame_path.display()).as_str(),
            );

            (0..frame_image.width()).into_par_iter().for_each(|i| {
                (0..frame_image.height()).into_par_iter().for_each(|j| {
                    let i = i as i32;
                    let j = j as i32;
                    let x = if sprite.flipped {
                        sprite.x + j - sprite.yr
                    } else {
                        sprite.x + i - sprite.xr
                    };
                    let y = if sprite.flipped {
                        atlas_height - (sprite.y + i) - 1 + sprite.xr
                    } else {
                        atlas_height - (sprite.y + j) - 1 + sprite.yr
                    };
                    if i >= sprite.xr
                        && i < (sprite.xr + sprite.width)
                        && j >= sprite.yr
                        && j < (sprite.yr + sprite.height)
                        && x >= 0
                        && x < atlas_width as i32
                        && y >= 0
                        && y < atlas_height as i32
                    {
                        let mut atlas = gen_atlas.lock().unwrap();
                        atlas.put_pixel(
                            x as u32,
                            y as u32,
                            frame_image
                                .get_pixel(i as u32, (frame_image.height() as i32 - j - 1) as u32),
                        );
                    }
                });
            });

            let sprite_num_ptr_clone = sprite_num_ptr.clone();
            let mut num = loop {
                match sprite_num_ptr_clone.try_lock() {
                    Ok(num) => break num,
                    Err(_) => {}
                }
            };
            *num += 1;
            let progress = *num as f32 / collection.sprites.len() as f32;
            tx.send(progress).expect("Failed to send progress value");
        });

        let atlas_path = rfd::FileDialog::new()
            .set_directory(&sprites_path)
            .set_file_name(format!("{}.png", collection.name.clone()).as_str())
            .add_filter("PNG Image", &["png"])
            .save_file()
            .expect("Failed to save generated atlas");
        gen_atlas
            .lock()
            .unwrap()
            .save(atlas_path)
            .expect("Failed to save generated atlas");

        drop(tx);
    }

    /// Pack a single collection.
    /// # Arguments
    /// * `collection_name` - The name of the collection
    /// * `app_handle` - The application handle
    /// * `state` - The application state
    fn pack_single_collection(&mut self, collection_name: String) {
        let collection = self.get_collection(collection_name.clone());
        let sprites_path = self.state.settings.sprites_path.clone();

        let (tx, rx) = mpsc::channel();
        self.progress_sender = Some(tx.clone());
        self.progress_receiver = Some(rx);
        thread::spawn(move || App::pack_collection(collection, sprites_path, tx.clone()));
    }

    /// Poll for changed sprites.
    fn poll_changed_sprites(&mut self) {
        if let Some(rx) = self.sprite_receiver.as_mut() {
            if let Ok(sprite) = rx.try_recv() {
                if sprite == Sprite::default() {
                    self.state.is_checking = false;
                    self.state.can_pack = true;
                    return;
                }
                if !self.state.changed_sprites.contains(&sprite) {
                    self.state.changed_sprites.push(sprite);
                }
            }
        }
    }

    /// Poll for the progress of the current pack.
    fn poll_progress(&mut self) {
        if let Some(rx) = self.progress_receiver.as_mut() {
            match rx.try_recv() {
                Ok(progress) => {
                    if progress < 0. {
                        self.state.is_packing = false;
                        return;
                    }
                    self.state.pack_progress = progress;
                    if progress >= 1. {
                        self.state.is_packing = false;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.state.is_packing = false;
                }
            }
        }
    }

    /// Check whether the UI should be enabled.
    /// # Returns
    /// * `bool` Whether the UI should be enabled
    fn ui_enabled(&self) -> bool {
        !self.state.is_packing && !self.state.is_checking
    }
}
