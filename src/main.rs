use egui::Color32;
use quadtree::{Locatable, Point, QuadTree, Rect};
use rand::Rng;
use rhai::packages::Package; // needed for 'Package' trait
use rhai::{CustomType, Engine, EvalAltResult, TypeBuilder};
use rhai_rand::RandomPackage;
use std::collections::HashMap;
use std::f32::consts::PI;
use uuid::Uuid;

mod quadtree;

const BOX_SIZE: f32 = 400.;

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
struct Controls {
    right: bool,
    left: bool,
    forward: bool,
    back: bool,
    eat: bool,
}

impl Controls {
    pub fn new() -> Self {
        Self {
            right: false,
            left: false,
            forward: false,
            back: false,
            eat: false,
        }
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_name("Controls")
            .with_fn("new_controls", Self::new);
    }
}

impl eframe::App for World {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        _ = self.update(0.1);
        egui::CentralPanel::default().show(ctx, |ui| {
            let painter = ui.painter();
            for microbe in self.microbes.items() {
                let player_pos = egui::pos2(
                    microbe.transform.position.x + BOX_SIZE,
                    microbe.transform.position.y + BOX_SIZE,
                );
                let size = (microbe.energy / (HEALTH)) + 1.;
                painter.circle_filled(player_pos, size, microbe.color);

                let direction = egui::vec2(
                    microbe.transform.rotation.cos(),
                    microbe.transform.rotation.sin(),
                );
                let line_end = player_pos + direction * size;
                painter.line_segment(
                    [player_pos, line_end],
                    egui::Stroke::new(1.0, egui::Color32::RED),
                );
            }
        });
        ctx.request_repaint();
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Vector2 {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Transform {
    position: Vector2,
    rotation: f32,
}

impl Transform {
    fn new(x: f32, y: f32, rotation: f32) -> Self {
        Self {
            position: Vector2 { x, y },
            rotation,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Microbe {
    id: Uuid,
    lineage: Uuid,
    transform: Transform,
    script_id: Uuid,
    energy: f32,
    color: Color32,
}

impl Locatable for Microbe {
    fn location(&self) -> Point {
        Point::new(self.transform.position.x, self.transform.position.y)
    }
}

impl Microbe {
    fn new(x: f32, y: f32, rotation: f32, script_id: Uuid, color: Color32) -> Self {
        Self {
            id: Uuid::new_v4(),
            lineage: Uuid::new_v4(),
            transform: Transform::new(x, y, rotation),
            script_id,
            energy: HEALTH,
            color,
        }
    }

    fn update(&mut self, controls: &Controls, _delta_time: f32) {
        // Apply controls to movement
        let speed = SPEED;
        self.energy -= ACTION_ENERGY_CONSUMPTION;

        // Update position based on controls
        if controls.forward {
            // self.energy -= ACTION_ENERGY_CONSUMPTION;
            self.transform.position.x += self.transform.rotation.cos() * speed;
            self.transform.position.y += self.transform.rotation.sin() * speed;
        } else if controls.back {
            // self.energy -= ACTION_ENERGY_CONSUMPTION;
            self.transform.position.x -= self.transform.rotation.cos() * speed;
            self.transform.position.y -= self.transform.rotation.sin() * speed;
        }

        // Update rotation based on controls
        let rotation_speed = ROTATION_SPEED;
        if controls.right {
            self.transform.rotation += rotation_speed;
        }
        if controls.left {
            self.transform.rotation -= rotation_speed;
        }

        if controls.eat {
            self.energy -= ACTION_ENERGY_CONSUMPTION;
        }

        self.transform.rotation %= 2.0 * PI;
    }
}

const HEALTH: f32 = 100.;
const SPEED: f32 = 1.5;
const ROTATION_SPEED: f32 = 1.;
const DETECT_RANGE_FAR: f32 = 40.;
const DETECT_RANGE_CLOSE: f32 = 10.;
const EAT_DAMAGE: f32 = 30.;
const ACTION_ENERGY_CONSUMPTION: f32 = 0.001;

#[derive(Debug)]
struct World {
    microbes: QuadTree<Microbe>,
    scripts: HashMap<Uuid, String>,
    engine: Engine,
    time: f32,
}

impl World {
    fn new() -> Result<Self, Box<EvalAltResult>> {
        let mut engine = Engine::new();
        engine.build_type::<Controls>();
        let random = RandomPackage::new();

        random.register_into_engine(&mut engine);
        Ok(Self {
            microbes: QuadTree::new(
                Rect::new(-BOX_SIZE, -BOX_SIZE, BOX_SIZE * 2., BOX_SIZE * 2.),
                10,
            ),
            scripts: HashMap::new(),
            engine,
            time: 0.0,
        })
    }

    fn add_microbe(
        &mut self,
        x: f32,
        y: f32,
        rotation: f32,
        script_id: Uuid,
        color: Color32,
    ) -> Uuid {
        let microbe = Microbe::new(x, y, rotation, script_id, color);
        let id = microbe.id;
        self.microbes.insert(microbe);
        id
    }

    fn update(&mut self, delta_time: f32) -> Result<(), Box<EvalAltResult>> {
        self.time += delta_time;

        let mut result =
            QuadTree::<Microbe>::new(self.microbes.root.bounds, self.microbes.root.capacity);

        let frozen = self.microbes.clone();
        let items = self.microbes.take_items();
        let microbes = items.into_iter().fold(HashMap::new(), |mut acc, i| {
            acc.insert(i.id, i);
            acc
        });

        let mut microbe_controls = HashMap::<Uuid, (Controls, Vec<Uuid>)>::new();
        for microbe in microbes.values() {
            let transform = microbe.transform;

            let close_range = DETECT_RANGE_CLOSE;
            let far_range = DETECT_RANGE_FAR;

            let microbes_front_microbes_close = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation,
                close_range,
            );
            let sense_front_close = microbes_front_microbes_close.len() as i64;
            let sense_left_close = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation - (PI * 0.5),
                close_range,
            )
            .len() as i64;
            let sense_right_close = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation + (PI * 0.5),
                close_range,
            )
            .len() as i64;
            let sense_back_close = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation + PI,
                close_range,
            )
            .len() as i64;

            let sense_front = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation,
                far_range,
            )
            .len() as i64;
            let sense_left = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation - (PI * 0.5),
                far_range,
            )
            .len() as i64;
            let sense_right = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation + (PI * 0.5),
                far_range,
            )
            .len() as i64;
            let sense_back = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation + PI,
                far_range,
            )
            .len() as i64;

            // dbg!(
            //     &sense_back,
            //     &sense_front,
            //     &sense_left,
            //     &sense_right
            // );

            let sense_front_close = move || sense_front_close;
            let sense_left_close = move || sense_left_close;
            let sense_right_close = move || sense_right_close;
            let sense_back_close = move || sense_back_close;

            let sense_front = move || sense_front;
            let sense_left = move || sense_left;
            let sense_right = move || sense_right;
            let sense_back = move || sense_back;

            let energy = microbe.energy;
            let energy = move || energy;

            self.engine
                .register_fn("sense_front_close", sense_front_close);
            self.engine
                .register_fn("sense_left_close", sense_left_close);
            self.engine
                .register_fn("sense_right_close", sense_right_close);
            self.engine
                .register_fn("sense_back_close", sense_back_close);

            self.engine.register_fn("sense_front", sense_front);
            self.engine.register_fn("sense_left", sense_left);
            self.engine.register_fn("sense_right", sense_right);
            self.engine.register_fn("sense_back", sense_back);

            self.engine.register_fn("energy", energy);

            let controls = self
                .engine
                .eval::<Controls>(self.scripts.get(&microbe.script_id).unwrap())
                .expect("msg");

            microbe_controls.insert(
                microbe.id,
                (
                    controls,
                    microbes_front_microbes_close.iter().map(|i| i.id).collect(),
                ),
            );
        }

        let mut eaten = HashMap::<Uuid, i32>::new();
        let mut ate = HashMap::<Uuid, i32>::new();

        for (id, (controls, edible_ids)) in &microbe_controls {
            if controls.eat {
                for edible in edible_ids {
                    if let Some((edible_controls, _)) = microbe_controls.get(edible) {
                        if edible_controls.eat {
                            if microbes.get(id).unwrap().energy
                                > microbes.get(edible).unwrap().energy
                            {
                                eaten.insert(*edible, *eaten.get(edible).unwrap_or(&0) + 1);
                                ate.insert(*id, *ate.get(id).unwrap_or(&0) + 1);
                            }
                        } else {
                            eaten.insert(*edible, *eaten.get(edible).unwrap_or(&0) + 1);
                            ate.insert(*id, *ate.get(id).unwrap_or(&0) + 1);
                        }
                    }
                }
            }
        }

        for mut microbe in microbes.into_values() {
            if let Some((controls, _)) = microbe_controls.get(&microbe.id) {
                microbe.update(controls, delta_time);
            }

            microbe.transform.position.x = microbe.transform.position.x.clamp(-BOX_SIZE, BOX_SIZE);
            microbe.transform.position.y = microbe.transform.position.y.clamp(-BOX_SIZE, BOX_SIZE);

            if let Some(_ate_amount) = ate.get(&microbe.id) {
                // microbe.energy += *ate_amount as f32 * EAT_DAMAGE
                microbe.energy += EAT_DAMAGE;
            }
            if let Some(eaten_amount) = eaten.get(&microbe.id) {
                microbe.energy -= *eaten_amount as f32 * EAT_DAMAGE
            }
            if microbe.energy >= HEALTH + HEALTH {
                // PROCREATE
                microbe.energy -= HEALTH;
                let mut child = microbe.clone();
                child.id = Uuid::new_v4();
                child.energy = HEALTH * 0.25;
                result.insert(child.clone());
                let mut child = microbe.clone();
                child.id = Uuid::new_v4();
                child.energy = HEALTH * 0.25;
                result.insert(child.clone());
                let mut child = microbe.clone();
                child.id = Uuid::new_v4();
                child.energy = HEALTH * 0.25;
                result.insert(child.clone());
                let mut child = microbe.clone();
                child.id = Uuid::new_v4();
                child.energy = HEALTH * 0.25;
                result.insert(child.clone());
            }
            if microbe.energy > 0. {
                // DEATH
                result.insert(microbe);
            }
        }
        self.microbes = result;
        Ok(())
    }

    fn get_nearby_microbes(
        microbes: &QuadTree<Microbe>,
        id: Uuid,
        lineage: Uuid,
        position: Vector2,
        angle: f32,
        range: f32,
    ) -> Vec<&Microbe> {
        microbes
            .query(&Rect::new(
                position.x - range * 0.5,
                position.y - range * 0.5,
                range,
                range,
            ))
            .into_iter()
            .filter(|m| {
                if id == m.id || lineage == m.lineage {
                    return false;
                }
                let dx = m.transform.position.x - position.x;
                let dy = m.transform.position.y - position.y;
                let distance = (dx * dx + dy * dy).sqrt();

                if distance > range {
                    return false;
                }

                let angle_to = dy.atan2(dx);
                let angle_diff = (angle_to - angle).abs() % (2.0 * PI);
                let cone = PI * 0.4;
                angle_diff < cone
            })
            .collect()
    }
}

