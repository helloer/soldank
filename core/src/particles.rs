use super::*;

// Notes:
// * Particle & constraint vectors are kept private to prevent mismatch between
//   zero-based and one-based indexing.
// * These private vectors are zero-based (no dummy element at zero index).
// * Everything here that is called 'num' is a one-based index.

#[derive(Debug, Copy, Clone)]
pub struct Particle {
    pub active: bool,
    pub pos: Vec2,
    pub old_pos: Vec2,
    pub velocity: Vec2,
    pub force: Vec2,
    pub one_over_mass: f32,
    pub timestep: f32,
    pub gravity: f32,
    pub e_damping: f32,
    pub v_damping: f32,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Constraint {
    pub active: bool,
    pub particle_num: (usize, usize),
    pub rest_length: f32,
}

#[derive(Debug, Default, Clone)]
pub struct ParticleSystem {
    particles: Vec<Particle>,
    constraints: Vec<Constraint>,
}

impl Default for Particle {
    fn default() -> Particle {
        Particle {
            active: false,
            pos: Vec2::ZERO,
            old_pos: Vec2::ZERO,
            velocity: Vec2::ZERO,
            force: Vec2::ZERO,
            one_over_mass: 0.0,
            timestep: 0.0,
            gravity: 0.0,
            e_damping: 0.0,
            v_damping: 0.0,
        }
    }
}

impl Particle {
    pub fn euler(&mut self) {
        self.old_pos = self.pos;
        self.force.y += self.gravity;
        self.velocity += self.force * self.one_over_mass * self.timestep.powi(2);
        self.pos += self.velocity;
        self.velocity *= self.e_damping;
        self.force = Vec2::ZERO;
    }

    pub fn verlet(&mut self) {
        let a = self.pos * (1.0 + self.v_damping);
        let b = self.old_pos * self.v_damping;

        self.old_pos = self.pos;
        self.force.y += self.gravity;
        self.pos = a - b + self.force * self.one_over_mass * self.timestep.powi(2);
        self.force = Vec2::ZERO;
    }
}

impl Constraint {
    pub fn new(a_num: usize, b_num: usize, rest_length: f32) -> Constraint {
        Constraint {
            active: true,
            particle_num: (a_num, b_num),
            rest_length,
        }
    }
}

impl ParticleSystem {
    #[allow(dead_code)]
    pub fn new() -> ParticleSystem {
        Default::default()
    }

    pub fn active(&self, particle_num: usize) -> bool {
        self.particles[particle_num - 1].active
    }

    pub fn pos(&self, particle_num: usize) -> Vec2 {
        self.particles[particle_num - 1].pos
    }

    pub fn pos_mut(&mut self, particle_num: usize) -> &mut Vec2 {
        &mut self.particles[particle_num - 1].pos
    }

    pub fn old_pos(&self, particle_num: usize) -> Vec2 {
        self.particles[particle_num - 1].old_pos
    }

    pub fn old_pos_mut(&mut self, particle_num: usize) -> &mut Vec2 {
        &mut self.particles[particle_num - 1].old_pos
    }

    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    /// Enables or disables constraint `num` (1-based, as in Soldat).
    pub fn set_constraint_active(&mut self, num: usize, active: bool) {
        if let Some(constraint) = self.constraints.get_mut(num - 1) {
            constraint.active = active;
        }
    }

    pub fn activate_all_constraints(&mut self) {
        for constraint in &mut self.constraints {
            constraint.active = true;
        }
    }

    /// Sets timestep, gravity (multiplier * Grav) and verlet damping of every particle
    /// (Soldat keeps them per particle system).
    pub fn set_physics(&mut self, timestep: f32, gravity: f32, v_damping: f32) {
        for particle in &mut self.particles {
            particle.timestep = timestep;
            particle.gravity = gravity;
            particle.v_damping = v_damping;
        }
    }

    pub fn do_verlet_timestep(&mut self) {
        for particle in self.particles.iter_mut() {
            if particle.active {
                particle.verlet();
            }
        }

        self.satisfy_constraints();
    }

