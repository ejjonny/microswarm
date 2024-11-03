#[derive(Debug, Clone, Copy)]
pub struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn intersects(&self, other: &Rect) -> bool {
        !(other.x > self.x + self.width
            || other.x + other.width < self.x
            || other.y > self.y + self.height
            || other.y + other.height < self.y)
    }

    fn contains_point(&self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }
}

#[derive(Debug, Clone)]
pub struct QuadTreeNode<T: Locatable> {
    pub(crate) bounds: Rect,
    pub(crate) capacity: usize,
    items: Vec<T>,
    children: Option<Box<[QuadTreeNode<T>; 4]>>,
}

#[derive(Debug, Clone, Copy)]
pub struct Point {
    x: f32,
    y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

pub trait Locatable {
    fn location(&self) -> Point;
}

impl<T: Locatable> QuadTreeNode<T> {
    fn new(bounds: Rect, capacity: usize) -> Self {
        QuadTreeNode {
            bounds,
            capacity,
            items: Vec::new(),
            children: None,
        }
    }

    fn subdivide(&mut self) {
        let x = self.bounds.x;
        let y = self.bounds.y;
        let half_width = self.bounds.width / 2.0;
        let half_height = self.bounds.height / 2.0;

        let children = Box::new([
            // Northwest
            QuadTreeNode::new(Rect::new(x, y, half_width, half_height), self.capacity),
            // Northeast
            QuadTreeNode::new(
                Rect::new(x + half_width, y, half_width, half_height),
                self.capacity,
            ),
            // Southwest
            QuadTreeNode::new(
                Rect::new(x, y + half_height, half_width, half_height),
                self.capacity,
            ),
            // Southeast
            QuadTreeNode::new(
                Rect::new(x + half_width, y + half_height, half_width, half_height),
                self.capacity,
            ),
        ]);

        self.children = Some(children);
    }

    fn insert(&mut self, data: T) -> Option<T> {
        if !self.bounds.contains_point(data.location()) {
            return Some(data);
        }

        if self.items.len() < self.capacity {
            self.items.push(data);
            return None;
        }

        if self.children.is_none() {
            self.subdivide();
        }

        let mut data = data;
        if let Some(ref mut children) = self.children {
            for child in children.iter_mut() {
                if let Some(rejected) = child.insert(data) {
                    data = rejected;
                } else {
                    return None;
                }
            }
        }

        None
    }

    fn take_items(&mut self) -> Vec<T> {
        let mut found_items = Vec::new();
        if let Some(ref mut children) = self.children {
            found_items = children.iter_mut().fold(found_items, |mut acc, c| {
                acc.extend(c.take_items());
                acc
            });
        }
        found_items.append(&mut self.items);
        found_items
    }

    fn items(&self) -> Vec<&T> {
        let mut found_items = Vec::<&T>::new();
        if let Some(ref children) = self.children {
            found_items = children.iter().fold(found_items, |mut acc, c| {
                acc.extend(c.items());
                acc
            });
        }
        found_items.extend(self.items.iter());
        found_items
    }

    fn query(&self, rect: &Rect) -> Vec<&T> {
        let mut found_items = Vec::new();

        if !self.bounds.intersects(rect) {
            return found_items;
        }

        for item in &self.items {
            if rect.contains_point(item.location()) {
                found_items.push(item);
            }
        }

        if let Some(ref children) = self.children {
            for child in children.iter() {
                found_items.extend(child.query(rect));
            }
        }

        found_items
    }
}

#[derive(Debug, Clone)]
pub struct QuadTree<T: Locatable> {
    pub(crate) root: QuadTreeNode<T>,
}

impl<T: Locatable> QuadTree<T> {
    pub fn new(bounds: Rect, capacity: usize) -> Self {
        QuadTree {
            root: QuadTreeNode::new(bounds, capacity),
        }
    }

    pub fn insert(&mut self, data: T) -> bool {
        self.root.insert(data).is_none()
    }

    pub fn take_items(&mut self) -> Vec<T> {
        self.root.take_items()
    }

    pub fn items(&self) -> Vec<&T> {
        self.root.items()
    }

    pub fn query(&self, rect: &Rect) -> Vec<&T> {
        self.root.query(rect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug)]
    struct Item {
        tag: String,
        location: Point,
    }

    impl Locatable for Item {
        fn location(&self) -> Point {
            self.location
        }
    }

    fn create_item(tag: &str, x: f32, y: f32) -> Item {
        Item {
            tag: tag.to_owned(),
            location: Point::new(x, y),
        }
    }