fn main() -> eframe::Result {
    let mut world = World::new().unwrap();

    let mut rng = rand::thread_rng();
    let random_script_id = Uuid::new_v4();
    world.scripts.insert(random_script_id, random_script());
    let hunter_script_id = Uuid::new_v4();
    world
        .scripts
        .insert(hunter_script_id, aggressive_hunter_script());
    let script_b = Uuid::new_v4();
    world.scripts.insert(script_b, vampire_microbe_script());
    let script_c = Uuid::new_v4();
    world.scripts.insert(script_c, timid_herbivore_script());
    for _ in 0..500 {
        if rng.gen_bool(0.5) {
            world.add_microbe(
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(0.0..=(2. * PI)),
                script_b,
                Color32::from_rgb(
                    100,
                    // 100,
                    // rng.gen_range(0..=255),
                    rng.gen_range(0..=255),
                    rng.gen_range(0..=255),
                ),
            );
        } else if rng.gen_bool(0.5) {
            world.add_microbe(
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(0.0..=(2. * PI)),
                script_c,
                Color32::from_rgb(
                    255,
                    rng.gen_range(0..=50),
                    // rng.gen_range(0..=255),
                    rng.gen_range(0..=50),
                ),
            );
        } else {
            world.add_microbe(
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(0.0..=(2. * PI)),
                hunter_script_id,
                Color32::from_rgb(
                    rng.gen_range(0..=255),
                    255,
                    // rng.gen_range(0..=255),
                    rng.gen_range(0..=255),
                ),
            );
        }
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([BOX_SIZE * 2., BOX_SIZE * 2.])
            .with_min_inner_size([BOX_SIZE * 2., BOX_SIZE * 2.]),
        ..Default::default()
    };
    eframe::run_native(
        "Game Visualization",
        native_options,
        Box::new(|_cc| Ok(Box::new(world))),
    )?;
    Ok(())
}

