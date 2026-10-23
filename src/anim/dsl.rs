anim_macro::math_kernel! {
    name = dsl_plasma,
    params = { a = 5.7, b = 4.9, c = 2.3, d = 0.8 },
    expr = "0.5 + (2+3)*0.036 * (sin(x*a + t) + cos(y*b - t) + sin((x+y)*c + t*d)) + 0.1 * sin(sqrt((x-0.5)^2 + (y-0.5)^2) * 18.0 - t*3.0)"
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_kernel_normalizes() {
        for (x, y, t) in [(0.0, 0.0, 0.0), (0.4, 0.8, 2.0), (1.0, 1.0, 4.0)] {
            let v = super::dsl_plasma(x, y, t);
            assert!((0.0..=1.0).contains(&v));
        }
    }
}