    #[test]
    fn test_insert_and_query() {
        let mut qt = QuadTree::new(Rect::new(0.0, 0.0, 100.0, 100.0), 4);

        // Insert some points
        qt.insert(Item {
            tag: "A".to_owned(),
            location: Point::new(10., 10.),
        });
        qt.insert(Item {
            tag: "B".to_owned(),
            location: Point::new(20., 20.),
        });
        qt.insert(Item {
            tag: "C".to_owned(),
            location: Point::new(31., 31.),
        });

        // Query a range that should include two points
        let query_bounds = Rect::new(5.0, 5.0, 25.0, 25.0);
        let results = qt.query(&query_bounds);

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|&data| data.tag == "A"));
        assert!(results.iter().any(|&data| data.tag == "B"));
    }

    #[test]
    fn test_subdivision_triggers() {
        let mut qt = QuadTree::new(Rect::new(0.0, 0.0, 100.0, 100.0), 4);

        // Add items to trigger subdivision
        qt.insert(create_item("A", 10.0, 10.0));
        qt.insert(create_item("B", 11.0, 11.0));
        qt.insert(create_item("C", 12.0, 12.0));
        qt.insert(create_item("D", 13.0, 13.0));
        qt.insert(create_item("E", 14.0, 14.0)); // This should trigger subdivision

        // Query small region containing all points
        let results = qt.query(&Rect::new(9.0, 9.0, 6.0, 6.0));
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_subdivision_distribution() {
        let mut qt = QuadTree::new(Rect::new(0.0, 0.0, 100.0, 100.0), 4);

        // Add points in different quadrants
        // Northwest quadrant
        qt.insert(create_item("NW1", 20.0, 20.0));
        qt.insert(create_item("NW2", 21.0, 21.0));

        // Northeast quadrant
        qt.insert(create_item("NE1", 70.0, 20.0));
        qt.insert(create_item("NE2", 71.0, 21.0));

        // Southwest quadrant
        qt.insert(create_item("SW1", 20.0, 70.0));
        qt.insert(create_item("SW2", 21.0, 71.0));

        // Southeast quadrant
        qt.insert(create_item("SE1", 70.0, 70.0));
        qt.insert(create_item("SE2", 71.0, 71.0));

        // Query each quadrant separately
        let nw_results = qt.query(&Rect::new(0.0, 0.0, 50.0, 50.0));
        let ne_results = qt.query(&Rect::new(50.0, 0.0, 50.0, 50.0));
        let sw_results = qt.query(&Rect::new(0.0, 50.0, 50.0, 50.0));
        let se_results = qt.query(&Rect::new(50.0, 50.0, 50.0, 50.0));

        assert_eq!(nw_results.len(), 2);
        assert_eq!(ne_results.len(), 2);
        assert_eq!(sw_results.len(), 2);
        assert_eq!(se_results.len(), 2);
    }

    #[test]
    fn test_deep_subdivision() {
        let mut qt = QuadTree::new(Rect::new(0.0, 0.0, 100.0, 100.0), 2); // Smaller capacity to force deeper subdivision

        // Add many points in a small area to force multiple subdivisions
        for i in 0..16 {
            let x = 10.0 + (i as f32) * 0.1;
            let y = 10.0 + (i as f32) * 0.1;
            qt.insert(create_item(&format!("P{}", i), x, y));
        }

        // Query the small area containing all points
        let results = qt.query(&Rect::new(9.0, 9.0, 3.0, 3.0));
        assert_eq!(results.len(), 16);

        // Query a very small area containing just a few points
        let small_results = qt.query(&Rect::new(10.0, 10.0, 0.2, 0.2));
        assert!(!small_results.is_empty() && small_results.len() < 16);
    }

    #[test]
    fn test_boundary_cases() {
        let mut qt = QuadTree::new(Rect::new(0.0, 0.0, 100.0, 100.0), 4);

        // Test points exactly on boundaries
        qt.insert(create_item("Center", 50.0, 50.0)); // Center point
        qt.insert(create_item("Edge", 100.0, 50.0)); // Right edge
        qt.insert(create_item("Corner", 100.0, 100.0)); // Corner point
        qt.insert(create_item("TopEdge", 50.0, 0.0)); // Top edge

        // Query including boundary points
        let results = qt.query(&Rect::new(49.0, 49.0, 2.0, 2.0));
        assert_eq!(results.len(), 1); // Should only find center point

        // Query exact boundary
        let edge_results = qt.query(&Rect::new(100.0, 0.0, 0.0, 100.0));
        assert_eq!(edge_results.len(), 2); // Should find edge and corner points
    }

    #[test]
    fn test_empty_areas() {
        let mut qt = QuadTree::new(Rect::new(0.0, 0.0, 100.0, 100.0), 4);

        // Add points only in one quadrant
        qt.insert(create_item("A", 10.0, 10.0));
        qt.insert(create_item("B", 11.0, 11.0));
        qt.insert(create_item("C", 12.0, 12.0));

        // Query empty quadrants
        let empty_ne = qt.query(&Rect::new(51.0, 0.0, 49.0, 49.0));
        let empty_sw = qt.query(&Rect::new(0.0, 51.0, 49.0, 49.0));
        let empty_se = qt.query(&Rect::new(51.0, 51.0, 49.0, 49.0));

        assert_eq!(empty_ne.len(), 0);
        assert_eq!(empty_sw.len(), 0);
        assert_eq!(empty_se.len(), 0);
    }
}