// Create & modify a `Contols` object to return to the application
// All actions besides turning cost a small amount of energy
// let controls = new_controls();
// controls.forward = true;
// controls.left = true;
// controls.right = true;
// controls.back = true;
// controls.eat = true;
//
// Returns the # of enemy microbes in range, in all 4 directions
// let front_far = sense_front();
// let left_far = sense_left();
// let right_far = sense_right();
// let back_far = sense_back();
//
// Returns the # of enemy microbes within attack range, in all 4 directions
// (You can only attack microbes in front of you)
// let front = sense_front_close();
// let left = sense_left_close();
// let right = sense_right_close();
// let back = sense_back_close();
//
// Returns your current energy amount, you must eat to survive!
// let my_energy = energy();

// Aggressive hunter that directly chases the nearest microbe
fn aggressive_hunter_script() -> String {
    r#"
        let controls = new_controls();

        // Check all directions for closest target
        let front_far = sense_front();
        let left_far = sense_left();
        let right_far = sense_right();
        let back_far = sense_back();

        // Periodically turn to search
        if rand(0..=100) > 95 {
            controls.forward = true;
            if rand(0..=1) > 0.5 {
                controls.right = true;
            } else {
                controls.left = true;
            }
        }
        if front_far > 0 {
            controls.forward = true;
        }

        if left_far > 0 {
            controls.left = true;
            controls.forward = true;
        }
        else
        if right_far > 0 {
            controls.right = true;
            controls.forward = true;
        }

        if sense_front_close() > 0 {
            controls.eat = true;
        }

        return controls;
    "#
    .to_string()
}

