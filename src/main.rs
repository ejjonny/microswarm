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

const BOX_SIZE: f32 = 300.;

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
                    microbe.transform.position.x + BOX_SIZE + 10.,
                    microbe.transform.position.y + BOX_SIZE + 10.,
                );
                let size = (microbe.energy / (HEALTH * 2.)) + 1.;
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
    script: String,
    energy: f32,
    color: Color32,
}

impl Locatable for Microbe {
    fn location(&self) -> Point {
        Point::new(self.transform.position.x, self.transform.position.y)
    }
}

impl Microbe {
    fn new(x: f32, y: f32, rotation: f32, script: String, color: Color32) -> Self {
        Self {
            id: Uuid::new_v4(),
            lineage: Uuid::new_v4(),
            transform: Transform::new(x, y, rotation),
            script,
            energy: HEALTH,
            color,
        }
    }

    fn update(&mut self, controls: &Controls, _delta_time: f32) {
        // Apply controls to movement
        let speed = SPEED;

        // Update position based on controls
        if controls.forward {
            // self.energy -= 1;
            self.transform.position.x += self.transform.rotation.cos() * speed;
            self.transform.position.y += self.transform.rotation.sin() * speed;
        } else if controls.back {
            // self.energy -= 1;
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
            self.energy -= 0.1;
        }

        self.transform.rotation %= 2.0 * PI;
    }
}

const HEALTH: f32 = 100.;
const SPEED: f32 = 2.;
const ROTATION_SPEED: f32 = 0.5;
const DETECT_RANGE_FAR: f32 = 75.;
const DETECT_RANGE_CLOSE: f32 = 10.;
const EAT_DAMAGE: f32 = 10.;

#[derive(Debug)]
struct World {
    microbes: QuadTree<Microbe>,
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
            engine,
            time: 0.0,
        })
    }

    fn add_microbe(
        &mut self,
        x: f32,
        y: f32,
        rotation: f32,
        script: String,
        color: Color32,
    ) -> Uuid {
        let microbe = Microbe::new(x, y, rotation, script, color);
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

            let microbes_forward_items = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation,
                close_range,
            );
            let microbes_forward_close = microbes_forward_items.len() as i64;
            let microbes_left_close = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation - (PI * 0.5),
                close_range,
            )
            .len() as i64;
            let microbes_right_close = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation + (PI * 0.5),
                close_range,
            )
            .len() as i64;
            let microbes_backward_close = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation + PI,
                close_range,
            )
            .len() as i64;

            let microbes_forward_far = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation,
                far_range,
            )
            .len() as i64;
            let microbes_left_far = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation - (PI * 0.5),
                far_range,
            )
            .len() as i64;
            let microbes_right_far = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation + (PI * 0.5),
                far_range,
            )
            .len() as i64;
            let microbes_backward_far = World::get_nearby_microbes(
                &frozen,
                microbe.id,
                microbe.lineage,
                transform.position,
                transform.rotation + PI,
                far_range,
            )
            .len() as i64;

            // dbg!(
            //     &microbes_backward_far,
            //     &microbes_forward_far,
            //     &microbes_left_far,
            //     &microbes_right_far
            // );

            let microbes_forward_close = move || microbes_forward_close;
            let microbes_left_close = move || microbes_left_close;
            let microbes_right_close = move || microbes_right_close;
            let microbes_backward_close = move || microbes_backward_close;

            let microbes_forward_far = move || microbes_forward_far;
            let microbes_left_far = move || microbes_left_far;
            let microbes_right_far = move || microbes_right_far;
            let microbes_backward_far = move || microbes_backward_far;

            self.engine
                .register_fn("microbes_forward_close", microbes_forward_close);
            self.engine
                .register_fn("microbes_left_close", microbes_left_close);
            self.engine
                .register_fn("microbes_right_close", microbes_right_close);
            self.engine
                .register_fn("microbes_backward_close", microbes_backward_close);

            self.engine
                .register_fn("microbes_forward_far", microbes_forward_far);
            self.engine
                .register_fn("microbes_left_far", microbes_left_far);
            self.engine
                .register_fn("microbes_right_far", microbes_right_far);
            self.engine
                .register_fn("microbes_backward_far", microbes_backward_far);

            let controls = self.engine.eval::<Controls>(&microbe.script).expect("msg");

            microbe_controls.insert(
                microbe.id,
                (
                    controls,
                    microbes_forward_items.iter().map(|i| i.id).collect(),
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
                child.energy = HEALTH / 2.;
                result.insert(child.clone());
                // let mut child = microbe.clone();
                // child.id = Uuid::new_v4();
                // child.energy = 5;
                // result.insert(child.clone());
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
    for _ in 0..1000 {
        if rng.gen_bool(0.01) {
            world.add_microbe(
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(0.0..=(2. * PI)),
                random_script(),
                Color32::from_rgb(
                    rng.gen_range(0..=255),
                    rng.gen_range(0..=255),
                    rng.gen_range(0..=255),
                ),
            );
        } else {
            world.add_microbe(
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(0.0..=(2. * PI)),
                aggressive_hunter_script(),
                Color32::from_rgb(
                    rng.gen_range(0..=255),
                    rng.gen_range(0..=255),
                    rng.gen_range(0..=255),
                ),
            );
        }
    }

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Game Visualization",
        native_options,
        Box::new(|_cc| Ok(Box::new(world))),
    )?;
    Ok(())
}

// Aggressive hunter that directly chases the nearest microbe
fn aggressive_hunter_script() -> String {
    r#"
        let controls = new_controls();

        // Check all directions for closest target
        let front_far = microbes_forward_far();
        let left_far = microbes_left_far();
        let right_far = microbes_right_far();
        let back_far = microbes_backward_far();

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

        if microbes_forward_close() > 0 {
            controls.eat = true;
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
                script: String::from("Test"),
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
                script: String::from("Test"),
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
                script: String::from("Test"),
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
                script: String::from("Test"),
                energy: 100.,
                color: Color32::WHITE,
            },
            &mut microbes,
        );
    }
}