    pub fn do_verlet_timestep_for(&mut self, particle_num: usize, constraint_num: usize) {
        self.particles[particle_num - 1].verlet();
        self.satisfy_constraint_for(constraint_num);
    }

    #[allow(dead_code)]
    pub fn do_eurler_timestep(&mut self) {
        for particle in self.particles.iter_mut() {
            if particle.active {
                particle.euler();
            }
        }
    }

    #[allow(dead_code)]
    pub fn do_eurler_timestep_for(&mut self, particle_num: usize) {
        self.particles[particle_num - 1].euler()
    }

    pub fn satisfy_constraints(&mut self) {
        for constraint in self.constraints.iter() {
            if constraint.active {
                Self::satisfy_constraint(constraint, &mut self.particles);
            }
        }
    }

    pub fn satisfy_constraint_for(&mut self, constraint_num: usize) {
        Self::satisfy_constraint(&self.constraints[constraint_num - 1], &mut self.particles);
    }

    fn satisfy_constraint(constraint: &Constraint, particles: &mut [Particle]) {
        let (a, b) = (constraint.particle_num.0 - 1, constraint.particle_num.1 - 1);

        let delta = particles[b].pos - particles[a].pos;
        let length = delta.length();

        if length > 0.0 {
            let diff = (length - constraint.rest_length) / length;

            if particles[a].one_over_mass > 0.0 {
                particles[a].pos += delta * diff / 2.0;
            }

            if particles[b].one_over_mass > 0.0 {
                particles[b].pos -= delta * diff / 2.0;
            }
        }
    }

    /// Parses a `.po` object: particle coordinates until `CONSTRAINTS`, then pairs of
    /// `Pn` particle references until `ENDFILE`.
    pub fn parse(
        file: &str,
        text: &str,
        scale: f32,
        timestep: f32,
        gravity: f32,
        e_damping: f32,
        v_damping: f32,
    ) -> Result<ParticleSystem, DataError> {
        let mut lines = text.lines().map(str::trim);
        let mut particles: Vec<Particle> = Vec::new();
        let mut constraints: Vec<Constraint> = Vec::new();

        let error = |what: &str, line: Option<&str>| {
            DataError::parse(file, format!("expected {what}, got {line:?}"))
        };

        // StrToFloat returns Extended, so the scaling happens in extended precision
        let number = |lines: &mut dyn Iterator<Item = &str>| -> Result<f64, DataError> {
            let line = lines.next();
            line.and_then(|l| l.parse().ok())
                .ok_or_else(|| error("a number", line))
        };

        // particle references look like "P12"
        let index = |line: Option<&str>, count: usize| -> Result<usize, DataError> {
            line.and_then(|l| l.get(1..))
                .and_then(|l| l.trim().parse().ok())
                .filter(|i| (1..=count).contains(i))
                .ok_or_else(|| error("a particle reference", line))
        };

        loop {
            match lines.next() {
                Some("CONSTRAINTS") => break,
                None => return Err(error("CONSTRAINTS", None)),
                Some(_) => {}
            }

            let x = number(&mut lines)?;
            let _y = number(&mut lines)?;
            let z = number(&mut lines)?;
            let p = vec2(fpc(-x * ext(scale) / 1.2), fpc(-z * ext(scale)));

            particles.push(Particle {
                active: true,
                pos: p,
                old_pos: p,
                velocity: Vec2::ZERO,
                force: Vec2::ZERO,
                one_over_mass: 1.0,
                timestep,
                gravity,
                e_damping,
                v_damping,
            });
        }

        loop {
            let pa_num = match lines.next() {
                None | Some("") | Some("ENDFILE") => break,
                line => index(line, particles.len())?,
            };

            let pb_num = index(lines.next(), particles.len())?;

            let delta = particles[pa_num - 1].pos - particles[pb_num - 1].pos;
            constraints.push(Constraint::new(pa_num, pb_num, delta.length()));
        }

        Ok(ParticleSystem {
            particles,
            constraints,
        })
    }
}