fn vampire_microbe_script() -> String {
    r#"
        let controls = new_controls();
        let energy = energy();

        // Sensing at different ranges
        let front_far = sense_front();
        let front_close = sense_front_close();
        let left_far = sense_left();
        let right_far = sense_right();
        let left_close = sense_left_close();
        let right_close = sense_right_close();

        // Energy conservation mode when low
        if energy < 30 {
            // If prey is right in front, still take the opportunity
            if front_close > 0 {
                controls.eat = true;
                return controls;
            }

            // Otherwise minimize movement and wait for energy regeneration
            if front_far > 0 || left_far > 0 || right_far > 0 {
                controls.back = true;
                return controls;
            }

            // Occasional random movement to avoid getting stuck
            if rand(0..=100) > 95 {
                controls.forward = true;
            }
            return controls;
        }

        // Hunting mode when energy is sufficient
        if front_close > 0 {
            // Attack if prey is in range
            controls.eat = true;
        } else if front_far > 0 {
            // Stalk prey that's further away
            controls.forward = true;
        } else if left_close > 0 || left_far > 0 {
            // Turn towards nearby prey
            controls.left = true;
            if left_close == 0 {  // If not too close, move forward while turning
                controls.forward = true;
            }
        } else if right_close > 0 || right_far > 0 {
            // Turn towards nearby prey
            controls.right = true;
            if right_close == 0 {  // If not too close, move forward while turning
                controls.forward = true;
            }
        } else {
            // Search pattern when no prey is detected
            controls.forward = true;
            if rand(0..=100) > 92 {
                if rand(0..=1) > 0.5 {
                    controls.left = true;
                } else {
                    controls.right = true;
                }
            }
        }

        return controls;
    "#
    .to_string()
}

