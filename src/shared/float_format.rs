pub trait FloatFormat {
    fn write_f64(f: &mut impl core::fmt::Write, value: f64) -> core::fmt::Result;
    fn write_f32(f: &mut impl core::fmt::Write, value: f32) -> core::fmt::Result;
}

pub struct DefaultFloatFormat;
impl FloatFormat for DefaultFloatFormat {
    fn write_f64(f: &mut impl core::fmt::Write, value: f64) -> core::fmt::Result {
        const UPPER_BOUND_LIT:f64 = 1e6;
        const LOWER_BOUND_LIT:f64 = 1e-3;

        if value.is_finite() {
            if value.abs() < UPPER_BOUND_LIT && value.abs() >= LOWER_BOUND_LIT {
                write!(f, "{}", value)
            }
            else {
                write!(f, "{:e}", value)
            }
        }
        else {
            write!(f, "null")
        }
    }

    fn write_f32(f: &mut impl core::fmt::Write, value: f32) -> core::fmt::Result {
        const UPPER_BOUND_LIT:f32 = 1e6;
        const LOWER_BOUND_LIT:f32 = 1e-3;

        if value.is_finite() {
            if value.abs() < UPPER_BOUND_LIT && value.abs() >= LOWER_BOUND_LIT {
                write!(f, "{}", value)
            }
            else {
                write!(f, "{:e}", value)
            }
        }
        else {
            write!(f, "null")
        }
    }
}
