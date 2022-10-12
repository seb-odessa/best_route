use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::fmt;

use std::collections::HashMap;
use std::env;

const ROOT: &str = "https://esi.evetech.net/latest";
const PARAM: &str = "?datasource=tranquility&language=en";

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
struct Position {
    x: f64,
    y: f64,
    z: f64,
}
impl Position {
    #[allow(dead_code)]
    pub fn new(x: &f64, y: &f64, z: &f64) -> Self {
        Self {
            x: *x,
            y: *y,
            z: *z,
        }
    }

    pub fn distance(a: &Self, b: &Self) -> f64 {
        ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
    }
}
impl fmt::Display for Position {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default, Eq)]
struct Planet {
    asteroid_belts: Option<Vec<i32>>,
    moons: Option<Vec<i32>>,
    planet_id: i32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
struct AsteroidBelt {
    name: String,
    position: Position,
    system_id: i32,
}
impl AsteroidBelt {
    pub async fn load(id: &i32) -> anyhow::Result<Self> {
        let url = format!("{ROOT}/universe/asteroid_belts/{id}/{PARAM}");
        debug!("url: {url}");
        Ok(reqwest::get(url).await?.json::<Self>().await?)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
struct System {
    constellation_id: i32,
    name: String,
    planets: Option<Vec<Planet>>,
    // position: Position,
    // security_class
    security_status: f32,
    // star_id
    // stargates
    // stations
    system_id: i32,
}
impl System {
    pub async fn load(id: &i32) -> anyhow::Result<Self> {
        let url = format!("{ROOT}/universe/systems/{id}/{PARAM}");
        debug!("url: {url}");
        Ok(reqwest::get(url).await?.json::<Self>().await?)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
struct Object {
    id: i32,
    name: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
struct Universe {
    agents: Option<Vec<Object>>,
    alliances: Option<Vec<Object>>,
    characters: Option<Vec<Object>>,
    constellations: Option<Vec<Object>>,
    corporations: Option<Vec<Object>>,
    factions: Option<Vec<Object>>,
    inventory_types: Option<Vec<Object>>,
    regions: Option<Vec<Object>>,
    stations: Option<Vec<Object>>,
    systems: Option<Vec<Object>>,
}
impl Universe {
    pub async fn load(names: &Vec<String>) -> anyhow::Result<Self> {
        let url = format!("{ROOT}/universe/ids/{PARAM}");
        debug!("url: {url}");
        let client = reqwest::Client::new();
        Ok(client
            .post(url)
            .json(names)
            .send()
            .await?
            .json::<Self>()
            .await?)
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
struct Belt {
    id: i32,
    name: String,
    position: Position,
}
impl Belt {
    pub fn new(id: &i32, name: &String, position: &Position) -> Self {
        Self {
            id: id.clone(),
            name: name.clone(),
            position: position.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
struct Cloud {
    belts: HashMap<i32, Belt>,
    distances: HashMap<i32, HashMap<i32, f64>>,
}
impl Cloud {
    pub fn new() -> Self {
        Self {
            belts: HashMap::new(),
            distances: HashMap::new(),
        }
    }

    pub fn add(&mut self, id: &i32, name: &String, position: &Position) {
        let belt = Belt::new(id, &name, &position);

        for (destination, belt) in &self.belts {
            let distance = Position::distance(&position, &belt.position);
            debug!("Distance between {} and {} - {}", name, belt.name, distance);

            self.distances
                .entry(*id)
                .or_insert(HashMap::new())
                .insert(*destination, distance);

            self.distances
                .entry(*destination)
                .or_insert(HashMap::new())
                .insert(*id, distance);
        }

        if let Some(old) = self.belts.insert(*id, belt) {
            warn!("The old value for {id} was replaced with: {:?}", old);
        }
    }

    fn distance_between(&self, a: &i32, b: &i32) -> Option<f64> {
        if let Some(ref value) = self.distances.get(a) {
            return value.get(b).cloned();
        }
        return None;
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_cloud() {
        let mut cloud = Cloud::new();
        assert_eq!(cloud.distance_between(&0, &1), None);

        cloud.add(&0, &String::from(" 0 "), &Position::new(&0.0, &0.0, &0.0));
        cloud.add(&1, &String::from(" 1 "), &Position::new(&1.0, &0.0, &0.0));

        assert_eq!(cloud.distance_between(&0, &1), Some(1.0));
        assert_eq!(cloud.distance_between(&1, &0), Some(1.0));

        //assert_eq!(add(1, 2), 3);
    }
}

async fn load_system_asteroids(system: &System) -> anyhow::Result<Vec<Cloud>> {
    let mut clouds = Vec::new();
    if let Some(ref planets) = system.planets {
        for planet in planets {
            let mut cloud = Cloud::new();
            if let Some(ref ids) = planet.asteroid_belts {
                for id in ids {
                    let belt = AsteroidBelt::load(id).await?;
                    info!("Belt: {id} - {}: {}", belt.name, belt.position);
                    cloud.add(id, &belt.name, &belt.position);
                }
            }
            if !cloud.belts.is_empty() {
                clouds.push(cloud);
            }
        }
    }
    Ok(clouds)
}

fn fmt(distance: &f64) -> String {
    format!("{} million Km.", (distance / 1000000.0).round())
}

async fn run(id: &i32) -> anyhow::Result<()> {
    let system = System::load(id).await?;
    info!("system_name: {}", system.name);

    let clouds = load_system_asteroids(&system).await?;
    info!("Belt clouds: {}", clouds.len());

    for cloud in &clouds {
        let mut belts = cloud.belts.values().cloned().collect::<Vec<_>>();
        belts.sort_by(|x, y| x.name.cmp(&y.name));
        let mut total = 0.0;
        let mut sorted_belts = belts.into_iter();

        if let Some(first_belt) = sorted_belts.next() {
            let mut last_belt = first_belt.clone();
            while let Some(belt) = sorted_belts.next() {
                if let Some(dist) = cloud.distance_between(&last_belt.id, &belt.id) {
                    total += dist;
                    info!(
                        "Distance between `{}` and `{}` is {}",
                        last_belt.name,
                        belt.name,
                        fmt(&dist)
                    );
                }
                last_belt = belt;
            }
            let dist = Position::distance(&last_belt.position, &first_belt.position);
            total += dist;
            if 0.0 < total {
                info!(
                    "Distance between `{}` and `{}` is {}",
                    last_belt.name,
                    first_belt.name,
                    fmt(&dist)
                );
            }
        }

        if 0.0 < total {
            info!("The total route distance is {} million Km.", fmt(&total));
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let args: Vec<String> = env::args().collect();

    if let Some((cmd, names_ref)) = args.split_first() {
        let names = names_ref.to_vec();
        if names.is_empty() {
            println!("Usage\n\t{} <EveSystemName>", cmd);
        } else {
            let universe = Universe::load(&names);

            if let Some(systems) = universe.await?.systems {
                for obj in &systems {
                    info!("id: {} - {}", obj.id, obj.name);
                    run(&obj.id).await?;
                }
            }
        }
    }

    Ok(())
}
