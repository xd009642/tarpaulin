pub enum Shape {
    Circle(u32),
    Square(u32),
}

pub fn area<T: Into<u32>>(shape: &Shape, scale: T) -> u32 {
    let scale = scale.into();
    match shape {
        Shape::Circle(r) => {
            let d = r * 2;
            d * scale
        }
        Shape::Square(s) => {
            let t = s * s;
            t * scale
        }
    }
}

pub struct Painter<'a> {
    pub name: &'a str,
}

impl<'a> Painter<'a> {
    pub fn paint(&self, shape: &Shape) -> usize {
        match shape {
            Shape::Circle(r) => {
                let total = *r as usize;
                total + self.name.len()
            }
            Shape::Square(s) => {
                let total = *s as usize;
                total * self.name.len()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_arm_runs() {
        assert_eq!(area(&Shape::Circle(1), 3u8), 6);
        assert_eq!(area(&Shape::Square(2), 1u8), 4);
        let painter = Painter { name: "ab" };
        assert_eq!(painter.paint(&Shape::Circle(1)), 3);
        assert_eq!(painter.paint(&Shape::Square(3)), 6);
    }
}
