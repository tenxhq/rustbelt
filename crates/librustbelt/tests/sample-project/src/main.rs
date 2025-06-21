/// Sample Rust file for testing type hints and other IDE features
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Person {
    pub name: String,
    pub age: u32,
    pub email: Option<String>,
}

impl Person {
    pub fn new(name: String, age: u32) -> Self {
        Self {
            name,
            age,
            email: None,
        }
    }

    pub fn with_email(mut self, email: String) -> Self {
        self.email = Some(email);
        self
    }

    pub fn is_adult(&self) -> bool {
        self.age >= 18
    }
}

pub fn main() {
    let mut people: HashMap<String, Person> = HashMap::new();

    let person = Person::new("Alice".to_string(), 25).with_email("alice@example.com".to_string());

    people.insert(person.name.clone(), person);

    let result = calculate_average_age(&people);
    println!("Average age: {}", result);

    // Test various expressions for type hints
    let numbers = vec![1, 2, 3, 4, 5];
    let doubled: Vec<i32> = numbers.iter().map(|x| x * 2).collect();
    let sum = doubled.iter().fold(0, |acc, x| acc + x);

    // Complex generic types
    let nested: Vec<Option<Result<String, &str>>> =
        vec![Some(Ok("hello".to_string())), Some(Err("error")), None];

    for item in nested {
        match item {
            Some(Ok(s)) => println!("Success: {}", s),
            Some(Err(e)) => println!("Error: {}", e),
            None => println!("None"),
        }
    }
}

fn calculate_average_age(people: &HashMap<String, Person>) -> f64 {
    if people.is_empty() {
        return 0.0;
    }

    let total_age: u32 = people.values().map(|p| p.age).sum();
    total_age as f64 / people.len() as f64
}

// Async function for testing
pub async fn fetch_data(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Simulate async work
    // tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(format!("Data from {}", url))
}

// Generic function
pub fn process_items<T, F, R>(items: Vec<T>, processor: F) -> Vec<R>
where
    F: Fn(T) -> R,
{
    items.into_iter().map(processor).collect()
}
