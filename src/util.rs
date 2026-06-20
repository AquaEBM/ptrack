pub struct ThreeSmooth {
    s: Vec<usize>,
    i2: usize,
    i3: usize,
}

impl ThreeSmooth {
    #[inline]
    pub fn new() -> Self {
        Self {
            s: vec![1],
            i2: 0,
            i3: 0,
        }
    }
}

impl Iterator for ThreeSmooth {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let current = *self.s.last().unwrap();

        let n2 = 2 * self.s[self.i2];
        let n3 = 3 * self.s[self.i3];

        let (push, add) = if n2 <= n3 {
            (n2, &mut self.i2)
        } else {
            (n3, &mut self.i3)
        };

        self.s.push(push);
        *add += 1;

        Some(current)
    }
}

#[inline(always)]
pub fn parabolic_argmin(prev: f32, curr: f32, next: f32) -> f32 {
    -0.5 * (next - prev) / curr.mul_add(-2., prev + next)
}