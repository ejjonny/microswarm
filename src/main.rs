use rand::Rng;
use rhai::packages::Package; // needed for 'Package' trait
use rhai::{CustomType, Engine, EvalAltResult, TypeBuilder};
use rhai_rand::RandomPackage;
use std::collections::HashMap;
use std::f64::consts::PI;
use uuid::Uuid;

const BOX_SIZE: f64 = 300.;

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
struct Controls {
    right: i64,
    left: i64,
    forward: i64,
    back: i64,
}

impl Controls {
    pub fn new() -> Self {
        Self {
            right: 0,
            left: 0,
            forward: 0,
            back: 0,
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
            for microbe in self.microbes.values() {
                let player_pos = egui::pos2(
                    microbe.transform.position.x as f32 + BOX_SIZE as f32 + 10.,
                    microbe.transform.position.y as f32 + BOX_SIZE as f32 + 10.,
                );
                painter.circle_filled(player_pos, 3.0, egui::Color32::WHITE);

                let direction = egui::vec2(
                    microbe.transform.rotation.cos() as f32,
                    microbe.transform.rotation.sin() as f32,
                );
                let line_end = player_pos + direction * 3.0;
                painter.line_segment(
                    [player_pos, line_end],
                    egui::Stroke::new(2.0, egui::Color32::RED),
                );
            }
        });
        ctx.request_repaint();
    }
}

// Position and movement related structs
#[derive(Debug, Clone, Copy, PartialEq)]
struct Vector2 {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Transform {
    position: Vector2,
    rotation: f64,
}

impl Transform {
    fn new(x: f64, y: f64, rotation: f64) -> Self {
        Self {
            position: Vector2 { x, y },
            rotation,
        }
    }
}

// Main microbe struct
#[derive(Debug, Clone, PartialEq)]
struct Microbe {
    id: Uuid,
    transform: Transform,
    script: String,
    energy: f64,
    age: u32,
}

impl Microbe {
    fn new(x: f64, y: f64, rotation: f64, script: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            transform: Transform::new(x, y, rotation),
            script,
            energy: 100.0,
            age: 0,
        }
    }

    fn update(&mut self, controls: &Controls, delta_time: f64) {
        // Apply controls to movement
        let speed = SPEED * delta_time;

        // Update position based on controls
        if controls.forward > 0 {
            self.transform.position.x += self.transform.rotation.cos() * speed;
            self.transform.position.y += self.transform.rotation.sin() * speed;
        }
        if controls.back > 0 {
            self.transform.position.x -= self.transform.rotation.cos() * speed;
            self.transform.position.y -= self.transform.rotation.sin() * speed;
        }

        // Update rotation based on controls
        let rotation_speed = ROTATION_SPEED * delta_time;
        if controls.right > 0 {
            self.transform.rotation += rotation_speed;
        }
        if controls.left > 0 {
            self.transform.rotation -= rotation_speed;
        }

        // Normalize rotation to keep it between 0 and 2π
        self.transform.rotation %= 2.0 * PI;

        // Update energy and age
        self.energy -= delta_time * 2.0; // Basic energy consumption
        self.age += 1;
    }
}

const SPEED: f64 = 30.0;
const ROTATION_SPEED: f64 = 10.;

#[derive(Debug)]
struct World {
    microbes: HashMap<Uuid, Microbe>,
    engine: Engine,
    time: f64,
}

impl World {
    fn new() -> Result<Self, Box<EvalAltResult>> {
        let mut engine = Engine::new();
        engine.build_type::<Controls>();
        let random = RandomPackage::new();

        random.register_into_engine(&mut engine);

        Ok(Self {
            microbes: HashMap::new(),
            engine,
            time: 0.0,
        })
    }

    fn add_microbe(&mut self, x: f64, y: f64, rotation: f64, script: String) -> Uuid {
        let microbe = Microbe::new(x, y, rotation, script);
        let id = microbe.id;
        self.microbes.insert(id, microbe);
        id
    }