fn timid_herbivore_script() -> String {
    r#"
        let controls = new_controls();

        // Detect threats
        let front_far = sense_front();
        let left_far = sense_left();
        let right_far = sense_right();
        let back_far = sense_back();

        // Check for food in eating range
        let front_close = sense_front_close();

        // Run away if any threats are detected
        if front_far > 0 || front_close > 0 {
            controls.back = true;
            // Pick random direction to flee
            if rand(0..=1) > 0.5 {
                controls.left = true;
            } else {
                controls.right = true;
            }
            return controls;
        }

        if left_far > 0 {
            controls.right = true;
            controls.forward = true;
            return controls;
        }

        if right_far > 0 {
            controls.left = true;
            controls.forward = true;
            return controls;
        }

        // If something is directly in front, try to eat it
        // (game will only let us eat valid food)
        if front_close > 0 {
            controls.eat = true;
            return controls;
        }

        // When no threats, occasionally move to find food
        if rand(0..=100) > 80 {
            controls.forward = true;
            // Sometimes turn while moving
            if rand(0..=100) > 70 {
                if rand(0..=1) > 0.5 {
                    controls.left = true;
                } else {
                    controls.right = true;
                }
            }
        }

        return controls;
    "#
    .to_string()
}

fn random_script() -> String {
    r#"
        let controls = new_controls();

        if rand(0..=1) > 0.5 {
            controls.right = true;
        } else {
            controls.left = true;
        }
        controls.forward = true;

        return controls;
    "#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_get_nearby_microbes() {
        let mut microbes = QuadTree::new(Rect::new(-3., -3., 3., 3.), 10);

        fn assert_detected(angle: f32, m: Microbe, ms: &mut QuadTree<Microbe>) {
            let position = Vector2 { x: 0.0, y: 0.0 };
            let range = 10.0;
            let id = Uuid::new_v4();
            let lineage = Uuid::new_v4();
            ms.insert(m.clone());
            assert!(
                World::get_nearby_microbes(ms, id, lineage, position, angle, range).contains(&&m)
            );
        }

        // FORWARD
        assert_detected(
            0.,
            Microbe {
                id: Uuid::new_v4(),
                lineage: Uuid::new_v4(),
                transform: Transform {
                    position: Vector2 { x: 1.0, y: 0.0 },
                    rotation: 0.0,
                },
                script_id: Uuid::new_v4(),
                energy: 100.,
                color: Color32::WHITE,
            },
            &mut microbes,
        );

        // BACK
        assert_detected(
            PI,
            Microbe {
                id: Uuid::new_v4(),
                lineage: Uuid::new_v4(),
                transform: Transform {
                    position: Vector2 { x: -1.0, y: 0.0 },
                    rotation: 0.0,
                },
                script_id: Uuid::new_v4(),
                energy: 100.,
                color: Color32::WHITE,
            },
            &mut microbes,
        );

        // RIGHT
        assert_detected(
            PI * 0.5,
            Microbe {
                id: Uuid::new_v4(),
                lineage: Uuid::new_v4(),
                transform: Transform {
                    position: Vector2 { x: 0.0, y: 1.0 },
                    rotation: 0.0,
                },
                script_id: Uuid::new_v4(),
                energy: 100.,
                color: Color32::WHITE,
            },
            &mut microbes,
        );

        // LEFT
        assert_detected(
            -PI * 0.5,
            Microbe {
                id: Uuid::new_v4(),
                lineage: Uuid::new_v4(),
                transform: Transform {
                    position: Vector2 { x: 0.0, y: -1.0 },
                    rotation: 0.0,
                },
                script_id: Uuid::new_v4(),
                energy: 100.,
                color: Color32::WHITE,
            },
            &mut microbes,
        );
    }
}
