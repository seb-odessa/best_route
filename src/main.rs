use itertools::Itertools;
use log::{debug, info, warn};
use septem::Roman;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::LinkedList;
use std::env;
use std::fmt;

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
struct Place {
    id: i32,
    name: String,
    position: Position,
    cloud_number: u32,
    belt_number: u32,
}
impl Place {
    pub fn new(id: &i32, name: &String, position: &Position) -> Self {
        let tokens = name.trim().split_whitespace().collect::<Vec<&str>>();
        assert_eq!(6, tokens.len());

        Self {
            id: id.clone(),
            name: name.clone(),
            position: position.clone(),
            cloud_number: *tokens[1].parse::<Roman>().unwrap(),
            belt_number: tokens[5].parse::<u32>().unwrap_or_default(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
struct Cloud {
    places: HashMap<i32, Place>,
    distances: HashMap<i32, HashMap<i32, f64>>,
}
impl Cloud {
    pub fn new() -> Self {
        Self {
            places: HashMap::new(),
            distances: HashMap::new(),
        }
    }

    pub fn get_name(&self, id: &i32) -> Option<String> {
        if let Some(belt) = self.places.get(id) {
            Some(belt.name.clone())
        } else {
            None
        }
    }

    pub fn add(&mut self, id: &i32, name: &String, position: &Position) {
        let belt = Place::new(id, &name, &position);

        for (destination, belt) in &self.places {
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

        if let Some(old) = self.places.insert(*id, belt) {
            warn!("The old value for {id} was replaced with: {:?}", old);
        }
    }

    pub fn distance_between(&self, a: &i32, b: &i32) -> Option<f64> {
        if let Some(ref value) = self.distances.get(a) {
            return value.get(b).cloned();
        }
        return None;
    }

    fn route_distance(&self, route: &Vec<&i32>) -> f64 {
        let mut distance = 0.0;
        route.iter().reduce(|a, b| {
            distance += self.distance_between(&a, &b).unwrap_or(0.0);
            return b;
        });
        return distance;
    }

    fn get_ids_sorted_by_name(&self) -> Vec<i32> {
        let mut places = self.places.values().cloned().collect::<Vec<Place>>();
        places.sort_by(|a, b| {
            if a.cloud_number == b.cloud_number {
                a.belt_number.cmp(&b.belt_number)
            } else {
                a.cloud_number.cmp(&b.cloud_number)
            }
        });
        places.into_iter().map(|belt| belt.id).collect::<Vec<i32>>()
    }

    pub fn get_ordinal_route(&self) -> (f64, Vec<i32>) {
        let points = self.get_ids_sorted_by_name();
        let refs = points.iter().collect::<Vec<&i32>>();
        (self.route_distance(&refs), points)
    }

    pub fn get_best_route(&self) -> (f64, Vec<i32>) {
        let points = self.get_ids_sorted_by_name();
        if points.is_empty() {
            (0.0, vec![])
        } else if 1 == points.len() {
            (0.0, points.clone())
        } else if 2 == points.len() {
            let refs = points.iter().collect::<Vec<&i32>>();
            (self.route_distance(&refs), points.clone())
        } else if points.len() < 10 {
            self.brute_force(&points)
        } else {
            self.lazzy_walker(&points)
        }
    }

    fn lazzy_walker(&self, points: &Vec<i32>) -> (f64, Vec<i32>) {
        let mut starts = LinkedList::new();
        for point in points {
            starts.push_back(point);
        }

        let mut min_dist = f64::MAX;
        let mut min_route = Vec::new();
        let mut count = points.len();
        while count > 0 {
            if let Some(point) = starts.pop_front() {
                let tail = starts.iter().cloned().cloned().collect::<Vec<i32>>();
                let (dist, route) = self.lazzy_walker_impl(vec![*point], tail);
                if dist < min_dist {
                    min_dist = dist;
                    min_route = route;
                }
                starts.push_back(point);
            }

            count -= 1;
        }

        return (min_dist, min_route);
    }

    fn lazzy_walker_impl(&self, mut route: Vec<i32>, mut points: Vec<i32>) -> (f64, Vec<i32>) {
        if points.is_empty() {
            let refs = route.iter().collect::<Vec<&i32>>();
            return (self.route_distance(&refs), route);
        }

        if let Some(point) = route.iter().last() {
            points.sort_by(|a, b| {
                let d_a = self.distance_between(point, a).unwrap();
                let d_b = self.distance_between(point, b).unwrap();
                d_b.partial_cmp(&d_a).unwrap()
            });
            if let Some(closest) = points.pop() {
                route.push(closest);
            }
        }
        return self.lazzy_walker_impl(route, points);
    }

    fn brute_force(&self, points: &Vec<i32>) -> (f64, Vec<i32>) {
        let mut minimal = f64::MAX;
        let mut route = Vec::new();
        let mut calculated = HashSet::new();
        for path in points.iter().permutations(points.len()) {
            if !calculated.contains(&path) {
                let mut reversed = path.clone();
                reversed.reverse();
                calculated.insert(reversed);

                let distance = self.route_distance(&path);
                if distance < minimal {
                    minimal = distance;
                    route = path.into_iter().cloned().collect();
                }
            }
        }

        return (minimal, route);
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
                    println!("Belt: {id} - {}: {}", belt.name, belt.position);
                    cloud.add(id, &belt.name, &belt.position);
                }
            }
            if !cloud.places.is_empty() {
                clouds.push(cloud);
            }
        }
    }
    Ok(clouds)
}

fn fmt(distance: &f64) -> String {
    format!("{} Mm", (distance / 1000000.0).round())
}

fn display_route(cloud: &Cloud, (minimum, route): (f64, Vec<i32>)) {
    let mut step = 1;
    if 1 == route.len() {
        let id = route[0];
        let name = cloud.get_name(&id).unwrap_or_default();
        println!("{:>2} Warp to `{name}`", step);
    } else {
        let mut first_time = true;
        route.iter().reduce(|a, b| {
            let dist = cloud.distance_between(&a, &b).unwrap_or(0.0);
            let name_a = cloud.get_name(a).unwrap_or_default();
            let name_b = cloud.get_name(b).unwrap_or_default();
            if first_time {
                println!("{:>2} Warp to `{name_a}`", step);
                first_time = false;
                step += 1;
            }

            println!("{:>2} Warp to `{name_b}` - {}", step, fmt(&dist));
            step += 1;
            return b;
        });
        println!("The length of the route: {}", fmt(&minimum));
    }
}

async fn make_route(id: &i32) -> anyhow::Result<()> {
    let system = System::load(id).await?;
    info!("system_name: {}", system.name);

    let clouds = load_system_asteroids(&system).await?;
    info!("Clouds: {}", clouds.len());

    println!("\n\t-=[Ordinal route]=-");
    for cloud in &clouds {
        display_route(&cloud, cloud.get_ordinal_route());
    }

    println!("\n\t-=[Shortest route]=-");
    for cloud in &clouds {
        display_route(&cloud, cloud.get_best_route());
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("warn"));
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
                    make_route(&obj.id).await?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_cloud() {
        let mut cloud = Cloud::new();
        assert_eq!(cloud.distance_between(&0, &1), None);

        cloud.add(
            &1,
            &String::from("System I - Asteroid Belt 1"),
            &Position::new(&0.0, &0.0, &0.0),
        );
        cloud.add(
            &2,
            &String::from("System I - Asteroid Belt 2"),
            &Position::new(&1.0, &0.0, &0.0),
        );

        assert_eq!(cloud.distance_between(&1, &2), Some(1.0));
        assert_eq!(cloud.distance_between(&2, &1), Some(1.0));
    }

    #[test]
    fn test_cloud_get_best_route() {
        let mut cloud = Cloud::new();
        assert_eq!((0.0, vec![]), cloud.get_best_route());

        cloud.add(
            &1,
            &String::from("System I - Asteroid Belt 1"),
            &Position::new(&0.0, &0.0, &0.0),
        );
        assert_eq!((0.0, vec![1]), cloud.get_best_route());

        cloud.add(
            &2,
            &String::from("System I - Asteroid Belt 2"),
            &Position::new(&1.0, &0.0, &0.0),
        );
        assert_eq!(1.0, cloud.get_best_route().0);

        cloud.add(
            &3,
            &String::from("System I - Asteroid Belt 3"),
            &Position::new(&2.0, &0.0, &0.0),
        );
        assert_eq!(2.0, cloud.get_best_route().0);

        cloud.add(
            &4,
            &String::from("System I - Asteroid Belt 4"),
            &Position::new(&3.0, &0.0, &0.0),
        );
        assert_eq!(3.0, cloud.get_best_route().0);
    }
}