    fn update(&mut self, delta_time: f64) -> Result<(), Box<EvalAltResult>> {
        self.time += delta_time;

        let microbe_ids: Vec<Uuid> = self.microbes.keys().cloned().collect();

        for id in microbe_ids {
            let transform = self.microbes.get(&id).unwrap().transform;

            let close_range = 10.;
            let far_range = 60.;

            let microbes_forward_close = World::get_nearby_microbes(
                &self.microbes,
                id,
                transform.position,
                transform.rotation,
                close_range,
            )
            .len() as i64;
            let microbes_left_close = World::get_nearby_microbes(
                &self.microbes,
                id,
                transform.position,
                transform.rotation - (PI * 0.5),
                close_range,
            )
            .len() as i64;
            let microbes_right_close = World::get_nearby_microbes(
                &self.microbes,
                id,
                transform.position,
                transform.rotation + (PI * 0.5),
                close_range,
            )
            .len() as i64;
            let microbes_backward_close = World::get_nearby_microbes(
                &self.microbes,
                id,
                transform.position,
                transform.rotation + PI,
                close_range,
            )
            .len() as i64;

            let microbes_forward_far = World::get_nearby_microbes(
                &self.microbes,
                id,
                transform.position,
                transform.rotation,
                far_range,
            )
            .len() as i64;
            let microbes_left_far = World::get_nearby_microbes(
                &self.microbes,
                id,
                transform.position,
                transform.rotation - (PI * 0.5),
                far_range,
            )
            .len() as i64;
            let microbes_right_far = World::get_nearby_microbes(
                &self.microbes,
                id,
                transform.position,
                transform.rotation + (PI * 0.5),
                far_range,
            )
            .len() as i64;
            let microbes_backward_far = World::get_nearby_microbes(
                &self.microbes,
                id,
                transform.position,
                transform.rotation + PI,
                far_range,
            )
            .len() as i64;

            let microbes_forward_close = move || microbes_forward_close;
            let microbes_left_close = move || microbes_left_close;
            let microbes_right_close = move || microbes_right_close;
            let microbes_backward_close = move || microbes_backward_close;

            let microbes_forward_far = move || microbes_forward_far;
            let microbes_left_far = move || microbes_left_far;
            let microbes_right_far = move || microbes_right_far;
            let microbes_backward_far = move || microbes_backward_far;

            if let Some(microbe) = self.microbes.get_mut(&id) {
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

                microbe.update(&controls, delta_time);

                microbe.transform.position.x =
                    microbe.transform.position.x.clamp(-BOX_SIZE, BOX_SIZE);
                microbe.transform.position.y =
                    microbe.transform.position.y.clamp(-BOX_SIZE, BOX_SIZE);

                // if microbe.energy <= 0.0 {
                //     self.microbes.remove(&id);
                // }
            }
        }
        Ok(())
    }

    fn get_nearby_microbes(
        microbes: &HashMap<Uuid, Microbe>,
        id: Uuid,
        position: Vector2,
        angle: f64,
        range: f64,
    ) -> Vec<&Microbe> {
        microbes
            .values()
            .filter(|m| {
                if id == m.id {
                    return false;
                }
                let dx = m.transform.position.x - position.x;
                let dy = m.transform.position.y - position.y;
                let distance = (dx * dx + dy * dy).sqrt();

                if distance < 1. || distance > range {
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

// Example usage
fn main() -> eframe::Result {
    let mut world = World::new().unwrap();

    let mut rng = rand::thread_rng();
    for _ in 0..800 {
        if rng.gen_bool(0.1) {
            world.add_microbe(
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(0.0..=(2. * PI)),
                random_script(),
            );
        } else {
            world.add_microbe(
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(-BOX_SIZE..BOX_SIZE),
                rng.gen_range(0.0..=(2. * PI)),
                aggressive_hunter_script(),
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
            if rand(0..=1) > 0.5 {
                controls.right = 1;
            } else {
                controls.left = 1;
            }
        }
        if front_far > 0 {
            controls.forward = 1;
        }

        if left_far > 0 {
            controls.left = 1;
            controls.forward = 1;
        }
        else
        if right_far > 0 {
            controls.right = 1;
            controls.forward = 1;
        }

        return controls;
    "#
    .to_string()
}

fn random_script() -> String {
    r#"
        let controls = new_controls();

        if rand(0..=1) > 0.5 {
            controls.right = 1;
        } else {
            controls.left = 1;
        }
        controls.forward = 1;

        return controls;
    "#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn test_get_nearby_microbes() {
        // Set up test data
        let mut microbes = HashMap::new();

        fn assert_detected(angle: f64, m: Microbe, ms: &mut HashMap<Uuid, Microbe>) {
            let position = Vector2 { x: 0.0, y: 0.0 };
            let range = 10.0;
            let id = Uuid::new_v4();
            ms.insert(m.id, m.clone());
            assert!(World::get_nearby_microbes(ms, id, position, angle, range).contains(&&m));
        }

        // FORWARD
        assert_detected(
            0.,
            Microbe {
                id: Uuid::new_v4(),
                transform: Transform {
                    position: Vector2 { x: 1.0, y: 0.0 },
                    rotation: 0.0,
                },
                script: String::from("Test"),
                energy: 100.0,
                age: 1,
            },
            &mut microbes,
        );

        // BACK
        assert_detected(
            PI,
            Microbe {
                id: Uuid::new_v4(),
                transform: Transform {
                    position: Vector2 { x: -1.0, y: 0.0 },
                    rotation: 0.0,
                },
                script: String::from("Test"),
                energy: 100.0,
                age: 1,
            },
            &mut microbes,
        );

        // RIGHT
        assert_detected(
            PI * 0.5,
            Microbe {
                id: Uuid::new_v4(),
                transform: Transform {
                    position: Vector2 { x: 0.0, y: 1.0 },
                    rotation: 0.0,
                },
                script: String::from("Test"),
                energy: 100.0,
                age: 1,
            },
            &mut microbes,
        );

        // LEFT
        assert_detected(
            -PI * 0.5,
            Microbe {
                id: Uuid::new_v4(),
                transform: Transform {
                    position: Vector2 { x: 0.0, y: -1.0 },
                    rotation: 0.0,
                },
                script: String::from("Test"),
                energy: 100.0,
                age: 1,
            },
            &mut microbes,
        );
    }
}
